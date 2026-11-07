use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs::File;
use std::path::Path;

pub fn pack_dir_to_fossil(dir: &Path, output: &Path) -> Result<()> {
    let file = File::create(output).with_context(|| format!("creating {}", output.display()))?;
    let enc = GzEncoder::new(file, Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_dir_all(".", dir)
        .with_context(|| format!("archiving {}", dir.display()))?;
    tar.finish()?;
    Ok(())
}

pub fn unpack_fossil_to_dir(fossil_path: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    let file = File::open(fossil_path)
        .with_context(|| format!("opening {}", fossil_path.display()))?;
    let dec = GzDecoder::new(file);
    let mut tar = tar::Archive::new(dec);
    tar.unpack(dest)
        .with_context(|| format!("extracting {}", fossil_path.display()))?;
    Ok(())
}
