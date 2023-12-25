use std::path::PathBuf;

use clap::Parser;

use ironplcc::cli;
use ironplcc::logger;
use ironplcc::lsp;
use ironplcc::project::FileBackedProject;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(name = "ironplcc", about = "IronPLC compiler")]
struct Args {
    /// Turn on verbose logging. Repeat to increase verbosity.
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Selects the subcommand.
    #[command(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    /// Check a file (or set of files) for syntax and semantic correctness.
    ///
    /// When multiple files specified, then the files are checked as a single
    /// compilation unit (essentially by combining the files) for analysis.
    Check {
        /// Files to include in the check. Directory names can be given to
        /// add all files in the given directory.
        files: Vec<PathBuf>,
    },
    /// Run in Language Server Protocol mode to integrate with development tools.
    Lsp {
        #[arg(long)]
        stdio: bool,
    },
    /// Prints the version number of the compiler.
    Version,
}

pub fn main() -> Result<(), String> {
    let args = Args::parse();

    logger::configure(args.verbose)?;

    match args.action {
        Action::Lsp { stdio: _ } => {
            let proj = Box::new(FileBackedProject::new());
            lsp::start(proj)
        }
        Action::Check { files } => {
            cli::check(files, false).map_err(|_e| String::from("Error running check"))
        }
        Action::Version => {
            println!("ironplcc version {}", VERSION);
            Ok(())
        }
    }
}
