use std::path::PathBuf;

use clap::Parser;

use ironplcc::cli;
use ironplcc::lsp;

#[derive(Parser, Debug)]
#[command(name = "ironplcc", about = "IronPLC compiler")]
struct Args {
    #[command(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    Check {
        files: Vec<PathBuf>,
    },
    Lsp {
        #[arg(long)]
        stdio: bool,
    },
}

pub fn main() -> Result<(), String> {
    let args = Args::parse();

    match args.action {
        Action::Lsp { stdio: _ } => lsp::start(),
        Action::Check { files } => cli::check(files, false),
    }
}
