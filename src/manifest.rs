use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const FOSSIL_FORMAT_VERSION: &str = "1";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FileRecord {
    pub path: String,
    pub sha256: String,
    pub blake3: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SourceInfo {
    pub file_count: usize,
    pub files: Vec<FileRecord>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ToolInfo {
    pub name: String,
    pub version: String,
    pub path: Option<String>,
}

impl ToolInfo {
    /// Human-readable "name version" string, avoiding duplication when the
    /// detected version banner already repeats the tool name (e.g. gcc/clang).
    pub fn display(&self) -> String {
        if self.version.to_lowercase().contains(&self.name.to_lowercase()) {
            self.version.clone()
        } else {
            format!("{} {}", self.name, self.version)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ToolchainInfo {
    pub compiler: Option<ToolInfo>,
    pub build_system: Option<ToolInfo>,
    pub other_tools: Vec<ToolInfo>,
    pub os: String,
    pub arch: String,
    pub container: Option<ContainerInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ContainerInfo {
    /// "docker" or "podman".
    pub engine: String,
    /// Local image tag, for readability only - not what gets pinned.
    pub image_tag: String,
    /// Content-addressed image ID (e.g. "sha256:..."), pinned for replay.
    pub image_digest: String,
    /// Path to the Dockerfile inside the sealed package.
    pub dockerfile: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct BuildInfo {
    pub command: String,
    pub working_dir: String,
    pub env: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BinaryRecord {
    pub path: String,
    pub sha256: String,
    pub blake3: String,
    pub size: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct HardwareInfo {
    pub target: String,
    pub arch: String,
    pub notes: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct SigningInfo {
    pub algorithm: String,
    pub public_key_hex: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Manifest {
    pub fossil_format_version: String,
    pub fossil_cli_version: String,
    pub project_name: String,
    pub created_at: String,
    pub source: SourceInfo,
    pub toolchain: ToolchainInfo,
    pub build: BuildInfo,
    pub binaries: Vec<BinaryRecord>,
    pub hardware: HardwareInfo,
    pub signing: SigningInfo,
}

impl Manifest {
    /// Canonical bytes used for hashing/signing: pretty-printed with sorted keys
    /// is not guaranteed by serde_json map order for structs, but struct field
    /// order is stable, so compact JSON serialization is deterministic here.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("manifest serialization cannot fail")
    }
}
