use crate::manifest::Manifest;
use crate::util::open_fossil;
use anyhow::Result;
use colored::Colorize;
use std::collections::BTreeMap;
use std::path::Path;

pub fn run(a_path: &Path, b_path: &Path) -> Result<()> {
    let (_tmp_a, a) = open_fossil(a_path)?;
    let (_tmp_b, b) = open_fossil(b_path)?;

    println!("{} {}  vs  {} {}", "A:".bold(), a.project_name, "B:".bold(), b.project_name);
    println!();

    diff_toolchain(&a, &b);
    diff_build(&a, &b);
    diff_sources(&a, &b);
    diff_binaries(&a, &b);

    Ok(())
}

fn diff_toolchain(a: &Manifest, b: &Manifest) {
    println!("{}", "Toolchain".bold());
    let fmt = |m: &Manifest| -> String {
        m.toolchain
            .compiler
            .as_ref()
            .map(|c| c.display())
            .unwrap_or_else(|| "unknown".to_string())
    };
    let ca = fmt(a);
    let cb = fmt(b);
    if ca == cb {
        println!("  compiler unchanged: {ca}");
    } else {
        println!("  {} {} -> {}", "compiler changed:".yellow(), ca, cb);
    }
    println!();
}

fn diff_build(a: &Manifest, b: &Manifest) {
    println!("{}", "Build command".bold());
    if a.build.command == b.build.command {
        println!("  unchanged: {}", a.build.command);
    } else {
        println!("  {}", "changed:".yellow());
        println!("    A: {}", a.build.command);
        println!("    B: {}", b.build.command);
    }
    println!();
}

fn diff_sources(a: &Manifest, b: &Manifest) {
    println!("{}", "Sources".bold());
    let map_a: BTreeMap<_, _> = a.source.files.iter().map(|f| (f.path.clone(), f)).collect();
    let map_b: BTreeMap<_, _> = b.source.files.iter().map(|f| (f.path.clone(), f)).collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();

    for (path, fb) in &map_b {
        match map_a.get(path) {
            None => added.push(path.clone()),
            Some(fa) if fa.sha256 != fb.sha256 => changed.push(path.clone()),
            _ => {}
        }
    }
    for path in map_a.keys() {
        if !map_b.contains_key(path) {
            removed.push(path.clone());
        }
    }

    println!("  {} added, {} removed, {} changed", added.len(), removed.len(), changed.len());
    for p in &added {
        println!("    {} {}", "+".green(), p);
    }
    for p in &removed {
        println!("    {} {}", "-".red(), p);
    }
    for p in &changed {
        println!("    {} {}", "~".yellow(), p);
    }
    println!();
}

fn diff_binaries(a: &Manifest, b: &Manifest) {
    println!("{}", "Binaries".bold());
    let map_a: BTreeMap<_, _> = a.binaries.iter().map(|f| (f.path.clone(), f)).collect();
    let map_b: BTreeMap<_, _> = b.binaries.iter().map(|f| (f.path.clone(), f)).collect();
    for (path, fb) in &map_b {
        match map_a.get(path) {
            None => println!("    {} {} (new)", "+".green(), path),
            Some(fa) if fa.sha256 != fb.sha256 => {
                println!(
                    "    {} {}: sha256:{} -> sha256:{}",
                    "~".yellow(),
                    path,
                    &fa.sha256[..12],
                    &fb.sha256[..12]
                );
            }
            Some(_) => println!("    = {} unchanged", path),
        }
    }
    for path in map_a.keys() {
        if !map_b.contains_key(path) {
            println!("    {} {} (removed)", "-".red(), path);
        }
    }
}
