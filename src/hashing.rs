use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

pub struct FileHashes {
    pub sha256: String,
    pub blake3: String,
    pub size: u64,
}

pub fn hash_file(path: &Path) -> Result<FileHashes> {
    let file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut sha256 = Sha256::new();
    let mut blake3 = blake3::Hasher::new();
    let mut buf = [0u8; 64 * 1024];
    let mut size: u64 = 0;
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        sha256.update(&buf[..n]);
        blake3.update(&buf[..n]);
        size += n as u64;
    }
    Ok(FileHashes {
        sha256: hex::encode(sha256.finalize()),
        blake3: blake3.finalize().to_hex().to_string(),
        size,
    })
}
