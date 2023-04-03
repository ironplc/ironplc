//! Implements the command line behavior.

use codespan_reporting::{
    diagnostic::{Diagnostic, Label, LabelStyle, Severity},
    files::SimpleFiles,
    term::{
        self,
        termcolor::{ColorChoice, StandardStream},
    },
};
use std::{
    fs::{metadata, read_dir, File},
    io::Read,
    ops::Range,
    path::PathBuf,
};

use crate::stages::analyze;

// Checks specified files.
pub fn check(paths: Vec<PathBuf>, suppress_output: bool) -> Result<(), String> {
    let mut files: Vec<PathBuf> = vec![];
    for path in paths {
        match enumerate_files(path) {
            Ok(mut paths) => files.append(&mut paths),
            Err(err) => return Err(err),
        }
    }

    for filename in files {
        let mut file = File::open(filename.clone())
            .map_err(|e| format!("Failed opening file {}. {}", filename.display(), e))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|err| format!("Failed to read file {}\n {}", filename.display(), err))?;

        let writer = StandardStream::stderr(ColorChoice::Always);
        let config = codespan_reporting::term::Config::default();

        match analyze(&contents, &filename) {
            Ok(_) => {
                println!("OK");
            }
            Err(diagnostic) => {
                let mut files = SimpleFiles::new();
                files.add(filename.display().to_string(), contents);

                let diagnostic = map_diagnostic(diagnostic);

                if !suppress_output {
                    term::emit(&mut writer.lock(), &config, &files, &diagnostic)
                        .map_err(|_| "Failed writing to terminal")?;
                }
                return Err(String::from("Error"));
            }
        }
    }

    Ok(())
}

fn enumerate_files(path: PathBuf) -> Result<Vec<PathBuf>, String> {
    let metadata = metadata(path.as_path()).map_err(|e| e.to_string())?;
    if metadata.is_dir() {
        let paths = read_dir(path).map_err(|e| e.to_string())?;
        let paths: Vec<PathBuf> = paths
            .into_iter()
            .filter_map(|entry| match entry {
                Ok(entry) => Some(entry.path()),
                Err(_) => None,
            })
            .collect();
        return Ok(paths);
    }
    if metadata.is_file() {
        return Ok(vec![path]);
    }
    if metadata.is_symlink() {
        panic!("Sorry. Symlinks are not supported.")
    }
    Ok(vec![])
}

fn map_label(label: ironplc_dsl::diagnostic::Label, style: LabelStyle) -> Label<usize> {
    let range = match label.location {
        ironplc_dsl::diagnostic::Location::QualifiedPosition(pos) => Range {
            start: pos.offset,
            end: pos.offset,
        },
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

#[cfg(test)]
mod tests {
    use crate::{cli::check, test_helpers::resource_path};

    #[test]
    fn first_steps_when_invalid_syntax_then_error() {
        let paths = vec![resource_path("first_steps_semantic_error.st")];
        let result = check(paths, true);
        assert!(result.is_err())
    }

    #[test]
    fn first_steps_when_valid_syntax_then_ok() {
        let paths = vec![resource_path("first_steps.st")];
        let result = check(paths, true);
        assert!(result.is_ok())
    }
}
