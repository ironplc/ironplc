use std::path::PathBuf;

use clap::Parser;

mod cli;
mod logger;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(name = "ironplcvm", about = "IronPLC bytecode virtual machine")]
struct Args {
    /// Turn on verbose logging. Repeat to increase verbosity.
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Sets the logging to write to a file.
    #[arg(short, long)]
    log_file: Option<PathBuf>,

    /// Selects the subcommand.
    #[command(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    /// Loads and executes a bytecode container file.
    Run {
        /// Path to the bytecode container file (.iplc).
        file: PathBuf,

        /// Write variable dump to the specified file after execution.
        #[arg(long)]
        dump_vars: Option<PathBuf>,

        /// Run N scheduling rounds then stop (default: continuous until Ctrl+C).
        #[arg(long)]
        scans: Option<u64>,
    },
    /// Benchmarks a bytecode container by running it many times and reporting timing statistics.
    Benchmark {
        /// Path to the bytecode container file (.iplc).
        file: PathBuf,

        /// Number of measured scan cycles (default: 10000).
        #[arg(long, default_value_t = 10000)]
        cycles: u64,

        /// Number of warmup scan cycles before measurement (default: 1000).
        #[arg(long, default_value_t = 1000)]
        warmup: u64,
    },
    /// Prints the version number of the virtual machine.
    Version,
}

pub fn main() -> Result<(), String> {
    let args = Args::parse();

    logger::configure(args.verbose, args.log_file)?;

    match args.action {
        Action::Run {
            file,
            dump_vars,
            scans,
        } => cli::run(&file, dump_vars.as_deref(), scans),
        Action::Benchmark {
            file,
            cycles,
            warmup,
        } => cli::benchmark(&file, cycles, warmup),
        Action::Version => {
            println!("ironplcvm version {VERSION}");
            Ok(())
        }
    }
}
