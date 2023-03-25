use std::{fs::File, io::Read, ops::Range, path::PathBuf};

use clap::Parser;
use codespan_reporting::{
    diagnostic::{Diagnostic, Label, LabelStyle, Severity},
    files::SimpleFiles,
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
                        let mut files = SimpleFiles::new();
                        files.add(filename.display().to_string(), contents);

                        term::emit(
                            &mut writer.lock(),
                            &config,
                            &files,
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

fn map_label(label: ironplc_dsl::diagnostic::Label, style: LabelStyle) -> Label<usize> {
    let range = match label.location {
        ironplc_dsl::diagnostic::Location::QualifiedPosition(_pos) => Range { start: 0, end: 0 },
        ironplc_dsl::diagnostic::Location::OffsetRange(offset) => Range {
            start: offset.start,
            end: offset.end,
        },
    };
    Label::new(style, 0, range).with_message(label.message)
}

fn map_diagnostic(diagnostic: ironplc_dsl::diagnostic::Diagnostic) -> Diagnostic<usize> {
    // Set the primary labels
    let mut labels = vec![map_label(diagnostic.primary, LabelStyle::Primary)];

    // Add any secondary labels
    labels.extend(
        diagnostic
            .secondary
            .into_iter()
            .map(|lbl| map_label(lbl, LabelStyle::Secondary)),
    );

    Diagnostic::new(Severity::Error)
        .with_code(diagnostic.code)
        .with_message(diagnostic.description)
        .with_labels(labels)
}
