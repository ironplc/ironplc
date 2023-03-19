use std::{fs::File, io::Read, path::PathBuf};

use clap::Parser;
use codespan_reporting::{
    diagnostic::{Diagnostic, Label, LabelStyle, Severity},
    files::SimpleFile,
    term::{
        self,
        termcolor::{ColorChoice, StandardStream},
    },
};
use ironplcc::analyze;
use ironplcc::lsp;

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
            lsp::start()?;
        }
        Action::Check { files } => {
            for filename in files {
                let mut file = File::open(filename.clone())
                    .map_err(|e| format!("Failed opening file {}. {}", filename.display(), e))?;
                let mut contents = String::new();
                file.read_to_string(&mut contents)
                    .map_err(|_| "Failed to reach file")?;

                let writer = StandardStream::stderr(ColorChoice::Always);
                let config = codespan_reporting::term::Config::default();

                match analyze(&contents, &filename) {
                    Ok(_) => {
                        println!("OK");
                    }
                    Err(diagnostic) => {
                        let file = SimpleFile::new(filename.display().to_string(), contents);
                        term::emit(
                            &mut writer.lock(),
                            &config,
                            &file,
                            &map_diagnostic(diagnostic),
                        )
                        .map_err(|_| "Failed writing to terminal")?;
                        std::process::exit(-1)
                    }
                }
            }
        }
    }

    Ok(())
}

fn map_diagnostic(diagnostic: ironplc_dsl::diagnostic::Diagnostic) -> Diagnostic<()> {
    // TODO this ignores the position and doesn't include secondary information
    let labels = vec![Label::new(LabelStyle::Primary, (), 0..0)];
    Diagnostic::new(Severity::Error)
        .with_code(diagnostic.code)
        .with_message(diagnostic.description)
        .with_labels(labels)
}
