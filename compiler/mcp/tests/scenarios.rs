//! Scenario tests for the IronPLC MCP server.
//!
//! Each test scripts a realistic multi-step agent workflow using the tool
//! `build_response` functions directly — no subprocess, no MCP wire protocol.
//! This keeps the tests deterministic while exercising the cross-tool contracts
//! that individual unit tests cannot catch.

use ironplc_mcp::tools::common::SourceInput;
use ironplc_mcp::tools::{check, explain_diagnostic, list_options};

fn ed2_options() -> serde_json::Value {
    serde_json::json!({"dialect": "iec61131-3-ed2"})
}

/// Scenario: agent self-healing loop.
///
/// 1. Agent drafts broken code and calls `check` — expects failure with a
///    diagnostic code.
/// 2. Agent calls `explain_diagnostic` on that code — expects a usable
///    explanation.
/// 3. Agent fixes the code and calls `check` again — expects success.
#[test]
fn scenario_agent_self_heals_syntax_error() {
    // Step 1: broken code fails check
    let broken = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nVAR x : INT END_VAR\nEND_PROGRAM".into(),
    }];
    let r1 = check::build_response(&broken, &ed2_options());
    assert!(!r1.ok, "broken code should fail check");
    assert!(
        !r1.diagnostics.is_empty(),
        "should have at least one diagnostic"
    );

    let code = r1.diagnostics[0]["code"]
        .as_str()
        .expect("diagnostic must have a code field");
    assert!(!code.is_empty(), "diagnostic code must not be empty");

    // Step 2: agent looks up the diagnostic code
    let explanation = explain_diagnostic::build_response(code);
    assert!(
        explanation.ok,
        "explain_diagnostic should succeed for a real code"
    );
    assert!(explanation.found, "code returned by check must be known");
    assert!(
        explanation
            .description
            .as_deref()
            .map(|d| !d.is_empty())
            .unwrap_or(false),
        "explanation must have a non-empty description"
    );

    // Step 3: agent fixes the missing semicolon and re-checks
    let fixed = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nVAR x : INT; END_VAR\nEND_PROGRAM".into(),
    }];
    let r2 = check::build_response(&fixed, &ed2_options());
    assert!(
        r2.ok,
        "fixed code should pass check; diagnostics: {:?}",
        r2.diagnostics
    );
    assert!(r2.diagnostics.is_empty());
}

/// Scenario: options discovery before first call.
///
/// 1. Agent calls `list_options` to discover available dialects.
/// 2. Agent picks the first dialect id from the response.
/// 3. Agent passes that id verbatim to `check` — must not get a
///    validation error (unknown dialect), regardless of whether the
///    source itself is valid.
#[test]
fn scenario_options_discovery_then_check_accepts_dialect() {
    // Step 1: discover dialects
    let options_resp = list_options::build_response();
    assert!(
        !options_resp.dialects.is_empty(),
        "list_options must return at least one dialect"
    );

    // Step 2: take the first dialect id
    let dialect_id = &options_resp.dialects[0].id;
    assert!(!dialect_id.is_empty());

    // Step 3: use that id in a check call
    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: "PROGRAM p\nEND_PROGRAM".into(),
    }];
    let opts = serde_json::json!({"dialect": dialect_id});
    let resp = check::build_response(&sources, &opts);

    // The source is valid, so we expect ok: true. More importantly, if the
    // dialect id from list_options were not accepted by check, we would get
    // a validation diagnostic with code P8001 — assert that does not happen.
    let has_validation_error = resp
        .diagnostics
        .iter()
        .any(|d| d["code"].as_str() == Some("P8001"));
    assert!(
        !has_validation_error,
        "dialect id '{}' from list_options was rejected by check as unknown",
        dialect_id
    );
    assert!(
        resp.ok,
        "valid source with discovered dialect should pass check"
    );
}
