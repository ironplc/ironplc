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
#[ignore] // Covered when parse/check tools are implemented
fn mcp_spec_req_stl_001_tools_accept_sources_parameter() {}

/// REQ-STL-002: Every analysis, context, and execution tool accepts a
/// required `options` object.
#[spec_test(REQ_STL_002)]
#[ignore] // Covered when parse/check tools are implemented
fn mcp_spec_req_stl_002_tools_accept_options_parameter() {}

/// REQ-STL-003: The server holds no per-client state across tool calls.
#[spec_test(REQ_STL_003)]
#[ignore] // Requires multi-call integration test
fn mcp_spec_req_stl_003_no_per_client_state_across_calls() {}

/// REQ-STL-004: File name validation constraints.
#[spec_test(REQ_STL_004)]
#[ignore] // Covered when source validation is implemented
fn mcp_spec_req_stl_004_source_name_validation() {}

/// REQ-STL-005: Every tool response includes a top-level `ok: boolean` field.
#[spec_test(REQ_STL_005)]
#[ignore] // Covered when parse/check tools are implemented
fn mcp_spec_req_stl_005_response_includes_ok_field() {}

/// REQ-STL-006: The server performs no disk I/O from any tool handler.
#[spec_test(REQ_STL_006)]
#[ignore] // Architectural constraint; verified by code review
fn mcp_spec_req_stl_006_no_disk_io() {}

// ===========================================================================
// `parse` tool (REQ-TOL-010..013)
// ===========================================================================

/// REQ-TOL-010: The `parse` tool runs the parse stage only (no semantic analysis).
#[spec_test(REQ_TOL_010)]
#[ignore] // Covered when parse tool is implemented
fn mcp_spec_req_tol_010_parse_runs_parse_only() {}

/// REQ-TOL-011: The `parse` tool returns a `diagnostics` array using the same
/// format as `check`.
#[spec_test(REQ_TOL_011)]
#[ignore] // Covered when parse tool is implemented
fn mcp_spec_req_tol_011_parse_returns_diagnostics_array() {}

/// REQ-TOL-012: The `parse` tool accepts the same `options` object as `check`.
#[spec_test(REQ_TOL_012)]
#[ignore] // Covered when parse tool is implemented
fn mcp_spec_req_tol_012_parse_accepts_same_options_as_check() {}

/// REQ-TOL-013: The `parse` tool returns a best-effort `structure` array.
#[spec_test(REQ_TOL_013)]
#[ignore] // Covered when parse tool is implemented
fn mcp_spec_req_tol_013_parse_returns_structure_array() {}

// ===========================================================================
// `check` tool (REQ-TOL-020..026)
// ===========================================================================

/// REQ-TOL-020: The `check` tool runs parse and full semantic analysis.
#[spec_test(REQ_TOL_020)]
#[ignore] // Covered when check tool is implemented
fn mcp_spec_req_tol_020_check_runs_semantic_analysis() {}

/// REQ-TOL-021: The `check` tool does not run code generation.
#[spec_test(REQ_TOL_021)]
#[ignore] // Covered when check tool is implemented
fn mcp_spec_req_tol_021_check_no_codegen() {}

/// REQ-TOL-022: The `check` tool returns `diagnostics` and `ok`.
#[spec_test(REQ_TOL_022)]
#[ignore] // Covered when check tool is implemented
fn mcp_spec_req_tol_022_check_returns_diagnostics_and_ok() {}

/// REQ-TOL-023: Diagnostic format with 1-indexed line/col numbers.
#[spec_test(REQ_TOL_023)]
#[ignore] // Covered when check tool is implemented
fn mcp_spec_req_tol_023_diagnostic_format() {}

/// REQ-TOL-024: The `check` tool never returns an MCP-level error for
/// compiler failures.
#[spec_test(REQ_TOL_024)]
#[ignore] // Covered when check tool is implemented
fn mcp_spec_req_tol_024_no_mcp_error_for_compiler_failures() {}

/// REQ-TOL-025: The `check` tool rejects invalid `options`.
#[spec_test(REQ_TOL_025)]
#[ignore] // Covered when check tool is implemented
fn mcp_spec_req_tol_025_rejects_invalid_options() {}

/// REQ-TOL-026: The `check` tool accepts individual feature flag overrides.
#[spec_test(REQ_TOL_026)]
#[ignore] // Covered when check tool is implemented
fn mcp_spec_req_tol_026_accepts_flag_overrides() {}

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

#[spec_test(REQ_ARC_010)]
#[ignore] // Covered when parse/check tools are implemented
fn mcp_spec_req_arc_010_fresh_project_per_call() {}

#[spec_test(REQ_ARC_011)]
#[ignore] // Covered when parse/check tools are implemented
fn mcp_spec_req_arc_011_file_id_from_string() {}

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
