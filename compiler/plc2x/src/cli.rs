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
use ironplc_parser::{options::ParseOptions, tokenize_program};
use ironplc_plc2plc::write_to_string;
use ironplc_problems::Problem;
use ironplc_sources::{xml, FileType};
use log::{debug, error, trace};
use std::{
    collections::{HashMap, HashSet},
    fs::{canonicalize, metadata, read_dir},
    ops::Range,
    path::{Path, PathBuf},
};

use crate::project::{FileBackedProject, Project};

// Checks specified files.
pub fn check(paths: &[PathBuf], suppress_output: bool) -> Result<(), String> {
    let mut project = create_project(paths, suppress_output)?;

    // Analyze the set
    if let Err(err) = project.semantic() {
        trace!("Errors {err:?}");
        handle_diagnostics(&err, Some(&project), suppress_output);
        return Err(String::from("Error during analysis"));
    }

    Ok(())
}

pub fn echo(paths: &[PathBuf], suppress_output: bool) -> Result<(), String> {
    let mut project = create_project(paths, suppress_output)?;

    // Collect the results and output after because getting the results may change
    // the project itself
    let mut results = vec![];
    for src in project.sources_mut() {
        results.push(src.library());
    }

    let mut has_error = false;

    for result in results {
        match result {
            Ok(library) => {
                let output = write_to_string(library).map_err(|e| {
                    handle_diagnostics(&e, None, suppress_output);
                    String::from("Error echo source")
                })?;

                print!("{output}");
            }
            Err(diagnostics) => {
                let diagnostics: Vec<Diagnostic> = diagnostics;
                // TODO this needs to be improved but will wait for changes to source
                handle_diagnostics(&diagnostics, None, suppress_output);

                print!("Syntax error");

                has_error = true;
            }
        }
    }

    match has_error {
        true => Err("Tokenize error".to_owned()),
        false => Ok(()),
    }
}

pub fn tokenize(paths: &[PathBuf], suppress_output: bool) -> Result<(), String> {
    let project = create_project(paths, suppress_output)?;

    // Write the set
    for src in project.sources() {
        match src.file_type() {
            FileType::Xml => {
                tokenize_xml(src.as_string(), src.file_id(), &project, suppress_output)?;
            }
            FileType::StructuredText | FileType::Unknown => {
                tokenize_st(src.as_string(), src.file_id(), &project, suppress_output)?;
            }
        }
    }

    Ok(())
}

fn tokenize_st(
    content: &str,
    file_id: &FileId,
    project: &FileBackedProject,
    suppress_output: bool,
) -> Result<(), String> {
    let (tokens, diagnostics) = tokenize_program(content, file_id, &ParseOptions::default(), 0, 0);

    let tokens = tokens
        .iter()
        .fold(String::new(), |s1, s2| s1 + "\n" + s2.describe().as_str())
        .trim_start()
        .to_string();

    debug!("{tokens}");
    println!("{tokens}");

    if !diagnostics.is_empty() {
        println!("Number of errors {}", diagnostics.len());
        handle_diagnostics(&diagnostics, Some(project), suppress_output);
        return Err(String::from("Not valid"));
    }

    Ok(())
}

fn tokenize_xml(
    content: &str,
    file_id: &FileId,
    project: &FileBackedProject,
    suppress_output: bool,
) -> Result<(), String> {
    // Parse the XML document
    let xml_project = xml::parse_plcopen_xml(content, file_id).map_err(|diag| {
        handle_diagnostics(&[diag], Some(project), suppress_output);
        String::from("XML parsing error")
    })?;

    let mut had_error = false;
    let mut first_pou = true;

    // Tokenize each POU's ST body
    for pou in &xml_project.types.pous.pou {
        let pou_type = match pou.pou_type {
            xml::PouType::Function => "function",
            xml::PouType::FunctionBlock => "functionBlock",
            xml::PouType::Program => "program",
        };

        if let Some(body) = &pou.body {
            if let Some(st_body) = body.st_body() {
                // Add separator between POUs
                if !first_pou {
                    println!();
                }
                first_pou = false;

                println!("=== POU: {} ({}) ===", pou.name, pou_type);

                let (tokens, diagnostics) = tokenize_program(
                    &st_body.text,
                    file_id,
                    &ParseOptions::default(),
                    st_body.line_offset,
                    st_body.col_offset,
                );

                let tokens = tokens
                    .iter()
                    .fold(String::new(), |s1, s2| s1 + "\n" + s2.describe().as_str())
                    .trim_start()
                    .to_string();

                debug!("{tokens}");
                println!("{tokens}");

                if !diagnostics.is_empty() {
                    println!("Number of errors {}", diagnostics.len());
                    handle_diagnostics(&diagnostics, Some(project), suppress_output);
                    had_error = true;
                }
            } else if let Some(lang) = body.unsupported_language() {
                // Add separator between POUs
                if !first_pou {
                    println!();
                }
                first_pou = false;

                println!(
                    "=== POU: {} ({}) - {} body (skipped) ===",
                    pou.name, pou_type, lang
                );
            }
        }

        // Handle actions
        if let Some(actions) = &pou.actions {
            for action in &actions.action {
                if let Some(st_body) = action.body.st_body() {
                    if !first_pou {
                        println!();
                    }
                    first_pou = false;

                    println!("=== Action: {}.{} ===", pou.name, action.name);

                    let (tokens, diagnostics) = tokenize_program(
                        &st_body.text,
                        file_id,
                        &ParseOptions::default(),
                        st_body.line_offset,
                        st_body.col_offset,
                    );

                    let tokens = tokens
                        .iter()
                        .fold(String::new(), |s1, s2| s1 + "\n" + s2.describe().as_str())
                        .trim_start()
                        .to_string();

                    debug!("{tokens}");
                    println!("{tokens}");

                    if !diagnostics.is_empty() {
                        println!("Number of errors {}", diagnostics.len());
                        handle_diagnostics(&diagnostics, Some(project), suppress_output);
                        had_error = true;
                    }
                }
            }
        }

        // Handle transitions
        if let Some(transitions) = &pou.transitions {
            for transition in &transitions.transition {
                if let Some(st_body) = transition.body.st_body() {
                    if !first_pou {
                        println!();
                    }
                    first_pou = false;

                    println!("=== Transition: {}.{} ===", pou.name, transition.name);

                    let (tokens, diagnostics) = tokenize_program(
                        &st_body.text,
                        file_id,
                        &ParseOptions::default(),
                        st_body.line_offset,
                        st_body.col_offset,
                    );

                    let tokens = tokens
                        .iter()
                        .fold(String::new(), |s1, s2| s1 + "\n" + s2.describe().as_str())
                        .trim_start()
                        .to_string();

                    debug!("{tokens}");
                    println!("{tokens}");

                    if !diagnostics.is_empty() {
                        println!("Number of errors {}", diagnostics.len());
                        handle_diagnostics(&diagnostics, Some(project), suppress_output);
                        had_error = true;
                    }
                }
            }
        }
    }

    if had_error {
        return Err(String::from("Tokenize errors in XML"));
    }

    Ok(())
}

fn create_project(paths: &[PathBuf], suppress_output: bool) -> Result<FileBackedProject, String> {
    trace!("Reading paths {paths:?}");
    let mut files: Vec<PathBuf> = vec![];
    let mut had_error = false;

    for path in paths {
        match enumerate_files(path) {
            Ok(mut paths) => files.append(&mut paths),
            Err(err) => {
                handle_diagnostics(&err, None, suppress_output);
                had_error = true;
            }
        }
    }

    if had_error {
        return Err(String::from("Error enumerating files"));
    }

    // Create the project
    let mut project = FileBackedProject::new();
    let mut errors: Vec<Diagnostic> = vec![];

    for file_path in files {
        let res = project.push(FileId::from_path(&file_path));
        match res {
            Ok(_) => {}
            Err(err) => {
                errors.push(err);
            }
        }
    }

    if !errors.is_empty() {
        handle_diagnostics(&errors, Some(&project), suppress_output);
        return Err(String::from("Error reading source files"));
    }

    Ok(project)
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
    diagnostics: &[Diagnostic],
    project: Option<&FileBackedProject>,
    suppress_output: bool,
) {
    if !suppress_output {
        let writer = StandardStream::stderr(ColorChoice::Always);
        let config = codespan_reporting::term::Config::default();

        let mut files: SimpleFiles<String, &str> = SimpleFiles::new();

        let mut unique_files: HashSet<&FileId> = HashSet::new();
        for diagnostic in diagnostics {
            for file_id in diagnostic.file_ids() {
                unique_files.insert(file_id);
            }
        }

        let mut files_to_ids: HashMap<&FileId, usize> = HashMap::new();
        let empty_source = &"".to_owned();
        match project {
            Some(set) => {
                for file_id in unique_files {
                    if let Some(content) = set.get(file_id) {
                        let id = files.add(file_id.to_string(), content.as_string());
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
                error!("Failed writing to terminal: {err}");
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
    use ironplc_test::shared_resource_path;

    use crate::{cli::check, cli::echo, cli::tokenize, test_helpers::resource_path};

    #[test]
    fn check_first_steps_when_invalid_syntax_then_error() {
        let paths = vec![shared_resource_path("first_steps_semantic_error.st")];
        let result = check(&paths, true);
        assert!(result.is_err())
    }

    #[test]
    fn check_first_steps_when_valid_syntax_then_ok() {
        let paths = vec![shared_resource_path("first_steps.st")];
        let result = check(&paths, true);
        assert!(result.is_ok())
    }

    #[test]
    fn check_first_steps_dir_when_valid_syntax_then_ok() {
        let paths = vec![resource_path("set")];
        let result = check(&paths, true);
        assert!(result.is_ok())
    }

    #[test]
    fn echo_first_steps_when_invalid_syntax_then_error() {
        let paths = vec![shared_resource_path("first_steps_syntax_error.st")];
        let result = check(&paths, true);
        assert!(result.is_err())
    }

    #[test]
    fn echo_first_steps_when_valid_syntax_then_ok() {
        let paths = vec![shared_resource_path("first_steps.st")];
        let result = echo(&paths, true);
        assert!(result.is_ok())
    }

    #[test]
    fn tokenize_first_steps_when_valid_syntax_then_ok() {
        let paths = vec![shared_resource_path("first_steps.st")];
        let result = echo(&paths, true);
        assert!(result.is_ok())
    }

    #[test]
    fn tokenize_xml_when_valid_syntax_then_ok() {
        let paths = vec![resource_path("simple.xml")];
        let result = tokenize(&paths, true);
        assert!(result.is_ok())
    }
}
