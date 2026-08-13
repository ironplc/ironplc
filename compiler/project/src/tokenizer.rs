//! Tokenization support for different file types.
//!
//! This module provides tokenization functionality for both Structured Text
//! and PLCopen XML files.

use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::{options::CompilerOptions, tokenize_program};
use ironplc_sources::{xml, FileType, Source};
use log::debug;

/// Tokenize a source file based on its type.
///
/// For Structured Text files, tokenizes the entire content.
/// For XML files, extracts and tokenizes each POU's ST body.
///
/// Token dumps are printed to stdout as each body is processed. Returns
/// every diagnostic produced — an empty vector means the source tokenized
/// cleanly. Rendering (and exit-status) decisions belong to the caller.
pub fn tokenize_source(src: &Source) -> Vec<Diagnostic> {
    match src.file_type() {
        FileType::Xml => tokenize_xml(src.as_string(), src.file_id()),
        FileType::StructuredText | FileType::TwinCat | FileType::Unknown => {
            tokenize_st(src.as_string(), src.file_id())
        }
    }
}

fn tokenize_st(content: &str, file_id: &FileId) -> Vec<Diagnostic> {
    let (tokens, diagnostics) =
        tokenize_program(content, file_id, &CompilerOptions::default(), 0, 0);

    let tokens = format_tokens(&tokens);

    debug!("{tokens}");
    println!("{tokens}");

    if !diagnostics.is_empty() {
        println!("Number of errors {}", diagnostics.len());
    }

    diagnostics
}

fn tokenize_xml(content: &str, file_id: &FileId) -> Vec<Diagnostic> {
    // Parse the XML document
    let xml_project = match xml::parse_plcopen_xml(content, file_id) {
        Ok(project) => project,
        Err(diag) => return vec![diag],
    };

    let mut diagnostics = vec![];
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
                diagnostics.extend(tokenize_st_body(
                    &mut first_pou,
                    &format!("POU: {} ({})", pou.name, pou_type),
                    st_body,
                    file_id,
                ));
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
                    diagnostics.extend(tokenize_st_body(
                        &mut first_pou,
                        &format!("Action: {}.{}", pou.name, action.name),
                        st_body,
                        file_id,
                    ));
                }
            }
        }

        // Handle transitions
        if let Some(transitions) = &pou.transitions {
            for transition in &transitions.transition {
                if let Some(st_body) = transition.body.st_body() {
                    diagnostics.extend(tokenize_st_body(
                        &mut first_pou,
                        &format!("Transition: {}.{}", pou.name, transition.name),
                        st_body,
                        file_id,
                    ));
                }
            }
        }
    }

    diagnostics
}

/// Tokenize a single ST body and print the results.
/// Returns the diagnostics produced for the body.
fn tokenize_st_body(
    first_pou: &mut bool,
    header: &str,
    st_body: &xml::StBody,
    file_id: &FileId,
) -> Vec<Diagnostic> {
    print_header(first_pou, header);

    let (tokens, diagnostics) = tokenize_program(
        &st_body.text,
        file_id,
        &CompilerOptions::default(),
        st_body.line_offset,
        st_body.col_offset,
    );

    let tokens = format_tokens(&tokens);

    debug!("{tokens}");
    println!("{tokens}");

    if !diagnostics.is_empty() {
        println!("Number of errors {}", diagnostics.len());
    }

    diagnostics
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
