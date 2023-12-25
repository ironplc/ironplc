//! Implements the command line behavior.

use codespan_reporting::{
    diagnostic::{Diagnostic as CodeSpanDiagnostic, Label as CodeSpanLabel, LabelStyle, Severity},
    files::SimpleFiles,
    term::{
        self,
        termcolor::{ColorChoice, StandardStream},
    },
};
use ironplc_dsl::{
    core::FileId,
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;
use log::{debug, error, trace};
use std::{
    fs::{canonicalize, metadata, read_dir},
    ops::Range,
    path::{PathBuf, Path},
};

use crate::{
    compilation_set::{CompilationSet, CompilationSource},
    stages::analyze,
};

// Checks specified files.
pub fn check(paths: Vec<PathBuf>, suppress_output: bool) -> Result<(), String> {
    trace!("Reading paths {:?}", paths);
    let mut files: Vec<PathBuf> = vec![];
    for path in paths {
        match enumerate_files(&path) {
            Ok(mut paths) => files.append(&mut paths),
            Err(err) => {
                handle_diagnostic(err, None, suppress_output);
                return Err(String::from("Error enumerating files"));
            }
        }
    }

    // Create the compilation set
    let mut compilation_set = CompilationSet::new();
    let sources: Result<Vec<_>, Diagnostic> = files.iter().map(path_to_source).collect();
    let sources = sources.map_err(|err| {
        handle_diagnostic(err, Some(&compilation_set), suppress_output);
        String::from("Error reading source files")
    })?;
    compilation_set.extend(sources);

    // Analyze the set
    if let Err(err) = analyze(&compilation_set) {
        handle_diagnostic(err, Some(&compilation_set), suppress_output);
        return Err(String::from("Error during analysis"));
    }

    println!("OK");
    Ok(())
}

/// Creates a compilation source item from the path (by reading the file).
fn path_to_source(path: &PathBuf) -> Result<CompilationSource, Diagnostic> {
    debug!("Reading file {}", path.display());

    let bytes = std::fs::read(path)
        .map_err(|e| diagnostic(Problem::CannotReadFile, path, e.to_string()))?;

    // We try different encoders and return the first one that matches. From section 2.1.1,
    // the allowed character set is one with characters consistent with ISO/IEC 10646-1 (UCS).
    // There are other valid encodings, so if encountered, it is reasonable to add more here.
    let decoders: [&'static encoding_rs::Encoding; 2] =
        [encoding_rs::UTF_8, encoding_rs::WINDOWS_1252];

    let result = decoders.iter().find_map(|d| {
        let (res, encoding_used, had_errors) = d.decode(&bytes);
        if had_errors {
            trace!(
                "Path {} did not match encoding {}",
                path.display(),
                encoding_used.name()
            );
            return None;
        }
        trace!(
            "Path {} matched encoding {}",
            path.display(),
            encoding_used.name()
        );
        Some(res)
    });

    match result {
        Some(res) => {
            let contents = String::from(res);
            return Ok(CompilationSource::Text((contents, FileId::from_path(path))));
        }
        None => Err(diagnostic(
            Problem::UnsupportedEncoding,
            path,
            String::from("The file is not UTF-8 or latin1"),
        )),
    }
}

/// Enumerates all files at the path.
///
/// If the path is a file, then returns the file. If the path is a directory,
/// then returns all files in the directory.
fn enumerate_files(path: &PathBuf) -> Result<Vec<PathBuf>, Diagnostic> {
    // Get the canonical path so that error messages are unambiguous
    let path = canonicalize(path).map_err(|e| {
        diagnostic(Problem::StructureDuplicatedElement, path, e.to_string())
    })?;

    // Determine what kind of path we have.
    let metadata = metadata(&path).map_err(|e| {
        diagnostic(Problem::CannotReadMetadata, &path, e.to_string())
    })?;
    if metadata.is_dir() {
        let paths = read_dir(&path).map_err(|e| {
            diagnostic(Problem::CannotReadDirectory, &path, e.to_string())
        })?;
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
        return Err(diagnostic(
            Problem::SymlinkUnsupported,
            &path,
            String::from(""),
        ));
    }
    Ok(vec![])
}

/// Converts an IronPLC diagnostic into the
fn handle_diagnostic(
    diagnostic: Diagnostic,
    compilation_set: Option<&CompilationSet>,
    suppress_output: bool,
) {
    if !suppress_output {
        let writer = StandardStream::stderr(ColorChoice::Always);
        let config = codespan_reporting::term::Config::default();

        let mut files: SimpleFiles<String, &String> = SimpleFiles::new();

        let empty_source = &"".to_owned();
        match compilation_set {
            Some(set) => {
                for file_id in diagnostic.file_ids() {
                    if let Some(content) = set.content(file_id) {
                        files.add(file_id.to_string(), content);
                    }
                }
            }
            None => {
                for file_id in diagnostic.file_ids() {
                    files.add(file_id.to_string(), empty_source);
                }
            }
        }

        let diagnostic = map_diagnostic(diagnostic);

        let _ = term::emit(&mut writer.lock(), &config, &files, &diagnostic).map_err(|err| {
            error!("Failed writing to terminal: {}", err);
            1usize
        });
    }
}

fn map_label(label: Label, style: LabelStyle) -> CodeSpanLabel<usize> {
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
    CodeSpanLabel::new(style, 0, range).with_message(label.message)
}

fn map_diagnostic(diagnostic: Diagnostic) -> CodeSpanDiagnostic<usize> {
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

    CodeSpanDiagnostic::new(Severity::Error)
        .with_code(diagnostic.code)
        .with_message(description)
        .with_labels(labels)
}

fn diagnostic(problem: Problem, path: &Path, message: String) -> Diagnostic {
    Diagnostic::problem(
        problem,
        Label::file(FileId::from_path(path), message),
    )
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
