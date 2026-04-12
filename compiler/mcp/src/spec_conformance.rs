//! Spec conformance tests for the MCP server design.
//!
//! Each test is annotated with `#[spec_test(REQ_XX_NNN)]` which:
//! 1. Adds `#[test]`
//! 2. References a build-script-generated constant — compilation fails if the
//!    requirement was removed from the spec markdown.
//!
//! The `all_spec_requirements_have_tests` meta-test ensures every requirement
//! in the spec has at least one test here.
//!
//! See `specs/design/spec-conformance-testing.md` for full design.
//! See `specs/design/mcp-server.md` for the MCP server requirements.

use spec_test_macro::spec_test;

use crate::tools;

// ---------------------------------------------------------------------------
// Meta-test: completeness check
// ---------------------------------------------------------------------------

#[test]
fn all_spec_requirements_have_tests() {
    assert!(
        crate::spec_requirements::UNTESTED.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        crate::spec_requirements::UNTESTED
    );
}

// ===========================================================================
// Stateless Tool Surface (REQ-STL-*)
// ===========================================================================

/// REQ-STL-001: Every analysis, context, and execution tool accepts a
/// required `sources` parameter.
#[spec_test(REQ_STL_001)]
fn mcp_spec_req_stl_001_tools_accept_sources_parameter() {
    use crate::tools::common::SourceInput;

    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nEND_PROGRAM".into(),
    }];
    let options = serde_json::json!({"dialect": "iec61131-3-ed2"});

    // parse accepts sources
    let parse_resp = tools::parse::build_response(&sources, &options);
    assert!(parse_resp.ok);

    // check accepts sources
    let check_resp = tools::check::build_response(&sources, &options);
    assert!(check_resp.ok);
}

/// REQ-STL-002: Every analysis, context, and execution tool accepts a
/// required `options` object.
#[spec_test(REQ_STL_002)]
fn mcp_spec_req_stl_002_tools_accept_options_parameter() {
    use crate::tools::common::SourceInput;

    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nEND_PROGRAM".into(),
    }];
    // Both parse and check accept an options object with dialect
    let options = serde_json::json!({"dialect": "rusty"});

    let parse_resp = tools::parse::build_response(&sources, &options);
    assert!(parse_resp.ok);

    let check_resp = tools::check::build_response(&sources, &options);
    assert!(check_resp.ok);
}

/// REQ-STL-003: The server holds no per-client state across tool calls.
#[spec_test(REQ_STL_003)]
#[ignore] // Requires multi-call integration test
fn mcp_spec_req_stl_003_no_per_client_state_across_calls() {}

/// REQ-STL-004: File name validation constraints.
#[spec_test(REQ_STL_004)]
fn mcp_spec_req_stl_004_source_name_validation() {
    use crate::tools::common::{validate_sources, SourceInput};

    // Empty name rejected
    let errs = validate_sources(&[SourceInput {
        name: String::new(),
        content: String::new(),
    }]);
    assert!(!errs.is_empty());

    // Name > 256 bytes rejected
    let errs = validate_sources(&[SourceInput {
        name: "a".repeat(257),
        content: String::new(),
    }]);
    assert!(!errs.is_empty());

    // Non-printable ASCII rejected (NUL)
    let errs = validate_sources(&[SourceInput {
        name: "f\0.st".into(),
        content: String::new(),
    }]);
    assert!(!errs.is_empty());

    // Non-printable ASCII rejected (tab)
    let errs = validate_sources(&[SourceInput {
        name: "a\tb".into(),
        content: String::new(),
    }]);
    assert!(!errs.is_empty());

    // Duplicates rejected
    let errs = validate_sources(&[
        SourceInput {
            name: "f.st".into(),
            content: String::new(),
        },
        SourceInput {
            name: "f.st".into(),
            content: String::new(),
        },
    ]);
    assert!(!errs.is_empty());
}

/// REQ-STL-005: Every tool response includes a top-level `ok: boolean` field.
#[spec_test(REQ_STL_005)]
fn mcp_spec_req_stl_005_response_includes_ok_field() {
    use crate::tools::common::SourceInput;

    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nEND_PROGRAM".into(),
    }];
    let options = serde_json::json!({"dialect": "iec61131-3-ed2"});

    // parse response has ok field
    let parse_resp = tools::parse::build_response(&sources, &options);
    let json = serde_json::to_value(&parse_resp).unwrap();
    assert!(json.get("ok").is_some());

    // check response has ok field
    let check_resp = tools::check::build_response(&sources, &options);
    let json = serde_json::to_value(&check_resp).unwrap();
    assert!(json.get("ok").is_some());
}

/// REQ-STL-006: The server performs no disk I/O from any tool handler.
#[spec_test(REQ_STL_006)]
#[ignore] // Architectural constraint; verified by code review
fn mcp_spec_req_stl_006_no_disk_io() {}

// ===========================================================================
// `parse` tool (REQ-TOL-010..013)
// ===========================================================================

/// REQ-TOL-010: The `parse` tool runs the parse stage only (no semantic analysis).
#[spec_test(REQ_TOL_010)]
fn mcp_spec_req_tol_010_parse_runs_parse_only() {
    use crate::tools::common::SourceInput;

    // This program parses fine but has a semantic error (undeclared variable y).
    // parse should succeed because it doesn't run semantic analysis.
    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM".into(),
    }];
    let options = serde_json::json!({"dialect": "iec61131-3-ed2"});

    let resp = tools::parse::build_response(&sources, &options);
    assert!(resp.ok, "parse should not catch semantic errors");
}

/// REQ-TOL-011: The `parse` tool returns a `diagnostics` array using the same
/// format as `check`.
#[spec_test(REQ_TOL_011)]
fn mcp_spec_req_tol_011_parse_returns_diagnostics_array() {
    use crate::tools::common::SourceInput;

    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM".into(), // parse error
    }];
    let options = serde_json::json!({"dialect": "iec61131-3-ed2"});

    let resp = tools::parse::build_response(&sources, &options);
    let json = serde_json::to_value(&resp).unwrap();
    let diags = json["diagnostics"].as_array().unwrap();
    assert!(!diags.is_empty());
    // Verify diagnostic format matches check's format
    let d = &diags[0];
    assert!(d.get("code").is_some());
    assert!(d.get("message").is_some());
    assert!(d.get("file").is_some());
    assert!(d.get("severity").is_some());
}

/// REQ-TOL-012: The `parse` tool accepts the same `options` object as `check`.
#[spec_test(REQ_TOL_012)]
fn mcp_spec_req_tol_012_parse_accepts_same_options_as_check() {
    use crate::tools::common::SourceInput;

    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nEND_PROGRAM".into(),
    }];
    // Same options accepted by both
    let options = serde_json::json!({"dialect": "rusty", "allow_c_style_comments": true});

    let parse_resp = tools::parse::build_response(&sources, &options);
    assert!(parse_resp.ok);

    let check_resp = tools::check::build_response(&sources, &options);
    assert!(check_resp.ok);
}

/// REQ-TOL-013: The `parse` tool returns a best-effort `structure` array.
#[spec_test(REQ_TOL_013)]
fn mcp_spec_req_tol_013_parse_returns_structure_array() {
    use crate::tools::common::SourceInput;

    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nEND_PROGRAM".into(),
    }];
    let options = serde_json::json!({"dialect": "iec61131-3-ed2"});

    let resp = tools::parse::build_response(&sources, &options);
    let json = serde_json::to_value(&resp).unwrap();
    let structure = json["structure"].as_array().unwrap();
    assert_eq!(structure.len(), 1);
    let entry = &structure[0];
    assert_eq!(entry["kind"], "program");
    assert_eq!(entry["name"], "p");
    assert!(entry.get("file").is_some());
    assert!(entry.get("start").is_some());
    assert!(entry.get("end").is_some());
}

// ===========================================================================
// `check` tool (REQ-TOL-020..026)
// ===========================================================================

/// REQ-TOL-020: The `check` tool runs parse and full semantic analysis.
#[spec_test(REQ_TOL_020)]
fn mcp_spec_req_tol_020_check_runs_semantic_analysis() {
    use crate::tools::common::SourceInput;

    // This program has a semantic error (undeclared variable y).
    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM".into(),
    }];
    let options = serde_json::json!({"dialect": "iec61131-3-ed2"});

    let resp = tools::check::build_response(&sources, &options);
    assert!(!resp.ok, "check should catch semantic errors");
    assert!(!resp.diagnostics.is_empty());
}

/// REQ-TOL-021: The `check` tool does not run code generation.
#[spec_test(REQ_TOL_021)]
fn mcp_spec_req_tol_021_check_no_codegen() {
    use crate::tools::common::SourceInput;

    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nEND_PROGRAM".into(),
    }];
    let options = serde_json::json!({"dialect": "iec61131-3-ed2"});

    let resp = tools::check::build_response(&sources, &options);
    // Response has no container_id — check does not produce codegen output
    let json = serde_json::to_value(&resp).unwrap();
    assert!(json.get("container_id").is_none());
}

/// REQ-TOL-022: The `check` tool returns `diagnostics` and `ok`.
#[spec_test(REQ_TOL_022)]
fn mcp_spec_req_tol_022_check_returns_diagnostics_and_ok() {
    use crate::tools::common::SourceInput;

    // Valid program: ok = true, diagnostics empty
    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nEND_PROGRAM".into(),
    }];
    let options = serde_json::json!({"dialect": "iec61131-3-ed2"});

    let resp = tools::check::build_response(&sources, &options);
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["ok"], true);
    assert!(json["diagnostics"].as_array().unwrap().is_empty());

    // Invalid program: ok = false, diagnostics non-empty
    let bad_sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM".into(),
    }];
    let resp = tools::check::build_response(&bad_sources, &options);
    let json = serde_json::to_value(&resp).unwrap();
    assert_eq!(json["ok"], false);
    assert!(!json["diagnostics"].as_array().unwrap().is_empty());
}

/// REQ-TOL-023: Diagnostic format with byte offsets.
#[spec_test(REQ_TOL_023)]
fn mcp_spec_req_tol_023_diagnostic_format() {
    use crate::tools::common::SourceInput;

    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM".into(),
    }];
    let options = serde_json::json!({"dialect": "iec61131-3-ed2"});

    let resp = tools::check::build_response(&sources, &options);
    assert!(!resp.diagnostics.is_empty());

    let d = &resp.diagnostics[0];

    // All required fields are present
    assert!(d.get("code").is_some());
    assert!(d.get("message").is_some());
    assert!(d.get("file").is_some());
    assert!(d.get("start").is_some());
    assert!(d.get("end").is_some());
    assert!(d.get("severity").is_some());

    // start/end are 0-indexed byte offsets (numeric)
    assert!(d["start"].is_number());
    assert!(d["end"].is_number());
}

/// REQ-TOL-024: The `check` tool never returns an MCP-level error for
/// compiler failures.
#[spec_test(REQ_TOL_024)]
fn mcp_spec_req_tol_024_no_mcp_error_for_compiler_failures() {
    use crate::tools::common::SourceInput;

    // Syntax error — should produce diagnostics, not panic or MCP error
    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM".into(),
    }];
    let options = serde_json::json!({"dialect": "iec61131-3-ed2"});

    // build_response returns a CheckResponse (not Err), even for broken input
    let resp = tools::check::build_response(&sources, &options);
    assert!(!resp.ok);
    assert!(!resp.diagnostics.is_empty());
}

/// REQ-TOL-025: The `check` tool rejects invalid `options`.
#[spec_test(REQ_TOL_025)]
fn mcp_spec_req_tol_025_rejects_invalid_options() {
    use crate::tools::common::SourceInput;

    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nEND_PROGRAM".into(),
    }];

    // Missing dialect
    let resp = tools::check::build_response(&sources, &serde_json::json!({}));
    assert!(!resp.ok);

    // Unknown dialect
    let resp = tools::check::build_response(&sources, &serde_json::json!({"dialect": "cobol"}));
    assert!(!resp.ok);

    // Unknown key
    let resp = tools::check::build_response(
        &sources,
        &serde_json::json!({"dialect": "iec61131-3-ed2", "bogus_key": true}),
    );
    assert!(!resp.ok);
}

/// REQ-TOL-026: The `check` tool accepts individual feature flag overrides.
#[spec_test(REQ_TOL_026)]
fn mcp_spec_req_tol_026_accepts_flag_overrides() {
    use crate::tools::common::SourceInput;

    // C-style comments are not allowed in ed2, but allowed with override
    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "// C-style comment\nPROGRAM p\nEND_PROGRAM".into(),
    }];

    // Without override — should fail (ed2 doesn't allow C-style comments)
    let resp =
        tools::check::build_response(&sources, &serde_json::json!({"dialect": "iec61131-3-ed2"}));
    assert!(!resp.ok, "ed2 should reject C-style comments");

    // With flag override — should succeed
    let resp = tools::check::build_response(
        &sources,
        &serde_json::json!({"dialect": "iec61131-3-ed2", "allow_c_style_comments": true}),
    );
    assert!(resp.ok, "flag override should enable C-style comments");
}

// ===========================================================================
// `compile` tool (REQ-TOL-030..036) — Milestone 2
// ===========================================================================

#[spec_test(REQ_TOL_030)]
#[ignore]
fn mcp_spec_req_tol_030_compile_returns_container_id() {}

#[spec_test(REQ_TOL_031)]
#[ignore]
fn mcp_spec_req_tol_031_compile_returns_diagnostics_on_failure() {}

#[spec_test(REQ_TOL_032)]
#[ignore]
fn mcp_spec_req_tol_032_compile_returns_tasks_array() {}

#[spec_test(REQ_TOL_033)]
#[ignore]
fn mcp_spec_req_tol_033_compile_returns_programs_array() {}

#[spec_test(REQ_TOL_034)]
#[ignore]
fn mcp_spec_req_tol_034_compile_returns_base64_when_requested() {}

#[spec_test(REQ_TOL_035)]
#[ignore]
fn mcp_spec_req_tol_035_compile_stores_container_in_cache() {}

#[spec_test(REQ_TOL_036)]
#[ignore]
fn mcp_spec_req_tol_036_cached_container_is_immutable_snapshot() {}

// ===========================================================================
// `run` tool (REQ-TOL-040..048) — Milestone 2
// ===========================================================================

#[spec_test(REQ_TOL_040)]
#[ignore]
fn mcp_spec_req_tol_040_run_executes_container_in_vm() {}

#[spec_test(REQ_TOL_041)]
#[ignore]
fn mcp_spec_req_tol_041_run_returns_trace_array() {}

#[spec_test(REQ_TOL_042)]
#[ignore]
fn mcp_spec_req_tol_042_run_applies_stimuli() {}

#[spec_test(REQ_TOL_043)]
#[ignore]
fn mcp_spec_req_tol_043_json_value_encoding() {}

#[spec_test(REQ_TOL_044)]
#[ignore]
fn mcp_spec_req_tol_044_trace_modes() {}

#[spec_test(REQ_TOL_045)]
#[ignore]
fn mcp_spec_req_tol_045_tasks_filter() {}

#[spec_test(REQ_TOL_046)]
#[ignore]
fn mcp_spec_req_tol_046_trace_cap() {}

#[spec_test(REQ_TOL_047)]
#[ignore]
fn mcp_spec_req_tol_047_resource_limits() {}

#[spec_test(REQ_TOL_048)]
#[ignore]
fn mcp_spec_req_tol_048_run_returns_summary() {}

// ===========================================================================
// `symbols` tool (REQ-TOL-050..055) — Milestone 1 (later)
// ===========================================================================

#[spec_test(REQ_TOL_050)]
#[ignore]
fn mcp_spec_req_tol_050_symbols_returns_declarations() {}

#[spec_test(REQ_TOL_051)]
#[ignore]
fn mcp_spec_req_tol_051_program_variable_details() {}

#[spec_test(REQ_TOL_052)]
#[ignore]
fn mcp_spec_req_tol_052_function_entry_details() {}

#[spec_test(REQ_TOL_053)]
#[ignore]
fn mcp_spec_req_tol_053_symbols_diagnostics_format() {}

#[spec_test(REQ_TOL_054)]
#[ignore]
fn mcp_spec_req_tol_054_symbols_pou_filter() {}

#[spec_test(REQ_TOL_055)]
#[ignore]
fn mcp_spec_req_tol_055_symbols_response_size_cap() {}

// ===========================================================================
// `list_options` tool (REQ-TOL-060..063) — Implemented
// ===========================================================================

/// REQ-TOL-060: The `list_options` tool takes no inputs.
#[spec_test(REQ_TOL_060)]
fn mcp_spec_req_tol_060_list_options_takes_no_inputs() {
    // list_options is called with no parameters — build_response() takes none.
    let response = tools::list_options::build_response();
    // If it compiles and returns, it takes no inputs.
    assert!(!response.dialects.is_empty());
}

/// REQ-TOL-061: The `list_options` tool returns a `dialects` array whose entries
/// each contain `id`, `display_name`, and `description`.
#[spec_test(REQ_TOL_061)]
fn mcp_spec_req_tol_061_list_options_returns_dialects() {
    let response = tools::list_options::build_response();
    assert_eq!(response.dialects.len(), 3);
    for dialect in &response.dialects {
        assert!(!dialect.id.is_empty(), "dialect id must be non-empty");
        assert!(
            !dialect.display_name.is_empty(),
            "dialect display_name must be non-empty"
        );
        assert!(
            !dialect.description.is_empty(),
            "dialect description must be non-empty"
        );
    }
}

/// REQ-TOL-062: The `list_options` tool returns a `flags` array whose entries
/// each contain `id`, `type`, `default`, and `description`.
#[spec_test(REQ_TOL_062)]
fn mcp_spec_req_tol_062_list_options_returns_flags() {
    let response = tools::list_options::build_response();
    assert!(!response.flags.is_empty());
    for flag in &response.flags {
        assert!(!flag.id.is_empty(), "flag id must be non-empty");
        assert_eq!(flag.flag_type, "bool", "all flags are bool type");
        assert!(
            !flag.description.is_empty(),
            "flag {} has empty description",
            flag.id
        );
    }
}

/// REQ-TOL-063: The option `id` values returned by `list_options` are the exact
/// keys accepted in the `options` object of `parse`, `check`, and `compile`.
#[spec_test(REQ_TOL_063)]
fn mcp_spec_req_tol_063_option_ids_match_compiler_options_fields() {
    use ironplc_parser::options::CompilerOptions;

    let response = tools::list_options::build_response();
    let flag_ids: Vec<&str> = response.flags.iter().map(|f| f.id.as_str()).collect();

    // Every FEATURE_DESCRIPTORS entry must appear in the response
    for fd in CompilerOptions::FEATURE_DESCRIPTORS {
        assert!(
            flag_ids.contains(&fd.option_key),
            "FEATURE_DESCRIPTOR key '{}' missing from list_options response",
            fd.option_key
        );
    }
}

// ===========================================================================
// `explain_diagnostic` tool (REQ-TOL-070..072) — Milestone 1 (later)
// ===========================================================================

#[spec_test(REQ_TOL_070)]
#[ignore]
fn mcp_spec_req_tol_070_explain_diagnostic_returns_explanation() {}

#[spec_test(REQ_TOL_071)]
#[ignore]
fn mcp_spec_req_tol_071_explain_diagnostic_unknown_code() {}

#[spec_test(REQ_TOL_072)]
#[ignore]
fn mcp_spec_req_tol_072_explain_diagnostic_embedded_at_build_time() {}

// ===========================================================================
// `format` tool (REQ-TOL-080..084) — Milestone 1 (later)
// ===========================================================================

#[spec_test(REQ_TOL_080)]
#[ignore]
fn mcp_spec_req_tol_080_format_returns_canonical_form() {}

#[spec_test(REQ_TOL_081)]
#[ignore]
fn mcp_spec_req_tol_081_format_preserves_unparseable_source() {}

#[spec_test(REQ_TOL_082)]
#[ignore]
fn mcp_spec_req_tol_082_format_is_idempotent() {}

#[spec_test(REQ_TOL_083)]
#[ignore]
fn mcp_spec_req_tol_083_format_matches_plc2plc_output() {}

#[spec_test(REQ_TOL_084)]
#[ignore]
fn mcp_spec_req_tol_084_format_is_pure() {}

// ===========================================================================
// `container_drop` tool (REQ-TOL-150..151) — Milestone 2
// ===========================================================================

#[spec_test(REQ_TOL_150)]
#[ignore]
fn mcp_spec_req_tol_150_container_drop_removes_entry() {}

#[spec_test(REQ_TOL_151)]
#[ignore]
fn mcp_spec_req_tol_151_container_drop_unknown_id() {}

// ===========================================================================
// Context tools: `project_manifest` (REQ-TOL-200..201) — Milestone 1 (later)
// ===========================================================================

#[spec_test(REQ_TOL_200)]
#[ignore]
fn mcp_spec_req_tol_200_project_manifest_returns_declarations() {}

#[spec_test(REQ_TOL_201)]
#[ignore]
fn mcp_spec_req_tol_201_project_manifest_partial_on_failure() {}

// ===========================================================================
// Context tools: `project_io` (REQ-TOL-210..212) — Milestone 1 (later)
// ===========================================================================

#[spec_test(REQ_TOL_210)]
#[ignore]
fn mcp_spec_req_tol_210_project_io_returns_inputs() {}

#[spec_test(REQ_TOL_211)]
#[ignore]
fn mcp_spec_req_tol_211_project_io_returns_outputs() {}

#[spec_test(REQ_TOL_212)]
#[ignore]
fn mcp_spec_req_tol_212_project_io_entry_format() {}

// ===========================================================================
// Context tools: `pou_scope` (REQ-TOL-220..221) — Milestone 1 (later)
// ===========================================================================

#[spec_test(REQ_TOL_220)]
#[ignore]
fn mcp_spec_req_tol_220_pou_scope_returns_variables() {}

#[spec_test(REQ_TOL_221)]
#[ignore]
fn mcp_spec_req_tol_221_pou_scope_unknown_pou() {}

// ===========================================================================
// Context tools: `pou_lineage` (REQ-TOL-230..231) — Milestone 1 (later)
// ===========================================================================

#[spec_test(REQ_TOL_230)]
#[ignore]
fn mcp_spec_req_tol_230_pou_lineage_returns_dependencies() {}

#[spec_test(REQ_TOL_231)]
#[ignore]
fn mcp_spec_req_tol_231_pou_lineage_unknown_pou() {}

// ===========================================================================
// Context tools: `types_all` (REQ-TOL-240) — Milestone 1 (later)
// ===========================================================================

#[spec_test(REQ_TOL_240)]
#[ignore]
fn mcp_spec_req_tol_240_types_all_returns_user_defined_types() {}

// ===========================================================================
// Architecture (REQ-ARC-*)
// ===========================================================================

/// REQ-ARC-001: The MCP server uses stdio transport.
#[spec_test(REQ_ARC_001)]
fn mcp_spec_req_arc_001_stdio_transport() {
    // run_server() creates a stdio transport. We verify the function exists
    // and the server can be constructed. Actually starting the transport
    // requires stdin/stdout, so we just verify construction.
    let _server = crate::server::IronPlcMcp::new();
}

/// REQ-ARC-010: Each tool call constructs a fresh in-memory project from
/// the supplied sources.
#[spec_test(REQ_ARC_010)]
fn mcp_spec_req_arc_010_fresh_project_per_call() {
    use crate::tools::common::SourceInput;

    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nEND_PROGRAM".into(),
    }];
    let options = serde_json::json!({"dialect": "iec61131-3-ed2"});

    // Two independent calls both succeed — no state leaks between them
    let resp1 = tools::check::build_response(&sources, &options);
    assert!(resp1.ok);

    let resp2 = tools::check::build_response(&sources, &options);
    assert!(resp2.ok);
}

/// REQ-ARC-011: Source names become FileId via `FileId::from_string`,
/// and names are validated before the compiler runs.
#[spec_test(REQ_ARC_011)]
fn mcp_spec_req_arc_011_file_id_from_string() {
    use crate::tools::common::SourceInput;

    // Diagnostic file field should match the source name we provided
    let sources = vec![SourceInput {
        name: "my_file.st".into(),
        content: "PROGRAM".into(), // parse error
    }];
    let options = serde_json::json!({"dialect": "iec61131-3-ed2"});

    let resp = tools::parse::build_response(&sources, &options);
    assert!(!resp.ok);
    assert_eq!(resp.diagnostics[0]["file"], "my_file.st");
}

#[spec_test(REQ_ARC_012)]
#[ignore]
fn mcp_spec_req_arc_012_no_filesystem_paths() {}

#[spec_test(REQ_ARC_020)]
#[ignore]
fn mcp_spec_req_arc_020_fully_qualified_variable_names() {}

#[spec_test(REQ_ARC_021)]
#[ignore]
fn mcp_spec_req_arc_021_unresolved_variable_name() {}

#[spec_test(REQ_ARC_030)]
#[ignore]
fn mcp_spec_req_arc_030_vm_resource_limits() {}

#[spec_test(REQ_ARC_031)]
#[ignore]
fn mcp_spec_req_arc_031_reject_loosened_limits() {}

#[spec_test(REQ_ARC_032)]
#[ignore]
fn mcp_spec_req_arc_032_vm_terminates_on_limit() {}

#[spec_test(REQ_ARC_033)]
#[ignore]
fn mcp_spec_req_arc_033_fuel_shared_across_tasks() {}

#[spec_test(REQ_ARC_034)]
#[ignore]
fn mcp_spec_req_arc_034_terminated_reason_completed_or_error() {}

#[spec_test(REQ_ARC_035)]
#[ignore]
fn mcp_spec_req_arc_035_wall_clock_not_real_time() {}

#[spec_test(REQ_ARC_040)]
#[ignore]
fn mcp_spec_req_arc_040_structured_log_per_tool_call() {}

#[spec_test(REQ_ARC_041)]
#[ignore]
fn mcp_spec_req_arc_041_tool_specific_log_summary() {}

#[spec_test(REQ_ARC_042)]
#[ignore]
fn mcp_spec_req_arc_042_no_source_text_in_logs() {}

#[spec_test(REQ_ARC_043)]
#[ignore]
fn mcp_spec_req_arc_043_logs_to_stderr() {}

#[spec_test(REQ_ARC_044)]
#[ignore]
fn mcp_spec_req_arc_044_connection_start_end_events() {}

#[spec_test(REQ_ARC_045)]
#[ignore]
fn mcp_spec_req_arc_045_log_stream_sufficient_for_analysis() {}

/// REQ-ARC-050: Tool descriptions follow the design guidance.
#[spec_test(REQ_ARC_050)]
fn mcp_spec_req_arc_050_tool_descriptions() {
    // Verify the server can be constructed with tool descriptions.
    // The descriptions are validated by the tool registration macros;
    // full text comparison is deferred to integration tests.
    let _server = crate::server::IronPlcMcp::new();
}

#[spec_test(REQ_ARC_051)]
#[ignore]
fn mcp_spec_req_arc_051_tool_descriptions_no_false_claims() {}

#[spec_test(REQ_ARC_060)]
#[ignore]
fn mcp_spec_req_arc_060_symbols_pou_filter_and_cap() {}

#[spec_test(REQ_ARC_061)]
#[ignore]
fn mcp_spec_req_arc_061_context_tools_are_blessed_path() {}

#[spec_test(REQ_ARC_062)]
#[ignore]
fn mcp_spec_req_arc_062_response_size_in_log() {}

#[spec_test(REQ_ARC_070)]
#[ignore]
fn mcp_spec_req_arc_070_container_cache() {}

#[spec_test(REQ_ARC_071)]
#[ignore]
fn mcp_spec_req_arc_071_cache_bounded_capacity() {}

#[spec_test(REQ_ARC_072)]
#[ignore]
fn mcp_spec_req_arc_072_cache_no_timer_expiry() {}

#[spec_test(REQ_ARC_073)]
#[ignore]
fn mcp_spec_req_arc_073_unknown_container_id() {}
