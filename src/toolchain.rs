use crate::manifest::ToolInfo;
use std::process::Command;

const COMPILER_CANDIDATES: &[&str] = &[
    "arm-none-eabi-gcc",
    "riscv64-unknown-elf-gcc",
    "gcc",
    "clang",
    "gnatmake",
    "gprbuild",
];

const BUILD_SYSTEM_CANDIDATES: &[&str] = &["cmake", "make", "ninja", "cargo"];

fn tool_version(bin: &str) -> Option<String> {
    for flag in ["--version", "-version", "-v"] {
        if let Ok(out) = Command::new(bin).arg(flag).output() {
            let text = if !out.stdout.is_empty() {
                String::from_utf8_lossy(&out.stdout).to_string()
            } else {
                String::from_utf8_lossy(&out.stderr).to_string()
            };
            if let Some(first_line) = text.lines().next() {
                if !first_line.trim().is_empty() {
                    return Some(first_line.trim().to_string());
                }
            }
        }
    }
    None
}

fn find_tool(bin: &str) -> Option<ToolInfo> {
    let path = which::which(bin).ok()?;
    let version = tool_version(bin).unwrap_or_else(|| "unknown".to_string());
    Some(ToolInfo {
        name: bin.to_string(),
        version,
        path: Some(path.display().to_string()),
    })
}

pub fn detect_compiler(preferred: Option<&str>) -> Option<ToolInfo> {
    if let Some(name) = preferred {
        if let Some(tool) = find_tool(name) {
            return Some(tool);
        }
    }
    COMPILER_CANDIDATES.iter().find_map(|c| find_tool(c))
}

pub fn detect_build_system(preferred: Option<&str>, project_dir: &std::path::Path) -> Option<ToolInfo> {
    if let Some(name) = preferred {
        if let Some(tool) = find_tool(name) {
            return Some(tool);
        }
    }
    if project_dir.join("CMakeLists.txt").exists() {
        return find_tool("cmake");
    }
    if project_dir.join("Cargo.toml").exists() {
        return find_tool("cargo");
    }
    if project_dir.join("Makefile").exists() || project_dir.join("makefile").exists() {
        return find_tool("make");
    }
    BUILD_SYSTEM_CANDIDATES.iter().find_map(|c| find_tool(c))
}

pub fn detect_linker() -> Option<ToolInfo> {
    find_tool("ld")
}

pub fn detect_os_arch() -> (String, String) {
    (std::env::consts::OS.to_string(), std::env::consts::ARCH.to_string())
}
