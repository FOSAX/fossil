use crate::hashing::hash_file;
use crate::keys::verify as verify_sig;
use crate::util::open_fossil;
use anyhow::{Context, Result};
use colored::Colorize;
use std::fs;
use std::path::{Path, PathBuf};

pub fn run(fossil_path: &Path) -> Result<bool> {
    println!("{} Verifying {}", "->".cyan(), fossil_path.display());
    let (tmp, manifest) = open_fossil(fossil_path)?;
    let root = tmp.path();
    let mut ok = true;

    // Source files.
    let mut mismatches = Vec::new();
    for record in &manifest.source.files {
        let path = root.join("sources").join(&record.path);
        match hash_file(&path) {
            Ok(h) if h.sha256 == record.sha256 => {}
            Ok(_) => mismatches.push(format!("hash mismatch: {}", record.path)),
            Err(_) => mismatches.push(format!("missing file: {}", record.path)),
        }
    }
    if mismatches.is_empty() {
        println!("{} Source tree matches manifest ({} files)", "OK:".green(), manifest.source.files.len());
    } else {
        ok = false;
        println!("{} Source mismatches found:", "FAIL:".red());
        for m in &mismatches {
            println!("    - {m}");
        }
    }

    // Binaries.
    for record in &manifest.binaries {
        let path: PathBuf = root.join(&record.path);
        match hash_file(&path) {
            Ok(h) if h.sha256 == record.sha256 => {
                println!("{} Binary matches manifest: {}", "OK:".green(), record.path);
            }
            Ok(h) => {
                ok = false;
                println!(
                    "{} Binary hash mismatch: {} (expected {}, got {})",
                    "FAIL:".red(),
                    record.path,
                    &record.sha256[..12],
                    &h.sha256[..12]
                );
            }
            Err(_) => {
                ok = false;
                println!("{} Binary missing from archive: {}", "FAIL:".red(), record.path);
            }
        }
    }

    // Signature.
    let manifest_bytes = manifest.canonical_bytes();
    let sig_path = root.join("signatures/manifest.sig");
    let sig_hex = fs::read_to_string(&sig_path)
        .with_context(|| format!("reading {}", sig_path.display()))?;
    let sig_valid = verify_sig(&manifest.signing.public_key_hex, &manifest_bytes, sig_hex.trim())?;
    if sig_valid {
        println!(
            "{} Signature valid (ed25519, key {}...)",
            "OK:".green(),
            &manifest.signing.public_key_hex[..12]
        );
    } else {
        ok = false;
        println!("{} Signature INVALID - manifest may have been tampered with", "FAIL:".red().bold());
    }

    if ok {
        println!("{}", "Fossil package verified: consistent and signed.".green().bold());
    } else {
        println!("{}", "Fossil package FAILED verification.".red().bold());
    }
    Ok(ok)
}
