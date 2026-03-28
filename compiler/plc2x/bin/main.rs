use std::path::PathBuf;

use clap::Parser;

use ironplc_parser::options::{describe_dialects, Dialect, ParseOptions};
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

/// Language dialect preset.
///
/// A dialect selects the IEC 61131-3 edition and a default set of vendor
/// extensions.  Individual `--allow-*` flags can still override on top.
#[derive(clap::ValueEnum, Clone, Debug, Default)]
enum CliDialect {
    /// Strict IEC 61131-3:2003 (Edition 2).  No vendor extensions.
    #[default]
    #[value(name = "iec61131-3-ed2")]
    Iec61131_3Ed2,
    /// Strict IEC 61131-3:2013 (Edition 3).  No vendor extensions.
    #[value(name = "iec61131-3-ed3")]
    Iec61131_3Ed3,
    /// RuSTy-compatible: Edition 2 base with REF_TO support and all
    /// vendor extensions enabled.
    #[value(name = "rusty")]
    Rusty,
}

impl CliDialect {
    fn to_dialect(&self) -> Dialect {
        match self {
            CliDialect::Iec61131_3Ed2 => Dialect::Iec61131_3Ed2,
            CliDialect::Iec61131_3Ed3 => Dialect::Iec61131_3Ed3,
            CliDialect::Rusty => Dialect::Rusty,
        }
    }
}

/// Shared arguments for commands that operate on source files.
#[derive(clap::Args, Debug)]
struct FileArgs {
    /// Files to include. Directory names can be given to
    /// add all files in the given directory.
    files: Vec<PathBuf>,

    /// Select the language dialect.
    /// Defaults to strict IEC 61131-3:2003 (Edition 2).
    /// Individual --allow-* flags can override the dialect's defaults.
    #[arg(long, default_value = "iec61131-3-ed2")]
    dialect: CliDialect,

    /// Allow missing semicolons after keyword statements like END_IF and END_STRUCT.
    #[arg(long)]
    allow_missing_semicolon: bool,

    /// Allow VAR_GLOBAL declarations at the top level (outside CONFIGURATION).
    /// This is a vendor extension not part of the IEC 61131-3 standard.
    #[arg(long)]
    allow_top_level_var_global: bool,

    /// Allow constant references in type parameters (e.g., STRING[MY_CONST]).
    /// This is a vendor extension not part of the IEC 61131-3 standard.
    #[arg(long)]
    allow_constant_type_params: bool,

    /// Allow empty variable blocks (VAR END_VAR, VAR_INPUT END_VAR, etc.).
    /// This is a vendor extension not part of the IEC 61131-3 standard.
    #[arg(long)]
    allow_empty_var_blocks: bool,

    /// Allow TIME to be used as a function name (e.g., TIME()).
    /// Required for OSCAT compatibility where TIME() reads the PLC system clock.
    #[arg(long)]
    allow_time_as_function_name: bool,

    /// Allow C-style comments (// line comments and /* block comments */).
    /// These are not part of the IEC 61131-3 standard.
    #[arg(long)]
    allow_c_style_comments: bool,

    /// Allow REF_TO, REF(), and NULL syntax without enabling full Edition 3.
    /// This is useful for libraries like OSCAT that use references but also
    /// use Edition 3 type names (LDT, LTIME) as identifiers.
    #[arg(long)]
    allow_ref_to: bool,

    /// Allow REF() on stack-allocated variables (VAR_TEMP, FUNCTION VAR_INPUT/VAR_OUTPUT).
    /// Required for OSCAT type-punning patterns where the reference doesn't escape.
    #[arg(long)]
    allow_ref_stack_variables: bool,

    /// Allow assigning between REF_TO types of different base types (type punning).
    /// Required for OSCAT patterns like interpreting REAL bits as DWORD via
    /// REF(real_var) into a REF_TO DWORD.
    #[arg(long)]
    allow_ref_type_punning: bool,

    /// Allow integer literals (0 or 1) as BOOL variable initializers.
    /// This is a vendor extension supported by CoDeSys, TwinCAT, and RuSTy.
    #[arg(long)]
    allow_int_to_bool_initializer: bool,
}

impl FileArgs {
    fn parse_options(&self) -> ParseOptions {
        let mut options = ParseOptions::from_dialect(self.dialect.to_dialect());
        // Individual flags override (can only enable, never disable).
        options.allow_missing_semicolon |= self.allow_missing_semicolon;
        options.allow_top_level_var_global |= self.allow_top_level_var_global;
        options.allow_constant_type_params |= self.allow_constant_type_params;
        options.allow_empty_var_blocks |= self.allow_empty_var_blocks;
        options.allow_time_as_function_name |= self.allow_time_as_function_name;
        options.allow_c_style_comments |= self.allow_c_style_comments;
        options.allow_ref_to |= self.allow_ref_to;
        options.allow_ref_stack_variables |= self.allow_ref_stack_variables;
        options.allow_ref_type_punning |= self.allow_ref_type_punning;
        options.allow_int_to_bool_initializer |= self.allow_int_to_bool_initializer;
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
    /// Show available dialects and which features each enables.
    Dialects,
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
        Action::Dialects => {
            print!("{}", describe_dialects());
            Ok(())
        }
        Action::Version => {
            println!("ironplcc version {VERSION}");
            Ok(())
        }
    }
}
