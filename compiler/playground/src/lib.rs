//! Browser-based playground for IronPLC.
//!
//! Exposes functions to JavaScript:
//! - [`compile`] - Parse IEC 61131-3 source and produce bytecode
//! - [`run`] - Execute pre-compiled bytecode (.iplc)
//! - [`run_source`] - Compile and execute in one step
//! - [`load_program`] - Compile source and create a stepping session
//! - [`step`] - Execute N scans within a stepping session
//! - [`reset_session`] - Clear the stepping session

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Cursor;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ironplc_analyzer::stages::analyze;
use ironplc_codegen::compile_reachable as codegen_compile;
use ironplc_container::debug_section::iec_type_tag;
use ironplc_container::Container;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::ParseOptions;
use ironplc_sources::{parse_source, FileType};
use ironplc_vm::{Slot, Vm, VmBuffers};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Persistent state for step-through execution.
///
/// Stores compiled bytecode and variable buffer so variables persist
/// across calls to [`step`]. The VM is re-created each step because
/// `VmRunning` borrows all buffers and cannot be stored across WASM calls.
struct VmSession {
    container_bytes: Vec<u8>,
    var_buf: Vec<Slot>,
    data_region: Vec<u8>,
    scan_count: u64,
    cycle_time_us: u64,
    faulted: bool,
}

thread_local! {
    static SESSION: RefCell<Option<VmSession>> = const { RefCell::new(None) };
}

/// Build [`ParseOptions`] from an edition string.
///
/// `"2013"` enables IEC 61131-3 Edition 3 keywords (LTIME, REF_TO, etc.).
/// Any other value (including empty) uses the default Edition 2 (2003).
fn parse_options_from_edition(edition: &str) -> ParseOptions {
    if edition == "2013" {
        ParseOptions {
            allow_iec_61131_3_2013: true,
            ..Default::default()
        }
    } else {
        ParseOptions::default()
    }
}

/// Install a panic hook that logs to `console.error` with a full stack trace.
///
/// Called once from JavaScript before using any other exports.
#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

/// Return the crate version so the playground can include it in problem-code URLs.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Result of a compilation attempt.
#[derive(Serialize, Deserialize)]
struct CompileResult {
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bytecode: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<DiagnosticInfo>,
}

/// A single diagnostic (error or warning) from compilation.
#[derive(Debug, Serialize, Deserialize)]
struct DiagnosticInfo {
    code: String,
    message: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    label: String,
    start: usize,
    end: usize,
}

/// Result of executing bytecode.
#[derive(Serialize, Deserialize)]
struct RunResult {
    ok: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    variables: Vec<VariableInfo>,
    scans_completed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// A variable value read from the VM after execution.
#[derive(Serialize, Deserialize)]
struct VariableInfo {
    index: u16,
    value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    type_name: String,
}

/// Debug metadata for a variable, extracted from the container's debug section.
struct VarDebugInfo {
    name: String,
    type_name: String,
    iec_type_tag: u8,
}

/// Builds a lookup map from var_index to debug info from the container's debug section.
fn build_var_debug_map(container: &Container) -> HashMap<u16, VarDebugInfo> {
    let mut map = HashMap::new();
    if let Some(debug) = &container.debug_section {
        for entry in &debug.var_names {
            map.insert(
                entry.var_index,
                VarDebugInfo {
                    name: entry.name.clone(),
                    type_name: entry.type_name.clone(),
                    iec_type_tag: entry.iec_type_tag,
                },
            );
        }
    }
    map
}

/// Formats a raw 64-bit slot value according to the IEC type tag.
fn format_variable_value(raw: u64, tag: u8) -> String {
    match tag {
        iec_type_tag::BOOL => {
            if (raw as i32) != 0 {
                "TRUE".into()
            } else {
                "FALSE".into()
            }
        }
        iec_type_tag::SINT => format!("{}", raw as i32 as i8),
        iec_type_tag::INT => format!("{}", raw as i32 as i16),
        iec_type_tag::DINT => format!("{}", raw as i32),
        iec_type_tag::LINT => format!("{}", raw as i64),
        iec_type_tag::USINT => format!("{}", raw as u8),
        iec_type_tag::UINT => format!("{}", raw as u16),
        iec_type_tag::UDINT => format!("{}", raw as u32),
        iec_type_tag::ULINT => format!("{}", raw),
        iec_type_tag::REAL => format!("{}", f32::from_bits(raw as u32)),
        iec_type_tag::LREAL => format!("{}", f64::from_bits(raw)),
        iec_type_tag::BYTE => format!("16#{:02X}", raw as u8),
        iec_type_tag::WORD => format!("16#{:04X}", raw as u16),
        iec_type_tag::DWORD => format!("16#{:08X}", raw as u32),
        iec_type_tag::LWORD => format!("16#{:016X}", raw),
        iec_type_tag::TIME => format_time_value_ms(raw as i32),
        iec_type_tag::LTIME => format_time_value_ms(raw as i64),
        iec_type_tag::DATE => format_date_value(raw as u32),
        iec_type_tag::TIME_OF_DAY => format_tod_value(raw as u32),
        iec_type_tag::DATE_AND_TIME => format_dt_value(raw),
        _ => format!("{}", raw as i32), // fallback
    }
}

/// Formats a TIME/LTIME value (stored as milliseconds) as an IEC 61131-3 duration.
///
/// Uses `T#<value>ms` for values under 1 second, `T#<value>s` for values at or
/// above 1 second (with decimal for sub-millisecond precision).
fn format_time_value_ms<T: Into<i64>>(ms: T) -> String {
    let ms: i64 = ms.into();
    if ms == 0 {
        return "T#0ms".to_string();
    }
    let abs_ms = ms.unsigned_abs();
    let sign = if ms < 0 { "-" } else { "" };
    if abs_ms < 1000 {
        format!("{sign}T#{abs_ms}ms")
    } else {
        let secs = abs_ms / 1000;
        let frac_ms = abs_ms % 1000;
        if frac_ms == 0 {
            format!("{sign}T#{secs}s")
        } else {
            let total_s = secs as f64 + frac_ms as f64 / 1000.0;
            let formatted = format!("{total_s}");
            format!("{sign}T#{formatted}s")
        }
    }
}

/// Formats a DATE value (stored as seconds since 1970-01-01) as D#YYYY-MM-DD.
///
/// Uses the inverse Julian day algorithm to convert the internal second count
/// back into year/month/day components without requiring the `time` crate.
fn format_date_value(secs: u32) -> String {
    // Convert seconds since 1970-01-01 to Julian day number.
    const UNIX_EPOCH_JULIAN_DAY: i64 = 2_440_588; // 1970-01-01
    let days = secs as i64 / 86_400;
    let j = UNIX_EPOCH_JULIAN_DAY + days;

    // Richards' algorithm (Meeus, Astronomical Algorithms) for Julian day → calendar date.
    let f = j + 1401 + ((4 * j + 274277) / 146097) * 3 / 4 - 38;
    let e = 4 * f + 3;
    let g = (e % 1461) / 4;
    let h = 5 * g + 2;
    let d = (h % 153) / 5 + 1;
    let m = (h / 153 + 2) % 12 + 1;
    let y = e / 1461 - 4716 + (12 + 2 - m) / 12;

    format!("D#{y}-{m:02}-{d:02}")
}

/// Formats a TIME_OF_DAY value (stored as ms since midnight) as TOD#HH:MM:SS.mmm.
fn format_tod_value(ms: u32) -> String {
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let frac = ms % 1_000;
    if frac == 0 {
        format!("TOD#{h:02}:{m:02}:{s:02}")
    } else {
        format!("TOD#{h:02}:{m:02}:{s:02}.{frac:03}")
    }
}

/// Formats a DATE_AND_TIME value (stored as u32 seconds since 1970-01-01) as DT#YYYY-MM-DD-HH:MM:SS.
///
/// The raw u64 parameter is the zero-extended u32 value from the VM slot.
fn format_dt_value(raw: u64) -> String {
    let secs = raw as u32;
    let date_secs = secs - (secs % 86_400);
    let tod_secs = secs % 86_400;
    let date_part = format_date_value(date_secs);
    // Extract date portion (after "D#")
    let date_str = &date_part[2..];
    let h = tod_secs / 3_600;
    let m = (tod_secs % 3_600) / 60;
    let s = tod_secs % 60;
    format!("DT#{date_str}-{h:02}:{m:02}:{s:02}")
}

/// Result of compile-and-run (combines both).
#[derive(Serialize, Deserialize)]
struct RunSourceResult {
    ok: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<DiagnosticInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    variables: Vec<VariableInfo>,
    scans_completed: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Result of a step-through operation (load or step).
#[derive(Serialize, Deserialize)]
struct StepResult {
    ok: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<DiagnosticInfo>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    variables: Vec<VariableInfo>,
    total_scans: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Parse IEC 61131-3 source code and produce bytecode.
///
/// Returns a JSON string with shape:
/// ```json
/// { "ok": true, "bytecode": "<base64>" }
/// ```
/// or on error:
/// ```json
/// { "ok": false, "diagnostics": [{"code": "...", "message": "...", "start": N, "end": N}] }
/// ```
#[wasm_bindgen]
pub fn compile(source: &str, edition: &str) -> String {
    let result = compile_inner(source, edition);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"ok":false,"diagnostics":[{{"code":"INTERNAL","message":"Serialization error: {e}","label":"","start":0,"end":0}}]}}"#)
    })
}

fn compile_inner(source: &str, edition: &str) -> CompileResult {
    let file_type = FileType::from_content(source);
    let options = parse_options_from_edition(edition);
    let library = match parse_source(file_type, source, &FileId::default(), &options) {
        Ok(lib) => lib,
        Err(diag) => {
            return CompileResult {
                ok: false,
                bytecode: None,
                diagnostics: vec![DiagnosticInfo {
                    code: diag.code.clone(),
                    message: diag.description(),
                    label: diag.primary.message.clone(),
                    start: diag.primary.location.start,
                    end: diag.primary.location.end,
                }],
            };
        }
    };

    // Run the full analysis pipeline: type resolution + semantic checks.
    // Type resolution populates expr.resolved_type so codegen can select
    // correct opcodes. Semantic checks catch errors like undeclared variables,
    // wrong argument counts, type mismatches, etc.
    let (library, context) = match analyze(&[&library]) {
        Ok((resolved_lib, ctx)) => (resolved_lib, ctx),
        Err(diagnostics) => {
            return CompileResult {
                ok: false,
                bytecode: None,
                diagnostics: diagnostics
                    .into_iter()
                    .map(|d| DiagnosticInfo {
                        code: d.code.clone(),
                        message: d.description(),
                        label: d.primary.message.clone(),
                        start: d.primary.location.start,
                        end: d.primary.location.end,
                    })
                    .collect(),
            };
        }
    };

    // Report any semantic diagnostics (non-fatal errors found during analysis).
    if context.has_diagnostics() {
        return CompileResult {
            ok: false,
            bytecode: None,
            diagnostics: context
                .diagnostics()
                .iter()
                .map(|d| DiagnosticInfo {
                    code: d.code.clone(),
                    message: d.description(),
                    label: d.primary.message.clone(),
                    start: d.primary.location.start,
                    end: d.primary.location.end,
                })
                .collect(),
        };
    }

    let container = match codegen_compile(
        &library,
        context.functions(),
        context.types(),
        Some(context.reachable()),
    ) {
        Ok(c) => c,
        Err(diag) => {
            return CompileResult {
                ok: false,
                bytecode: None,
                diagnostics: vec![DiagnosticInfo {
                    code: diag.code.clone(),
                    message: diag.description(),
                    label: diag.primary.message.clone(),
                    start: diag.primary.location.start,
                    end: diag.primary.location.end,
                }],
            };
        }
    };

    let mut buf = Vec::new();
    if let Err(e) = container.write_to(&mut buf) {
        return CompileResult {
            ok: false,
            bytecode: None,
            diagnostics: vec![DiagnosticInfo {
                code: "INTERNAL".to_string(),
                message: format!("Failed to serialize bytecode: {e}"),
                label: String::new(),
                start: 0,
                end: 0,
            }],
        };
    }

    CompileResult {
        ok: true,
        bytecode: Some(BASE64.encode(&buf)),
        diagnostics: vec![],
    }
}

/// Execute pre-compiled bytecode (.iplc format).
///
/// `bytecode_base64` is the base64-encoded .iplc file content.
/// `scans` is the number of scan cycles to run.
///
/// Returns a JSON string with variable values after execution.
#[wasm_bindgen]
pub fn run(bytecode_base64: &str, scans: u32) -> String {
    let result = run_inner(bytecode_base64, scans);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"ok":false,"variables":[],"scans_completed":0,"error":"Serialization error: {e}"}}"#)
    })
}

fn run_inner(bytecode_base64: &str, scans: u32) -> RunResult {
    let bytes = match BASE64.decode(bytecode_base64) {
        Ok(b) => b,
        Err(e) => {
            return RunResult {
                ok: false,
                variables: vec![],
                scans_completed: 0,
                error: Some(format!("Invalid base64: {e}")),
            };
        }
    };

    run_bytes(&bytes, scans)
}

fn run_bytes(bytes: &[u8], scans: u32) -> RunResult {
    let container = match Container::read_from(&mut Cursor::new(bytes)) {
        Ok(c) => c,
        Err(e) => {
            return RunResult {
                ok: false,
                variables: vec![],
                scans_completed: 0,
                error: Some(format!("Invalid bytecode container: {e}")),
            };
        }
    };

    let mut bufs = VmBuffers::from_container(&container);

    let mut running = match Vm::new()
        .load(
            &container,
            &mut bufs.stack,
            &mut bufs.vars,
            &mut bufs.data_region,
            &mut bufs.temp_buf,
            &mut bufs.tasks,
            &mut bufs.programs,
            &mut bufs.ready,
        )
        .start()
    {
        Ok(vm) => vm,
        Err(ctx) => {
            return RunResult {
                ok: false,
                variables: vec![],
                scans_completed: 0,
                error: Some(format!(
                    "VM trap during init: {} (task {}, instance {})",
                    ctx.trap, ctx.task_id, ctx.instance_id
                )),
            };
        }
    };

    let debug_map = build_var_debug_map(&container);

    for round in 0..scans {
        let current_us = (round as u64) * 1000;
        if let Err(ctx) = running.run_round(current_us) {
            let faulted = running.fault(ctx);
            let variables = read_all_variables_faulted(&faulted, &debug_map);
            return RunResult {
                ok: false,
                variables,
                scans_completed: round as u64,
                error: Some(format!(
                    "VM trap: {} (task {}, instance {})",
                    faulted.trap(),
                    faulted.task_id(),
                    faulted.instance_id()
                )),
            };
        }
    }

    let num_vars = running.num_variables();
    let variables = read_all_variables_running(&running, num_vars, &debug_map);
    let scans_completed = running.scan_count();
    running.stop();

    RunResult {
        ok: true,
        variables,
        scans_completed,
        error: None,
    }
}

/// Compile IEC 61131-3 source and execute in one step.
///
/// Returns a JSON string with both compilation diagnostics and execution results.
#[wasm_bindgen]
pub fn run_source(source: &str, scans: u32, edition: &str) -> String {
    let result = run_source_inner(source, scans, edition);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"ok":false,"diagnostics":[],"variables":[],"scans_completed":0,"error":"Serialization error: {e}"}}"#)
    })
}

fn run_source_inner(source: &str, scans: u32, edition: &str) -> RunSourceResult {
    let compile_result = compile_inner(source, edition);
    if !compile_result.ok {
        return RunSourceResult {
            ok: false,
            diagnostics: compile_result.diagnostics,
            variables: vec![],
            scans_completed: 0,
            error: None,
        };
    }

    let bytecode_b64 = compile_result.bytecode.unwrap();
    let bytes = BASE64.decode(&bytecode_b64).unwrap();
    let run_result = run_bytes(&bytes, scans);

    RunSourceResult {
        ok: run_result.ok,
        diagnostics: vec![],
        variables: run_result.variables,
        scans_completed: run_result.scans_completed,
        error: run_result.error,
    }
}

fn read_all_variables_running(
    vm: &ironplc_vm::VmRunning,
    num_vars: u16,
    debug_map: &HashMap<u16, VarDebugInfo>,
) -> Vec<VariableInfo> {
    (0..num_vars)
        .filter_map(|i| {
            vm.read_variable_raw(i).ok().map(|raw| {
                let (name, type_name, value) = if let Some(info) = debug_map.get(&i) {
                    (
                        info.name.clone(),
                        info.type_name.clone(),
                        format_variable_value(raw, info.iec_type_tag),
                    )
                } else {
                    (String::new(), String::new(), format!("{}", raw as i32))
                };
                VariableInfo {
                    index: i,
                    value,
                    name,
                    type_name,
                }
            })
        })
        .collect()
}

fn read_all_variables_faulted(
    vm: &ironplc_vm::VmFaulted,
    debug_map: &HashMap<u16, VarDebugInfo>,
) -> Vec<VariableInfo> {
    let num_vars = vm.num_variables();
    (0..num_vars)
        .filter_map(|i| {
            vm.read_variable_raw(i).ok().map(|raw| {
                let (name, type_name, value) = if let Some(info) = debug_map.get(&i) {
                    (
                        info.name.clone(),
                        info.type_name.clone(),
                        format_variable_value(raw, info.iec_type_tag),
                    )
                } else {
                    (String::new(), String::new(), format!("{}", raw as i32))
                };
                VariableInfo {
                    index: i,
                    value,
                    name,
                    type_name,
                }
            })
        })
        .collect()
}

/// Compile IEC 61131-3 source and create a stepping session.
///
/// The session stores compiled bytecode and a variable buffer that persists
/// across calls to [`step`]. Returns a JSON `StepResult` with `total_scans: 0`.
#[wasm_bindgen]
pub fn load_program(source: &str, cycle_time_us: u32, edition: &str) -> String {
    let result = load_program_inner(source, cycle_time_us, edition);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"ok":false,"diagnostics":[],"variables":[],"total_scans":0,"error":"Serialization error: {e}"}}"#)
    })
}

fn load_program_inner(source: &str, cycle_time_us: u32, edition: &str) -> StepResult {
    let compile_result = compile_inner(source, edition);
    if !compile_result.ok {
        return StepResult {
            ok: false,
            diagnostics: compile_result.diagnostics,
            variables: vec![],
            total_scans: 0,
            error: None,
        };
    }

    let bytecode_b64 = compile_result.bytecode.unwrap();
    let container_bytes = BASE64.decode(&bytecode_b64).unwrap();

    let container = match Container::read_from(&mut Cursor::new(&container_bytes)) {
        Ok(c) => c,
        Err(e) => {
            return StepResult {
                ok: false,
                diagnostics: vec![],
                variables: vec![],
                total_scans: 0,
                error: Some(format!("Failed to load bytecode: {e}")),
            };
        }
    };

    // Run the init function once to apply initial values to the variable buffer.
    // Subsequent calls to step() will use resume() to skip re-initialization.
    let mut bufs = VmBuffers::from_container(&container);

    match Vm::new()
        .load(
            &container,
            &mut bufs.stack,
            &mut bufs.vars,
            &mut bufs.data_region,
            &mut bufs.temp_buf,
            &mut bufs.tasks,
            &mut bufs.programs,
            &mut bufs.ready,
        )
        .start()
    {
        Ok(running) => {
            running.stop();
        }
        Err(ctx) => {
            return StepResult {
                ok: false,
                diagnostics: vec![],
                variables: vec![],
                total_scans: 0,
                error: Some(format!("VM init trap: {}", ctx.trap)),
            };
        }
    }

    SESSION.with(|cell| {
        *cell.borrow_mut() = Some(VmSession {
            container_bytes,
            var_buf: bufs.vars,
            data_region: bufs.data_region,
            scan_count: 0,
            cycle_time_us: cycle_time_us as u64,
            faulted: false,
        });
    });

    StepResult {
        ok: true,
        diagnostics: vec![],
        variables: vec![],
        total_scans: 0,
        error: None,
    }
}

/// Execute N scan cycles within the current stepping session.
///
/// Variable values persist between calls. Returns a JSON `StepResult`
/// with accumulated `total_scans`.
#[wasm_bindgen]
pub fn step(scans: u32) -> String {
    let result = step_inner(scans);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"ok":false,"diagnostics":[],"variables":[],"total_scans":0,"error":"Serialization error: {e}"}}"#)
    })
}

fn step_inner(scans: u32) -> StepResult {
    SESSION.with(|cell| {
        let mut borrow = cell.borrow_mut();
        let session = match borrow.as_mut() {
            Some(s) => s,
            None => {
                return StepResult {
                    ok: false,
                    diagnostics: vec![],
                    variables: vec![],
                    total_scans: 0,
                    error: Some("No program loaded. Call load_program first.".to_string()),
                };
            }
        };

        if session.faulted {
            return StepResult {
                ok: false,
                diagnostics: vec![],
                variables: vec![],
                total_scans: 0,
                error: Some("Session is faulted. Call reset_session to start over.".to_string()),
            };
        }

        let container = match Container::read_from(&mut Cursor::new(&session.container_bytes)) {
            Ok(c) => c,
            Err(e) => {
                return StepResult {
                    ok: false,
                    diagnostics: vec![],
                    variables: vec![],
                    total_scans: 0,
                    error: Some(format!("Failed to load bytecode: {e}")),
                };
            }
        };

        let (variables, total_scans, error) = run_vm_step(
            &container,
            &mut session.var_buf,
            &mut session.data_region,
            session.scan_count,
            scans,
            session.cycle_time_us,
        );

        session.scan_count = total_scans;
        if error.is_some() {
            session.faulted = true;
        }

        StepResult {
            ok: error.is_none(),
            diagnostics: vec![],
            variables,
            total_scans,
            error,
        }
    })
}

/// Run an ephemeral VM for N scans using the given container and variable buffer.
///
/// Uses [`VmReady::resume`] to skip re-initialization so that variable values
/// (including initial values) persist across calls. The VM's internal scan
/// counter is the source of truth for total cycles executed.
///
/// Returns `(variables, total_scan_count, error)`.
fn run_vm_step(
    container: &Container,
    var_buf: &mut [Slot],
    data_region: &mut [u8],
    base_scan_count: u64,
    scans: u32,
    cycle_time_us: u64,
) -> (Vec<VariableInfo>, u64, Option<String>) {
    let mut bufs = VmBuffers::from_container(container);

    let mut running = Vm::new()
        .load(
            container,
            &mut bufs.stack,
            var_buf,
            data_region,
            &mut bufs.temp_buf,
            &mut bufs.tasks,
            &mut bufs.programs,
            &mut bufs.ready,
        )
        .resume(base_scan_count);

    for _ in 0..scans {
        let current_us = running.scan_count() * cycle_time_us;
        if let Err(ctx) = running.run_round(current_us) {
            let total_scans = running.scan_count();
            let faulted = running.fault(ctx);
            let debug_map = build_var_debug_map(container);
            let variables = read_all_variables_faulted(&faulted, &debug_map);
            let error = format!(
                "VM trap: {} (task {}, instance {})",
                faulted.trap(),
                faulted.task_id(),
                faulted.instance_id()
            );
            return (variables, total_scans, Some(error));
        }
    }

    let debug_map = build_var_debug_map(container);
    let num_vars = running.num_variables();
    let variables = read_all_variables_running(&running, num_vars, &debug_map);
    let total_scans = running.scan_count();
    running.stop();
    (variables, total_scans, None)
}

/// Clear the stepping session.
///
/// Returns `{"ok":true}`.
#[wasm_bindgen]
pub fn reset_session() -> String {
    SESSION.with(|cell| {
        *cell.borrow_mut() = None;
    });
    r#"{"ok":true}"#.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_when_valid_source_then_returns_bytecode() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42;
END_PROGRAM
";
        let result: CompileResult = serde_json::from_str(&compile(source, "")).unwrap();
        assert!(result.ok);
        assert!(result.bytecode.is_some());
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn compile_when_syntax_error_then_returns_diagnostics() {
        let source = "PROGRAM main INVALID END_PROGRAM";
        let result: CompileResult = serde_json::from_str(&compile(source, "")).unwrap();
        assert!(!result.ok);
        assert!(result.bytecode.is_none());
        assert!(!result.diagnostics.is_empty());
        assert!(!result.diagnostics[0].label.is_empty());
    }

    #[test]
    fn run_when_valid_bytecode_then_returns_variables() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42;
END_PROGRAM
";
        let compile_result: CompileResult = serde_json::from_str(&compile(source, "")).unwrap();
        let bytecode = compile_result.bytecode.unwrap();

        let result: RunResult = serde_json::from_str(&run(&bytecode, 1)).unwrap();
        assert!(result.ok);
        assert_eq!(result.scans_completed, 1);
        assert!(!result.variables.is_empty());
        assert_eq!(result.variables[0].value, "42");
    }

    #[test]
    fn run_when_invalid_base64_then_returns_error() {
        let result: RunResult = serde_json::from_str(&run("not-valid-base64!!!", 1)).unwrap();
        assert!(!result.ok);
        assert!(result.error.is_some());
    }

    #[test]
    fn run_when_invalid_container_then_returns_error() {
        let bytes = BASE64.encode(b"not a container");
        let result: RunResult = serde_json::from_str(&run(&bytes, 1)).unwrap();
        assert!(!result.ok);
        assert!(result.error.is_some());
    }

    #[test]
    fn run_source_when_steel_thread_then_returns_values() {
        let source = "
PROGRAM main
  VAR
    x : INT;
    y : INT;
  END_VAR
  x := 10;
  y := x + 32;
END_PROGRAM
";
        let result: RunSourceResult = serde_json::from_str(&run_source(source, 1, "")).unwrap();
        assert!(result.ok);
        assert!(result.diagnostics.is_empty());
        assert!(result.error.is_none());
        assert_eq!(result.scans_completed, 1);
        assert!(result.variables.len() >= 2);
        assert_eq!(result.variables[0].value, "10");
        assert_eq!(result.variables[1].value, "42");
    }

    #[test]
    fn run_source_when_syntax_error_then_returns_diagnostics() {
        let source = "PROGRAM main INVALID END_PROGRAM";
        let result: RunSourceResult = serde_json::from_str(&run_source(source, 1, "")).unwrap();
        assert!(!result.ok);
        assert!(!result.diagnostics.is_empty());
        assert_eq!(result.scans_completed, 0);
    }

    #[test]
    fn run_source_when_multiple_scans_then_correct_count() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 99;
END_PROGRAM
";
        let result: RunSourceResult = serde_json::from_str(&run_source(source, 5, "")).unwrap();
        assert!(result.ok);
        assert_eq!(result.scans_completed, 5);
        assert_eq!(result.variables[0].value, "99");
    }

    #[test]
    fn compile_when_valid_source_then_bytecode_is_valid_base64() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 1;
END_PROGRAM
";
        let result: CompileResult = serde_json::from_str(&compile(source, "")).unwrap();
        let bytecode = result.bytecode.unwrap();
        let decoded = BASE64.decode(&bytecode);
        assert!(decoded.is_ok());
        assert!(!decoded.unwrap().is_empty());
    }

    #[test]
    fn run_when_zero_scans_then_returns_zero_variables() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42;
END_PROGRAM
";
        let compile_result: CompileResult = serde_json::from_str(&compile(source, "")).unwrap();
        let bytecode = compile_result.bytecode.unwrap();

        let result: RunResult = serde_json::from_str(&run(&bytecode, 0)).unwrap();
        assert!(result.ok);
        assert_eq!(result.scans_completed, 0);
    }

    // --- Stepping tests ---

    #[test]
    fn load_program_when_valid_source_then_creates_session() {
        reset_session();
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42;
END_PROGRAM
";
        let result: StepResult = serde_json::from_str(&load_program(source, 100_000, "")).unwrap();
        assert!(result.ok);
        assert_eq!(result.total_scans, 0);
        assert!(result.diagnostics.is_empty());
        assert!(result.error.is_none());
    }

    #[test]
    fn load_program_when_syntax_error_then_returns_diagnostics() {
        reset_session();
        let source = "PROGRAM main INVALID END_PROGRAM";
        let result: StepResult = serde_json::from_str(&load_program(source, 100_000, "")).unwrap();
        assert!(!result.ok);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn step_when_no_session_then_returns_error() {
        reset_session();
        let result: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(!result.ok);
        assert!(result.error.unwrap().contains("No program loaded"));
    }

    #[test]
    fn step_when_session_loaded_then_returns_variables() {
        reset_session();
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42;
END_PROGRAM
";
        load_program(source, 100_000, "");
        let result: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(result.ok);
        assert_eq!(result.total_scans, 1);
        assert!(!result.variables.is_empty());
        assert_eq!(result.variables[0].value, "42");
    }

    #[test]
    fn step_when_called_twice_then_variables_persist() {
        reset_session();
        let source = "
PROGRAM main
  VAR
    count : DINT;
  END_VAR
  count := count + 1;
END_PROGRAM
";
        load_program(source, 100_000, "");

        let r1: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(r1.ok);
        assert_eq!(r1.variables[0].value, "1");

        let r2: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(r2.ok);
        assert_eq!(r2.variables[0].value, "2");
    }

    #[test]
    fn step_when_called_twice_then_total_scans_accumulate() {
        reset_session();
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 1;
END_PROGRAM
";
        load_program(source, 100_000, "");

        let r1: StepResult = serde_json::from_str(&step(3)).unwrap();
        assert_eq!(r1.total_scans, 3);

        let r2: StepResult = serde_json::from_str(&step(2)).unwrap();
        assert_eq!(r2.total_scans, 5);
    }

    #[test]
    fn step_when_session_faulted_then_returns_error() {
        reset_session();
        let source = "
PROGRAM main
  VAR
    x : DINT;
    y : DINT;
  END_VAR
  y := 0;
  x := 1 / y;
END_PROGRAM
";
        load_program(source, 100_000, "");

        // First step should fault (divide by zero)
        let r1: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(!r1.ok);
        assert!(r1.error.as_ref().unwrap().contains("VM trap"));

        // Subsequent step should report faulted session
        let r2: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(!r2.ok);
        assert!(r2.error.unwrap().contains("faulted"));
    }

    #[test]
    fn reset_session_when_session_exists_then_clears_it() {
        let source = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 1;
END_PROGRAM
";
        load_program(source, 100_000, "");
        step(1);

        reset_session();

        // After reset, step should fail with no session
        let result: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(!result.ok);
        assert!(result.error.unwrap().contains("No program loaded"));
    }

    #[test]
    fn compile_when_valid_xml_then_returns_bytecode() {
        let source = r#"<?xml version="1.0" encoding="utf-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
  <fileHeader companyName="Test" productName="Test" productVersion="1.0" creationDateTime="2024-01-01T00:00:00"/>
  <contentHeader name="TestProject">
    <coordinateInfo>
      <fbd><scaling x="1" y="1"/></fbd>
      <ld><scaling x="1" y="1"/></ld>
      <sfc><scaling x="1" y="1"/></sfc>
    </coordinateInfo>
  </contentHeader>
  <types>
    <dataTypes/>
    <pous>
      <pou name="main" pouType="program">
        <interface>
          <localVars>
            <variable name="bSwitch">
              <type><BOOL/></type>
            </variable>
          </localVars>
        </interface>
        <body>
          <ST>
            <xhtml xmlns="http://www.w3.org/1999/xhtml">
bSwitch := TRUE;
            </xhtml>
          </ST>
        </body>
      </pou>
    </pous>
  </types>
</project>"#;
        let result: CompileResult = serde_json::from_str(&compile(source, "")).unwrap();
        assert!(
            result.ok,
            "Expected ok but got diagnostics: {:?}",
            result.diagnostics
        );
        assert!(result.bytecode.is_some());
    }

    #[test]
    fn compile_when_twincat_xml_then_returns_bytecode() {
        let source = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="main" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
    <Declaration><![CDATA[PROGRAM main
VAR
    x : DINT;
END_VAR]]></Declaration>
    <Implementation>
      <ST><![CDATA[x := 42;]]></ST>
    </Implementation>
  </POU>
</TcPlcObject>"#;
        let result: CompileResult = serde_json::from_str(&compile(source, "")).unwrap();
        assert!(
            result.ok,
            "Expected ok but got diagnostics: {:?}",
            result.diagnostics
        );
        assert!(result.bytecode.is_some());
    }

    #[test]
    fn compile_when_malformed_xml_then_returns_diagnostics() {
        let source = "<?xml version=\"1.0\"?><project><invalid";
        let result: CompileResult = serde_json::from_str(&compile(source, "")).unwrap();
        assert!(!result.ok);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn load_program_when_called_twice_then_replaces_session() {
        reset_session();
        let source_a = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 10;
END_PROGRAM
";
        load_program(source_a, 100_000, "");
        let r1: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert_eq!(r1.variables[0].value, "10");

        let source_b = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 20;
END_PROGRAM
";
        load_program(source_b, 100_000, "");
        let r2: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert_eq!(r2.variables[0].value, "20");
        assert_eq!(r2.total_scans, 1);
    }

    #[test]
    fn step_when_variable_has_initial_value_then_persists_across_steps() {
        reset_session();
        let source = "
PROGRAM main
  VAR
    exponentially : INT := 1;
  END_VAR
  exponentially := exponentially * 2;
END_PROGRAM
";
        load_program(source, 100_000, "");

        let r1: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(r1.ok);
        assert_eq!(r1.total_scans, 1);
        assert_eq!(r1.variables[0].value, "2"); // 1 * 2

        let r2: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(r2.ok);
        assert_eq!(r2.total_scans, 2);
        assert_eq!(r2.variables[0].value, "4"); // 2 * 2

        let r3: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(r3.ok);
        assert_eq!(r3.total_scans, 3);
        assert_eq!(r3.variables[0].value, "8"); // 4 * 2
    }

    #[test]
    fn run_source_when_bcd_to_int_with_literal_then_returns_value() {
        let source = "
PROGRAM main
  VAR
    int_val : USINT;
  END_VAR
  int_val := BCD_TO_INT(BYTE#16#42);
END_PROGRAM
";
        let result: RunSourceResult = serde_json::from_str(&run_source(source, 1, "")).unwrap();
        assert!(result.ok, "Expected ok but got error: {:?}", result.error);
        assert_eq!(result.variables[0].value, "42");
    }

    #[test]
    fn run_source_when_int_to_bcd_with_literal_then_returns_value() {
        let source = "
PROGRAM main
  VAR
    bcd_val : BYTE;
  END_VAR
  bcd_val := INT_TO_BCD(USINT#42);
END_PROGRAM
";
        let result: RunSourceResult = serde_json::from_str(&run_source(source, 1, "")).unwrap();
        assert!(result.ok, "Expected ok but got error: {:?}", result.error);
        assert_eq!(result.variables[0].value, "16#42");
    }

    #[test]
    fn compile_when_undeclared_variable_then_returns_diagnostic() {
        let source = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := undeclared_var;
END_PROGRAM
";
        let result: CompileResult = serde_json::from_str(&compile(source, "")).unwrap();
        assert!(!result.ok);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn step_when_ton_then_q_transitions_to_true() {
        reset_session();
        // PT = T#5s = 5000 ms. With cycle_time_us = 100_000 (100ms per step),
        // Q should become TRUE after 50 steps (50 * 100ms = 5s).
        let source = "
PROGRAM main
  VAR
    myTimer : TON;
    start : BOOL := TRUE;
    done : BOOL;
    elapsed : TIME;
  END_VAR
  myTimer(IN := start, PT := T#5s, Q => done, ET => elapsed);
END_PROGRAM
";
        let load: StepResult = serde_json::from_str(&load_program(source, 100_000, "")).unwrap();
        assert!(
            load.ok,
            "load failed: error={:?}, diagnostics={:?}",
            load.error, load.diagnostics
        );

        // After 10 steps (1s elapsed), Q should still be FALSE
        let r1: StepResult = serde_json::from_str(&step(10)).unwrap();
        assert!(r1.ok, "step(10) failed: {:?}", r1.error);
        let done_var = r1.variables.iter().find(|v| v.name == "done").unwrap();
        assert_eq!(done_var.value, "FALSE");

        // After 50 total steps (5s elapsed), Q should be TRUE
        let r2: StepResult = serde_json::from_str(&step(41)).unwrap();
        assert!(r2.ok, "step(41) failed: {:?}", r2.error);
        let done_var = r2.variables.iter().find(|v| v.name == "done").unwrap();
        assert_eq!(done_var.value, "TRUE");

        // Verify TIME variable displays correct type name
        let elapsed_var = r2.variables.iter().find(|v| v.name == "elapsed").unwrap();
        assert_eq!(
            elapsed_var.type_name, "TIME",
            "TIME variable should display as TIME, not TIME_OF_DAY"
        );
    }

    #[test]
    fn step_when_tof_then_q_transitions_to_false() {
        reset_session();
        // PT = T#5s = 5000 ms. With cycle_time_us = 100_000 (100ms per step),
        // Q should become FALSE after 50 steps of IN=FALSE (50 * 100ms = 5s).
        let source = "
PROGRAM main
  VAR
    myTimer : TOF;
    run : BOOL := TRUE;
    active : BOOL;
    elapsed : TIME;
  END_VAR
  myTimer(IN := run, PT := T#5s, Q => active, ET => elapsed);
END_PROGRAM
";
        let load: StepResult = serde_json::from_str(&load_program(source, 100_000, "")).unwrap();
        assert!(
            load.ok,
            "load failed: error={:?}, diagnostics={:?}",
            load.error, load.diagnostics
        );

        // After 10 steps with IN=TRUE, Q should be TRUE
        let r1: StepResult = serde_json::from_str(&step(10)).unwrap();
        assert!(r1.ok, "step(10) failed: {:?}", r1.error);
        let active_var = r1.variables.iter().find(|v| v.name == "active").unwrap();
        assert_eq!(active_var.value, "TRUE");
    }

    #[test]
    fn format_time_value_ms_when_zero_then_returns_zero_ms() {
        assert_eq!(format_time_value_ms(0i32), "T#0ms");
    }

    #[test]
    fn format_time_value_ms_when_whole_milliseconds_then_no_decimal() {
        assert_eq!(format_time_value_ms(5i32), "T#5ms");
    }

    #[test]
    fn format_time_value_ms_when_whole_seconds_then_no_decimal() {
        assert_eq!(format_time_value_ms(3000i32), "T#3s");
    }

    #[test]
    fn format_time_value_ms_when_fractional_seconds_then_shows_decimal() {
        assert_eq!(format_time_value_ms(1500i32), "T#1.5s");
    }

    #[test]
    fn format_time_value_ms_when_negative_then_shows_sign() {
        assert_eq!(format_time_value_ms(-2000i32), "-T#2s");
    }

    #[test]
    fn format_time_value_ms_when_i64_ltime_then_formats_correctly() {
        assert_eq!(format_time_value_ms(5000i64), "T#5s");
    }

    #[test]
    fn compile_when_edition_2013_then_accepts_ltime() {
        let source = "
PROGRAM main
  VAR
    duration : LTIME;
  END_VAR
  duration := LTIME#100ms;
END_PROGRAM
";
        let result: CompileResult = serde_json::from_str(&compile(source, "2013")).unwrap();
        assert!(
            result.ok,
            "Expected ok but got diagnostics: {:?}",
            result.diagnostics
        );
        assert!(result.bytecode.is_some());
    }

    #[test]
    fn compile_when_default_edition_then_rejects_ltime_as_type() {
        let source = "
PROGRAM main
  VAR
    duration : LTIME;
  END_VAR
  duration := LTIME#100ms;
END_PROGRAM
";
        let result: CompileResult = serde_json::from_str(&compile(source, "")).unwrap();
        assert!(!result.ok);
        assert!(!result.diagnostics.is_empty());
    }

    #[test]
    fn load_program_when_edition_2013_then_runs_ltime_program() {
        reset_session();
        let source = "
PROGRAM main
  VAR
    duration : LTIME;
  END_VAR
  duration := LTIME#500ms;
END_PROGRAM
";
        let result: StepResult =
            serde_json::from_str(&load_program(source, 100_000, "2013")).unwrap();
        assert!(
            result.ok,
            "Expected ok but got error: {:?}, diagnostics: {:?}",
            result.error, result.diagnostics
        );

        let r1: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert!(r1.ok, "step failed: {:?}", r1.error);
        assert!(!r1.variables.is_empty());
    }
}
