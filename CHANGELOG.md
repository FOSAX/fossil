# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project intends to follow [Semantic Versioning](https://semver.org/)
once it leaves the 0.x alpha line.

## [0.1.0] - 2026-11-07

### Added

- `fossil seal`: archives sources, toolchain metadata, binaries and docs into a
  signed `.fossil` package.
  - Auto-detects compiler, build system and linker on the host.
  - If `--binary` isn't given, builds the project itself rather than trusting a
    file already sitting in the directory.
  - Generates a `Dockerfile` matching the detected toolchain and, if Docker or
    Podman is available, builds a hermetic image, pins its content-addressed
    image ID in the manifest, and builds the canonical binary inside that image.
  - Records the compiler/build-system versions actually present in the hermetic
    image (not the host's), so the manifest never misrepresents what produced
    the binary.
  - Signs the manifest with Ed25519 (key auto-generated on first use, stored at
    `~/.config/fossil/ed25519.key`).
- `fossil verify`: recomputes SHA-256/BLAKE3 hashes of every source file and
  binary, and checks the manifest's Ed25519 signature.
- `fossil replay`: rebuilds the firmware offline and compares the result
  bit-for-bit against the sealed binary. Supports `--engine auto|host|docker|podman`
  to control whether the pinned hermetic image or the local toolchain is used.
- `fossil inspect`: prints hardware target, toolchain, build command, source and
  binary listing, and signing info (`--json` for the raw manifest).
- `fossil diff`: compares two `.fossil` packages (toolchain, build command,
  added/removed/changed sources, differing binaries).
- `fossil explain`: generates a human-readable report about a package's
  provenance, toolchain, and integrity status.
- `install.sh`: detects OS/architecture and installs the matching prebuilt
  binary from GitHub Releases.
- `action.yml`: composite GitHub Action wrapping install + any Fossil subcommand
  for use in CI workflows.
- `.github/workflows/release.yml`: builds and publishes binaries for
  Linux (x86_64, aarch64), macOS (x86_64, aarch64) and Windows (x86_64) on every
  `vX.Y.Z` tag.

### Known limitations

- Initial target is C projects (Make/CMake). Ada/SPARK and embedded Rust are
  planned but not yet exercised end-to-end.
- The generated `Dockerfile` is a simple heuristic; exotic toolchains need
  hand-editing before re-running `seal`.
- No non-determinism detection (embedded timestamps, absolute paths in debug
  symbols, link order) - a non-reproducible build shows up as a `replay`
  mismatch without explaining the exact cause.
- No SBOM, no signed provenance file, no PKI - only the self-hosted Ed25519
  signature exists for now.
- No automated test suite yet.

[0.1.0]: https://github.com/FOSAX/fossil/releases/tag/v0.1.0
