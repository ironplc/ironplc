//! Implements the command line behavior.

use codespan_reporting::{
    diagnostic::{Diagnostic, Label, LabelStyle, Severity},
    files::SimpleFiles,
    term::{
        self,
        termcolor::{ColorChoice, StandardStream},
    },
};
use ironplc_dsl::core::FileId;
use std::{
    fs::{metadata, read_dir, File},
    io::Read,
    ops::Range,
    path::PathBuf,
};

use crate::{compilation_set::{CompilationSet, CompilationSource}, stages::analyze};

// Checks specified files.
pub fn check(paths: Vec<PathBuf>, suppress_output: bool) -> Result<(), String> {
    let mut files: Vec<PathBuf> = vec![];
    for path in paths {
        match enumerate_files(&path) {
            Ok(mut paths) => files.append(&mut paths),
            Err(err) => return Err(err),
        }
    }

    let mut compilation_set = CompilationSet::new();

    let sources : Result<Vec<_>, String> = files.iter().map(|path| {
        let mut file = File::open(path).map_err(|e| {
            println!("Failed opening file {}. {}", path.display(), e);
            e.to_string()
        })?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).map_err(|err| {
            format!("Failed to read file {}\n {}", path.display(), err);
            err.to_string()
        })?;

        Ok(CompilationSource::Text((contents, FileId::from_path(path))))
    }).collect();

    compilation_set.extend(sources?);

    // Analyze the set
    analyze(&compilation_set).map_err(|e| {
        handle_diagnostic(e, &compilation_set, suppress_output);
        1usize
    }).map_err(|e| format!("Number of errors: {}", e))?;

    println!("OK");
    Ok(())
}

fn enumerate_files(path: &PathBuf) -> Result<Vec<PathBuf>, String> {
    let metadata = metadata(path).map_err(|e| e.to_string())?;
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
        return Ok(vec![path.to_path_buf()]);
    }
    if metadata.is_symlink() {
        panic!("Sorry. Symlinks are not supported.")
    }
    Ok(vec![])
}

fn handle_diagnostic(
    diagnostic: ironplc_dsl::diagnostic::Diagnostic,
    compilation_set: &CompilationSet,
    suppress_output: bool,
) {
    if !suppress_output {
        let writer = StandardStream::stderr(ColorChoice::Always);
        let config = codespan_reporting::term::Config::default();

        let mut files: SimpleFiles<String, &String> = SimpleFiles::new();

        for file_id in diagnostic.file_ids() {
            if let Some(content) = compilation_set.content(file_id) {
                files.add(file_id.to_string(), content);
            }
        }

        let diagnostic = map_diagnostic(diagnostic);

        let _ = term::emit(&mut writer.lock(), &config, &files, &diagnostic).map_err(|err| {
            println!("Failed writing to terminal: {}", err);
            1usize
        });
    }
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
    let description = diagnostic.description();

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
        .with_message(description)
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

    #[test]
    fn first_steps_dir_when_valid_syntax_then_ok() {
        let paths = vec![resource_path("set")];
        let result = check(paths, true);
        assert!(result.is_ok())
    }
}
