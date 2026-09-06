//! CLI integration tests for the ironplcmcp binary.
//!
//! These tests spawn the binary as a subprocess and drive the MCP protocol
//! over stdin/stdout, verifying that the binary starts correctly, responds
//! to the standard MCP handshake, and returns correct results for tool calls.

use assert_cmd::Command;
use ironplc_test::fixtures::{
    COUNTER_PROGRAM, COUNTER_PROGRAM_WITH_TASK, ENUM_TYPE_PROGRAM, PROGRAM_USING_FB,
    PROGRAM_USING_STDLIB_FB, PROGRAM_WITH_INPUT, PROGRAM_WITH_INPUT_AND_LOCAL, PROGRAM_WITH_VAR,
    SEMANTIC_ERROR_PROGRAM, SYNTAX_ERROR_PROGRAM, USER_TYPES_PROGRAM, VALID_PROGRAM,
};
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
// Tool-call argument builders
//
// Most tool-call tests below differ only in (tool name, JSON args, expected
// substring). The source text comes from `ironplc_test::fixtures`, shared
// with the unit tests in `src/`, and these builders wrap it in the tool-call
// argument object so the JSON escaping lives in one place.
// ---------------------------------------------------------------------------

/// Options selecting the IEC 61131-3 second-edition dialect.
fn ed2_options() -> serde_json::Value {
    serde_json::json!({"dialect": "iec61131-3-ed2"})
}

/// Tool-call arguments: one source named `name` holding `content`, plus
/// `options`.
fn tool_args(name: &str, content: &str, options: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"sources": [{"name": name, "content": content}], "options": options})
}

/// The common case: a source `main.st` under the ed2 dialect.
fn ed2_args(content: &str) -> String {
    tool_args("main.st", content, ed2_options()).to_string()
}

/// [`ed2_args`] plus the `pou` the context tools query.
fn ed2_args_for_pou(content: &str, pou: &str) -> String {
    let mut args = tool_args("main.st", content, ed2_options());
    args["pou"] = serde_json::json!(pou);
    args.to_string()
}

/// Source whose `name` is empty — triggers input validation (P8001).
fn empty_source_name_args() -> String {
    tool_args("", VALID_PROGRAM, ed2_options()).to_string()
}

/// Valid sources but `options` is missing the `dialect` field — triggers P8001.
fn missing_dialect_args() -> String {
    tool_args("main.st", VALID_PROGRAM, serde_json::json!({})).to_string()
}

// ---------------------------------------------------------------------------
// Per-tool wire dispatch + shared error-path representatives
//
// Each case invokes the named MCP tool with the given JSON arguments and
// asserts the expected substring appears in stdout.
//
// The table keeps one wire-level dispatch case per tool (preferring the
// tool's own response field, which also proves dispatch and ok:true) plus a
// single representative per shared error class (syntax error, semantic
// error, empty source name, missing dialect). The full tool × error-class
// matrix lives in each tool's unit tests (`src/tools/*.rs`); the error
// handling itself is owned by `tools::common` and the parser/analyzer
// crates, so re-running every combination through a subprocess added no
// signal.
// ---------------------------------------------------------------------------
#[rstest]
// parse
#[case::parse_valid_program_structure_program(
    "parse",
    ed2_args(VALID_PROGRAM),
    r#"\"kind\":\"program\""#
)]
#[case::parse_syntax_error_diagnostics_code(
    "parse",
    ed2_args(SYNTAX_ERROR_PROGRAM),
    r#"\"code\":"#
)]
// check (also the shared error-class representatives)
#[case::check_valid_program_ok_true("check", ed2_args(VALID_PROGRAM), r#"\"ok\":true"#)]
#[case::check_semantic_error_diagnostics("check", ed2_args(SEMANTIC_ERROR_PROGRAM), r#"\"code\":"#)]
#[case::check_empty_source_name_validation_error("check", empty_source_name_args(), "P8001")]
#[case::check_missing_dialect_validation_error("check", missing_dialect_args(), "P8001")]
// compile
#[case::compile_valid_program_container_id_present(
    "compile",
    ed2_args(COUNTER_PROGRAM),
    r#"\"container_id\":\"c_"#
)]
#[case::compile_with_config_tasks_populated(
    "compile",
    ed2_args(COUNTER_PROGRAM_WITH_TASK),
    r#"\"name\":\"plc_task\""#
)]
#[case::compile_with_config_programs_populated(
    "compile",
    ed2_args(COUNTER_PROGRAM_WITH_TASK),
    r#"\"name\":\"program1\""#
)]
// symbols
#[case::symbols_valid_program_programs_populated(
    "symbols",
    ed2_args(PROGRAM_WITH_VAR),
    r#"\"name\":\"p\""#
)]
// project_manifest
#[case::project_manifest_enum_type_in_enumerations(
    "project_manifest",
    ed2_args(ENUM_TYPE_PROGRAM),
    r#"\"enumerations\":[\"MyEnum\"]"#
)]
// project_io
#[case::project_io_valid_program_input_listed(
    "project_io",
    ed2_args(PROGRAM_WITH_INPUT),
    r#"\"name\":\"p.start\""#
)]
// pou_scope
#[case::pou_scope_valid_variable_listed(
    "pou_scope",
    ed2_args_for_pou(PROGRAM_WITH_INPUT_AND_LOCAL, "p"),
    r#"\"name\":\"start\""#
)]
#[case::pou_scope_missing_found_false(
    "pou_scope",
    ed2_args_for_pou(VALID_PROGRAM, "nonexistent"),
    r#"\"found\":false"#
)]
// pou_lineage
#[case::pou_lineage_valid_upstream_has_counter(
    "pou_lineage",
    ed2_args_for_pou(PROGRAM_USING_FB, "Main"),
    "Counter"
)]
#[case::pou_lineage_stdlib_upstream_tagged(
    "pou_lineage",
    ed2_args_for_pou(PROGRAM_USING_STDLIB_FB, "MotorStartStop"),
    r#"\"name\":\"TON\",\"source\":\"stdlib\""#
)]
#[case::pou_lineage_missing_found_false(
    "pou_lineage",
    ed2_args_for_pou(VALID_PROGRAM, "nonexistent"),
    r#"\"found\":false"#
)]
// types_all
#[case::types_all_valid_enum_kind(
    "types_all",
    ed2_args(USER_TYPES_PROGRAM),
    r#"\"kind\":\"enum\""#
)]
#[case::types_all_valid_struct_kind(
    "types_all",
    ed2_args(USER_TYPES_PROGRAM),
    r#"\"kind\":\"struct\""#
)]
fn tool_call_then_stdout_contains(
    #[case] tool: &str,
    #[case] arguments_json: String,
    #[case] expected: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(tool, &arguments_json);
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
    let stdin = mcp_tool_call("project_manifest", &ed2_args(VALID_PROGRAM));
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"files\":[\"main.st\"]"#))
        .stdout(predicate::str::contains(r#"\"programs\":[\"p\"]"#));
    Ok(())
}

// ---------------------------------------------------------------------------
// `run` tool
// ---------------------------------------------------------------------------

/// `run` with no container handle at all should fail fast with a diagnostic.
#[test]
fn run_when_no_container_handle_then_ok_false() -> Result<(), Box<dyn std::error::Error>> {
    let args = r#"{"duration_ms":100,"variables":[]}"#;
    let stdin = mcp_tool_call("run", args);
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"ok\":false"#));
    Ok(())
}

/// `run` with an unknown container_id should surface a P8001 diagnostic
/// that names the missing id.
#[test]
fn run_when_unknown_container_id_then_diagnostic_names_it() -> Result<(), Box<dyn std::error::Error>>
{
    let args = r#"{"container_id":"c_ghost","duration_ms":100,"variables":[]}"#;
    let stdin = mcp_tool_call("run", args);
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"ok\":false"#))
        .stdout(predicate::str::contains("c_ghost"));
    Ok(())
}

/// `run` guards Phase 11 features (stimuli) behind a diagnostic.
#[test]
fn run_when_stimuli_supplied_then_ok_false() -> Result<(), Box<dyn std::error::Error>> {
    let args = r#"{"container_id":"c_0","duration_ms":100,"stimuli":[{"time_ms":0,"set":{}}]}"#;
    let stdin = mcp_tool_call("run", args);
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"ok\":false"#));
    Ok(())
}

// ---------------------------------------------------------------------------
// Schema compatibility (MCP client interoperability)
//
// Some MCP clients — notably OpenCode (via the `@ai-sdk/openai-compatible`
// runtime) — reject a tool whose JSON Schema uses a *boolean* sub-schema
// (`true`/`false`) as the value of a `properties` entry, and drop the *entire*
// tool list when any single tool does so. `schemars` emits a bare `true` for an
// untyped `serde_json::Value` field unless that field carries a description, so
// this test guards every tool's input schema against silently regressing the
// integration.
// ---------------------------------------------------------------------------

/// Recursively asserts that every value inside any `properties` object is a
/// JSON object (a schema), never a boolean schema. `additionalProperties` and
/// keyword values like `default` are intentionally not checked — only schema
/// *positions* under `properties`, which is exactly what OpenCode rejects.
fn assert_no_boolean_property_schemas(value: &serde_json::Value, path: &str) {
    match value {
        serde_json::Value::Object(map) => {
            if let Some(serde_json::Value::Object(props)) = map.get("properties") {
                for (name, schema) in props {
                    assert!(
                        schema.is_object(),
                        "tool schema at {path}.properties.{name} is a boolean schema ({schema}); \
                         OpenCode and other MCP clients reject boolean property schemas and drop \
                         the whole tool list. Add a doc comment or #[schemars(description = ...)] \
                         to the field so schemars emits an object schema."
                    );
                }
            }
            for (key, child) in map {
                assert_no_boolean_property_schemas(child, &format!("{path}.{key}"));
            }
        }
        serde_json::Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                assert_no_boolean_property_schemas(child, &format!("{path}[{index}]"));
            }
        }
        _ => {}
    }
}

/// Every advertised tool must expose an object input schema with no boolean
/// `properties` values, so MCP clients such as OpenCode can load the tool list.
#[test]
fn tools_list_when_parsed_then_no_tool_uses_boolean_property_schema(
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::cargo_bin("ironplcmcp")?
        .write_stdin(MCP_TOOLS_LIST)
        .output()?;
    let stdout = String::from_utf8(output.stdout)?;
    // Find the tools/list response among the newline-delimited JSON-RPC messages
    // (the initialize response also has a `result`, but only this one has `tools`).
    let tools = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find_map(|message| message.get("result").and_then(|r| r.get("tools").cloned()))
        .unwrap();
    let tools = tools.as_array().unwrap();
    assert!(!tools.is_empty(), "expected at least one advertised tool");
    for tool in tools {
        let name = tool
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("<unnamed>");
        let schema = tool
            .get("inputSchema")
            .unwrap_or_else(|| panic!("tool {name} is missing inputSchema"));
        assert!(
            schema.is_object(),
            "tool {name} inputSchema must be an object"
        );
        assert_no_boolean_property_schemas(schema, &format!("{name}.inputSchema"));
    }
    Ok(())
}

/// `run` appears in the tools/list response so clients can discover it.
#[test]
fn tools_list_includes_run_tool() -> Result<(), Box<dyn std::error::Error>> {
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(MCP_TOOLS_LIST)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"name\":\"run\""));
    Ok(())
}

/// Compile then run: a two-step flow that exercises the compile → cache →
/// run handoff. Drives a simple counter program and asserts the trace
/// contains an increasing `Main.Counter` value.
#[test]
fn run_when_compile_then_run_counter_then_trace_shows_increment(
) -> Result<(), Box<dyn std::error::Error>> {
    let compile_args = ed2_args(COUNTER_PROGRAM_WITH_TASK);
    let compile_call = format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"compile","arguments":{compile_args}}}}}"#
    );
    // The first container assigned by a fresh cache has id "c_0".
    let run_args = r#"{"container_id":"c_0","duration_ms":500,"variables":["Main.Counter"]}"#;
    let run_call = format!(
        r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"run","arguments":{run_args}}}}}"#
    );
    let stdin = format!("{MCP_INITIALIZE}\n{MCP_INITIALIZED}\n{compile_call}\n{run_call}\n");

    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        // Compile response
        .stdout(predicate::str::contains(r#"\"container_id\":\"c_0\""#))
        // Run response
        .stdout(predicate::str::contains(
            r#"\"terminated_reason\":\"completed\""#,
        ))
        .stdout(predicate::str::contains("Main.Counter"));
    Ok(())
}
