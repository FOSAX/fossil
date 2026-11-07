use anyhow::{bail, Context, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;
use std::fs;
use std::path::PathBuf;

fn key_dir() -> Result<PathBuf> {
    let base = dirs_home()?;
    let dir = base.join(".config").join("fossil");
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn dirs_home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME environment variable is not set")
}

/// Loads the user's Fossil signing keypair, generating one on first use.
/// This is a trust-on-first-use identity: Fossil does not operate a PKI.
/// Long-term verification should pin the expected public key out of band.
pub fn load_or_create_signing_key() -> Result<SigningKey> {
    let path = key_dir()?.join("ed25519.key");
    if path.exists() {
        let bytes = fs::read(&path)?;
        if bytes.len() != 32 {
            bail!("corrupt signing key at {}", path.display());
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        Ok(SigningKey::from_bytes(&arr))
    } else {
        let key = SigningKey::generate(&mut OsRng);
        fs::write(&path, key.to_bytes())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o600);
            fs::set_permissions(&path, perms)?;
        }
        Ok(key)
    }
}

pub fn sign(key: &SigningKey, data: &[u8]) -> Signature {
    key.sign(data)
}

pub fn verify(public_key_hex: &str, data: &[u8], signature_hex: &str) -> Result<bool> {
    let pub_bytes = hex::decode(public_key_hex).context("invalid public key hex")?;
    if pub_bytes.len() != 32 {
        bail!("public key must be 32 bytes");
    }
    let mut pk_arr = [0u8; 32];
    pk_arr.copy_from_slice(&pub_bytes);
    let verifying_key = VerifyingKey::from_bytes(&pk_arr).context("invalid public key")?;

    let sig_bytes = hex::decode(signature_hex).context("invalid signature hex")?;
    if sig_bytes.len() != 64 {
        bail!("signature must be 64 bytes");
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);
    let signature = Signature::from_bytes(&sig_arr);

    Ok(verifying_key.verify(data, &signature).is_ok())
}
