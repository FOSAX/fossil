use crate::util::open_fossil;
use anyhow::Result;
use colored::Colorize;
use std::path::Path;

pub fn run(fossil_path: &Path, as_json: bool) -> Result<()> {
    let (_tmp, manifest) = open_fossil(fossil_path)?;

    if as_json {
        println!("{}", serde_json::to_string_pretty(&manifest)?);
        return Ok(());
    }

    println!("{}", manifest.project_name.bold());
    println!("  sealed:      {}", manifest.created_at);
    println!("  fossil fmt:  {}", manifest.fossil_format_version);
    println!("  cli version: {}", manifest.fossil_cli_version);
    println!();
    println!("{}", "Hardware".bold());
    println!("  target: {}", manifest.hardware.target);
    println!("  arch:   {}", manifest.hardware.arch);
    println!();
    println!("{}", "Toolchain".bold());
    if let Some(c) = &manifest.toolchain.compiler {
        println!("  compiler:     {}", c.display());
    } else {
        println!("  compiler:     unknown");
    }
    if let Some(b) = &manifest.toolchain.build_system {
        println!("  build system: {}", b.display());
    } else {
        println!("  build system: unknown");
    }
    println!("  host os/arch: {}/{}", manifest.toolchain.os, manifest.toolchain.arch);
    println!();
    println!("{}", "Build".bold());
    println!("  command: {}", manifest.build.command);
    println!("  cwd:     {}", manifest.build.working_dir);
    println!();
    println!("{}", "Sources".bold());
    println!("  files: {}", manifest.source.file_count);
    println!();
    println!("{}", "Binaries".bold());
    for b in &manifest.binaries {
        println!("  {} - sha256:{} ({} bytes)", b.path, &b.sha256[..16], b.size);
    }
    println!();
    println!("{}", "Signing".bold());
    println!("  algorithm:  {}", manifest.signing.algorithm);
    println!("  public key: {}", manifest.signing.public_key_hex);

    Ok(())
}
