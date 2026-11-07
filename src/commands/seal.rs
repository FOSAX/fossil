use crate::archive::pack_dir_to_fossil;
use crate::container;
use crate::hashing::hash_file;
use crate::keys::{load_or_create_signing_key, sign};
use crate::manifest::{
    BinaryRecord, BuildInfo, ContainerInfo, FileRecord, HardwareInfo, Manifest, SigningInfo,
    SourceInfo, ToolchainInfo, FOSSIL_FORMAT_VERSION,
};
use crate::toolchain::{detect_build_system, detect_compiler, detect_linker, detect_os_arch};
use crate::util::{default_output_path, is_excluded};
use anyhow::{bail, Context, Result};
use colored::Colorize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Environment variables whose presence can affect the build output; worth
/// recording even though Fossil does not replay them automatically yet.
const RECORDED_ENV_VARS: &[&str] = &["CC", "CXX", "CFLAGS", "CXXFLAGS", "LDFLAGS", "AR"];

pub struct SealArgs {
    pub project_dir: PathBuf,
    pub output: Option<PathBuf>,
    pub name: Option<String>,
    pub binaries: Vec<PathBuf>,
    pub build_command: Option<String>,
    pub compiler: Option<String>,
    pub build_system: Option<String>,
    pub hardware_target: Option<String>,
}

pub fn run(args: SealArgs) -> Result<()> {
    let project_dir = args
        .project_dir
        .canonicalize()
        .with_context(|| format!("resolving {}", args.project_dir.display()))?;
    if !project_dir.is_dir() {
        bail!("{} is not a directory", project_dir.display());
    }

    let name = args
        .name
        .clone()
        .unwrap_or_else(|| project_dir.file_name().unwrap().to_string_lossy().to_string());

    println!("{} Sealing {}", "->".cyan(), project_dir.display());

    let staging = tempfile::tempdir().context("creating staging dir")?;
    let staging_path = staging.path();
    let sources_dir = staging_path.join("sources");
    let toolchain_dir = staging_path.join("toolchain");
    let build_recipe_dir = staging_path.join("build-recipe");
    let signatures_dir = staging_path.join("signatures");
    let docs_dir = staging_path.join("docs");
    for d in [&sources_dir, &toolchain_dir, &build_recipe_dir, &signatures_dir, &docs_dir] {
        fs::create_dir_all(d)?;
    }

    // 1. Detect toolchain.
    let compiler = detect_compiler(args.compiler.as_deref());
    let build_system = detect_build_system(args.build_system.as_deref(), &project_dir);
    let linker = detect_linker();
    let (os, arch) = detect_os_arch();
    if let Some(c) = &compiler {
        println!("{} Toolchain captured: {}", "OK:".green(), c.display());
        fs::write(
            toolchain_dir.join("compiler.txt"),
            format!("{}\npath: {}\n", c.display(), c.path.clone().unwrap_or_default()),
        )?;
    } else {
        println!("{} No compiler detected on PATH; recorded as unknown", "WARN:".yellow());
    }
    if let Some(b) = &build_system {
        fs::write(
            toolchain_dir.join("build-system.txt"),
            format!("{}\npath: {}\n", b.display(), b.path.clone().unwrap_or_default()),
        )?;
    }
    if let Some(l) = &linker {
        fs::write(
            toolchain_dir.join("linker.txt"),
            format!("{}\npath: {}\n", l.display(), l.path.clone().unwrap_or_default()),
        )?;
    }

    // 2. Hermetic build environment: generate a Dockerfile matching the
    // detected toolchain and, if an engine is available, build it now and
    // pin its content-addressed image ID. `replay` later runs offline
    // inside this exact image instead of trusting whatever happens to be
    // installed on the replaying machine.
    let compiler_name = compiler.as_ref().map(|c| c.name.as_str()).unwrap_or("gcc");
    let build_system_name = build_system.as_ref().map(|b| b.name.as_str()).unwrap_or("make");
    let dockerfile = container::generate_dockerfile(compiler_name, build_system_name);
    fs::write(toolchain_dir.join("Dockerfile"), &dockerfile)?;

    let container_info = match container::detect_engine() {
        Some(engine) => {
            let tag = format!("fossil-{}:latest", sanitize_tag(&name));
            println!("{} Building hermetic {engine} image for replay...", "->".cyan());
            match container::build_image(engine, &toolchain_dir, &tag) {
                Ok(digest) => {
                    println!("{} Hermetic image built: {} ({})", "OK:".green(), tag, &digest[..19.min(digest.len())]);
                    Some(ContainerInfo {
                        engine: engine.to_string(),
                        image_tag: tag,
                        image_digest: digest,
                        dockerfile: "toolchain/Dockerfile".to_string(),
                    })
                }
                Err(e) => {
                    println!("{} Could not build hermetic image ({e}); replay will fall back to the host toolchain", "WARN:".yellow());
                    None
                }
            }
        }
        None => {
            println!(
                "{} Neither docker nor podman found on PATH; replay will use the host toolchain",
                "WARN:".yellow()
            );
            None
        }
    };

    let mut toolchain = ToolchainInfo {
        compiler,
        build_system: build_system.clone(),
        other_tools: linker.into_iter().collect(),
        os,
        arch,
        container: container_info,
    };

    // 3. Build recipe.
    let build_command = args.build_command.clone().unwrap_or_else(|| {
        build_system
            .as_ref()
            .map(|b| default_build_command(&b.name))
            .unwrap_or_else(|| "make".to_string())
    });
    fs::write(
        build_recipe_dir.join("build.sh"),
        format!("#!/bin/sh\nset -e\ncd \"$(dirname \"$0\")/../sources\"\n{build_command}\n"),
    )?;
    let mut env = BTreeMap::new();
    for var in RECORDED_ENV_VARS {
        if let Ok(val) = std::env::var(var) {
            env.insert(var.to_string(), val);
        }
    }
    let build = BuildInfo {
        command: build_command,
        working_dir: "sources".to_string(),
        env,
    };

    // 4. Produce the canonical binary. If the caller didn't pin specific
    // binaries with --binary, build fresh now - preferably inside the same
    // hermetic image that `replay` will use later. Sealing a binary that was
    // built by a different toolchain than the one `replay` rebuilds with
    // guarantees the two will never match bit-for-bit, so this is the step
    // that makes the "same manifest -> same binary" promise hold.
    if args.binaries.is_empty() {
        for stale in find_known_binaries(&project_dir)? {
            fs::remove_file(&stale).with_context(|| format!("removing stale artifact {}", stale.display()))?;
        }
        // A leftover CMake build/ directory embeds absolute paths from
        // whoever configured it last; reusing it across host <-> container
        // (or across machines) breaks the build, so start it fresh too.
        let stale_build_dir = project_dir.join("build");
        if stale_build_dir.is_dir() {
            fs::remove_dir_all(&stale_build_dir).context("removing stale build/ directory")?;
        }
        let built_in_container = toolchain.container.clone().filter(|c| which::which(&c.engine).is_ok());
        match &built_in_container {
            Some(c) => {
                println!("{} Building firmware inside hermetic {} image...", "->".cyan(), c.engine);
                let ok = container::run_build(&c.engine, &c.image_digest, &project_dir, &build.command)?;
                if !ok {
                    bail!("build failed inside hermetic {} image", c.engine);
                }
            }
            None => {
                println!("{} Building firmware with the host toolchain: {}", "->".cyan(), build.command);
                let status = std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&build.command)
                    .current_dir(&project_dir)
                    .status()
                    .context("launching build command")?;
                if !status.success() {
                    bail!("build command failed: {}", build.command);
                }
            }
        }

        // The compiler/build-system versions detected so far describe the
        // host, which is only what generated the Dockerfile. Once the
        // canonical binary is actually built inside the image, overwrite
        // those fields with what's really in there, so the manifest never
        // claims a toolchain that didn't produce the sealed binary.
        if let Some(c) = &built_in_container {
            if let Some(tool) = &mut toolchain.compiler {
                if let Ok(v) = container::exec_capture(&c.engine, &c.image_digest, &format!("{} --version", tool.name)) {
                    if let Some(line) = v.lines().next().filter(|l| !l.trim().is_empty()) {
                        tool.version = line.trim().to_string();
                        tool.path = None;
                    }
                }
            }
            if let Some(tool) = &mut toolchain.build_system {
                if let Ok(v) = container::exec_capture(&c.engine, &c.image_digest, &format!("{} --version", tool.name)) {
                    if let Some(line) = v.lines().next().filter(|l| !l.trim().is_empty()) {
                        tool.version = line.trim().to_string();
                        tool.path = None;
                    }
                }
            }
        }
    }

    // 5. Resolve which files are binary artifacts, so they are excluded from
    // the source snapshot: leaving them in sources/ would make `make` think
    // the artifact is already up to date and skip rebuilding during replay.
    let binary_paths = resolve_binaries(&project_dir, &args.binaries)?;

    // 6. Copy sources and hash them.
    let mut files = Vec::new();
    for entry in WalkDir::new(&project_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let rel = path.strip_prefix(&project_dir).unwrap();
        if rel.as_os_str().is_empty() || is_excluded(rel) || binary_paths.contains(&rel.to_path_buf()) {
            continue;
        }
        if entry.file_type().is_dir() {
            continue;
        }
        let dest = sources_dir.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(path, &dest).with_context(|| format!("copying {}", path.display()))?;
        let hashes = hash_file(&dest)?;
        files.push(FileRecord {
            path: rel.to_string_lossy().replace('\\', "/"),
            sha256: hashes.sha256,
            blake3: hashes.blake3,
            size: hashes.size,
        });
    }
    if files.is_empty() {
        bail!("no source files found under {}", project_dir.display());
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    println!("{} Sources archived ({} files)", "OK:".green(), files.len());

    // 7. Binaries.
    let mut binaries = Vec::new();
    for rel in &binary_paths {
        let src = project_dir.join(rel);
        let dest = staging_path.join(rel.file_name().unwrap());
        fs::copy(&src, &dest).with_context(|| format!("copying binary {}", src.display()))?;
        let hashes = hash_file(&dest)?;
        println!("{} Binary hashed: sha256:{}", "OK:".green(), &hashes.sha256[..12]);
        binaries.push(BinaryRecord {
            path: dest.file_name().unwrap().to_string_lossy().to_string(),
            sha256: hashes.sha256,
            blake3: hashes.blake3,
            size: hashes.size,
        });
    }

    // 8. Docs.
    for candidate in ["README.md", "README", "README.txt"] {
        let p = project_dir.join(candidate);
        if p.exists() {
            fs::copy(&p, docs_dir.join(candidate)).ok();
        }
    }
    fs::write(docs_dir.join("FOSSIL.md"), explain_doc(&name))?;

    // 9. Signing key.
    let signing_key = load_or_create_signing_key()?;
    let public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());

    let manifest = Manifest {
        fossil_format_version: FOSSIL_FORMAT_VERSION.to_string(),
        fossil_cli_version: env!("CARGO_PKG_VERSION").to_string(),
        project_name: name.clone(),
        created_at: chrono::Utc::now().to_rfc3339(),
        source: SourceInfo {
            file_count: files.len(),
            files,
        },
        toolchain,
        build,
        binaries,
        hardware: HardwareInfo {
            target: args.hardware_target.clone().unwrap_or_else(|| "unspecified".to_string()),
            arch: std::env::consts::ARCH.to_string(),
            notes: String::new(),
        },
        signing: SigningInfo {
            algorithm: "ed25519".to_string(),
            public_key_hex: public_key_hex.clone(),
        },
    };

    let manifest_bytes = manifest.canonical_bytes();
    fs::write(staging_path.join("manifest.json"), serde_json::to_vec_pretty(&manifest)?)?;

    let signature = sign(&signing_key, &manifest_bytes);
    fs::write(signatures_dir.join("manifest.sig"), hex::encode(signature.to_bytes()))?;
    fs::write(signatures_dir.join("public_key.hex"), &public_key_hex)?;
    println!("{} Manifest signed (ed25519)", "OK:".green());

    // 10. checksums.sha256 over every staged file (sha256sum compatible).
    write_checksums(staging_path)?;

    // 11. Pack.
    let output = args.output.clone().unwrap_or_else(|| default_output_path(&project_dir, &name));
    pack_dir_to_fossil(staging_path, &output)?;
    println!("{} Fossil created: {}", "OK:".green().bold(), output.display());

    Ok(())
}

fn sanitize_tag(name: &str) -> String {
    let lowered: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    if lowered.is_empty() {
        "project".to_string()
    } else {
        lowered
    }
}

fn default_build_command(build_system: &str) -> String {
    match build_system {
        "cmake" => "cmake -S . -B build && cmake --build build".to_string(),
        "cargo" => "cargo build --release".to_string(),
        "gprbuild" => "gprbuild -p".to_string(),
        "ninja" => "ninja".to_string(),
        _ => "make".to_string(),
    }
}

const BINARY_EXTENSIONS: &[&str] = &["elf", "bin", "hex", "out"];

/// `is_excluded` (used for the *source* snapshot) treats directories like
/// `build/` or `target/` as generated output and skips them - correctly, we
/// don't want to archive them as "source". But that's exactly where
/// CMake/Cargo put the binaries we need to find, so binary search only
/// skips version-control metadata and CMake's own internal scratch
/// directories (compiler-ABI detection probes, `try_compile` temporaries),
/// not build-output directories in general.
fn is_vcs_excluded(path: &Path) -> bool {
    path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        s == ".git" || s == "CMakeFiles" || s.starts_with("CMakeTmp")
    })
}

fn resolve_binaries(project_dir: &Path, requested: &[PathBuf]) -> Result<Vec<PathBuf>> {
    if !requested.is_empty() {
        for r in requested {
            if !project_dir.join(r).is_file() {
                bail!("binary not found: {}", r.display());
            }
        }
        return Ok(requested.to_vec());
    }
    let mut found = Vec::new();
    for entry in WalkDir::new(project_dir).max_depth(6).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let rel = path.strip_prefix(project_dir).unwrap();
        if is_vcs_excluded(rel) || !entry.file_type().is_file() {
            continue;
        }
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if BINARY_EXTENSIONS.contains(&ext) {
                found.push(rel.to_path_buf());
            }
        }
    }
    Ok(found)
}

/// Absolute paths of any leftover binary artifacts in `project_dir`, so seal
/// can delete them before rebuilding and force a real, fresh compilation
/// rather than letting `make` decide the stale artifact is up to date.
fn find_known_binaries(project_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for entry in WalkDir::new(project_dir).max_depth(6).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        let rel = path.strip_prefix(project_dir).unwrap();
        if is_vcs_excluded(rel) || !entry.file_type().is_file() {
            continue;
        }
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if BINARY_EXTENSIONS.contains(&ext) {
                found.push(path.to_path_buf());
            }
        }
    }
    Ok(found)
}

fn write_checksums(staging_path: &Path) -> Result<()> {
    let mut lines = Vec::new();
    for entry in WalkDir::new(staging_path).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = path.strip_prefix(staging_path).unwrap();
        if rel == Path::new("checksums.sha256") {
            continue;
        }
        let hashes = hash_file(path)?;
        lines.push(format!("{}  {}", hashes.sha256, rel.to_string_lossy().replace('\\', "/")));
    }
    lines.sort();
    fs::write(staging_path.join("checksums.sha256"), lines.join("\n") + "\n")?;
    Ok(())
}

fn explain_doc(name: &str) -> String {
    format!(
        "# {name} - Fossil package\n\n\
        This directory is a Fossil archive: a self-contained, signed snapshot of a\n\
        firmware project, created so it can be understood and rebuilt decades from now,\n\
        even without network access.\n\n\
        - `sources/` - exact source tree at seal time\n\
        - `toolchain/` - compiler and build-system identification\n\
        - `build-recipe/` - the exact command used to rebuild the binaries\n\
        - `manifest.json` - machine-readable description of everything in this package\n\
        - `checksums.sha256` - SHA-256 of every file in this package\n\
        - `signatures/` - Ed25519 signature over `manifest.json` and the signer's public key\n\n\
        Use the `fossil` CLI (https://github.com/fosax/fossil) to verify or rebuild this\n\
        package: `fossil verify <file>.fossil` and `fossil replay <file>.fossil`.\n"
    )
}
