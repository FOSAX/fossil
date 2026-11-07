# Fossil

[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-0.1.0-informational.svg)](Cargo.toml)
[![Rust](https://img.shields.io/badge/rust-1.94%2B-orange.svg)](https://www.rust-lang.org)

*Ship software that can still be rebuilt in 50 years.*

Fossil is an open source CLI tool that packages an embedded software project  - 
source code, toolchain metadata, binaries, documentation and integrity proofs - into a
self-contained, signed `.fossil` archive that stays rebuildable decades later, even
offline.

Embedded firmware is often shipped as a bare `firmware.elf`. Over time developers move
on, compilers disappear, dependencies drift, and the sources on file no longer exactly
match what's actually deployed. For a satellite, a probe, or any critical system meant
to run for 10, 20 or 50 years, that's a serious risk: nobody can prove what's running
on board, or faithfully rebuild the software if a fix becomes necessary.

Fossil is not a flight framework, an RTOS or a compiler. It does not replace F' (F Prime), VML,
SCL, FreeRTOS, Zephyr or a CI/CD system. It sits **on top of** the existing build chain
as a preservation, audit and reproducibility layer:

```text
C / Ada / Rust firmware
        |
        v
CMake / Make / Cargo / GPRbuild
        |
        v
Fossil: packaging, hermetic build, verification, rebuild
        |
        v
Self-contained long-term .fossil archive
```

## Table of contents

- [Installation](#installation)
- [Quick start](#quick-start)
- [Commands](#commands)
  - [`fossil seal`](#fossil-seal)
  - [`fossil verify`](#fossil-verify)
  - [`fossil replay`](#fossil-replay)
  - [`fossil inspect`](#fossil-inspect)
  - [`fossil diff`](#fossil-diff)
  - [`fossil explain`](#fossil-explain)
- [Archive format](#archive-format)
- [How `seal` builds the canonical binary](#how-seal-builds-the-canonical-binary)
- [Hermetic replay (Docker / Podman)](#hermetic-replay-docker--podman)
- [Trust model](#trust-model)
- [Toolchain detection](#toolchain-detection)
- [Current limitations](#current-limitations)
- [License](#license)

## Installation

Fossil is a single static-ish binary with no runtime dependencies of its own. Docker or
Podman are optional on every install method below, and only needed to get hermetic
replay (see [Hermetic replay](#hermetic-replay-docker--podman)) - Fossil works without
them, just with a weaker reproducibility guarantee.

### Supported platforms

| OS | Architecture | Target triple | Install method |
|---|---|---|---|
| Linux | x86_64 | `x86_64-unknown-linux-gnu` | `install.sh` or build from source |
| Linux | aarch64 | `aarch64-unknown-linux-gnu` | `install.sh` or build from source |
| macOS | x86_64 (Intel) | `x86_64-apple-darwin` | `install.sh` or build from source |
| macOS | aarch64 (Apple Silicon) | `aarch64-apple-darwin` | `install.sh` or build from source |
| Windows | x86_64 | `x86_64-pc-windows-msvc` | manual download (see below) or build from source |

Other platforms (other architectures, `musl` targets, BSDs, etc.) aren't published yet;
build from source on those.

### Method 1: prebuilt binary (recommended, Linux/macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/FOSAX/fossil/main/install.sh | sh
```

What this does, step by step:

1. Detects your OS (`uname -s`) and CPU architecture (`uname -m`).
2. Picks the matching release asset, e.g. `fossil-x86_64-unknown-linux-gnu.tar.gz`.
3. Downloads it from the project's GitHub Releases (latest tag by default).
4. Extracts the `fossil` binary and installs it to `~/.local/bin/fossil`.
5. Warns you if `~/.local/bin` isn't on your `PATH`.

No Rust toolchain needed - the binary is self-contained.

Environment variables to override the defaults:

| Variable | Default | Purpose |
|---|---|---|
| `FOSSIL_INSTALL_DIR` | `$HOME/.local/bin` | Where the binary gets installed. |
| `FOSSIL_VERSION` | `latest` | A specific release tag (e.g. `v0.1.0`) instead of the newest one. |
| `FOSSIL_REPO` | `FOSAX/fossil` | Fork or mirror to install from. |

Example: install a pinned version system-wide (needs `sudo` for `/usr/local/bin`):

```bash
curl -fsSL https://raw.githubusercontent.com/FOSAX/fossil/main/install.sh | \
    FOSSIL_VERSION=v0.1.0 FOSSIL_INSTALL_DIR=/usr/local/bin sh
```

To uninstall: `rm "$(command -v fossil)"`.

### Method 1b: manual download (Windows, or any platform without `sh`)

`install.sh` is a POSIX shell script and doesn't run natively on Windows (use it from
WSL or Git Bash if you have either). Otherwise:

1. Go to the project's GitHub Releases page.
2. Download `fossil-x86_64-pc-windows-msvc.zip` (or the archive matching your platform).
3. Extract it; you get a `fossil.exe` (or `fossil` on Linux/macOS) plus `README.md` and
   `LICENSE`.
4. Move the binary somewhere on your `PATH`, or add its folder to `PATH` directly
   (System Properties -> Environment Variables on Windows).
5. Confirm it works: `fossil --version`.

### Method 2: build from source with Cargo

Use this if you're on an unpublished platform, want a specific git ref, or plan to
modify Fossil itself.

Requirements: [Rust](https://rustup.rs/) (edition 2021, installed via `rustup`).

```bash
git clone https://github.com/FOSAX/fossil.git
cd fossil
cargo build --release
./target/release/fossil --help
```

The binary ends up at `target/release/fossil`. Install it onto your `PATH`:

```bash
cp target/release/fossil ~/.local/bin/
# or, to always build the latest checkout straight onto your PATH:
cargo install --path . --locked
```

### Method 2b: via crates.io

Once published, this works on any platform with Rust installed, with no git clone
needed. The crate is named `fossil-cli` (the `fossil` name was already taken on
crates.io), but the installed binary is still called `fossil`:

```bash
cargo install fossil-cli --locked
fossil --version
```

### Method 3: in a GitHub Actions workflow

Use the bundled composite action instead of installing manually in CI:

```yaml
- uses: FOSAX/fossil@v0.1.0
  with:
    command: seal
    args: "./firmware --hardware \"ARM Cortex-M4\""
```

Under the hood this just runs `install.sh` (pinned to the `version` input) and then
`fossil <command> <args>`. See [`action.yml`](action.yml) for the full list of inputs
(`command`, `args`, `version`).

### Verifying your installation

Regardless of the method used:

```bash
fossil --version
fossil --help
```

If Docker or Podman is installed, `fossil seal` will automatically use it for hermetic
builds - no extra configuration needed.

## Quick start

```bash
# Inside a C project with a Makefile or a CMakeLists.txt:
fossil seal ./satellite-flight-controller --hardware "ARM Cortex-M4"

# Years later, on a different machine, offline:
fossil verify satellite-flight-controller.fossil
fossil replay satellite-flight-controller.fossil
fossil inspect satellite-flight-controller.fossil
fossil explain satellite-flight-controller.fossil
```

## Commands

### `fossil seal`

```text
fossil seal <PROJECT_DIR>
            [-o, --output <FILE>]
            [-n, --name <NAME>]
            [-b, --binary <PATH>]...
            [--build-command <CMD>]
            [--compiler <NAME>]
            [--build-system <NAME>]
            [--hardware <DESCRIPTION>]
```

Archives a project into a `.fossil` package:

| Option | Effect |
|---|---|
| `PROJECT_DIR` | Project directory to seal (required). |
| `-o, --output` | Path of the produced `.fossil` file. Defaults to `<name>.fossil` in the current directory. |
| `-n, --name` | Project name recorded in the manifest. Defaults to the directory name. |
| `-b, --binary` | Relative path(s) of the binary/binaries to include, repeatable. If omitted, Fossil **builds the project itself** (see below) and auto-detects the produced artifacts (`.elf`, `.bin`, `.hex`, `.out`). |
| `--build-command` | Exact command to record and later replay. Defaults to one derived from the detected build system (`make`, `cmake -S . -B build && cmake --build build`, `cargo build --release`, `gprbuild -p`, `ninja`). |
| `--compiler` | Force the compiler name instead of auto-detection (gcc, clang, arm-none-eabi-gcc, riscv64-unknown-elf-gcc, gnatmake, gprbuild). |
| `--build-system` | Force the build system name instead of auto-detection (make, cmake, ninja, cargo). |
| `--hardware` | Free-text hardware target description (e.g. `"ARM Cortex-M4"`), stored in the manifest. |

Example:

```bash
$ fossil seal ./demo-firmware --hardware "ARM Cortex-M4"
-> Sealing /path/to/demo-firmware
OK: Toolchain captured: gcc (Ubuntu 15.2.0-16ubuntu1) 15.2.0
-> Building hermetic docker image for replay...
OK: Hermetic image built: fossil-demo-firmware:latest (sha256:ebd948a53b98)
-> Building firmware inside hermetic docker image...
gcc -O2 -o firmware.elf main.c
OK: Sources archived (3 files)
OK: Binary hashed: sha256:c43f22998aad
OK: Manifest signed (ed25519)
OK: Fossil created: /path/to/demo-firmware.fossil
```

### `fossil verify`

```text
fossil verify <FOSSIL_FILE>
```

Recomputes the SHA-256/BLAKE3 of every source file and binary in the archive, compares
them against the manifest, then checks the manifest's Ed25519 signature. Exits with
code `1` if anything doesn't match (missing file, hash mismatch, invalid signature).

```bash
$ fossil verify demo-firmware.fossil
-> Verifying demo-firmware.fossil
OK: Source tree matches manifest (3 files)
OK: Binary matches manifest: firmware.elf
OK: Signature valid (ed25519, key 3b1debe59db7...)
Fossil package verified: consistent and signed.
```

### `fossil replay`

```text
fossil replay <FOSSIL_FILE>
              [-o, --output-dir <DIR>]
              [-e, --engine auto|host|docker|podman]
```

Extracts the archive, replays the recorded build command, and compares the result
bit-for-bit against the sealed binary.

| `--engine` | Behavior |
|---|---|
| `auto` (default) | Uses the container image pinned in the manifest if Docker/Podman is installed; otherwise falls back to the host toolchain with a warning. |
| `host` | Ignores any recorded image, builds directly with whatever toolchain is installed on the current machine. |
| `docker` / `podman` | Requires an image from the requested engine in the manifest and that engine to be installed; fails with a clear error otherwise (never falls back silently). |

`-o, --output-dir` keeps the extracted/rebuilt files in a persistent directory instead
of a temp directory discarded afterwards - useful for inspecting a failure.

```bash
$ fossil replay demo-firmware.fossil
-> Replaying demo-firmware.fossil
-> Running build command inside hermetic docker image (sha256:ebd948a53b98...), network disabled
    make
gcc -O2 -o firmware.elf main.c
OK: Rebuilt binary matches original: firmware.elf
Replay succeeded: rebuilt binary is bit-identical.
```

### `fossil inspect`

```text
fossil inspect <FOSSIL_FILE> [--json]
```

Prints the hardware target, toolchain (compiler, build system, host), build command,
source and binary listing, and signing information. `--json` prints the full raw
manifest, handy for scripting.

### `fossil diff`

```text
fossil diff <FOSSIL_A> <FOSSIL_B>
```

Compares two `.fossil` archives: compiler, build command, added/removed/changed source
files (by hash), and differing binaries.

```bash
$ fossil diff v1.fossil v2.fossil
A: demo-firmware  vs  B: demo-firmware

Toolchain
  compiler unchanged: gcc (Ubuntu 15.2.0-16ubuntu1) 15.2.0

Build command
  unchanged: make

Sources
  0 added, 0 removed, 1 changed
    ~ main.c

Binaries
    ~ firmware.elf: sha256:a1dfc75ef657 -> sha256:c7b4988e77db
```

### `fossil explain`

```text
fossil explain <FOSSIL_FILE>
```

Generates a human-readable Markdown report: provenance, toolchain and build system
used, hardware target, binaries and their hashes, signature status, and known caveats
(e.g. "no compiler detected at seal time").

## Archive format

A `.fossil` file is a tar.gz archive containing:

```text
my-firmware.fossil/
|-- sources/              # exact source snapshot (excluding binaries and build artifacts)
|-- toolchain/
|   |-- compiler.txt       # compiler actually used to produce the binary
|   |-- build-system.txt
|   |-- linker.txt
|   `-- Dockerfile          # generated hermetic environment (base of the pinned image)
|-- build-recipe/
|   `-- build.sh            # shell script reproducing the recorded build command
|-- <binaries>              # binary artifacts (.elf, .bin, .hex, .out, ...)
|-- manifest.json           # machine-readable description of the whole package
|-- checksums.sha256        # SHA-256 of every file in the package (sha256sum format)
|-- signatures/
|   |-- manifest.sig         # Ed25519 signature over the manifest (hex)
|   `-- public_key.hex       # signer's public key
`-- docs/
    |-- FOSSIL.md             # auto-generated explanation of the format
    `-- README.md              # original project's README, copied if present
```

`manifest.json` includes, among other fields: `fossil_format_version`,
`fossil_cli_version`, `project_name`, `created_at`, the `source.files[]` list (path,
sha256, blake3, size), `toolchain` (compiler, build system, linker, host OS/arch, and
`container` if a hermetic image was built), `build` (command, working directory, and
relevant captured environment variables - `CC`, `CXX`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`,
`AR`), `binaries[]`, `hardware` (target, architecture, notes), and `signing` (algorithm,
public key).

## How `seal` builds the canonical binary

If `--binary` isn't given, `fossil seal` doesn't just look for a file that's already
sitting in the directory: it **builds the project itself**, to guarantee that the
signed binary is actually reproducible by `replay` later on.

1. Detect the installed compiler, build system and linker.
2. Generate a matching `Dockerfile`, and build the image if Docker/Podman is available  - 
   its content-addressed image ID (`sha256:...`) gets pinned in the manifest.
3. Delete any existing binary artifacts (`.elf`, `.bin`, `.hex`, `.out`) and any leftover
   CMake `build/` directory from a previous run (a stale `CMakeCache.txt` embeds
   absolute paths that break the next build).
4. Build the project **inside the hermetic image** if one exists, otherwise with the
   host toolchain.
5. If the build ran in a container, the compiler and build-system versions recorded in
   the manifest are the ones **actually present in the image** (not the ones on the
   machine that ran `seal`) - otherwise the manifest would misrepresent what produced
   the binary.

With `--binary`, this behavior is disabled: the given file(s) are captured as-is, with
no rebuild (useful when the binary was intentionally produced elsewhere).

## Hermetic replay (Docker / Podman)

The goal: `fossil replay` rebuilds inside the **same frozen environment** that produced
the sealed binary, not on whatever machine happens to run `replay`.

```bash
fossil replay ./demo.fossil                  # auto: pinned image if available, else host
fossil replay ./demo.fossil --engine docker  # requires the recorded image, errors otherwise
fossil replay ./demo.fossil --engine host    # ignores the image, uses the local toolchain
```

With `--engine docker`/`podman`, the rebuild runs with `--network none` (no network
access at all) and bind-mounts the extracted sources directory at `/work` - the result
lands directly on the host with no copy step, then gets compared bit-for-bit against the
sealed binary.

The generated `Dockerfile` is a simple heuristic based on `debian:bookworm-slim` plus
`apt` packages chosen from the detected compiler (`build-essential`, `clang`,
`gcc-arm-none-eabi`, `gnat`/`gprbuild`, `cmake`, `ninja-build`, ...). It's stored in plain
text at `toolchain/Dockerfile` and stays hand-editable for toolchains this heuristic
doesn't know about, before re-running `seal`.

If no container engine is found at `seal` time, Fossil says so clearly in its output and
falls back to the host toolchain - `replay` still works, just without the hermetic
guarantee.

## Trust model

The Ed25519 signing key is generated automatically on the first `seal`
(`~/.config/fossil/ed25519.key`, `0600` permissions): this is a *trust-on-first-use*
model. Fossil does not run a certificate authority. For reliable long-term
verification, the signer's public key needs to be communicated and archived through an
independent channel (a separate repository, a paper record, etc.) - that's what proves
identity, not the signature check that `fossil verify` performs on its own.

## Toolchain detection

Compilers are probed in this order: `arm-none-eabi-gcc`, `riscv64-unknown-elf-gcc`,
`gcc`, `clang`, `gnatmake`, `gprbuild`. Build systems: detected first from project files
(`CMakeLists.txt` -> cmake, `Cargo.toml` -> cargo, `Makefile` -> make), then falling back to
`cmake`, `make`, `ninja`, `cargo` in that order. `--compiler` and `--build-system` let you
override the guess when auto-detection picks the wrong one or several toolchains coexist
on the machine.

## Current limitations

- Initial target: C projects (Make/CMake). Ada/SPARK and embedded Rust are planned but
  not yet exercised end-to-end.
- The generated `Dockerfile` is a simple heuristic; exotic toolchains (specific
  ARM/RISC-V cross-compilers, pinned-to-the-patch versions) still need hand-editing
  before re-running `seal`.
- No non-determinism detection (embedded timestamps, absolute paths in debug symbols,
  link order): if the build isn't inherently reproducible, `replay` will report a
  mismatch without explaining the exact cause.
- No SBOM (CycloneDX/SPDX), no signed provenance file (source repo/commit), no PKI  - 
  only the self-hosted Ed25519 signature exists for now.
- No automatic detection of communication protocols or the firmware's memory map.
- The canonical build inside the container always runs with no network access, but
  building the image itself (`docker build`) does need network access to download `apt`
  packages - only the later `replay` is guaranteed offline, once the image is built and
  cached locally.

## License

Apache-2.0 - see [LICENSE](LICENSE).
