//! Hand-rolled Debug Adapter Protocol message types for the v1 server.
//!
//! These model only the small v1 surface (see `specs/design/debugger-support.md`
//! §"v1 Scope Decisions"): the handshake, line
//! breakpoints, one synthetic thread, stack/scope/variable inspection, and the
//! four execution-control commands. Everything wider — logpoints, `evaluate`,
//! custom `ironplc/*` requests, variable forcing — is deferred and not modelled
//! here.
//!
//! **Why hand-rolled and not the `dap` crate?** The `dap` crate is alpha,
//! effectively unmaintained, and used by nothing mainstream; the established
//! Rust DAP implementations (Helix, Lapce, probe-rs) all define their own
//! types. Our v1 surface is a handful of small `serde` structs — trivial to own
//! and not worth an alpha dependency on the public build.
//!
//! The types are consumed by the request-dispatch loop in [`super::server`].
#![allow(dead_code)]

use ironplc_container::{SourceColumn, SourceLine};
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};

/// Serde glue between DAP's JSON numbers and the container's source-coordinate
/// newtypes.
///
/// DAP carries line and column numbers as JSON numbers; the debug section
/// stores them as `u16`. Converting here — in the (de)serialization layer, and
/// only here — means every other module holds a [`SourceLine`] or
/// [`SourceColumn`], which the compiler will not let you swap for each other
/// or for a plain count the way two bare `i64` fields would.
mod source_coords {
    use super::{SourceColumn, SourceLine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize_line<S: Serializer>(line: &SourceLine, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u16(line.raw())
    }

    pub fn serialize_column<S: Serializer>(column: &SourceColumn, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u16(column.raw())
    }

    pub fn serialize_opt_line<S: Serializer>(
        line: &Option<SourceLine>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match line {
            Some(line) => s.serialize_u16(line.raw()),
            None => s.serialize_none(),
        }
    }

    /// Narrows an incoming line to [`SourceLine`], yielding `None` when it
    /// falls outside what the debug section can represent.
    ///
    /// Deliberately lenient rather than an error: a hard failure here would
    /// reject the entire `setBreakpoints` request, taking the valid
    /// breakpoints down with the malformed one. `None` instead marks just
    /// that breakpoint unverified. Rejecting also beats an `as` cast, which
    /// would silently fold 65546 into a plausible-looking line 10.
    pub fn deserialize_opt_line<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<SourceLine>, D::Error> {
        let raw = i64::deserialize(d)?;
        Ok(u16::try_from(raw).ok().map(SourceLine::new))
    }

    /// The column counterpart of [`deserialize_opt_line`]; absent and
    /// out-of-range both yield `None`.
    pub fn deserialize_opt_column<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<SourceColumn>, D::Error> {
        let raw = Option::<i64>::deserialize(d)?;
        Ok(raw.and_then(|raw| u16::try_from(raw).ok().map(SourceColumn::new)))
    }
}

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
    /// single-threaded loop has no interactive `pause`). Absent means no
    /// bound; there is no sentinel value that spells "unlimited".
    ///
    /// Held as the raw JSON number the client sent so `launch::check_scan_limit`
    /// can report what is wrong with it. Deserializing straight into an integer
    /// would fail the *whole* argument parse on a negative or fractional value,
    /// which the server can only report as a missing `program`.
    #[serde(default)]
    pub scan_limit: Option<Number>,
    /// Cycle time to assume for a program whose task declares no `INTERVAL`,
    /// in milliseconds. Defaults to 100 ms. A freewheeling task has no rate of
    /// its own, so the debugger has nothing to advance program time by; the
    /// session reports whichever value it used.
    #[serde(default)]
    pub freewheeling_interval_ms: Option<f64>,
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
    /// The requested line, narrowed to the container's representation.
    /// `None` when the client sent a value the debug section cannot hold —
    /// such a breakpoint is answered unverified.
    #[serde(deserialize_with = "source_coords::deserialize_opt_line")]
    pub line: Option<SourceLine>,
    #[serde(default, deserialize_with = "source_coords::deserialize_opt_column")]
    pub column: Option<SourceColumn>,
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
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "source_coords::serialize_opt_line"
    )]
    pub line: Option<SourceLine>,
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
    #[serde(serialize_with = "source_coords::serialize_line")]
    pub line: SourceLine,
    #[serde(serialize_with = "source_coords::serialize_column")]
    pub column: SourceColumn,
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
    fn launch_arguments_when_scan_limit_negative_then_parses_for_validation_to_reject() {
        // A negative `scanLimit` must not fail the whole argument parse: the
        // server would then report it as a missing `program` (see #1515).
        let args: LaunchRequestArguments =
            serde_json::from_value(json!({ "program": "demo.iplc", "scanLimit": -1 })).unwrap();
        assert_eq!(args.program, "demo.iplc");
        assert_eq!(
            args.scan_limit.map(|n| n.to_string()).as_deref(),
            Some("-1")
        );
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
        assert_eq!(args.breakpoints[1].line, Some(SourceLine::new(20)));
        assert_eq!(args.breakpoints[1].column, Some(SourceColumn::new(3)));
    }

    #[test]
    fn set_breakpoints_arguments_when_line_out_of_range_then_none_without_failing_request() {
        // A line the debug section cannot represent narrows to `None` (an
        // unverified breakpoint) rather than erroring, so the valid
        // breakpoints in the same request still resolve. Truncating instead
        // would have folded 65546 into a plausible-looking line 10.
        let args: SetBreakpointsArguments = serde_json::from_value(json!({
            "source": { "path": "/x/demo.st" },
            "breakpoints": [{ "line": -1 }, { "line": 65546 }, { "line": 7 }]
        }))
        .unwrap();
        assert_eq!(args.breakpoints[0].line, None);
        assert_eq!(args.breakpoints[1].line, None);
        assert_eq!(args.breakpoints[2].line, Some(SourceLine::new(7)));
    }

    #[test]
    fn breakpoint_when_serialized_then_line_is_a_plain_number() {
        let value = serde_json::to_value(Breakpoint {
            verified: true,
            line: Some(SourceLine::new(12)),
            source: None,
            message: None,
        })
        .unwrap();
        assert_eq!(value["line"], 12);

        // An unresolvable line is omitted entirely rather than sent as 0.
        let value = serde_json::to_value(Breakpoint {
            verified: false,
            line: None,
            source: None,
            message: Some("nope".to_string()),
        })
        .unwrap();
        assert!(value.get("line").is_none());
    }

    #[test]
    fn stack_frame_when_serialized_then_line_and_column_are_plain_numbers() {
        let value = serde_json::to_value(StackFrame {
            id: 0,
            name: "MAIN".to_string(),
            line: SourceLine::new(42),
            column: SourceColumn::new(7),
            source: None,
        })
        .unwrap();
        assert_eq!(value["line"], 42);
        assert_eq!(value["column"], 7);
    }

    #[test]
    fn stack_trace_body_when_serialized_then_uses_camel_case_keys() {
        let body = StackTraceResponseBody {
            stack_frames: vec![StackFrame {
                id: 1,
                name: "main".to_string(),
                line: SourceLine::new(5),
                column: SourceColumn::new(1),
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
