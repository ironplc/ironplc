//! The single-threaded DAP event loop.
//!
//! This commit implements the `initialize` → `launch` → `disconnect`
//! handshake against the Phase 3 VM engine. Every request is gated through the
//! [`state::legal`] table; an illegal or not-yet-supported request is answered
//! with a DAP error whose message is `requestNotApplicable`.
//!
//! The loop is split at the launch boundary so lifetimes stay simple: the
//! *pre-launch* loop ([`serve`]) handles `initialize` / `disconnect` and the
//! `launch` preconditions with nothing borrowed; once preconditions pass it
//! hands the owned [`Container`] to [`launched_session`], which sizes the VM
//! buffers, starts the VM, and runs the *post-launch* loop borrowing them.
//!
//! The full run/stop loop — `configurationDone` starting execution,
//! `continue`/`next`/`stepIn`/`stepOut`, `stackTrace`/`scopes`/`variables`, and
//! the `stopped`/`terminated` events — is commit 4. It slots into
//! [`launched_session`]'s post-launch loop, where the live [`VmRunning`] is
//! already in scope.

use std::io::{self, BufRead, Write};
use std::path::Path;

use ironplc_container::Container;
use ironplc_vm::VmBuffers;
use serde::Serialize;

use super::framing;
use super::launch;
use super::state::{self, Command, Phase};
use super::types::{Capabilities, Event, LaunchRequestArguments, Request, Response};

/// The DAP `message` returned for any request that is illegal in the current
/// phase or not supported by this server slice.
const REQUEST_NOT_APPLICABLE: &str = "requestNotApplicable";

/// Serializes a DAP message and writes it with Content-Length framing.
fn send<W: Write, T: Serialize>(writer: &mut W, message: &T) -> io::Result<()> {
    let body =
        serde_json::to_vec(message).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    framing::write_message(writer, &body)
}

/// Returns the current outgoing sequence number and advances the counter.
fn take_seq(seq: &mut i64) -> i64 {
    let current = *seq;
    *seq += 1;
    current
}

/// Runs the DAP server over `reader`/`writer` until the client disconnects or
/// the stream ends.
///
/// This is the pre-launch loop: it services `initialize` and `disconnect`,
/// evaluates the `launch` preconditions, and — on a satisfied `launch` — hands
/// off to [`launched_session`] and returns whatever that returns. A launch that
/// fails a precondition is answered with an error and the loop continues, so
/// the client may retry or disconnect.
pub fn serve<R: BufRead, W: Write>(reader: &mut R, writer: &mut W) -> io::Result<()> {
    let mut seq: i64 = 1;
    let mut phase = Phase::Initialized;

    loop {
        let Some(body) = framing::read_message(reader)? else {
            // Clean end-of-stream between messages: the client went away.
            return Ok(());
        };
        let Ok(request) = serde_json::from_slice::<Request>(&body) else {
            // A frame we cannot parse as a request carries no seq to answer;
            // skip it rather than crash the session.
            continue;
        };

        let command = Command::from_request(&request.command);
        let legal_here = command.is_some_and(|c| state::legal(phase, c));

        match command {
            Some(Command::Initialize) if legal_here => {
                let caps = Capabilities {
                    supports_configuration_done_request: true,
                };
                let body = serde_json::to_value(caps).ok();
                send(
                    writer,
                    &Response::success(take_seq(&mut seq), &request, body),
                )?;
                // DAP: the `initialized` event follows the initialize response.
                send(writer, &Event::new(take_seq(&mut seq), "initialized", None))?;
                phase = Phase::Configuring;
            }
            Some(Command::Launch) if legal_here => {
                match load_and_check(&request) {
                    Ok(container) => {
                        // Preconditions hold: own the container and run the
                        // rest of the session against a live VM.
                        return launched_session(reader, writer, &mut seq, container, &request);
                    }
                    Err(message) => {
                        send(
                            writer,
                            &Response::error(take_seq(&mut seq), &request, message),
                        )?;
                    }
                }
            }
            Some(Command::Disconnect) => {
                send(
                    writer,
                    &Response::success(take_seq(&mut seq), &request, None),
                )?;
                return Ok(());
            }
            _ => {
                // Illegal in this phase, an unknown command, or a request this
                // slice does not yet implement.
                send(
                    writer,
                    &Response::error(take_seq(&mut seq), &request, REQUEST_NOT_APPLICABLE),
                )?;
            }
        }
    }
}

/// Parses the `launch` arguments, loads the container, and checks the launch
/// preconditions. Returns the loaded container on success, or the DAP error
/// message to report on failure.
fn load_and_check(request: &Request) -> Result<Container, String> {
    let args: LaunchRequestArguments = request
        .arguments
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .ok_or_else(|| launch::LaunchError::ProgramArgMissing.message())?;

    let container = launch::load_container(Path::new(&args.program)).map_err(|e| e.message())?;
    launch::check_preconditions(&container).map_err(|e| e.message())?;
    Ok(container)
}

/// Owns the loaded `container`, starts the VM, answers the `launch` request,
/// and runs the post-launch service loop.
///
/// The `container` and the buffers sized from it live here so the [`VmRunning`]
/// can borrow them for the remainder of the session. Commit 4 replaces the
/// handshake-only loop below with the run/stop loop; the live VM it needs is
/// already constructed here.
fn launched_session<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    seq: &mut i64,
    container: Container,
    launch_request: &Request,
) -> io::Result<()> {
    let mut bufs = VmBuffers::from_container(&container);

    // Construct + start the VM. Buffer sizing (operand stack, variable table,
    // data region, and the frame stack from `header.max_call_depth`) is done by
    // `VmBuffers::from_container`, reused from the `ironplcvm` embedding path.
    // Commit 4 keeps `_running` live to drive the run/stop loop; the handshake
    // only needs to prove the VM builds and starts.
    let _running = match launch::start_vm(&container, &mut bufs) {
        Ok(running) => running,
        Err(err) => {
            send(
                writer,
                &Response::error(take_seq(seq), launch_request, err.message()),
            )?;
            return Ok(());
        }
    };

    // Preconditions, buffer sizing, and start all succeeded.
    send(
        writer,
        &Response::success(take_seq(seq), launch_request, None),
    )?;

    // Post-launch loop. The DAP phase stays `Configuring` until
    // `configurationDone` starts the run (commit 4). For the handshake slice we
    // service `disconnect` and refuse everything else with
    // `requestNotApplicable`.
    loop {
        let Some(body) = framing::read_message(reader)? else {
            return Ok(());
        };
        let Ok(request) = serde_json::from_slice::<Request>(&body) else {
            continue;
        };

        match Command::from_request(&request.command) {
            Some(Command::Disconnect) => {
                send(writer, &Response::success(take_seq(seq), &request, None))?;
                return Ok(());
            }
            _ => {
                send(
                    writer,
                    &Response::error(take_seq(seq), &request, REQUEST_NOT_APPLICABLE),
                )?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_container::debug_section::{iec_type_tag, var_section, VarNameEntry};
    use ironplc_container::{
        ContainerBuilder, FunctionId, InstanceId, ProgramInstanceEntry, TaskEntry, TaskId,
        TaskType, VarIndex,
    };
    use serde_json::{json, Value};
    use std::io::Cursor;

    fn a_var_name() -> VarNameEntry {
        VarNameEntry {
            var_index: VarIndex::new(0),
            function_id: FunctionId::GLOBAL_SCOPE,
            var_section: var_section::VAR,
            iec_type_tag: iec_type_tag::DINT,
            name: "x".into(),
            type_name: "DINT".into(),
        }
    }

    fn a_task(task_id: TaskId) -> TaskEntry {
        TaskEntry {
            task_id,
            priority: 0,
            task_type: TaskType::Freewheeling,
            flags: 0x01,
            interval_us: 0,
            single_var_index: VarIndex::NO_SINGLE_VAR,
            watchdog_us: 0,
            input_image_offset: 0,
            output_image_offset: 0,
            reserved: [0; 4],
        }
    }

    fn a_program(instance_id: InstanceId, task_id: TaskId) -> ProgramInstanceEntry {
        ProgramInstanceEntry {
            instance_id,
            task_id,
            entry_function_id: FunctionId::new(0),
            var_table_offset: 0,
            var_table_count: 1,
            fb_instance_offset: 0,
            fb_instance_count: 0,
            init_function_id: FunctionId::new(0),
        }
    }

    /// Writes a single-instance container with a debug section to a temp file
    /// and returns the file (kept alive by the caller) plus its path string.
    fn single_instance_debug_container_file() -> (tempfile::NamedTempFile, String) {
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
            .add_var_name(a_var_name())
            .build();
        write_container_to_temp(&container)
    }

    fn no_debug_container_file() -> (tempfile::NamedTempFile, String) {
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
            .build();
        write_container_to_temp(&container)
    }

    fn multi_instance_container_file() -> (tempfile::NamedTempFile, String) {
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
            .add_var_name(a_var_name())
            .add_task(a_task(TaskId::new(0)))
            .add_task(a_task(TaskId::new(1)))
            .add_program_instance(a_program(InstanceId::new(0), TaskId::new(0)))
            .add_program_instance(a_program(InstanceId::new(1), TaskId::new(1)))
            .build();
        write_container_to_temp(&container)
    }

    /// Passes the launch preconditions (debug section, single instance) but the
    /// init function divides by zero, so `start()` traps.
    fn init_traps_container_file() -> (tempfile::NamedTempFile, String) {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00, // LOAD_CONST_I32 pool[0] (10)
            0x00, 0x01, 0x00, // LOAD_CONST_I32 pool[1] (0)
            0x30,             // DIV_I32 -> DivideByZero
            0x8C,             // RET_VOID
        ];
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_i32_constant(10)
            .add_i32_constant(0)
            .add_function(FunctionId::new(0), &bytecode, 2, 1, 0)
            .add_var_name(a_var_name())
            .build();
        write_container_to_temp(&container)
    }

    fn write_container_to_temp(
        container: &ironplc_container::Container,
    ) -> (tempfile::NamedTempFile, String) {
        use std::io::Write as _;
        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(&buf).unwrap();
        file.flush().unwrap();
        let path = file.path().to_string_lossy().into_owned();
        (file, path)
    }

    fn frame(request: &Value) -> Vec<u8> {
        let mut buf = Vec::new();
        let body = serde_json::to_vec(request).unwrap();
        framing::write_message(&mut buf, &body).unwrap();
        buf
    }

    /// Feeds `requests` (each already a DAP request value) through `serve` and
    /// returns the framed responses/events it wrote, decoded as JSON.
    fn run_server(requests: &[Value]) -> Vec<Value> {
        let mut input = Vec::new();
        for req in requests {
            input.extend_from_slice(&frame(req));
        }
        let mut reader = Cursor::new(input);
        let mut writer: Vec<u8> = Vec::new();
        serve(&mut reader, &mut writer).unwrap();

        let mut out_reader = Cursor::new(writer);
        let mut messages = Vec::new();
        while let Some(body) = framing::read_message(&mut out_reader).unwrap() {
            messages.push(serde_json::from_slice(&body).unwrap());
        }
        messages
    }

    #[test]
    fn serve_when_initialize_then_returns_capabilities_and_initialized_event() {
        let out = run_server(&[json!({"seq": 1, "type": "request", "command": "initialize"})]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0]["type"], "response");
        assert_eq!(out[0]["command"], "initialize");
        assert_eq!(out[0]["success"], true);
        assert_eq!(out[0]["body"]["supportsConfigurationDoneRequest"], true);
        // Only the one capability is advertised in this phase.
        assert_eq!(out[0]["body"].as_object().unwrap().len(), 1);
        assert_eq!(out[1]["type"], "event");
        assert_eq!(out[1]["event"], "initialized");
    }

    #[test]
    fn serve_when_initialize_launch_disconnect_then_full_handshake_succeeds() {
        let (_file, path) = single_instance_debug_container_file();
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch",
                   "arguments": {"program": path}}),
            json!({"seq": 3, "type": "request", "command": "disconnect"}),
        ]);
        // initialize response, initialized event, launch response, disconnect response.
        assert_eq!(out.len(), 4);
        let launch = &out[2];
        assert_eq!(launch["command"], "launch");
        assert_eq!(launch["success"], true);
        assert_eq!(launch["request_seq"], 2);
        let disconnect = &out[3];
        assert_eq!(disconnect["command"], "disconnect");
        assert_eq!(disconnect["success"], true);
    }

    #[test]
    fn serve_when_launch_before_initialize_then_request_not_applicable() {
        let (_file, path) = single_instance_debug_container_file();
        let out = run_server(&[json!({"seq": 1, "type": "request", "command": "launch",
                                      "arguments": {"program": path}})]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["success"], false);
        assert_eq!(out[0]["message"], "requestNotApplicable");
    }

    #[test]
    fn serve_when_launch_container_without_debug_then_no_debug_info_error() {
        let (_file, path) = no_debug_container_file();
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch",
                   "arguments": {"program": path}}),
        ]);
        let launch = out.last().unwrap();
        assert_eq!(launch["command"], "launch");
        assert_eq!(launch["success"], false);
        assert!(launch["message"].as_str().unwrap().contains("NoDebugInfo"));
    }

    #[test]
    fn serve_when_launch_multi_instance_then_multi_instance_error() {
        let (_file, path) = multi_instance_container_file();
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch",
                   "arguments": {"program": path}}),
        ]);
        let launch = out.last().unwrap();
        assert_eq!(launch["success"], false);
        assert!(launch["message"]
            .as_str()
            .unwrap()
            .contains("MultiInstanceUnsupported"));
    }

    #[test]
    fn serve_when_launch_vm_fails_to_start_then_launch_error() {
        // Preconditions pass, but the init function traps → launch fails.
        let (_file, path) = init_traps_container_file();
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch",
                   "arguments": {"program": path}}),
        ]);
        let launch = out.last().unwrap();
        assert_eq!(launch["command"], "launch");
        assert_eq!(launch["success"], false);
        assert!(launch["message"]
            .as_str()
            .unwrap()
            .contains("launch failed to start"));
    }

    #[test]
    fn serve_when_launch_precondition_fails_then_session_continues_to_disconnect() {
        // A failed precondition leaves the pre-launch loop live: a subsequent
        // disconnect is still serviced.
        let (_file, path) = no_debug_container_file();
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch",
                   "arguments": {"program": path}}),
            json!({"seq": 3, "type": "request", "command": "disconnect"}),
        ]);
        let disconnect = out.last().unwrap();
        assert_eq!(disconnect["command"], "disconnect");
        assert_eq!(disconnect["success"], true);
    }

    #[test]
    fn serve_when_launch_missing_program_arg_then_error() {
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch", "arguments": {}}),
        ]);
        let launch = out.last().unwrap();
        assert_eq!(launch["success"], false);
        assert!(launch["message"].as_str().unwrap().contains("'program'"));
    }

    #[test]
    fn serve_when_pause_after_initialize_then_request_not_applicable() {
        // `pause` is a modelled-but-cut request: always requestNotApplicable.
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "pause"}),
        ]);
        let pause = out.last().unwrap();
        assert_eq!(pause["command"], "pause");
        assert_eq!(pause["success"], false);
        assert_eq!(pause["message"], "requestNotApplicable");
    }

    #[test]
    fn serve_when_unknown_command_then_request_not_applicable() {
        let out = run_server(&[json!({"seq": 1, "type": "request",
                                      "command": "ironplc/stepScan"})]);
        assert_eq!(out[0]["success"], false);
        assert_eq!(out[0]["message"], "requestNotApplicable");
    }

    #[test]
    fn serve_when_disconnect_after_launch_then_post_launch_loop_tears_down() {
        // Exercise the post-launch loop's requestNotApplicable branch, then
        // disconnect.
        let (_file, path) = single_instance_debug_container_file();
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch",
                   "arguments": {"program": path}}),
            json!({"seq": 3, "type": "request", "command": "threads"}),
            json!({"seq": 4, "type": "request", "command": "disconnect"}),
        ]);
        // Post-launch `threads` is refused for now.
        let threads = &out[3];
        assert_eq!(threads["command"], "threads");
        assert_eq!(threads["success"], false);
        assert_eq!(threads["message"], "requestNotApplicable");
        // Then disconnect is honored.
        let disconnect = out.last().unwrap();
        assert_eq!(disconnect["command"], "disconnect");
        assert_eq!(disconnect["success"], true);
    }

    #[test]
    fn serve_when_stream_ends_without_disconnect_then_returns_ok() {
        // No trailing disconnect: a clean EOF just ends the session.
        let out = run_server(&[json!({"seq": 1, "type": "request", "command": "initialize"})]);
        // Handshake still produced its two messages; the loop returned on EOF.
        assert_eq!(out.len(), 2);
    }
}
