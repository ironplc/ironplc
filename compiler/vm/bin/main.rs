use std::path::PathBuf;

use clap::Parser;

use ironplc_vm::cli;
use ironplc_vm::logger;

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
        Action::Version => {
            println!("ironplcvm version {VERSION}");
            Ok(())
        }
    }
}
