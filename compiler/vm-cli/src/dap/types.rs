//! Hand-rolled Debug Adapter Protocol message types for the v1 server.
//!
//! These model only the small v1 surface (see
//! `specs/plans/2026-06-25-dap-server-scaffold.md`): the handshake, line
//! breakpoints, one synthetic thread, stack/scope/variable inspection, and the
//! four execution-control commands. Everything wider — logpoints, `evaluate`,
//! custom `ironplc/*` requests, variable forcing — is deferred and not modelled
//! here.
//!
//! **Why hand-rolled and not the `dap` crate?** The `dap` crate is alpha,
//! effectively unmaintained, and used by nothing mainstream; the established
//! Rust DAP implementations (Helix, Lapce, probe-rs) all define their own
//! types. Our v1 surface is a handful of small `serde` structs — trivial to own
//! and not worth an alpha dependency on the public build. See the plan's
//! "DAP types: hand-rolled" section for the full rationale.
//!
//! The types are consumed by the request-dispatch loop that lands in a later
//! commit (Phase 4.4); for this commit they are exercised only by the wire
//! round-trip unit tests below.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Base protocol messages
// ---------------------------------------------------------------------------

/// An incoming DAP request. The `type` field ("request") is not needed for
/// dispatch and is ignored; unknown fields are tolerated so protocol additions
/// on the client side do not break deserialization.
#[derive(Debug, Deserialize)]
pub struct Request {
    pub seq: i64,
    pub command: String,
    /// Command-specific arguments, decoded per-command by the handler.
    #[serde(default)]
    pub arguments: Option<Value>,
}

/// An outgoing DAP response to a request.
#[derive(Debug, Serialize)]
pub struct Response {
    pub seq: i64,
    #[serde(rename = "type")]
    pub message_type: &'static str,
    pub request_seq: i64,
    pub success: bool,
    pub command: String,
    /// Present on failure: a short, human-readable error message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Present on success for requests that return data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

impl Response {
    /// A successful response to `request`, optionally carrying a `body`.
    pub fn success(seq: i64, request: &Request, body: Option<Value>) -> Self {
        Self {
            seq,
            message_type: "response",
            request_seq: request.seq,
            success: true,
            command: request.command.clone(),
            message: None,
            body,
        }
    }

    /// A failing response to `request` with a short error `message`. The v1
    /// server uses this for illegal-in-this-state requests
    /// (`requestNotApplicable`) and launch-precondition failures.
    pub fn error(seq: i64, request: &Request, message: impl Into<String>) -> Self {
        Self {
            seq,
            message_type: "response",
            request_seq: request.seq,
            success: false,
            command: request.command.clone(),
            message: Some(message.into()),
            body: None,
        }
    }
}

/// An outgoing DAP event (`stopped`, `terminated`, `initialized`, …).
#[derive(Debug, Serialize)]
pub struct Event {
    pub seq: i64,
    #[serde(rename = "type")]
    pub message_type: &'static str,
    pub event: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

impl Event {
    /// An event named `event`, optionally carrying a `body`.
    pub fn new(seq: i64, event: &'static str, body: Option<Value>) -> Self {
        Self {
            seq,
            message_type: "event",
            event,
            body,
        }
    }
}

// ---------------------------------------------------------------------------
// initialize
// ---------------------------------------------------------------------------

/// Arguments to `initialize`. Only the coordinate-base flags matter to the v1
/// server (they govern the source-line ↔ bytecode mapping added in a later
/// commit); everything else the client advertises is accepted and ignored.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequestArguments {
    #[serde(default)]
    pub adapter_id: Option<String>,
    /// Whether the client's line numbers start at 1 (DAP default true).
    #[serde(default)]
    pub lines_start_at1: Option<bool>,
    /// Whether the client's column numbers start at 1 (DAP default true).
    #[serde(default)]
    pub columns_start_at1: Option<bool>,
}

/// Capabilities advertised in the `initialize` response.
///
/// The v1 server advertises exactly one: it handles `configurationDone`. Every
/// optional capability (`supportsLogPoints`, `supportsConditionalBreakpoints`,
/// `supportsEvaluateForHovers`, `supportsSetVariable`,
/// `supportsStepInTargetsRequest`, …) is off, so it is simply omitted from the
/// serialized body.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub supports_configuration_done_request: bool,
}

// ---------------------------------------------------------------------------
// launch
// ---------------------------------------------------------------------------

/// Arguments to `launch`: the container to debug plus optional run bounds.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequestArguments {
    /// Path to the compiled `.iplc` container to debug.
    pub program: String,
    /// Pause on entry before executing the first instruction.
    #[serde(default)]
    pub stop_on_entry: bool,
    /// Upper bound on scan cycles, to bound a runaway program (the
    /// single-threaded loop has no interactive `pause`).
    #[serde(default)]
    pub scan_limit: Option<u64>,
}

// ---------------------------------------------------------------------------
// setBreakpoints
// ---------------------------------------------------------------------------

/// A source file reference. The v1 server keys breakpoints off `path`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Source {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// A breakpoint the client wants set, at a source line. `logMessage` (which
/// would make this a logpoint) is deliberately not modelled — logpoints are
/// deferred out of the first phase.
#[derive(Debug, Deserialize)]
pub struct SourceBreakpoint {
    pub line: i64,
    #[serde(default)]
    pub column: Option<i64>,
}

/// Arguments to `setBreakpoints`: replace all breakpoints in one `source`.
#[derive(Debug, Deserialize)]
pub struct SetBreakpointsArguments {
    pub source: Source,
    #[serde(default)]
    pub breakpoints: Vec<SourceBreakpoint>,
}

/// A breakpoint as resolved by the server, echoed back to the client.
#[derive(Debug, Serialize)]
pub struct Breakpoint {
    /// Whether the breakpoint could be bound to an executable location.
    pub verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    /// Present when `verified` is false: why the location was rejected.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Body of the `setBreakpoints` response: one entry per requested breakpoint,
/// in request order.
#[derive(Debug, Serialize)]
pub struct SetBreakpointsResponseBody {
    pub breakpoints: Vec<Breakpoint>,
}

// ---------------------------------------------------------------------------
// threads
// ---------------------------------------------------------------------------

/// A DAP thread. The v1 server exposes exactly one synthetic thread for the
/// single program instance.
#[derive(Debug, Serialize)]
pub struct Thread {
    pub id: i64,
    pub name: String,
}

/// Body of the `threads` response.
#[derive(Debug, Serialize)]
pub struct ThreadsResponseBody {
    pub threads: Vec<Thread>,
}

// ---------------------------------------------------------------------------
// stackTrace / scopes / variables
// ---------------------------------------------------------------------------

/// Arguments to `stackTrace`. Paging fields are accepted; the v1 server returns
/// the whole (short) stack and may ignore them.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceArguments {
    pub thread_id: i64,
    #[serde(default)]
    pub start_frame: Option<i64>,
    #[serde(default)]
    pub levels: Option<i64>,
}

/// One frame in the stack trace.
#[derive(Debug, Serialize)]
pub struct StackFrame {
    pub id: i64,
    pub name: String,
    pub line: i64,
    pub column: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
}

/// Body of the `stackTrace` response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackTraceResponseBody {
    pub stack_frames: Vec<StackFrame>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_frames: Option<i64>,
}

/// Arguments to `scopes`: the frame whose scopes are requested.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopesArguments {
    pub frame_id: i64,
}

/// A named variable scope (e.g. `VAR`, `VAR_INPUT`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scope {
    pub name: String,
    /// Handle passed back in a `variables` request to enumerate this scope.
    pub variables_reference: i64,
    pub expensive: bool,
}

/// Body of the `scopes` response.
#[derive(Debug, Serialize)]
pub struct ScopesResponseBody {
    pub scopes: Vec<Scope>,
}

/// Arguments to `variables`: which scope (or structured variable) to expand.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariablesArguments {
    pub variables_reference: i64,
}

/// One variable's rendered name/value.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Variable {
    pub name: String,
    pub value: String,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    /// Non-zero when the variable is itself structured and can be expanded.
    pub variables_reference: i64,
}

/// Body of the `variables` response.
#[derive(Debug, Serialize)]
pub struct VariablesResponseBody {
    pub variables: Vec<Variable>,
}

// ---------------------------------------------------------------------------
// execution control: continue / next / stepIn / stepOut
// ---------------------------------------------------------------------------

/// Arguments shared by the thread-scoped execution-control requests
/// (`continue`, `next`, `stepIn`, `stepOut`). The v1 server has a single
/// thread, so `thread_id` is validated but otherwise unused.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadArguments {
    pub thread_id: i64,
}

/// Body of the `continue` response.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContinueResponseBody {
    pub all_threads_continued: bool,
}

// ---------------------------------------------------------------------------
// disconnect
// ---------------------------------------------------------------------------

/// Arguments to `disconnect`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectArguments {
    #[serde(default)]
    pub restart: Option<bool>,
    #[serde(default)]
    pub terminate_debuggee: Option<bool>,
}

// ---------------------------------------------------------------------------
// events
// ---------------------------------------------------------------------------

/// Body of a `stopped` event. `reason` is one of `"breakpoint"`, `"step"`,
/// `"entry"`, or `"exception"` (a trap).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoppedEventBody {
    pub reason: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether all threads stopped (always true — the v1 server is
    /// single-threaded).
    pub all_threads_stopped: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_when_deserialized_then_reads_seq_command_and_arguments() {
        let wire = r#"{"seq":3,"type":"request","command":"launch",
                       "arguments":{"program":"demo.iplc"}}"#;
        let req: Request = serde_json::from_str(wire).unwrap();
        assert_eq!(req.seq, 3);
        assert_eq!(req.command, "launch");
        assert_eq!(req.arguments.unwrap()["program"], "demo.iplc");
    }

    #[test]
    fn request_when_no_arguments_then_arguments_is_none() {
        let req: Request =
            serde_json::from_str(r#"{"seq":1,"type":"request","command":"threads"}"#).unwrap();
        assert!(req.arguments.is_none());
    }

    #[test]
    fn request_when_unknown_fields_present_then_still_deserializes() {
        // Protocol additions on the client must not break us.
        let req: Request = serde_json::from_str(
            r#"{"seq":1,"type":"request","command":"initialize","futureField":42}"#,
        )
        .unwrap();
        assert_eq!(req.command, "initialize");
    }

    #[test]
    fn response_success_when_serialized_then_marks_success_and_echoes_command() {
        let req = Request {
            seq: 7,
            command: "threads".to_string(),
            arguments: None,
        };
        let resp = Response::success(11, &req, Some(json!({ "threads": [] })));
        let value = serde_json::to_value(&resp).unwrap();
        assert_eq!(value["type"], "response");
        assert_eq!(value["request_seq"], 7);
        assert_eq!(value["success"], true);
        assert_eq!(value["command"], "threads");
        assert_eq!(value["body"], json!({ "threads": [] }));
        // No error message on success.
        assert!(value.get("message").is_none());
    }

    #[test]
    fn response_error_when_serialized_then_carries_message_and_omits_body() {
        let req = Request {
            seq: 4,
            command: "pause".to_string(),
            arguments: None,
        };
        let resp = Response::error(5, &req, "requestNotApplicable");
        let value = serde_json::to_value(&resp).unwrap();
        assert_eq!(value["success"], false);
        assert_eq!(value["message"], "requestNotApplicable");
        assert!(value.get("body").is_none());
    }

    #[test]
    fn event_when_no_body_then_omits_body_field() {
        let event = Event::new(2, "initialized", None);
        let value = serde_json::to_value(&event).unwrap();
        assert_eq!(value["type"], "event");
        assert_eq!(value["event"], "initialized");
        assert!(value.get("body").is_none());
    }

    #[test]
    fn capabilities_when_serialized_then_only_advertises_configuration_done() {
        let caps = Capabilities {
            supports_configuration_done_request: true,
        };
        let value = serde_json::to_value(&caps).unwrap();
        assert_eq!(value["supportsConfigurationDoneRequest"], true);
        // Nothing else is advertised in the first phase.
        assert_eq!(value.as_object().unwrap().len(), 1);
    }

    #[test]
    fn launch_arguments_when_only_program_given_then_defaults_apply() {
        let args: LaunchRequestArguments =
            serde_json::from_value(json!({ "program": "demo.iplc" })).unwrap();
        assert_eq!(args.program, "demo.iplc");
        assert!(!args.stop_on_entry);
        assert!(args.scan_limit.is_none());
    }

    #[test]
    fn set_breakpoints_arguments_when_camel_case_then_maps_to_fields() {
        let args: SetBreakpointsArguments = serde_json::from_value(json!({
            "source": { "path": "/x/demo.st" },
            "breakpoints": [{ "line": 12 }, { "line": 20, "column": 3 }]
        }))
        .unwrap();
        assert_eq!(args.source.path.as_deref(), Some("/x/demo.st"));
        assert_eq!(args.breakpoints.len(), 2);
        assert_eq!(args.breakpoints[1].line, 20);
        assert_eq!(args.breakpoints[1].column, Some(3));
    }

    #[test]
    fn stack_trace_body_when_serialized_then_uses_camel_case_keys() {
        let body = StackTraceResponseBody {
            stack_frames: vec![StackFrame {
                id: 1,
                name: "main".to_string(),
                line: 5,
                column: 1,
                source: Some(Source {
                    name: Some("demo.st".to_string()),
                    path: Some("/x/demo.st".to_string()),
                }),
            }],
            total_frames: Some(1),
        };
        let value = serde_json::to_value(&body).unwrap();
        assert_eq!(value["stackFrames"][0]["name"], "main");
        assert_eq!(value["totalFrames"], 1);
    }

    #[test]
    fn variable_when_no_type_then_omits_type_key() {
        let var = Variable {
            name: "count".to_string(),
            value: "42".to_string(),
            type_name: None,
            variables_reference: 0,
        };
        let value = serde_json::to_value(&var).unwrap();
        assert_eq!(value["value"], "42");
        assert_eq!(value["variablesReference"], 0);
        assert!(value.get("type").is_none());
    }

    #[test]
    fn stopped_event_body_when_serialized_then_uses_camel_case_keys() {
        let body = StoppedEventBody {
            reason: "breakpoint",
            thread_id: Some(1),
            description: None,
            all_threads_stopped: true,
        };
        let value = serde_json::to_value(&body).unwrap();
        assert_eq!(value["reason"], "breakpoint");
        assert_eq!(value["threadId"], 1);
        assert_eq!(value["allThreadsStopped"], true);
        assert!(value.get("description").is_none());
    }

    #[test]
    fn thread_arguments_when_camel_case_then_reads_thread_id() {
        let args: ThreadArguments = serde_json::from_value(json!({ "threadId": 1 })).unwrap();
        assert_eq!(args.thread_id, 1);
    }
}
