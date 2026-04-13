//! The `explain_diagnostic` MCP tool.
//!
//! Looks up the human-readable explanation for a compiler problem code
//! (e.g. `P0001`). The RST documentation is embedded at build time via
//! `include_str!` (REQ-TOL-072) — the tool handler performs no filesystem I/O.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::common::serialize_diagnostics;

// Build-time generated lookup: code → (rst_content, title).
include!(concat!(env!("OUT_DIR"), "/problem_docs.rs"));

/// Input for the `explain_diagnostic` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExplainDiagnosticInput {
    /// The problem code to look up (e.g. `"P0001"`). Case-insensitive.
    pub code: String,
}

/// Response for the `explain_diagnostic` tool.
#[derive(Debug, Serialize)]
pub struct ExplainDiagnosticResponse {
    pub ok: bool,
    pub found: bool,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_fix: Option<String>,
    pub diagnostics: Vec<serde_json::Value>,
}

/// Builds the explain_diagnostic response.
pub fn build_response(code: &str) -> ExplainDiagnosticResponse {
    let normalized = code.trim().to_uppercase();

    match lookup_problem_doc(&normalized) {
        Some((rst_content, title)) => {
            let (description, suggested_fix) = parse_rst(rst_content);
            ExplainDiagnosticResponse {
                ok: true,
                found: true,
                code: normalized,
                title: Some(title.to_string()),
                description: Some(description),
                suggested_fix,
                diagnostics: vec![],
            }
        }
        None => {
            let err = Diagnostic::problem(
                Problem::McpInputValidation,
                Label::span(
                    SourceSpan::default(),
                    format!("Unknown problem code '{normalized}'."),
                ),
            );
            ExplainDiagnosticResponse {
                ok: false,
                found: false,
                code: normalized,
                title: None,
                description: None,
                suggested_fix: None,
                diagnostics: serialize_diagnostics(&[err]),
            }
        }
    }
}

/// Parses RST content into `(description, suggested_fix)`.
///
/// The description is the plain-text rendering of everything after the
/// `.. problem-summary::` directive up to (but not including) the first
/// "To fix" paragraph. The suggested fix is the text from the "To fix"
/// paragraph through the end.
fn parse_rst(rst: &str) -> (String, Option<String>) {
    let lines: Vec<&str> = rst.lines().collect();

    // Skip the RST title block (===== / P#### / ===== / blank / .. problem-summary::)
    let body_start = lines
        .iter()
        .position(|line| line.starts_with(".. problem-summary::"))
        .map(|i| i + 1)
        .unwrap_or(0);

    let body_lines = &lines[body_start..];

    // Find the "To fix" split point.
    let fix_idx = body_lines.iter().position(|line| {
        let lower = line.to_lowercase();
        lower.starts_with("to fix this error")
            || lower.starts_with("to fix this,")
            || lower.starts_with("to fix this ")
    });

    let (desc_lines, fix_lines) = match fix_idx {
        Some(idx) => (&body_lines[..idx], Some(&body_lines[idx..])),
        None => (body_lines, None),
    };

    let description = strip_rst_markup(desc_lines);
    let suggested_fix = fix_lines.map(strip_rst_markup).filter(|s| !s.is_empty());

    (description, suggested_fix)
}

/// Strips RST markup from a slice of lines, producing plain text.
fn strip_rst_markup(lines: &[&str]) -> String {
    let mut out = Vec::new();

    for &line in lines {
        // Skip RST title underlines (lines of only = or - characters).
        let trimmed = line.trim();
        if !trimmed.is_empty()
            && (trimmed.chars().all(|c| c == '=') || trimmed.chars().all(|c| c == '-'))
        {
            continue;
        }

        // Skip RST directives (but NOT their indented content).
        if trimmed.starts_with(".. ") {
            continue;
        }

        // Skip RST reference targets.
        if trimmed.starts_with(".. _") {
            continue;
        }

        out.push(strip_inline_markup(line));
    }

    // Join and collapse excessive blank lines.
    let joined = out.join("\n");
    collapse_blank_lines(&joined)
}

/// Strips RST inline markup from a single line.
fn strip_inline_markup(line: &str) -> String {
    let mut result = line.to_string();

    // Strip :code:`text` → text
    while let Some(start) = result.find(":code:`") {
        if let Some(end) = result[start + 7..].find('`') {
            let text = result[start + 7..start + 7 + end].to_string();
            result = format!(
                "{}{}{}",
                &result[..start],
                text,
                &result[start + 7 + end + 1..]
            );
        } else {
            break;
        }
    }

    // Strip :doc:`text` → text (take the display text part)
    while let Some(start) = result.find(":doc:`") {
        if let Some(end) = result[start + 6..].find('`') {
            let raw = &result[start + 6..start + 6 + end];
            // :doc:`/path/to/doc` → extract the last path segment
            let text = raw.rsplit('/').next().unwrap_or(raw).to_string();
            result = format!(
                "{}{}{}",
                &result[..start],
                text,
                &result[start + 6 + end + 1..]
            );
        } else {
            break;
        }
    }

    // Strip ``backtick`` → backtick (double-backtick inline literals).
    while let Some(start) = result.find("``") {
        if let Some(end) = result[start + 2..].find("``") {
            let text = result[start + 2..start + 2 + end].to_string();
            result = format!(
                "{}{}{}",
                &result[..start],
                text,
                &result[start + 2 + end + 2..]
            );
        } else {
            break;
        }
    }

    result
}

/// Collapses runs of 3+ blank lines down to 2 (one visual blank line),
/// and trims leading/trailing whitespace.
fn collapse_blank_lines(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut blank_count = 0;

    for line in text.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                result.push('\n');
            }
        } else {
            blank_count = 0;
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(line);
        }
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_response_when_known_code_then_ok() {
        let resp = build_response("P0001");
        assert!(resp.ok);
        assert!(resp.found);
        assert_eq!(resp.code, "P0001");
        assert!(resp.title.is_some());
        assert!(!resp.title.unwrap().is_empty());
        assert!(resp.description.is_some());
        assert!(!resp.description.unwrap().is_empty());
        assert!(resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_unknown_code_then_not_found() {
        let resp = build_response("P9876");
        assert!(!resp.ok);
        assert!(!resp.found);
        assert_eq!(resp.code, "P9876");
        assert!(resp.title.is_none());
        assert!(resp.description.is_none());
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_lowercase_code_then_normalized() {
        let resp = build_response("p0001");
        assert!(resp.ok);
        assert!(resp.found);
        assert_eq!(resp.code, "P0001");
    }

    #[test]
    fn build_response_when_invalid_format_then_not_found() {
        let resp = build_response("INVALID");
        assert!(!resp.ok);
        assert!(!resp.found);
    }

    #[test]
    fn build_response_when_stub_doc_then_found_with_empty_description() {
        // P2014 is a stub file with only the title and directive.
        let resp = build_response("P2014");
        assert!(resp.ok);
        assert!(resp.found);
        assert_eq!(resp.code, "P2014");
        assert!(resp.title.is_some());
    }

    #[test]
    fn build_response_when_code_has_suggested_fix_then_populated() {
        let resp = build_response("P0001");
        assert!(resp.suggested_fix.is_some());
        let fix = resp.suggested_fix.unwrap();
        assert!(fix.contains("fix"));
    }

    #[test]
    fn build_response_when_code_has_no_fix_section_then_none() {
        // P9999 has "Known Limitations" but no "To fix this error" paragraph.
        let resp = build_response("P9999");
        assert!(resp.ok);
        assert!(resp.found);
        assert!(resp.suggested_fix.is_none());
    }

    #[test]
    fn parse_rst_when_code_blocks_then_preserves_content() {
        let rst = "=====\nP0001\n=====\n\n.. problem-summary:: P0001\n\nDescription text.\n\n.. code-block::\n\n   PROGRAM p\n   END_PROGRAM\n";
        let (desc, _) = parse_rst(rst);
        assert!(desc.contains("PROGRAM p"));
        assert!(desc.contains("Description text."));
    }

    #[test]
    fn parse_rst_when_backtick_markup_then_stripped() {
        let rst = ".. problem-summary:: P0001\n\nUse ``VAR_GLOBAL`` here.\n";
        let (desc, _) = parse_rst(rst);
        assert!(desc.contains("VAR_GLOBAL"));
        assert!(!desc.contains("``"));
    }

    #[test]
    fn build_response_when_whitespace_around_code_then_trimmed() {
        let resp = build_response("  P0001  ");
        assert!(resp.ok);
        assert!(resp.found);
        assert_eq!(resp.code, "P0001");
    }

    #[test]
    fn strip_inline_markup_when_code_role_then_extracted() {
        let result = strip_inline_markup("The :code:`LTIME` type.");
        assert_eq!(result, "The LTIME type.");
    }

    #[test]
    fn strip_inline_markup_when_doc_role_then_path_extracted() {
        let result =
            strip_inline_markup("See :doc:`/explanation/enabling-dialects-and-features` for more.");
        assert_eq!(result, "See enabling-dialects-and-features for more.");
    }
}
