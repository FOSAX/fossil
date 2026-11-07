use crate::util::open_fossil;
use anyhow::Result;
use std::path::Path;

pub fn run(fossil_path: &Path) -> Result<()> {
    let (_tmp, m) = open_fossil(fossil_path)?;

    let compiler = m
        .toolchain
        .compiler
        .as_ref()
        .map(|c| c.display())
        .unwrap_or_else(|| "an unrecorded compiler".to_string());
    let build_system = m
        .toolchain
        .build_system
        .as_ref()
        .map(|b| b.display())
        .unwrap_or_else(|| "an unrecorded build system".to_string());

    println!("# {}\n", m.project_name);
    println!(
        "This Fossil package was sealed on {} using fossil CLI v{}.",
        m.created_at, m.fossil_cli_version
    );
    println!(
        "\nIt preserves {} source files, built with {} using {} via the command:\n\n    {}\n",
        m.source.file_count, compiler, build_system, m.build.command
    );

    println!("## Hardware target");
    println!("- Target: {}", m.hardware.target);
    println!("- Architecture: {}", m.hardware.arch);

    println!("\n## Binaries");
    if m.binaries.is_empty() {
        println!("- No binaries were captured in this package.");
    }
    for b in &m.binaries {
        println!("- {} - {} bytes, sha256:{}", b.path, b.size, b.sha256);
    }

    println!("\n## Integrity");
    println!(
        "- The manifest is signed with Ed25519, key {}.",
        m.signing.public_key_hex
    );
    println!("- Run `fossil verify` to confirm the archive has not been altered.");
    println!("- Run `fossil replay` to rebuild the binaries offline and compare the result bit-for-bit.");

    println!("\n## Long-term notes");
    println!(
        "- This signature is self-contained (trust-on-first-use): Fossil does not operate a\n  certificate authority. To rely on it long-term, record the signer's public key\n  ({}...) through an independent channel at the time of sealing.",
        &m.signing.public_key_hex[..16]
    );
    if m.toolchain.compiler.is_none() {
        println!("- No compiler was detected at seal time; replay may require manual toolchain setup.");
    }

    Ok(())
}
