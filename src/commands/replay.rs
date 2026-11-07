use crate::container;
use crate::hashing::hash_file;
use crate::manifest::Manifest;
use crate::util::open_fossil;
use crate::Engine;
use anyhow::{bail, Result};
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// What to actually run the build in, resolved from the user's `--engine`
/// choice and whatever was recorded in the manifest at seal time.
enum Resolved {
    Host,
    Container { engine_bin: &'static str, image_ref: String },
}

fn resolve_engine(requested: Engine, manifest: &Manifest) -> Result<Resolved> {
    let container_info = manifest.toolchain.container.as_ref();
    match requested {
        Engine::Host => Ok(Resolved::Host),
        Engine::Docker | Engine::Podman => {
            let name: &'static str = if matches!(requested, Engine::Docker) { "docker" } else { "podman" };
            let info = container_info.ok_or_else(|| {
                anyhow::anyhow!(
                    "no hermetic container image was recorded in this fossil; re-seal with {name} \
                     available, or run with --engine host"
                )
            })?;
            if info.engine != name {
                bail!(
                    "this fossil was sealed with a {} image, not {name}; use --engine {} or --engine host",
                    info.engine,
                    info.engine
                );
            }
            if which::which(name).is_err() {
                bail!("{name} was requested but is not installed on this machine");
            }
            Ok(Resolved::Container { engine_bin: name, image_ref: info.image_digest.clone() })
        }
        Engine::Auto => match container_info {
            Some(info) if which::which(&info.engine).is_ok() => {
                let engine_bin: &'static str = if info.engine == "docker" { "docker" } else { "podman" };
                Ok(Resolved::Container { engine_bin, image_ref: info.image_digest.clone() })
            }
            Some(info) => {
                println!(
                    "{} Fossil recorded a {} image for hermetic replay, but {} isn't installed here; \
                     falling back to the host toolchain",
                    "WARN:".yellow(),
                    info.engine,
                    info.engine
                );
                Ok(Resolved::Host)
            }
            None => {
                println!(
                    "{} No hermetic image recorded for this fossil; using the host toolchain",
                    "WARN:".yellow()
                );
                Ok(Resolved::Host)
            }
        },
    }
}

pub fn run(fossil_path: &Path, output_dir: Option<PathBuf>, engine: Engine) -> Result<bool> {
    println!("{} Replaying {}", "->".cyan(), fossil_path.display());
    let (tmp, manifest) = open_fossil(fossil_path)?;
    let root = output_dir.clone().unwrap_or_else(|| tmp.path().to_path_buf());
    if let Some(dest) = &output_dir {
        copy_dir(tmp.path(), dest)?;
    }
    let sources_dir = root.join("sources");

    let resolved = resolve_engine(engine, &manifest)?;

    let build_ok = match &resolved {
        Resolved::Container { engine_bin, image_ref } => {
            println!(
                "{} Running build command inside hermetic {engine_bin} image ({}...), network disabled",
                "->".cyan(),
                &image_ref[..19.min(image_ref.len())]
            );
            println!("    {}", manifest.build.command);
            container::run_build(engine_bin, image_ref, &sources_dir, &manifest.build.command)?
        }
        Resolved::Host => {
            if let Some(compiler) = &manifest.toolchain.compiler {
                if which::which(&compiler.name).is_err() {
                    println!(
                        "{} Compiler '{}' not found on PATH; replay will likely fail. Recorded version: {}",
                        "WARN:".yellow(),
                        compiler.name,
                        compiler.version
                    );
                }
            }
            println!("{} Running build command on the host toolchain: {}", "->".cyan(), manifest.build.command);
            let status = Command::new("sh")
                .arg("-c")
                .arg(&manifest.build.command)
                .current_dir(&sources_dir)
                .status();
            match status {
                Ok(s) => s.success(),
                Err(e) => bail!("failed to launch build command: {e}"),
            }
        }
    };

    if !build_ok {
        println!("{} Build command exited with failure", "FAIL:".red());
        return Ok(false);
    }

    let mut all_match = true;
    for record in &manifest.binaries {
        let candidate = find_rebuilt_binary(&sources_dir, &record.path);
        match candidate.and_then(|p| hash_file(&p).ok()) {
            Some(h) if h.sha256 == record.sha256 => {
                println!("{} Rebuilt binary matches original: {}", "OK:".green(), record.path);
            }
            Some(h) => {
                all_match = false;
                println!(
                    "{} Rebuilt binary DIFFERS from original: {} (expected {}, got {})",
                    "FAIL:".red(),
                    record.path,
                    &record.sha256[..12],
                    &h.sha256[..12]
                );
            }
            None => {
                all_match = false;
                println!("{} Could not locate rebuilt binary for: {}", "FAIL:".red(), record.path);
            }
        }
    }

    if all_match {
        println!("{}", "Replay succeeded: rebuilt binary is bit-identical.".green().bold());
    } else {
        println!(
            "{}",
            "Replay completed but output differs from the sealed binary.".yellow().bold()
        );
    }
    Ok(all_match)
}

fn find_rebuilt_binary(sources_dir: &Path, original_name: &str) -> Option<PathBuf> {
    let direct = sources_dir.join(original_name);
    if direct.is_file() {
        return Some(direct);
    }
    walkdir::WalkDir::new(sources_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy() == original_name)
        .map(|e| e.path().to_path_buf())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in walkdir::WalkDir::new(src).into_iter().filter_map(|e| e.ok()) {
        let rel = entry.path().strip_prefix(src).unwrap();
        let target = dst.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}
