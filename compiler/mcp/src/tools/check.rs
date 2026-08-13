//! The `check` MCP tool.
//!
//! Runs parse and full semantic analysis, returning structured diagnostics.

use ironplc_dsl::core::FileId;
use ironplc_project::project::{MemoryBackedProject, Project};
use serde::Serialize;

use super::common::{parse_options, serialize_diagnostics, validate_sources, SourceInput};

/// Response returned by the `check` tool.
#[derive(Debug, Serialize)]
pub struct CheckResponse {
    pub ok: bool,
    pub diagnostics: Vec<serde_json::Value>,
}

/// Builds the check response from raw inputs.
pub fn build_response(sources: &[SourceInput], options_value: &serde_json::Value) -> CheckResponse {
    // Validate sources
    let source_errors = validate_sources(sources);
    if !source_errors.is_empty() {
        return CheckResponse {
            ok: false,
            diagnostics: serialize_diagnostics(&source_errors),
        };
    }

    // Parse options
    let options = match parse_options(options_value) {
        Ok(opts) => opts,
        Err(errs) => {
            return CheckResponse {
                ok: false,
                diagnostics: serialize_diagnostics(&errs),
            };
        }
    };

    // Construct a fresh in-memory project (REQ-ARC-mcp-010)
    let mut project = MemoryBackedProject::new(options);

    // Load sources (REQ-ARC-mcp-011)
    for src in sources {
        let file_id = FileId::from_string(&src.name);
        project.add_source(file_id, src.content.clone());
    }

    // Run parse + full semantic analysis
    let diagnostics = serialize_diagnostics(&project.semantic());
    CheckResponse {
        ok: diagnostics.is_empty(),
        diagnostics,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::test_support::{
        ed2_options, source, unnamed_source, SEMANTIC_ERROR_PROGRAM, SYNTAX_ERROR_PROGRAM,
        VALID_PROGRAM,
    };

    // Parse and analyze outcomes are owned by the parser and analyzer crates;
    // these tests only prove the tool wires the pipeline and validation in.

    #[test]
    fn build_response_when_valid_program_then_ok_true() {
        let resp = build_response(&source(VALID_PROGRAM), &ed2_options());
        assert!(resp.ok);
        assert!(resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_syntax_error_then_ok_false() {
        let resp = build_response(&source(SYNTAX_ERROR_PROGRAM), &ed2_options());
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_semantic_error_then_ok_false() {
        let resp = build_response(&source(SEMANTIC_ERROR_PROGRAM), &ed2_options());
        assert!(!resp.ok);
        assert!(resp
            .diagnostics
            .iter()
            .any(|d| d["code"].as_str().is_some_and(|c| !c.is_empty())));
    }

    #[test]
    fn build_response_when_invalid_sources_then_error_diagnostic() {
        let resp = build_response(&unnamed_source(), &ed2_options());
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_invalid_options_then_error_diagnostic() {
        let resp = build_response(&source(VALID_PROGRAM), &serde_json::json!({}));
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_parse_error_in_one_source_then_still_reports() {
        let sources = vec![
            SourceInput {
                name: "good.st".into(),
                content: "PROGRAM good\nEND_PROGRAM".into(),
            },
            SourceInput {
                name: "bad.st".into(),
                content: "PROGRAM".into(),
            },
        ];
        let resp = build_response(&sources, &ed2_options());
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_error_then_diagnostics_have_byte_offsets() {
        let resp = build_response(&source(SEMANTIC_ERROR_PROGRAM), &ed2_options());
        assert!(!resp.ok);
        let d = &resp.diagnostics[0];
        // Verify byte offset fields exist
        assert!(d.get("start").is_some());
        assert!(d.get("end").is_some());
    }
}
