//! Tokenization support for different file types.
//!
//! This module provides tokenization functionality for both Structured Text
//! and PLCopen XML files.

use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::{options::ParseOptions, tokenize_program};
use ironplc_sources::{xml, FileType, Source};
use log::debug;

use crate::project::FileBackedProject;

/// Callback type for handling diagnostics during tokenization.
pub type DiagnosticHandler<'a> = &'a dyn Fn(&[Diagnostic], Option<&FileBackedProject>, bool);

/// Tokenize a source file based on its type.
///
/// For Structured Text files, tokenizes the entire content.
/// For XML files, extracts and tokenizes each POU's ST body.
pub fn tokenize_source(
    src: &Source,
    project: &FileBackedProject,
    suppress_output: bool,
    handle_diagnostics: DiagnosticHandler,
) -> Result<(), String> {
    match src.file_type() {
        FileType::Xml => tokenize_xml(
            src.as_string(),
            src.file_id(),
            project,
            suppress_output,
            handle_diagnostics,
        ),
        FileType::StructuredText | FileType::Unknown => tokenize_st(
            src.as_string(),
            src.file_id(),
            project,
            suppress_output,
            handle_diagnostics,
        ),
    }
}

fn tokenize_st(
    content: &str,
    file_id: &FileId,
    project: &FileBackedProject,
    suppress_output: bool,
    handle_diagnostics: DiagnosticHandler,
) -> Result<(), String> {
    let (tokens, diagnostics) = tokenize_program(content, file_id, &ParseOptions::default(), 0, 0);

    let tokens = format_tokens(&tokens);

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
    handle_diagnostics: DiagnosticHandler,
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
                if tokenize_st_body(
                    &mut first_pou,
                    &format!("POU: {} ({})", pou.name, pou_type),
                    st_body,
                    file_id,
                    project,
                    suppress_output,
                    handle_diagnostics,
                ) {
                    had_error = true;
                }
            } else if let Some((lang, _range)) = body.unsupported_language() {
                print_header(
                    &mut first_pou,
                    &format!("POU: {} ({}) - {} body (skipped)", pou.name, pou_type, lang),
                );
            }
        }

        // Handle actions
        if let Some(actions) = &pou.actions {
            for action in &actions.action {
                if let Some(st_body) = action.body.st_body() {
                    if tokenize_st_body(
                        &mut first_pou,
                        &format!("Action: {}.{}", pou.name, action.name),
                        st_body,
                        file_id,
                        project,
                        suppress_output,
                        handle_diagnostics,
                    ) {
                        had_error = true;
                    }
                }
            }
        }

        // Handle transitions
        if let Some(transitions) = &pou.transitions {
            for transition in &transitions.transition {
                if let Some(st_body) = transition.body.st_body() {
                    if tokenize_st_body(
                        &mut first_pou,
                        &format!("Transition: {}.{}", pou.name, transition.name),
                        st_body,
                        file_id,
                        project,
                        suppress_output,
                        handle_diagnostics,
                    ) {
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

/// Tokenize a single ST body and print the results.
/// Returns true if there were errors.
fn tokenize_st_body(
    first_pou: &mut bool,
    header: &str,
    st_body: &xml::StBody,
    file_id: &FileId,
    project: &FileBackedProject,
    suppress_output: bool,
    handle_diagnostics: DiagnosticHandler,
) -> bool {
    print_header(first_pou, header);

    let (tokens, diagnostics) = tokenize_program(
        &st_body.text,
        file_id,
        &ParseOptions::default(),
        st_body.line_offset,
        st_body.col_offset,
    );

    let tokens = format_tokens(&tokens);

    debug!("{tokens}");
    println!("{tokens}");

    if !diagnostics.is_empty() {
        println!("Number of errors {}", diagnostics.len());
        handle_diagnostics(&diagnostics, Some(project), suppress_output);
        return true;
    }

    false
}

/// Print a section header, adding a blank line separator if not the first section.
fn print_header(first: &mut bool, header: &str) {
    if !*first {
        println!();
    }
    *first = false;
    println!("=== {} ===", header);
}

/// Format tokens into a displayable string.
fn format_tokens(tokens: &[ironplc_parser::token::Token]) -> String {
    tokens
        .iter()
        .fold(String::new(), |s1, s2| s1 + "\n" + s2.describe().as_str())
        .trim_start()
        .to_string()
}
