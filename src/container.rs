use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

/// Picks apt packages and a base image for a detected compiler/build-system
/// pair. This is intentionally simple: the generated Dockerfile is plain
/// text under `toolchain/Dockerfile` in the sealed package, so a maintainer
/// can hand-edit it for toolchains this heuristic doesn't know about.
pub fn generate_dockerfile(compiler_name: &str, build_system_name: &str) -> String {
    let mut packages: Vec<&str> = match compiler_name {
        "clang" => vec!["clang", "build-essential"],
        "arm-none-eabi-gcc" => vec!["gcc-arm-none-eabi"],
        "riscv64-unknown-elf-gcc" => vec!["gcc-riscv64-unknown-elf"],
        "gnatmake" | "gprbuild" => vec!["gnat", "gprbuild"],
        _ => vec!["build-essential"],
    };
    match build_system_name {
        "cmake" => packages.push("cmake"),
        "ninja" => packages.push("ninja-build"),
        _ => {}
    }
    packages.sort_unstable();
    packages.dedup();

    format!(
        "FROM debian:bookworm-slim\n\n\
         RUN apt-get update && apt-get install -y --no-install-recommends \\\n    {} \\\n    && rm -rf /var/lib/apt/lists/*\n\n\
         WORKDIR /work\n",
        packages.join(" \\\n    ")
    )
}

/// Returns the first of "docker"/"podman" present on PATH, preferring docker.
pub fn detect_engine() -> Option<&'static str> {
    if which::which("docker").is_ok() {
        Some("docker")
    } else if which::which("podman").is_ok() {
        Some("podman")
    } else {
        None
    }
}

/// Builds the image from the Dockerfile in `dockerfile_dir`, tags it, and
/// returns its content-addressed image ID (e.g. "sha256:abcd...") - this is
/// what gets pinned in the manifest instead of a mutable tag.
pub fn build_image(engine: &str, dockerfile_dir: &Path, tag: &str) -> Result<String> {
    let output = Command::new(engine)
        .args(["build", "-q", "-t", tag, "."])
        .current_dir(dockerfile_dir)
        .output()
        .with_context(|| format!("launching `{engine} build`"))?;
    if !output.status.success() {
        bail!(
            "`{engine} build` failed for {}: {}",
            dockerfile_dir.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let out = Command::new(engine)
        .args(["inspect", "--format", "{{.Id}}", tag])
        .output()
        .with_context(|| format!("launching `{engine} inspect`"))?;
    if !out.status.success() {
        bail!("`{engine} inspect` failed for {tag}");
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Runs `build_command` inside a network-isolated container, bind-mounting
/// `sources_dir` at /work so build output lands directly on the host.
pub fn run_build(engine: &str, image_ref: &str, sources_dir: &Path, build_command: &str) -> Result<bool> {
    let mut cmd = Command::new(engine);
    cmd.arg("run").arg("--rm").arg("--network").arg("none");

    #[cfg(unix)]
    {
        // Match the container process's uid:gid to the host's so build
        // output in the bind-mounted sources dir stays host-owned.
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };
        cmd.arg("--user").arg(format!("{uid}:{gid}"));
    }

    cmd.arg("-v")
        .arg(format!("{}:/work", sources_dir.display()))
        .arg("-w")
        .arg("/work")
        .arg(image_ref)
        .arg("sh")
        .arg("-c")
        .arg(build_command);

    let status = cmd.status().with_context(|| format!("launching `{engine} run`"))?;
    Ok(status.success())
}

/// Runs a throwaway command inside the image and returns its stdout  - 
/// used to record the *actual* tool versions baked into the hermetic
/// image, rather than trusting whatever happens to be on the host's PATH.
pub fn exec_capture(engine: &str, image_ref: &str, command: &str) -> Result<String> {
    let out = Command::new(engine)
        .args(["run", "--rm", "--network", "none", image_ref, "sh", "-c", command])
        .output()
        .with_context(|| format!("launching `{engine} run` for `{command}`"))?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
