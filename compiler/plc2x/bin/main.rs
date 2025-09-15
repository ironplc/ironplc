use std::path::PathBuf;

use clap::Parser;

use ironplcc::cli;
use ironplcc::logger;
use ironplcc::lsp;
use ironplcc::lsp_project::LspProject;
use ironplcc::project::FileBackedProject;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser, Debug)]
#[command(name = "ironplcc", about = "IronPLC compiler")]
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
    /// The check action checks a file (or set of files) for syntax and semantic correctness.
    ///
    /// When multiple files specified, then the files are checked as a single
    /// compilation unit (essentially by combining the files) for analysis.
    Check {
        /// Files to include in the check. Directory names can be given to
        /// add all files in the given directory.
        files: Vec<PathBuf>,
    },
    /// The echo action reads (parses) the libraries and writes the context to the
    /// standard output.
    ///
    /// The echo acton is primarily for diagnostics to understand the internal
    /// structure of the parsed files.
    Echo {
        /// Files to include in the check. Directory names can be given to
        /// add all files in the given directory.
        files: Vec<PathBuf>,
    },
    /// The tokenize action checks a file if it can be tokenized with all content
    /// matching a token.
    ///
    /// The tokenize acton is primarily for diagnostics to understand the internal
    /// structure of the parsed files.
    Tokenize {
        /// Files to tokenize.
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
    // The Err variant is a String so that the command line shows a nice message.
    let args = Args::parse();

    logger::configure(args.verbose, args.log_file)?;

    match args.action {
        Action::Lsp { stdio: _ } => {
            let proj = LspProject::new(Box::<FileBackedProject>::default());
            lsp::start(proj)
        }
        Action::Check { files } => cli::check(&files, false),
        Action::Echo { files } => cli::echo(&files, false),
        Action::Tokenize { files } => cli::tokenize(&files, false),
        Action::Version => {
            println!("ironplcc version {VERSION}");
            Ok(())
        }
    }
}
