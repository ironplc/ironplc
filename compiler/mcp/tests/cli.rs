//! CLI integration tests for the ironplcmcp binary.
//!
//! These tests spawn the binary as a subprocess and drive the MCP protocol
//! over stdin/stdout, verifying that the binary starts correctly and responds
//! to the standard MCP handshake.

use assert_cmd::Command;
use predicates::prelude::*;

/// The three-message sequence required to get a tools/list response:
/// initialize → notifications/initialized → tools/list.
const MCP_TOOLS_LIST: &str = concat!(
    "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test\",\"version\":\"0.1\"}}}\n",
    "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"params\":{}}\n",
    "{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"tools/list\",\"params\":{}}\n",
);

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
