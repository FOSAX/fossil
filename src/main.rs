mod archive;
mod commands;
mod container;
mod hashing;
mod keys;
mod manifest;
mod toolchain;
mod util;

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Which environment `fossil replay` should rebuild in.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Engine {
    /// Use the recorded container image if available, else fall back to the host toolchain.
    Auto,
    /// Rebuild directly with whatever toolchain is installed on this machine.
    Host,
    /// Rebuild inside the sealed Docker image (error if none was recorded).
    Docker,
    /// Rebuild inside the sealed Podman image (error if none was recorded).
    Podman,
}

/// Fossil - ship software that can still be rebuilt in 50 years.
#[derive(Parser)]
#[command(name = "fossil", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Archive sources, toolchain metadata, binaries and docs into a signed .fossil package.
    Seal {
        /// Directory containing the firmware project to seal.
        project_dir: PathBuf,
        /// Output .fossil file (default: <name>.fossil in the current directory).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Project name recorded in the manifest (default: directory name).
        #[arg(short, long)]
        name: Option<String>,
        /// Binary artifact(s) to include, relative to project_dir. Auto-detected if omitted.
        #[arg(short = 'b', long = "binary")]
        binaries: Vec<PathBuf>,
        /// Exact command used to build the binaries (recorded and replayed later).
        #[arg(long = "build-command")]
        build_command: Option<String>,
        /// Force a specific compiler name instead of auto-detection.
        #[arg(long)]
        compiler: Option<String>,
        /// Force a specific build system name instead of auto-detection.
        #[arg(long = "build-system")]
        build_system: Option<String>,
        /// Hardware target description (e.g. "ARM Cortex-M4").
        #[arg(long = "hardware")]
        hardware_target: Option<String>,
    },
    /// Check that a .fossil package's binary, sources and manifest are consistent and signed.
    Verify {
        fossil_file: PathBuf,
    },
    /// Rebuild the firmware from a .fossil package offline and compare to the sealed binary.
    Replay {
        fossil_file: PathBuf,
        /// Directory to extract and build in (default: a temp directory, discarded after).
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
        /// Rebuild environment: auto (default), host, docker or podman.
        #[arg(short, long, value_enum, default_value = "auto")]
        engine: Engine,
    },
    /// Print target hardware, toolchain, build command and binary metadata.
    Inspect {
        fossil_file: PathBuf,
        /// Print the full manifest as JSON instead of a formatted summary.
        #[arg(long)]
        json: bool,
    },
    /// Compare two .fossil packages: sources, toolchain, build config and binaries.
    Diff {
        fossil_a: PathBuf,
        fossil_b: PathBuf,
    },
    /// Generate a human-readable technical report about a .fossil package.
    Explain {
        fossil_file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Seal {
            project_dir,
            output,
            name,
            binaries,
            build_command,
            compiler,
            build_system,
            hardware_target,
        } => commands::seal::run(commands::seal::SealArgs {
            project_dir,
            output,
            name,
            binaries,
            build_command,
            compiler,
            build_system,
            hardware_target,
        }),
        Commands::Verify { fossil_file } => commands::verify::run(&fossil_file).map(|ok| {
            if !ok {
                std::process::exit(1);
            }
        }),
        Commands::Replay { fossil_file, output_dir, engine } => {
            commands::replay::run(&fossil_file, output_dir, engine).map(|ok| {
                if !ok {
                    std::process::exit(1);
                }
            })
        }
        Commands::Inspect { fossil_file, json } => commands::inspect::run(&fossil_file, json),
        Commands::Diff { fossil_a, fossil_b } => commands::diff::run(&fossil_a, &fossil_b),
        Commands::Explain { fossil_file } => commands::explain::run(&fossil_file),
    };

    if let Err(e) = result {
        eprintln!("fossil: error: {e:#}");
        std::process::exit(1);
    }
}
