use crate::archive::unpack_fossil_to_dir;
use crate::manifest::Manifest;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Unpacks a `.fossil` archive into a fresh temp dir and returns the dir
/// (kept alive by the returned `TempDir` guard) plus the parsed manifest.
pub fn open_fossil(fossil_path: &Path) -> Result<(TempDir, Manifest)> {
    let tmp = TempDir::new().context("creating temp dir")?;
    unpack_fossil_to_dir(fossil_path, tmp.path())?;
    let manifest = load_manifest(tmp.path())?;
    Ok((tmp, manifest))
}

pub fn load_manifest(root: &Path) -> Result<Manifest> {
    let path = root.join("manifest.json");
    let data = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let manifest: Manifest =
        serde_json::from_slice(&data).with_context(|| format!("parsing {}", path.display()))?;
    Ok(manifest)
}

pub const EXCLUDED_DIRS: &[&str] = &[
    ".git", "target", "build", "node_modules", "cmake-build-debug", "cmake-build-release", ".fossil-work",
];

pub fn is_excluded(path: &Path) -> bool {
    path.components().any(|c| {
        let s = c.as_os_str().to_string_lossy();
        EXCLUDED_DIRS.contains(&s.as_ref()) || s.ends_with(".fossil")
    })
}

pub fn default_output_path(project_dir: &Path, name: &str) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| project_dir.to_path_buf());
    cwd.join(format!("{name}.fossil"))
}
