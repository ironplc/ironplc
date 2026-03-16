use std::path::PathBuf;

use clap::Parser;

use ironplc_parser::options::ParseOptions;
use ironplcc::cli;
use ironplcc::logger;
use ironplcc::lsp;

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

/// IEC 61131-3 standard version to compile against.
#[derive(clap::ValueEnum, Clone, Debug)]
enum StdVersion {
    /// IEC 61131-3:2003 — Edition 2 (default).
    #[value(name = "2003")]
    Iec6113132003,
    /// IEC 61131-3:2013 — enables Edition 3 features such as LTIME.
    #[value(name = "2013")]
    Iec6113132013,
}

/// Shared arguments for commands that operate on source files.
#[derive(clap::Args, Debug)]
struct FileArgs {
    /// Files to include. Directory names can be given to
    /// add all files in the given directory.
    files: Vec<PathBuf>,

    /// Select the IEC 61131-3 standard version to compile against.
    /// Without this flag, only Edition 2 features are accepted.
    #[arg(long = "std-iec-61131-3", default_value = "2003")]
    std_version: StdVersion,

    /// Allow missing semicolons after keyword statements like END_IF and END_STRUCT.
    #[arg(long)]
    allow_missing_semicolon: bool,
}

impl FileArgs {
    fn parse_options(&self) -> ParseOptions {
        let mut options = match self.std_version {
            StdVersion::Iec6113132003 => ParseOptions::default(),
            StdVersion::Iec6113132013 => ParseOptions {
                allow_iec_61131_3_2013: true,
                ..Default::default()
            },
        };
        options.allow_missing_semicolon = self.allow_missing_semicolon;
        options
    }
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    /// The check action checks a file (or set of files) for syntax and semantic correctness.
    ///
    /// When multiple files specified, then the files are checked as a single
    /// compilation unit (essentially by combining the files) for analysis.
    Check {
        #[command(flatten)]
        file_args: FileArgs,
    },
    /// Compiles source files into a bytecode container (.iplc) file.
    ///
    /// When multiple files are specified, then the files are compiled as a single
    /// compilation unit (essentially by combining the files).
    Compile {
        #[command(flatten)]
        file_args: FileArgs,

        /// Output file path for the compiled bytecode container (.iplc).
        #[arg(short, long)]
        output: PathBuf,
    },
    /// The echo action reads (parses) the libraries and writes the context to the
    /// standard output.
    ///
    /// The echo acton is primarily for diagnostics to understand the internal
    /// structure of the parsed files.
    Echo {
        #[command(flatten)]
        file_args: FileArgs,
    },
    /// The tokenize action checks a file if it can be tokenized with all content
    /// matching a token.
    ///
    /// The tokenize acton is primarily for diagnostics to understand the internal
    /// structure of the parsed files.
    Tokenize {
        #[command(flatten)]
        file_args: FileArgs,
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
        Action::Lsp { stdio: _ } => lsp::start(),
        Action::Check { file_args } => {
            cli::check(&file_args.files, file_args.parse_options(), false)
        }
        Action::Compile { file_args, output } => {
            cli::compile(&file_args.files, &output, file_args.parse_options(), false)
        }
        Action::Echo { file_args } => cli::echo(&file_args.files, file_args.parse_options(), false),
        Action::Tokenize { file_args } => {
            cli::tokenize(&file_args.files, file_args.parse_options(), false)
        }
        Action::Version => {
            println!("ironplcc version {VERSION}");
            Ok(())
        }
    }
}
