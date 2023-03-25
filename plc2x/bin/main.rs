use std::path::PathBuf;

use clap::Parser;

use ironplcc::lsp;
use ironplcc::cli;

#[derive(Parser, Debug)]
#[command(name = "ironplcc", about = "IronPLC compiler")]
struct Args {
    #[command(subcommand)]
    action: Action,
}

#[derive(clap::Subcommand, Debug)]
enum Action {
    Check { files: Vec<PathBuf> },
    Lsp,
}

pub fn main() -> Result<(), String> {
    let args = Args::parse();

    match args.action {
        Action::Lsp => {
            lsp::start()
        }
        Action::Check { files } => {
            cli::check(files, false)
        }
    }
}
