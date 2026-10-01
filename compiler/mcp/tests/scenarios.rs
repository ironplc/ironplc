//! Scenario tests for the IronPLC MCP server.
//!
//! Each test scripts a realistic multi-step agent workflow using the tool
//! `build_response` functions directly — no subprocess, no MCP wire protocol.
//! This keeps the tests deterministic while exercising the cross-tool contracts
//! that individual unit tests cannot catch.

use std::sync::Mutex;

use ironplc_mcp::cache::ContainerCache;
use ironplc_mcp::tools::common::SourceInput;
use ironplc_mcp::tools::{check, compile, explain_diagnostic, list_options, project_io, run};
use ironplc_test::fixtures::{PROGRAM_WITH_VAR, VALID_PROGRAM};

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

    let code = r1.diagnostics[0]["code"].as_str().unwrap();
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
        content: PROGRAM_WITH_VAR.into(),
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
        content: VALID_PROGRAM.into(),
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

/// A configuration global that its program counts up once per scan.
const CONFIGURATION_GLOBAL_COUNTER: &str = "CONFIGURATION config
  VAR_GLOBAL count : INT; END_VAR
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM main_instance WITH plc_task : p;
  END_RESOURCE
END_CONFIGURATION
PROGRAM p
  VAR_EXTERNAL count : INT; END_VAR
  count := count + 1;
END_PROGRAM";

fn run_input(container_id: &str, variables: Vec<String>, trace_outputs: bool) -> run::RunInput {
    run::RunInput {
        container_id: Some(container_id.to_string()),
        container_base64: None,
        duration_ms: 500,
        freewheeling_interval_ms: None,
        variables,
        trace_outputs,
        stimuli: vec![],
        trace: None,
        limits: None,
        tasks: None,
    }
}

/// Scenario: agent plans a run from `project_io` over a global variable
/// (REQ-TOL-mcp-211, REQ-ARC-mcp-020).
///
/// 1. Agent calls `project_io` — the configuration global is an output,
///    named by its bare name.
/// 2. Agent compiles the source.
/// 3. Agent runs it, tracing the names `project_io` returned — they resolve,
///    and the global counts one per scan.
/// 4. Agent runs it again with `trace_outputs` — the global is traced.
#[test]
fn scenario_project_io_global_then_run_traces_it() {
    let sources = vec![SourceInput {
        name: "main.st".into(),
        content: CONFIGURATION_GLOBAL_COUNTER.into(),
    }];

    // Step 1: the global is an observable output.
    let io = project_io::build_response(&sources, &ed2_options());
    assert!(io.ok, "diagnostics: {:?}", io.diagnostics);
    let outputs: Vec<String> = io.outputs.iter().map(|e| e.name.clone()).collect();
    assert_eq!(outputs, vec!["count".to_string()]);

    // Step 2: compile.
    let cache = Mutex::new(ContainerCache::new(64, 64 * 1024 * 1024));
    let compiled = compile::build_response(&sources, &ed2_options(), false, &cache);
    assert!(compiled.ok, "diagnostics: {:?}", compiled.diagnostics);
    let container_id = compiled.container_id.unwrap();

    // Step 3: the names from project_io resolve in run.
    let traced = run::build_response(&run_input(&container_id, outputs, false), &cache);
    assert!(traced.ok, "diagnostics: {:?}", traced.diagnostics);
    assert_eq!(
        traced.summary.final_values["count"],
        traced.summary.completed_cycles["plc_task"]
    );

    // Step 4: trace_outputs covers the global too.
    let all_outputs = run::build_response(&run_input(&container_id, vec![], true), &cache);
    assert!(all_outputs.ok, "diagnostics: {:?}", all_outputs.diagnostics);
    let traced_names: Vec<&String> = all_outputs.summary.final_values.keys().collect();
    assert_eq!(traced_names, vec!["count"]);
}
