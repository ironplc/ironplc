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
use ironplc_parser::tokenize_program;
use ironplc_plc2plc::write_to_string;
use ironplc_problems::Problem;
use log::{debug, error, trace};
use std::{
    collections::{HashMap, HashSet},
    fs::{canonicalize, metadata, read_dir},
    ops::Range,
    path::{Path, PathBuf},
};

use crate::{
    compilation_set::{CompilationSet, CompilationSource},
    stages::{analyze, parse},
};

// Checks specified files.
pub fn check(paths: Vec<PathBuf>, suppress_output: bool) -> Result<(), String> {
    trace!("Reading paths {:?}", paths);
    let mut files: Vec<PathBuf> = vec![];
    let mut had_error = false;
    for path in paths {
        match enumerate_files(&path) {
            Ok(mut paths) => files.append(&mut paths),
            Err(err) => {
                handle_diagnostics(err, None, suppress_output);
                had_error = true;
            }
        }
    }

    if had_error {
        return Err(String::from("Error enumerating files"));
    }

    // Create the compilation set
    let mut compilation_set = CompilationSet::new();
    let sources: Result<Vec<_>, Vec<Diagnostic>> = files.iter().map(path_to_source).collect();
    let sources = sources.map_err(|err| {
        handle_diagnostics(err, Some(&compilation_set), suppress_output);
        String::from("Error reading source files")
    })?;
    compilation_set.extend(sources);

    // Analyze the set
    if let Err(err) = analyze(&compilation_set) {
        trace!("Errors {:?}", err);
        handle_diagnostics(err, Some(&compilation_set), suppress_output);
        return Err(String::from("Error during analysis"));
    }

    println!("OK");
    Ok(())
}

pub fn echo(paths: Vec<PathBuf>, suppress_output: bool) -> Result<(), String> {
    trace!("Reading paths {:?}", paths);
    let mut files: Vec<PathBuf> = vec![];
    let mut had_error = false;
    for path in paths {
        match enumerate_files(&path) {
            Ok(mut paths) => files.append(&mut paths),
            Err(err) => {
                handle_diagnostics(err, None, suppress_output);
                had_error = true;
            }
        }
    }

    if had_error {
        return Err(String::from("Error enumerating files"));
    }

    // Create the compilation set
    let mut compilation_set = CompilationSet::new();
    let sources: Result<Vec<_>, Vec<Diagnostic>> = files.iter().map(path_to_source).collect();
    let sources = sources.map_err(|err| {
        handle_diagnostics(err, Some(&compilation_set), suppress_output);
        String::from("Error reading source files")
    })?;
    compilation_set.extend(sources);

    // Write the set
    for src in compilation_set.sources {
        match src {
            CompilationSource::Library(lib) => {
                let output = write_to_string(&lib).map_err(|e| {
                    handle_diagnostics(e, None, suppress_output);
                    String::from("Error echo source")
                })?;

                print!("{}", output);
            }
            CompilationSource::Text(txt) => {
                let lib = parse(&txt.0, &txt.1).map_err(|e| {
                    handle_diagnostics(vec![e], None, suppress_output);
                    String::from("Error reading source files")
                })?;
                let output = write_to_string(&lib).map_err(|e| {
                    handle_diagnostics(e, None, suppress_output);
                    String::from("Error echo source")
                })?;

                print!("{}", output);
            }
            CompilationSource::TextRef(_) => {}
        }
    }

    Ok(())
}

pub fn tokenize(path: &PathBuf, suppress_output: bool) -> Result<(), String> {
    let contents = path_to_source(path).map_err(|diagnostics| {
        handle_diagnostics(diagnostics, None, suppress_output);
        "Problem reading file"
    })?;

    if let CompilationSource::Text(txt) = contents {
        let (tokens, diagnostics) = tokenize_program(txt.0.as_str(), &txt.1);

        let tokens = tokens
            .iter()
            .fold(String::new(), |s1, s2| s1 + "\n" + s2.to_string().as_str())
            .trim_start()
            .to_string();

        debug!("{}", tokens);
        println!("{}", tokens);

        if !diagnostics.is_empty() {
            println!("Number of errors {}", diagnostics.len());
            let mut set = CompilationSet::new();
            set.extend(vec![CompilationSource::Text((
                txt.0.clone(),
                txt.1.clone(),
            ))]);
            handle_diagnostics(diagnostics, Some(&set), suppress_output);
            return Err(String::from("Not valid"));
        }
    }

    println!("OK");
    Ok(())
}

/// Creates a compilation source item from the path (by reading the file).
fn path_to_source(path: &PathBuf) -> Result<CompilationSource, Vec<Diagnostic>> {
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
fn enumerate_files(path: &PathBuf) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    // Get the canonical path so that error messages are unambiguous
    let path = canonicalize(path).map_err(|e| {
        diagnostic(
            Problem::CannotCanonicalizePath,
            path,
            format!("{}, {}", path.display(), e),
        )
    })?;

    // Determine what kind of path we have.
    let metadata = metadata(&path)
        .map_err(|e| diagnostic(Problem::CannotReadMetadata, &path, e.to_string()))?;
    if metadata.is_dir() {
        let paths = read_dir(&path)
            .map_err(|e| diagnostic(Problem::CannotReadDirectory, &path, e.to_string()))?;
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
fn handle_diagnostics(
    diagnostics: Vec<Diagnostic>,
    compilation_set: Option<&CompilationSet>,
    suppress_output: bool,
) {
    if !suppress_output {
        let writer = StandardStream::stderr(ColorChoice::Always);
        let config = codespan_reporting::term::Config::default();

        let mut files: SimpleFiles<String, &String> = SimpleFiles::new();

        let mut unique_files: HashSet<&FileId> = HashSet::new();
        for diagnostic in &diagnostics {
            for file_id in diagnostic.file_ids() {
                unique_files.insert(file_id);
            }
        }

        let mut files_to_ids: HashMap<&FileId, usize> = HashMap::new();
        let empty_source = &"".to_owned();
        match compilation_set {
            Some(set) => {
                for file_id in unique_files {
                    if let Some(content) = set.content(file_id) {
                        let id = files.add(file_id.to_string(), content);
                        files_to_ids.insert(file_id, id);
                    }
                }
            }
            None => {
                for file_id in unique_files {
                    let id = files.add(file_id.to_string(), empty_source);
                    files_to_ids.insert(file_id, id);
                }
            }
        }

        diagnostics.iter().for_each(|d| {
            let diagnostic = map_diagnostic(d, &files_to_ids);

            let _ = term::emit(&mut writer.lock(), &config, &files, &diagnostic).map_err(|err| {
                error!("Failed writing to terminal: {}", err);
                1usize
            });
        });
    }
}

fn map_diagnostic(
    diagnostic: &Diagnostic,
    file_to_id: &HashMap<&FileId, usize>,
) -> CodeSpanDiagnostic<usize> {
    let description = diagnostic.description();

    // Set the primary labels
    let mut labels = vec![map_label(
        &diagnostic.primary,
        LabelStyle::Primary,
        file_to_id,
    )];

    // Add any secondary labels
    labels.extend(
        diagnostic
            .secondary
            .iter()
            .map(|lbl| map_label(lbl, LabelStyle::Secondary, file_to_id)),
    );

    CodeSpanDiagnostic::new(Severity::Error)
        .with_code(diagnostic.code.clone())
        .with_message(description)
        .with_labels(labels)
}

fn map_label(
    label: &Label,
    style: LabelStyle,
    file_to_id: &HashMap<&FileId, usize>,
) -> CodeSpanLabel<usize> {
    let range = Range {
        start: label.location.start,
        end: label.location.end,
    };
    let id = file_to_id.get(&label.file_id);
    CodeSpanLabel::new(style, *id.unwrap_or(&0), range).with_message(&label.message)
}

fn diagnostic(problem: Problem, path: &Path, message: String) -> Vec<Diagnostic> {
    vec![Diagnostic::problem(
        problem,
        Label::file(FileId::from_path(path), message),
    )]
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
