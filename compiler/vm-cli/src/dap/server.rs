//! The single-threaded DAP event loop.
//!
//! This module drives the `initialize` → `launch` → `setBreakpoints` →
//! `configurationDone` → (run) → `stopped` → inspect → `continue` →
//! `terminated` → `disconnect` lifecycle against the VM debug engine. Every
//! request is gated through the [`state::legal`] table; an illegal or
//! not-yet-supported request is answered with a DAP error whose message is
//! `requestNotApplicable`. The single-threaded design is described in
//! `specs/design/debugger-support.md` §"Single-threaded DAP loop (v1)".
//!
//! The loop is split at the launch boundary so lifetimes stay simple: the
//! *pre-launch* loop ([`serve`]) handles `initialize` / `disconnect` and the
//! `launch` preconditions with nothing borrowed; once preconditions pass it
//! hands the owned [`Container`] to [`launched_session`], which sizes the VM
//! buffers, starts the VM, and runs the *post-launch* run/stop loop borrowing
//! them.
//!
//! Not yet implemented: a trap ends the session as `terminated` rather than
//! surfacing a `stopped{reason:"exception"}`.

use std::io::{self, BufRead, Write};
use std::path::Path;

use ironplc_container::debug_section::DebugSection;
use ironplc_container::{Container, VarIndex};
use ironplc_vm::{
    BreakpointTable, DebuggerHook, PauseReason, RoundOutcome, StepMode, VmBuffers, VmRunning,
};
use serde::Serialize;
use serde_json::Value;

use super::debug_info;
use super::framing;
use super::launch;
use super::state::{self, Command, Phase};
use super::types::{
    Breakpoint, Capabilities, ContinueResponseBody, Event, LaunchRequestArguments, Request,
    Response, Scope, ScopesResponseBody, SetBreakpointsArguments, SetBreakpointsResponseBody,
    Source, StackFrame, StackTraceResponseBody, StoppedEventBody, Thread, ThreadsResponseBody,
    VariablesResponseBody,
};

/// The id of the single synthetic thread the v1 server exposes.
const THREAD_ID: i64 = 1;

/// The `variablesReference` handle for the one variable scope. Non-zero so DAP
/// treats it as expandable; the (flat) list of program variables is returned
/// for it. Structured expansion (nested FB fields) is a later phase, so every
/// returned [`Variable`](super::types::Variable) has `variablesReference: 0`.
const VARIABLES_REF: i64 = 1;

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
                    Ok((container, args)) => {
                        // Preconditions hold: own the container and run the
                        // rest of the session against a live VM.
                        return launched_session(
                            reader, writer, &mut seq, container, args, &request,
                        );
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
/// preconditions. Returns the loaded container and the parsed arguments (run
/// bounds) on success, or the DAP error message to report on failure.
fn load_and_check(request: &Request) -> Result<(Container, LaunchRequestArguments), String> {
    let args: LaunchRequestArguments = request
        .arguments
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .ok_or_else(|| launch::LaunchError::ProgramArgMissing.to_string())?;

    let container = launch::load_container(Path::new(&args.program)).map_err(|e| e.to_string())?;
    launch::check_preconditions(&container).map_err(|e| e.to_string())?;
    Ok((container, args))
}

/// Owns the loaded `container`, starts the VM, answers the `launch` request,
/// and runs the post-launch run/stop loop.
///
/// The `container` and the buffers sized from it live here so the [`VmRunning`]
/// can borrow them for the remainder of the session.
///
/// The loop alternates between two modes (see
/// `specs/design/debugger-support.md` §"Single-threaded DAP loop (v1)"): when
/// `Running`, it drives one `run_round_debug` and reacts to the outcome;
/// otherwise it reads and services one client request. Because the
/// [`BreakpointTable`] is mutated between rounds (a `setBreakpoints` at a
/// pause) while the [`DebuggerHook`] borrows it during a round, the hook is
/// built fresh per round; after a breakpoint pause the next hook is told to
/// suppress that location once so `continue` makes forward progress.
///
/// The loop keeps scanning: on `RoundOutcome::Completed` it runs the next scan
/// (so breakpoints re-fire every cycle and variables evolve across cycles)
/// rather than terminating after one scan. `scanLimit` bounds a runaway program
/// — the session terminates once `scan_count` reaches it — and `stopOnEntry`
/// pauses before the first instruction of the first scan.
///
/// Execution control is `continue` plus single-stepping (`next`/`stepIn`/
/// `stepOut`); a step is armed on the next round's hook, seeded from the paused
/// frames so it measures from the real pause point.
///
/// Current limitations: trap→`exception` is not yet implemented. A free-running
/// program with no breakpoint and no `scanLimit` scans until the client
/// disconnects — the single-threaded loop cannot service an interactive `pause`
/// (a Phase 6 cut).
fn launched_session<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    seq: &mut i64,
    container: Container,
    args: LaunchRequestArguments,
    launch_request: &Request,
) -> io::Result<()> {
    let mut bufs = VmBuffers::from_container(&container);

    // Construct + start the VM. Buffer sizing (operand stack, variable table,
    // data region, and the frame stack from `header.max_call_depth`) is done by
    // `VmBuffers::from_container`, reused from the `ironplcvm` embedding path.
    let mut running = match launch::start_vm(&container, &mut bufs) {
        Ok(running) => running,
        Err(err) => {
            send(
                writer,
                &Response::error(take_seq(seq), launch_request, err.to_string()),
            )?;
            return Ok(());
        }
    };

    // Preconditions, buffer sizing, and start all succeeded.
    send(
        writer,
        &Response::success(take_seq(seq), launch_request, None),
    )?;

    let debug = container.debug_section.as_ref();
    let mut breakpoints = BreakpointTable::new();
    // The session opens in `Configuring`: the client sets breakpoints and then
    // sends `configurationDone` to begin the run.
    let mut phase = Phase::Configuring;
    // A monotonic clock for the debug driver. `run_round_debug` bypasses the
    // scheduler and watchdog, so the exact value only feeds the uptime system
    // variable; a per-round bump keeps it non-decreasing.
    let mut current_time_us: u64 = 0;
    // Set after a breakpoint pause so the next resume skips that one location
    // instead of re-triggering in place.
    let mut suppress_bp = false;
    // Upper bound on scan cycles (runaway prevention); `None` runs until the
    // client disconnects.
    let scan_limit = args.scan_limit;
    // Armed once, before the first scan, when the launch requested `stopOnEntry`.
    let mut pending_stop_on_entry = args.stop_on_entry;
    // Set by a `next`/`stepIn`/`stepOut` request; armed on the next round's hook.
    let mut pending_step: Option<StepMode> = None;

    loop {
        if phase == Phase::Running {
            let outcome = {
                let mut hook = DebuggerHook::new(&breakpoints);
                if suppress_bp {
                    hook.suppress_next_breakpoint();
                    suppress_bp = false;
                }
                if pending_stop_on_entry {
                    hook.stop_on_entry();
                    pending_stop_on_entry = false;
                }
                if let Some(mode) = pending_step.take() {
                    // Seed the hook to the paused position so the step's origin
                    // is where the VM actually stopped, not scan entry. Frames
                    // are outermost-first; the entry frame is depth 0.
                    let frames = running.debug_frames();
                    let depth = frames.len().saturating_sub(1);
                    let offset = frames.last().map_or(0, |f| f.pc);
                    hook.seed_resume_position(depth, offset);
                    match mode {
                        StepMode::Over => hook.step_over(),
                        StepMode::In => hook.step_in(),
                        StepMode::Out => hook.step_out(),
                        StepMode::None => {}
                    }
                }
                running.run_round_debug(current_time_us, &mut hook)
            };
            current_time_us = current_time_us.saturating_add(1000);

            match outcome {
                // A completed scan keeps the debugger scanning: run the next
                // cycle unless a `scanLimit` bound has been reached.
                Ok(RoundOutcome::Completed) | Ok(RoundOutcome::PausedAfterScan) => {
                    if scan_limit.is_some_and(|limit| running.scan_count() >= limit) {
                        send(writer, &Event::new(take_seq(seq), "terminated", None))?;
                        phase = Phase::Terminated;
                    }
                    // Otherwise stay in `Running`: the loop drives the next scan.
                }
                Ok(RoundOutcome::Paused(reason)) => {
                    let dap_reason = match reason {
                        PauseReason::Breakpoint(_) => {
                            // Skip this location on the following resume.
                            suppress_bp = true;
                            "breakpoint"
                        }
                        // Neither is produced yet (no stepping, no
                        // stop-on-entry), but map them so the loop is total.
                        PauseReason::Step => {
                            suppress_bp = true;
                            "step"
                        }
                        PauseReason::Entry => "entry",
                    };
                    send(writer, &stopped_event(take_seq(seq), dap_reason))?;
                    phase = Phase::Paused;
                }
                // Trap-stop is not yet implemented; for now a trap ends the
                // session like a normal termination rather than surfacing an
                // `exception` stop.
                Err(_fault) => {
                    send(writer, &Event::new(take_seq(seq), "terminated", None))?;
                    phase = Phase::Terminated;
                }
            }
            continue;
        }

        // Stopped mode: read and service one request.
        let Some(body) = framing::read_message(reader)? else {
            return Ok(());
        };
        let Ok(request) = serde_json::from_slice::<Request>(&body) else {
            continue;
        };

        let command = Command::from_request(&request.command);
        let legal_here = command.is_some_and(|c| state::legal(phase, c));

        match command {
            Some(Command::Disconnect) => {
                send(writer, &Response::success(take_seq(seq), &request, None))?;
                return Ok(());
            }
            Some(Command::ConfigurationDone) if legal_here => {
                send(writer, &Response::success(take_seq(seq), &request, None))?;
                phase = Phase::Running;
            }
            Some(Command::SetBreakpoints) if legal_here => {
                let body = set_breakpoints(&request, debug, &mut breakpoints);
                send(writer, &Response::success(take_seq(seq), &request, body))?;
            }
            Some(Command::Threads) if legal_here => {
                let body = serde_json::to_value(ThreadsResponseBody {
                    threads: vec![Thread {
                        id: THREAD_ID,
                        name: "plc".to_string(),
                    }],
                })
                .ok();
                send(writer, &Response::success(take_seq(seq), &request, body))?;
            }
            Some(Command::StackTrace) if legal_here => {
                let body = stack_trace_body(&running, debug);
                send(writer, &Response::success(take_seq(seq), &request, body))?;
            }
            Some(Command::Scopes) if legal_here => {
                let body = serde_json::to_value(ScopesResponseBody {
                    scopes: vec![Scope {
                        name: "Variables".to_string(),
                        variables_reference: VARIABLES_REF,
                        expensive: false,
                    }],
                })
                .ok();
                send(writer, &Response::success(take_seq(seq), &request, body))?;
            }
            Some(Command::Variables) if legal_here => {
                let body = variables_body(&running, debug);
                send(writer, &Response::success(take_seq(seq), &request, body))?;
            }
            Some(Command::Continue) if legal_here => {
                let body = serde_json::to_value(ContinueResponseBody {
                    all_threads_continued: true,
                })
                .ok();
                send(writer, &Response::success(take_seq(seq), &request, body))?;
                phase = Phase::Running;
            }
            Some(Command::Next) if legal_here => {
                send(writer, &Response::success(take_seq(seq), &request, None))?;
                pending_step = Some(StepMode::Over);
                phase = Phase::Running;
            }
            Some(Command::StepIn) if legal_here => {
                send(writer, &Response::success(take_seq(seq), &request, None))?;
                pending_step = Some(StepMode::In);
                phase = Phase::Running;
            }
            Some(Command::StepOut) if legal_here => {
                send(writer, &Response::success(take_seq(seq), &request, None))?;
                pending_step = Some(StepMode::Out);
                phase = Phase::Running;
            }
            _ => {
                // Illegal in this phase or an unknown command.
                send(
                    writer,
                    &Response::error(take_seq(seq), &request, REQUEST_NOT_APPLICABLE),
                )?;
            }
        }
    }
}

/// Builds the `stopped` event for `reason`, scoped to the single thread.
fn stopped_event(seq: i64, reason: &'static str) -> Event {
    let body = serde_json::to_value(StoppedEventBody {
        reason,
        thread_id: Some(THREAD_ID),
        description: None,
        all_threads_stopped: true,
    })
    .ok();
    Event::new(seq, "stopped", body)
}

/// Applies a `setBreakpoints` request: replaces the breakpoint set and returns
/// the response body echoing each requested breakpoint's resolved state.
///
/// DAP `setBreakpoints` carries the full set for one source, so the table is
/// cleared and rebuilt. v1 debugs a single source, so a table-wide clear is
/// correct. Line→location resolution is delegated to [`debug_info`], which
/// snaps each breakpoint forward to the nearest executable line; the response
/// echoes the *bound* line so the editor moves the marker to where the
/// breakpoint actually took effect.
fn set_breakpoints(
    request: &Request,
    debug: Option<&DebugSection>,
    breakpoints: &mut BreakpointTable,
) -> Option<Value> {
    let args: SetBreakpointsArguments = request
        .arguments
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok())?;

    breakpoints.clear();
    let source_path = args.source.path.clone().unwrap_or_default();
    let source = Some(args.source.clone());

    let resolved: Vec<Breakpoint> = args
        .breakpoints
        .iter()
        .map(
            |bp| match debug_info::resolve_breakpoint(debug, &source_path, bp.line) {
                Some(resolved) => {
                    for (function_id, offset) in resolved.locations {
                        breakpoints.add(function_id, offset);
                    }
                    Breakpoint {
                        verified: true,
                        line: Some(resolved.line),
                        source: source.clone(),
                        message: None,
                    }
                }
                None => Breakpoint {
                    verified: false,
                    line: Some(bp.line),
                    source: source.clone(),
                    message: Some("no executable location for this line".to_string()),
                },
            },
        )
        .collect();

    serde_json::to_value(SetBreakpointsResponseBody {
        breakpoints: resolved,
    })
    .ok()
}

/// Builds the `stackTrace` response body from the paused instance's live
/// frames. DAP orders frames innermost-first; [`VmRunning::debug_frames`] is
/// outermost-first, so the walk is reversed. Each frame is resolved by
/// [`debug_info`] to its POU name and source location (FUNC_NAME + line map).
fn stack_trace_body(running: &VmRunning, debug: Option<&DebugSection>) -> Option<Value> {
    let frames = running.debug_frames();
    let stack_frames: Vec<StackFrame> = frames
        .iter()
        .enumerate()
        .rev()
        .map(|(index, frame)| {
            let info = debug_info::resolve_frame(debug, frame.function_id, frame.pc);
            StackFrame {
                id: index as i64,
                name: info.name,
                line: info.line,
                column: info.column,
                source: info.source.map(|(name, path)| Source {
                    name: Some(name),
                    path: Some(path),
                }),
            }
        })
        .collect();
    let total = stack_frames.len() as i64;
    serde_json::to_value(StackTraceResponseBody {
        stack_frames,
        total_frames: Some(total),
    })
    .ok()
}

/// Builds the `variables` response body: every program variable slot, rendered
/// by [`debug_info`] with its VAR_NAME name/type and a value formatted per its
/// IEC type tag (STRING values are read from the data region).
fn variables_body(running: &VmRunning, debug: Option<&DebugSection>) -> Option<Value> {
    let count = running.num_variables();
    let values: Vec<u64> = (0..count)
        .map(|i| running.read_variable_raw(VarIndex::new(i)).unwrap_or(0))
        .collect();
    let variables = debug_info::render_variables(debug, &values, running.data_region());
    serde_json::to_value(VariablesResponseBody { variables }).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_container::debug_section::{
        iec_type_tag, var_section, VarNameEntry, SOURCE_FILE_HASH_LEN,
    };
    use ironplc_container::{
        ContainerBuilder, FuncNameEntry, FunctionId, InstanceId, LineMapEntry,
        ProgramInstanceEntry, SourceColumn, SourceFileEntry, SourceFileId, SourceLine, TaskEntry,
        TaskId, TaskType, VarIndex,
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

    /// The source file every fixture container claims to be compiled from.
    fn demo_source_file() -> SourceFileEntry {
        SourceFileEntry {
            path: "demo.st".into(),
            content_hash: [0u8; SOURCE_FILE_HASH_LEN],
        }
    }

    /// A line-map entry for `demo.st` mapping `offset` in `function_id` to
    /// `line` (column 1).
    fn line_entry(function_id: FunctionId, offset: u16, line: u16) -> LineMapEntry {
        LineMapEntry {
            function_id,
            bytecode_offset: offset,
            file_id: SourceFileId::new(0),
            source_line: SourceLine::new(line),
            source_column: SourceColumn::new(1),
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
            .max_call_depth(1)
            .add_var_name(a_var_name())
            .build();
        write_container_to_temp(&container)
    }

    fn no_debug_container_file() -> (tempfile::NamedTempFile, String) {
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
            .max_call_depth(1)
            .build();
        write_container_to_temp(&container)
    }

    fn multi_instance_container_file() -> (tempfile::NamedTempFile, String) {
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
            .max_call_depth(1)
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
            .max_call_depth(1)
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
        // The message is V-coded (V6009) rather than a bare string.
        assert!(launch["message"].as_str().unwrap().starts_with("V6009 - "));
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
        let message = launch["message"].as_str().unwrap();
        assert!(message.starts_with("V6010 - "));
        assert!(message.contains("MultiInstanceUnsupported"));
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
        let message = launch["message"].as_str().unwrap();
        // The start-time trap surfaces its own V-code (divide by zero → V4001).
        assert!(message.starts_with("V4001 - "));
        assert!(message.contains("launch failed to start"));
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
        let message = launch["message"].as_str().unwrap();
        assert!(message.starts_with("V6008 - "));
        assert!(message.contains("'program'"));
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

    // -- run/stop loop ------------------------------------------------------

    /// A single-instance container whose **scan** entry function is
    /// [`FunctionId::SCAN`] (named `MAIN`), running `scan_bytecode`; `init`
    /// is a bare `RET_VOID`. The debug section maps each `(offset, line)`
    /// pair in `line_map` to `demo.st`, so tests set breakpoints by source
    /// line against it.
    fn scan_container_file(
        scan_bytecode: &[u8],
        max_stack: u16,
        line_map: &[(u16, u16)],
    ) -> (tempfile::NamedTempFile, String) {
        let mut builder = ContainerBuilder::new()
            .num_variables(1)
            .add_function(FunctionId::INIT, &[0x8C], 0, 1, 0)
            .add_function(FunctionId::SCAN, scan_bytecode, max_stack, 1, 0)
            .max_call_depth(1)
            .add_var_name(a_var_name())
            .add_func_name(FuncNameEntry {
                function_id: FunctionId::SCAN,
                name: "MAIN".into(),
            })
            .add_source_file(demo_source_file())
            .add_task(a_task(TaskId::new(0)))
            .add_program_instance(ProgramInstanceEntry {
                instance_id: InstanceId::new(0),
                task_id: TaskId::new(0),
                entry_function_id: FunctionId::SCAN,
                var_table_offset: 0,
                var_table_count: 1,
                fb_instance_offset: 0,
                fb_instance_count: 0,
                init_function_id: FunctionId::INIT,
            });
        for &(offset, line) in line_map {
            builder = builder.add_line_map_entry(line_entry(FunctionId::SCAN, offset, line));
        }
        let container = builder.build();
        write_container_to_temp(&container)
    }

    /// A single-instance container whose scan increments `x` (`var[0]`, a
    /// DINT) by one each cycle, then `RET_VOID`. The increment statement is
    /// source line 10 (offset 0) and the `RET_VOID` line 11 (offset 10), so
    /// a breakpoint on line 11 pauses *after* the increment — letting a test
    /// observe the variable evolve across scans.
    fn incrementing_scan_container_file() -> (tempfile::NamedTempFile, String) {
        #[rustfmt::skip]
        let scan: Vec<u8> = vec![
            0x0C, 0x00, 0x00, // LOAD_VAR_I32  var[0]   (offset 0, line 10)
            0x00, 0x00, 0x00, // LOAD_CONST_I32 pool[0] (1)
            0x20,             // ADD_I32
            0x10, 0x00, 0x00, // STORE_VAR_I32 var[0]   (offset 7)
            0x8C,             // RET_VOID               (offset 10, line 11)
        ];
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_i32_constant(1)
            .add_function(FunctionId::INIT, &[0x8C], 0, 1, 0)
            .add_function(FunctionId::SCAN, &scan, 2, 1, 0)
            .max_call_depth(1)
            .add_var_name(a_var_name())
            .add_func_name(FuncNameEntry {
                function_id: FunctionId::SCAN,
                name: "MAIN".into(),
            })
            .add_source_file(demo_source_file())
            .add_line_map_entry(line_entry(FunctionId::SCAN, 0, 10))
            .add_line_map_entry(line_entry(FunctionId::SCAN, 10, 11))
            .add_task(a_task(TaskId::new(0)))
            .add_program_instance(ProgramInstanceEntry {
                instance_id: InstanceId::new(0),
                task_id: TaskId::new(0),
                entry_function_id: FunctionId::SCAN,
                var_table_offset: 0,
                var_table_count: 1,
                fb_instance_offset: 0,
                fb_instance_count: 0,
                init_function_id: FunctionId::INIT,
            })
            .build();
        write_container_to_temp(&container)
    }

    /// All response messages for `command`, in order.
    fn responses<'a>(out: &'a [Value], command: &str) -> Vec<&'a Value> {
        out.iter()
            .filter(|m| m["type"] == "response" && m["command"] == command)
            .collect()
    }

    /// All event messages named `event`, in order.
    fn events<'a>(out: &'a [Value], event: &str) -> Vec<&'a Value> {
        out.iter()
            .filter(|m| m["type"] == "event" && m["event"] == event)
            .collect()
    }

    /// Index of the first message matching `pred`, for ordering assertions.
    fn index_of(out: &[Value], pred: impl Fn(&Value) -> bool) -> usize {
        out.iter().position(pred).unwrap()
    }

    #[test]
    fn serve_when_no_breakpoints_and_scan_limit_then_runs_to_bound_and_terminates() {
        // With no breakpoint, `scanLimit` is what bounds the run: the loop keeps
        // scanning until `scan_count` reaches it (without a bound this program
        // would scan forever, as the single-threaded loop has no `pause`).
        let (_file, path) = scan_container_file(&[0x8C], 0, &[(0, 10)]);
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch",
                   "arguments": {"program": path, "scanLimit": 1}}),
            json!({"seq": 3, "type": "request", "command": "configurationDone"}),
            json!({"seq": 4, "type": "request", "command": "disconnect"}),
        ]);
        assert_eq!(responses(&out, "configurationDone")[0]["success"], true);
        // The scan bound is reached → a single terminated event, then disconnect.
        assert_eq!(events(&out, "terminated").len(), 1);
        assert_eq!(responses(&out, "disconnect")[0]["success"], true);
        // terminated precedes the disconnect response.
        let terminated_at = index_of(&out, |m| m["event"] == "terminated");
        let disconnect_at = index_of(&out, |m| m["command"] == "disconnect");
        assert!(terminated_at < disconnect_at);
    }

    #[test]
    fn serve_when_breakpoint_then_stops_inspects_continues_and_terminates() {
        let (_file, path) = scan_container_file(&[0x8C], 0, &[(0, 10)]);
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch",
                   "arguments": {"program": path, "scanLimit": 1}}),
            json!({"seq": 3, "type": "request", "command": "setBreakpoints",
                   "arguments": {"source": {"path": "demo.st"},
                                 "breakpoints": [{"line": 10}]}}),
            json!({"seq": 4, "type": "request", "command": "configurationDone"}),
            json!({"seq": 5, "type": "request", "command": "threads"}),
            json!({"seq": 6, "type": "request", "command": "stackTrace",
                   "arguments": {"threadId": 1}}),
            json!({"seq": 7, "type": "request", "command": "scopes",
                   "arguments": {"frameId": 0}}),
            json!({"seq": 8, "type": "request", "command": "variables",
                   "arguments": {"variablesReference": 1}}),
            json!({"seq": 9, "type": "request", "command": "continue",
                   "arguments": {"threadId": 1}}),
            json!({"seq": 10, "type": "request", "command": "disconnect"}),
        ]);

        // The breakpoint is verified back to the client at its bound line.
        let sbp = responses(&out, "setBreakpoints");
        assert_eq!(sbp[0]["body"]["breakpoints"][0]["verified"], true);
        assert_eq!(sbp[0]["body"]["breakpoints"][0]["line"], 10);

        // The VM stops at the breakpoint before running to completion.
        let stopped = events(&out, "stopped");
        assert_eq!(stopped.len(), 1);
        assert_eq!(stopped[0]["body"]["reason"], "breakpoint");
        assert_eq!(stopped[0]["body"]["threadId"], 1);

        // One synthetic thread.
        assert_eq!(responses(&out, "threads")[0]["body"]["threads"][0]["id"], 1);

        // The stack has the single scan frame, resolved to its POU name and
        // source location.
        let st = responses(&out, "stackTrace");
        assert_eq!(st[0]["body"]["stackFrames"].as_array().unwrap().len(), 1);
        assert_eq!(st[0]["body"]["stackFrames"][0]["name"], "MAIN");
        assert_eq!(st[0]["body"]["stackFrames"][0]["line"], 10);
        assert_eq!(st[0]["body"]["stackFrames"][0]["source"]["path"], "demo.st");

        // One scope, whose handle enumerates the variables.
        let sc = responses(&out, "scopes");
        assert_eq!(
            sc[0]["body"]["scopes"][0]["variablesReference"],
            VARIABLES_REF
        );

        // The single program variable is rendered with its source name and
        // declared type.
        let vars = responses(&out, "variables");
        assert_eq!(vars[0]["body"]["variables"][0]["name"], "x");
        assert_eq!(vars[0]["body"]["variables"][0]["type"], "DINT");

        // Continue resumes, past the breakpoint, to completion.
        assert_eq!(
            responses(&out, "continue")[0]["body"]["allThreadsContinued"],
            true
        );
        assert_eq!(events(&out, "terminated").len(), 1);

        // Ordering: stop before inspection, terminate after continue.
        let stopped_at = index_of(&out, |m| m["event"] == "stopped");
        let stacktrace_at = index_of(&out, |m| m["command"] == "stackTrace");
        let continue_at = index_of(&out, |m| m["command"] == "continue");
        let terminated_at = index_of(&out, |m| m["event"] == "terminated");
        assert!(stopped_at < stacktrace_at);
        assert!(continue_at < terminated_at);
    }

    #[test]
    fn serve_when_breakpoint_then_refires_every_scan_and_variable_evolves() {
        // The core Phase 4c behavior: the loop keeps scanning, so a breakpoint
        // fires once per scan and `var[0]` grows across cycles (1, 2, 3, …).
        let (_file, path) = incrementing_scan_container_file();
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch",
                   "arguments": {"program": path}}),
            // Breakpoint on the RET_VOID line, after the increment statement.
            json!({"seq": 3, "type": "request", "command": "setBreakpoints",
                   "arguments": {"source": {"path": "demo.st"},
                                 "breakpoints": [{"line": 11}]}}),
            json!({"seq": 4, "type": "request", "command": "configurationDone"}),
            // Scan 1 paused: inspect, then continue to scan 2.
            json!({"seq": 5, "type": "request", "command": "variables",
                   "arguments": {"variablesReference": 1}}),
            json!({"seq": 6, "type": "request", "command": "continue",
                   "arguments": {"threadId": 1}}),
            // Scan 2 paused: inspect, then continue to scan 3.
            json!({"seq": 7, "type": "request", "command": "variables",
                   "arguments": {"variablesReference": 1}}),
            json!({"seq": 8, "type": "request", "command": "continue",
                   "arguments": {"threadId": 1}}),
            // Scan 3 paused: inspect, then tear down.
            json!({"seq": 9, "type": "request", "command": "variables",
                   "arguments": {"variablesReference": 1}}),
            json!({"seq": 10, "type": "request", "command": "disconnect"}),
        ]);

        // A breakpoint stop for each of the three scans (no `terminated`).
        assert_eq!(events(&out, "stopped").len(), 3);
        assert!(events(&out, "terminated").is_empty());
        for stopped in events(&out, "stopped") {
            assert_eq!(stopped["body"]["reason"], "breakpoint");
        }

        // The variable increments once per scan: 1, then 2, then 3.
        let vars = responses(&out, "variables");
        assert_eq!(vars.len(), 3);
        assert_eq!(vars[0]["body"]["variables"][0]["value"], "1");
        assert_eq!(vars[1]["body"]["variables"][0]["value"], "2");
        assert_eq!(vars[2]["body"]["variables"][0]["value"], "3");
    }

    #[test]
    fn serve_when_stop_on_entry_then_pauses_before_first_instruction() {
        // `stopOnEntry` pauses before any logic runs, so the incrementing
        // scan has not yet touched `var[0]` (still 0) at the entry stop.
        let (_file, path) = incrementing_scan_container_file();
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch",
                   "arguments": {"program": path, "stopOnEntry": true}}),
            json!({"seq": 3, "type": "request", "command": "configurationDone"}),
            json!({"seq": 4, "type": "request", "command": "variables",
                   "arguments": {"variablesReference": 1}}),
            json!({"seq": 5, "type": "request", "command": "disconnect"}),
        ]);

        let stopped = events(&out, "stopped");
        assert_eq!(stopped.len(), 1);
        assert_eq!(stopped[0]["body"]["reason"], "entry");
        // No logic has run yet: the increment has not happened.
        let vars = responses(&out, "variables");
        assert_eq!(vars[0]["body"]["variables"][0]["value"], "0");
    }

    #[test]
    fn serve_when_setbreakpoints_line_unresolvable_then_reports_unverified() {
        // A line past the last executable line has nothing to snap to.
        let (_file, path) = scan_container_file(&[0x8C], 0, &[(0, 10)]);
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch",
                   "arguments": {"program": path}}),
            json!({"seq": 3, "type": "request", "command": "setBreakpoints",
                   "arguments": {"source": {"path": "demo.st"},
                                 "breakpoints": [{"line": 9999}]}}),
            json!({"seq": 4, "type": "request", "command": "disconnect"}),
        ]);
        let sbp = responses(&out, "setBreakpoints");
        assert_eq!(sbp[0]["body"]["breakpoints"][0]["verified"], false);
        assert!(sbp[0]["body"]["breakpoints"][0]["message"].is_string());
    }

    /// A scan of four single-byte-operand statements at the same call depth,
    /// then `RET_VOID`, needing no constant pool: `LOAD_VAR var[0]; STORE_VAR
    /// var[0]; LOAD_VAR var[0]; STORE_VAR var[0]; RET_VOID`. Statement starts
    /// land at offsets 0, 3, 6, 9, 12 — so a step advances the paused pc by one
    /// statement each time.
    const MULTI_STATEMENT_SCAN: [u8; 13] = [
        0x0C, 0x00, 0x00, // LOAD_VAR_I32  var[0]  (offset 0)
        0x10, 0x00, 0x00, // STORE_VAR_I32 var[0]  (offset 3)
        0x0C, 0x00, 0x00, // LOAD_VAR_I32  var[0]  (offset 6)
        0x10, 0x00, 0x00, // STORE_VAR_I32 var[0]  (offset 9)
        0x8C, // RET_VOID                          (offset 12)
    ];

    /// Line map for [`MULTI_STATEMENT_SCAN`]: one source line per statement,
    /// lines 10–14 of `demo.st`.
    const MULTI_STATEMENT_LINES: [(u16, u16); 5] =
        [(0, 10), (3, 11), (6, 12), (9, 13), (12, 14)];

    #[test]
    fn serve_when_step_over_then_advances_paused_pc_statement_by_statement() {
        // From a breakpoint on the first statement, each `next` lands on the
        // next statement (no CALL to step over, so step-over lands on the
        // immediately-following instruction). The paused source line is read
        // back via `stackTrace`.
        let (_file, path) = scan_container_file(&MULTI_STATEMENT_SCAN, 1, &MULTI_STATEMENT_LINES);
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch",
                   "arguments": {"program": path}}),
            json!({"seq": 3, "type": "request", "command": "setBreakpoints",
                   "arguments": {"source": {"path": "demo.st"},
                                 "breakpoints": [{"line": 10}]}}),
            json!({"seq": 4, "type": "request", "command": "configurationDone"}),
            json!({"seq": 5, "type": "request", "command": "stackTrace",
                   "arguments": {"threadId": 1}}),
            json!({"seq": 6, "type": "request", "command": "next",
                   "arguments": {"threadId": 1}}),
            json!({"seq": 7, "type": "request", "command": "stackTrace",
                   "arguments": {"threadId": 1}}),
            json!({"seq": 8, "type": "request", "command": "next",
                   "arguments": {"threadId": 1}}),
            json!({"seq": 9, "type": "request", "command": "stackTrace",
                   "arguments": {"threadId": 1}}),
            json!({"seq": 10, "type": "request", "command": "disconnect"}),
        ]);

        // Breakpoint stop, then a step stop for each `next`.
        let stopped = events(&out, "stopped");
        assert_eq!(stopped.len(), 3);
        assert_eq!(stopped[0]["body"]["reason"], "breakpoint");
        assert_eq!(stopped[1]["body"]["reason"], "step");
        assert_eq!(stopped[2]["body"]["reason"], "step");

        // The paused line advances one statement per step: 10 → 11 → 12.
        let st = responses(&out, "stackTrace");
        assert_eq!(st[0]["body"]["stackFrames"][0]["line"], 10);
        assert_eq!(st[1]["body"]["stackFrames"][0]["line"], 11);
        assert_eq!(st[2]["body"]["stackFrames"][0]["line"], 12);

        for n in responses(&out, "next") {
            assert_eq!(n["success"], true);
        }
    }

    #[test]
    fn serve_when_step_in_and_step_out_then_wired_and_resume() {
        // Cover the `stepIn` and `stepOut` handlers. With no callee: `stepIn`
        // lands on the next instruction; `stepOut` from the sole (entry) frame
        // has no shallower frame to reach, so it runs the scan to completion.
        // `scanLimit: 1` bounds the run so completing that scan terminates the
        // session (rather than continuing into the next scan).
        let (_file, path) = scan_container_file(&MULTI_STATEMENT_SCAN, 1, &MULTI_STATEMENT_LINES);
        let out = run_server(&[
            json!({"seq": 1, "type": "request", "command": "initialize"}),
            json!({"seq": 2, "type": "request", "command": "launch",
                   "arguments": {"program": path, "scanLimit": 1}}),
            json!({"seq": 3, "type": "request", "command": "setBreakpoints",
                   "arguments": {"source": {"path": "demo.st"},
                                 "breakpoints": [{"line": 10}]}}),
            json!({"seq": 4, "type": "request", "command": "configurationDone"}),
            json!({"seq": 5, "type": "request", "command": "stepIn",
                   "arguments": {"threadId": 1}}),
            json!({"seq": 6, "type": "request", "command": "stackTrace",
                   "arguments": {"threadId": 1}}),
            json!({"seq": 7, "type": "request", "command": "stepOut",
                   "arguments": {"threadId": 1}}),
            json!({"seq": 8, "type": "request", "command": "disconnect"}),
        ]);

        // Breakpoint stop, then a step stop from `stepIn` landing on the
        // second statement (line 11).
        let stopped = events(&out, "stopped");
        assert_eq!(stopped.len(), 2);
        assert_eq!(stopped[1]["body"]["reason"], "step");
        assert_eq!(
            responses(&out, "stackTrace")[0]["body"]["stackFrames"][0]["line"],
            11
        );

        // stepOut from the top frame runs the scan to completion.
        assert_eq!(events(&out, "terminated").len(), 1);
        assert_eq!(responses(&out, "stepIn")[0]["success"], true);
        assert_eq!(responses(&out, "stepOut")[0]["success"], true);
    }
}
