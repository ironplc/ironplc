//! CLI integration tests for the ironplcmcp binary.
//!
//! These tests spawn the binary as a subprocess and drive the MCP protocol
//! over stdin/stdout, verifying that the binary starts correctly, responds
//! to the standard MCP handshake, and returns correct results for tool calls.

use assert_cmd::Command;
use predicates::prelude::*;
use rstest::rstest;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The MCP initialize request.
const MCP_INITIALIZE: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}"#;

/// The MCP initialized notification.
const MCP_INITIALIZED: &str =
    r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#;

/// The three-message sequence required to get a tools/list response:
/// initialize -> notifications/initialized -> tools/list.
const MCP_TOOLS_LIST: &str = concat!(
    "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test\",\"version\":\"0.1\"}}}\n",
    "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"params\":{}}\n",
    "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n",
);

/// Constructs the three-message MCP stdin sequence needed to invoke a tool:
/// initialize -> notifications/initialized -> the provided tools/call request.
fn mcp_tool_call(tool_name: &str, arguments_json: &str) -> String {
    let call = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"{}","arguments":{}}}}}"#,
        tool_name, arguments_json
    );
    format!("{MCP_INITIALIZE}\n{MCP_INITIALIZED}\n{call}\n")
}

// ---------------------------------------------------------------------------
// Handshake & tools/list (existing tests)
// ---------------------------------------------------------------------------

#[test]
fn initialize_when_valid_handshake_then_ok() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(MCP_TOOLS_LIST)
        .assert()
        .success()
        .stdout(predicate::str::contains("list_options"));
    Ok(())
}

#[test]
fn tools_list_when_valid_handshake_then_contains_list_options(
) -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(MCP_TOOLS_LIST)
        .assert()
        .stdout(predicate::str::contains("\"name\":\"list_options\""));
    Ok(())
}

#[test]
fn initialize_when_valid_handshake_then_returns_protocol_version(
) -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(MCP_TOOLS_LIST)
        .assert()
        .stdout(predicate::str::contains("2024-11-05"));
    Ok(())
}

// ---------------------------------------------------------------------------
// Tool-call argument fixtures
//
// Most tool-call tests below differ only in (tool name, JSON args, expected
// substring), so the JSON payloads are factored out as named constants and
// reused across the parametrized table. Add a new constant only when the
// payload is genuinely new — prefer reusing an existing one.
// ---------------------------------------------------------------------------

/// Single valid 2-line program; no semantic checks fail.
const ARGS_VALID_PROGRAM: &str = r#"{"sources":[{"name":"main.st","content":"PROGRAM p\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#;

/// Truncated program that fails parsing.
const ARGS_SYNTAX_ERROR: &str = r#"{"sources":[{"name":"main.st","content":"PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#;

/// Program that parses but references an undeclared variable `y`.
const ARGS_SEMANTIC_ERROR: &str = r#"{"sources":[{"name":"main.st","content":"PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#;

/// Source whose `name` is empty — triggers input validation (P8001).
const ARGS_EMPTY_SOURCE_NAME: &str = r#"{"sources":[{"name":"","content":"PROGRAM p\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#;

/// Valid sources but `options` is missing the `dialect` field — triggers P8001.
const ARGS_MISSING_DIALECT: &str =
    r#"{"sources":[{"name":"main.st","content":"PROGRAM p\nEND_PROGRAM"}],"options":{}}"#;

/// Compilable program with an explicit DINT initialization.
const ARGS_COMPILE_VALID: &str = r#"{"sources":[{"name":"main.st","content":"PROGRAM Main\nVAR\n  x : INT;\nEND_VAR\n  x := 1;\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#;

/// Compile input that also declares a CONFIGURATION + RESOURCE + TASK.
const ARGS_COMPILE_WITH_CONFIG: &str = r#"{"sources":[{"name":"main.st","content":"PROGRAM Main\nVAR\n  x : INT;\nEND_VAR\n  x := 1;\nEND_PROGRAM\n\nCONFIGURATION config\n  RESOURCE resource1 ON PLC\n    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);\n    PROGRAM program1 WITH plc_task : Main;\n  END_RESOURCE\nEND_CONFIGURATION"}],"options":{"dialect":"iec61131-3-ed2"}}"#;

/// Program with a single declared variable — used by the `symbols` happy path.
const ARGS_SYMBOLS_VALID: &str = r#"{"sources":[{"name":"main.st","content":"PROGRAM p\nVAR x : INT; END_VAR\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#;

/// Program that defines an enumerated TYPE — used by `project_manifest`.
const ARGS_ENUM_TYPE: &str = r#"{"sources":[{"name":"main.st","content":"TYPE MyEnum : (A, B, C); END_TYPE\nPROGRAM p\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#;

/// Program declaring one `VAR_INPUT` — used by the `project_io` happy path.
const ARGS_PROJECT_IO_INPUT: &str = r#"{"sources":[{"name":"main.st","content":"PROGRAM p\nVAR_INPUT start : BOOL; END_VAR\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#;

// ---------------------------------------------------------------------------
// Per-tool happy/error paths
//
// Each case invokes the named MCP tool with the given JSON arguments and
// asserts the expected substring appears in stdout. This single parametrized
// test replaces 29 hand-written `#[test] fn`s that all followed this exact
// shape (cargo-dupes group `36d0bb0d`).
// ---------------------------------------------------------------------------
#[rstest]
// parse
#[case::parse_valid_program_ok_true("parse", ARGS_VALID_PROGRAM, r#"\"ok\":true"#)]
#[case::parse_valid_program_structure_program(
    "parse",
    ARGS_VALID_PROGRAM,
    r#"\"kind\":\"program\""#
)]
#[case::parse_syntax_error_ok_false("parse", ARGS_SYNTAX_ERROR, r#"\"ok\":false"#)]
#[case::parse_syntax_error_diagnostics_code("parse", ARGS_SYNTAX_ERROR, r#"\"code\":"#)]
#[case::parse_empty_source_name_validation_error("parse", ARGS_EMPTY_SOURCE_NAME, "P8001")]
#[case::parse_missing_dialect_validation_error("parse", ARGS_MISSING_DIALECT, "P8001")]
// check
#[case::check_valid_program_ok_true("check", ARGS_VALID_PROGRAM, r#"\"ok\":true"#)]
#[case::check_syntax_error_ok_false("check", ARGS_SYNTAX_ERROR, r#"\"ok\":false"#)]
#[case::check_semantic_error_ok_false("check", ARGS_SEMANTIC_ERROR, r#"\"ok\":false"#)]
#[case::check_semantic_error_diagnostics("check", ARGS_SEMANTIC_ERROR, r#"\"code\":"#)]
#[case::check_empty_source_name_validation_error("check", ARGS_EMPTY_SOURCE_NAME, "P8001")]
#[case::check_missing_dialect_validation_error("check", ARGS_MISSING_DIALECT, "P8001")]
// compile
#[case::compile_valid_program_ok_true("compile", ARGS_COMPILE_VALID, r#"\"ok\":true"#)]
#[case::compile_valid_program_container_id_present(
    "compile",
    ARGS_COMPILE_VALID,
    r#"\"container_id\":\"c_"#
)]
#[case::compile_with_config_tasks_populated(
    "compile",
    ARGS_COMPILE_WITH_CONFIG,
    r#"\"name\":\"plc_task\""#
)]
#[case::compile_with_config_programs_populated(
    "compile",
    ARGS_COMPILE_WITH_CONFIG,
    r#"\"name\":\"program1\""#
)]
#[case::compile_syntax_error_ok_false("compile", ARGS_SYNTAX_ERROR, r#"\"ok\":false"#)]
#[case::compile_empty_source_name_validation_error("compile", ARGS_EMPTY_SOURCE_NAME, "P8001")]
#[case::compile_missing_dialect_validation_error("compile", ARGS_MISSING_DIALECT, "P8001")]
// symbols
#[case::symbols_valid_program_ok_true("symbols", ARGS_SYMBOLS_VALID, r#"\"ok\":true"#)]
#[case::symbols_valid_program_programs_populated(
    "symbols",
    ARGS_SYMBOLS_VALID,
    r#"\"name\":\"p\""#
)]
#[case::symbols_semantic_error_ok_false("symbols", ARGS_SEMANTIC_ERROR, r#"\"ok\":false"#)]
#[case::symbols_empty_source_name_validation_error("symbols", ARGS_EMPTY_SOURCE_NAME, "P8001")]
#[case::symbols_missing_dialect_validation_error("symbols", ARGS_MISSING_DIALECT, "P8001")]
// project_manifest
#[case::project_manifest_valid_program_ok_true(
    "project_manifest",
    ARGS_VALID_PROGRAM,
    r#"\"ok\":true"#
)]
#[case::project_manifest_enum_type_in_enumerations(
    "project_manifest",
    ARGS_ENUM_TYPE,
    r#"\"enumerations\":[\"MyEnum\"]"#
)]
#[case::project_manifest_semantic_error_ok_false(
    "project_manifest",
    ARGS_SEMANTIC_ERROR,
    r#"\"ok\":false"#
)]
#[case::project_manifest_empty_source_name_validation_error(
    "project_manifest",
    ARGS_EMPTY_SOURCE_NAME,
    "P8001"
)]
#[case::project_manifest_missing_dialect_validation_error(
    "project_manifest",
    ARGS_MISSING_DIALECT,
    "P8001"
)]
// project_io
#[case::project_io_valid_program_ok_true("project_io", ARGS_PROJECT_IO_INPUT, r#"\"ok\":true"#)]
#[case::project_io_valid_program_input_listed(
    "project_io",
    ARGS_PROJECT_IO_INPUT,
    r#"\"name\":\"p.start\""#
)]
#[case::project_io_semantic_error_ok_false("project_io", ARGS_SEMANTIC_ERROR, r#"\"ok\":false"#)]
#[case::project_io_empty_source_name_validation_error(
    "project_io",
    ARGS_EMPTY_SOURCE_NAME,
    "P8001"
)]
#[case::project_io_missing_dialect_validation_error("project_io", ARGS_MISSING_DIALECT, "P8001")]
fn tool_call_then_stdout_contains(
    #[case] tool: &str,
    #[case] arguments_json: &str,
    #[case] expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(tool, arguments_json);
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(expected));
    Ok(())
}

/// `project_manifest` returns both `files` and `programs` arrays for a valid
/// program. Asserts both substrings, so this case doesn't fit the single-
/// substring shape used by the parametrized table above.
#[test]
fn project_manifest_when_valid_program_then_files_and_programs_populated(
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call("project_manifest", ARGS_VALID_PROGRAM);
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"files\":[\"main.st\"]"#))
        .stdout(predicate::str::contains(r#"\"programs\":[\"p\"]"#));
    Ok(())
}
