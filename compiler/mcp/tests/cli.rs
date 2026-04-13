//! CLI integration tests for the ironplcmcp binary.
//!
//! These tests spawn the binary as a subprocess and drive the MCP protocol
//! over stdin/stdout, verifying that the binary starts correctly, responds
//! to the standard MCP handshake, and returns correct results for tool calls.

use assert_cmd::Command;
use predicates::prelude::*;

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
// parse tool
// ---------------------------------------------------------------------------

#[test]
fn parse_when_valid_program_then_ok_true() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "parse",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM p\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"ok\":true"#));
    Ok(())
}

#[test]
fn parse_when_valid_program_then_structure_contains_program(
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "parse",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM p\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"kind\":\"program\""#));
    Ok(())
}

#[test]
fn parse_when_syntax_error_then_ok_false() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "parse",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"ok\":false"#));
    Ok(())
}

#[test]
fn parse_when_syntax_error_then_diagnostics_contain_code() -> Result<(), Box<dyn std::error::Error>>
{
    let stdin = mcp_tool_call(
        "parse",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"code\":"#));
    Ok(())
}

#[test]
fn parse_when_empty_source_name_then_validation_error() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "parse",
        r#"{"sources":[{"name":"","content":"PROGRAM p\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains("P8001"));
    Ok(())
}

#[test]
fn parse_when_missing_dialect_then_validation_error() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "parse",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM p\nEND_PROGRAM"}],"options":{}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains("P8001"));
    Ok(())
}

// ---------------------------------------------------------------------------
// check tool
// ---------------------------------------------------------------------------

#[test]
fn check_when_valid_program_then_ok_true() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "check",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM p\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"ok\":true"#));
    Ok(())
}

#[test]
fn check_when_syntax_error_then_ok_false() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "check",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"ok\":false"#));
    Ok(())
}

#[test]
fn check_when_semantic_error_then_ok_false() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "check",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"ok\":false"#));
    Ok(())
}

#[test]
fn check_when_semantic_error_then_diagnostics_present() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "check",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM p\nVAR x : INT; END_VAR\nx := y;\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"code\":"#));
    Ok(())
}

#[test]
fn check_when_empty_source_name_then_validation_error() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "check",
        r#"{"sources":[{"name":"","content":"PROGRAM p\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains("P8001"));
    Ok(())
}

#[test]
fn check_when_missing_dialect_then_validation_error() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "check",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM p\nEND_PROGRAM"}],"options":{}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains("P8001"));
    Ok(())
}

// ---------------------------------------------------------------------------
// compile tool
// ---------------------------------------------------------------------------

#[test]
fn compile_when_valid_program_then_ok_true() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "compile",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM Main\nVAR\n  x : INT;\nEND_VAR\n  x := 1;\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"ok\":true"#));
    Ok(())
}

#[test]
fn compile_when_valid_program_then_container_id_present() -> Result<(), Box<dyn std::error::Error>>
{
    let stdin = mcp_tool_call(
        "compile",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM Main\nVAR\n  x : INT;\nEND_VAR\n  x := 1;\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"container_id\":\"c_"#));
    Ok(())
}

#[test]
fn compile_when_valid_program_with_config_then_tasks_populated(
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "compile",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM Main\nVAR\n  x : INT;\nEND_VAR\n  x := 1;\nEND_PROGRAM\n\nCONFIGURATION config\n  RESOURCE resource1 ON PLC\n    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);\n    PROGRAM program1 WITH plc_task : Main;\n  END_RESOURCE\nEND_CONFIGURATION"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"name\":\"plc_task\""#));
    Ok(())
}

#[test]
fn compile_when_valid_program_with_config_then_programs_populated(
) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "compile",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM Main\nVAR\n  x : INT;\nEND_VAR\n  x := 1;\nEND_PROGRAM\n\nCONFIGURATION config\n  RESOURCE resource1 ON PLC\n    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);\n    PROGRAM program1 WITH plc_task : Main;\n  END_RESOURCE\nEND_CONFIGURATION"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"name\":\"program1\""#));
    Ok(())
}

#[test]
fn compile_when_syntax_error_then_ok_false() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "compile",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains(r#"\"ok\":false"#));
    Ok(())
}

#[test]
fn compile_when_empty_source_name_then_validation_error() -> Result<(), Box<dyn std::error::Error>>
{
    let stdin = mcp_tool_call(
        "compile",
        r#"{"sources":[{"name":"","content":"PROGRAM p\nEND_PROGRAM"}],"options":{"dialect":"iec61131-3-ed2"}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains("P8001"));
    Ok(())
}

#[test]
fn compile_when_missing_dialect_then_validation_error() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = mcp_tool_call(
        "compile",
        r#"{"sources":[{"name":"main.st","content":"PROGRAM p\nEND_PROGRAM"}],"options":{}}"#,
    );
    Command::cargo_bin("ironplcmcp")?
        .write_stdin(stdin)
        .assert()
        .success()
        .stdout(predicate::str::contains("P8001"));
    Ok(())
}
