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
use ironplc_codegen::compile as codegen_compile;
use ironplc_container::debug_section::iec_type_tag;
use ironplc_container::Container;
use ironplc_dsl::core::FileId;
use ironplc_sources::{parse_source, FileType};
use ironplc_vm::{ProgramInstanceState, Slot, TaskState, Vm};
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
    total_scans: u64,
    faulted: bool,
}

thread_local! {
    static SESSION: RefCell<Option<VmSession>> = const { RefCell::new(None) };
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
        _ => format!("{}", raw as i32), // fallback
    }
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
pub fn compile(source: &str) -> String {
    let result = compile_inner(source);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"ok":false,"diagnostics":[{{"code":"INTERNAL","message":"Serialization error: {e}","label":"","start":0,"end":0}}]}}"#)
    })
}

fn compile_inner(source: &str) -> CompileResult {
    let file_type = FileType::from_content(source);
    let library = match parse_source(file_type, source, &FileId::default()) {
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

    let container = match codegen_compile(&library) {
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

    let h = &container.header;
    let mut stack_buf = vec![Slot::default(); h.max_stack_depth as usize];
    let mut var_buf = vec![Slot::default(); h.num_variables as usize];
    let mut data_region_buf = vec![0u8; h.data_region_bytes as usize];
    let temp_buf_total = h.num_temp_bufs as usize * h.max_temp_buf_bytes as usize;
    let mut temp_buf = vec![0u8; temp_buf_total];
    let task_count = container.task_table.tasks.len();
    let program_count = container.task_table.programs.len();
    let mut task_states = vec![TaskState::default(); task_count];
    let mut program_instances = vec![ProgramInstanceState::default(); program_count];
    let mut ready_buf = vec![0usize; task_count.max(1)];

    let mut running = match Vm::new()
        .load(
            &container,
            &mut stack_buf,
            &mut var_buf,
            &mut data_region_buf,
            &mut temp_buf,
            &mut task_states,
            &mut program_instances,
            &mut ready_buf,
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
pub fn run_source(source: &str, scans: u32) -> String {
    let result = run_source_inner(source, scans);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"ok":false,"diagnostics":[],"variables":[],"scans_completed":0,"error":"Serialization error: {e}"}}"#)
    })
}

fn run_source_inner(source: &str, scans: u32) -> RunSourceResult {
    let compile_result = compile_inner(source);
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
pub fn load_program(source: &str) -> String {
    let result = load_program_inner(source);
    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(r#"{{"ok":false,"diagnostics":[],"variables":[],"total_scans":0,"error":"Serialization error: {e}"}}"#)
    })
}

fn load_program_inner(source: &str) -> StepResult {
    let compile_result = compile_inner(source);
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

    let num_vars = container.header.num_variables as usize;

    SESSION.with(|cell| {
        *cell.borrow_mut() = Some(VmSession {
            container_bytes,
            var_buf: vec![Slot::default(); num_vars],
            total_scans: 0,
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
                total_scans: session.total_scans,
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
                    total_scans: session.total_scans,
                    error: Some(format!("Failed to load bytecode: {e}")),
                };
            }
        };

        let base_scans = session.total_scans;
        let (variables, scans_done, error) =
            run_vm_step(&container, &mut session.var_buf, base_scans, scans);

        session.total_scans += scans_done as u64;
        if error.is_some() {
            session.faulted = true;
        }

        StepResult {
            ok: error.is_none(),
            diagnostics: vec![],
            variables,
            total_scans: session.total_scans,
            error,
        }
    })
}

/// Run an ephemeral VM for N scans using the given container and variable buffer.
///
/// Returns `(variables, scans_completed, error)`.
fn run_vm_step(
    container: &Container,
    var_buf: &mut [Slot],
    base_scans: u64,
    scans: u32,
) -> (Vec<VariableInfo>, u32, Option<String>) {
    let h = &container.header;
    let mut stack_buf = vec![Slot::default(); h.max_stack_depth as usize];
    let mut data_region_buf = vec![0u8; h.data_region_bytes as usize];
    let temp_buf_total = h.num_temp_bufs as usize * h.max_temp_buf_bytes as usize;
    let mut temp_buf = vec![0u8; temp_buf_total];
    let task_count = container.task_table.tasks.len();
    let program_count = container.task_table.programs.len();
    let mut task_states = vec![TaskState::default(); task_count];
    let mut program_instances = vec![ProgramInstanceState::default(); program_count];
    let mut ready_buf = vec![0usize; task_count.max(1)];

    let mut running = match Vm::new()
        .load(
            container,
            &mut stack_buf,
            var_buf,
            &mut data_region_buf,
            &mut temp_buf,
            &mut task_states,
            &mut program_instances,
            &mut ready_buf,
        )
        .start()
    {
        Ok(r) => r,
        Err(ctx) => {
            let error = format!("VM init trap: {}", ctx.trap);
            return (vec![], 0, Some(error));
        }
    };

    for round in 0..scans {
        let current_us = (base_scans + round as u64) * 1000;
        if let Err(ctx) = running.run_round(current_us) {
            let faulted = running.fault(ctx);
            let debug_map = build_var_debug_map(container);
            let variables = read_all_variables_faulted(&faulted, &debug_map);
            let error = format!(
                "VM trap: {} (task {}, instance {})",
                faulted.trap(),
                faulted.task_id(),
                faulted.instance_id()
            );
            return (variables, round, Some(error));
        }
    }

    let debug_map = build_var_debug_map(container);
    let num_vars = running.num_variables();
    let variables = read_all_variables_running(&running, num_vars, &debug_map);
    running.stop();
    (variables, scans, None)
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
        let result: CompileResult = serde_json::from_str(&compile(source)).unwrap();
        assert!(result.ok);
        assert!(result.bytecode.is_some());
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn compile_when_syntax_error_then_returns_diagnostics() {
        let source = "PROGRAM main INVALID END_PROGRAM";
        let result: CompileResult = serde_json::from_str(&compile(source)).unwrap();
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
        let compile_result: CompileResult = serde_json::from_str(&compile(source)).unwrap();
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
        let result: RunSourceResult = serde_json::from_str(&run_source(source, 1)).unwrap();
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
        let result: RunSourceResult = serde_json::from_str(&run_source(source, 1)).unwrap();
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
        let result: RunSourceResult = serde_json::from_str(&run_source(source, 5)).unwrap();
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
        let result: CompileResult = serde_json::from_str(&compile(source)).unwrap();
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
        let compile_result: CompileResult = serde_json::from_str(&compile(source)).unwrap();
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
        let result: StepResult = serde_json::from_str(&load_program(source)).unwrap();
        assert!(result.ok);
        assert_eq!(result.total_scans, 0);
        assert!(result.diagnostics.is_empty());
        assert!(result.error.is_none());
    }

    #[test]
    fn load_program_when_syntax_error_then_returns_diagnostics() {
        reset_session();
        let source = "PROGRAM main INVALID END_PROGRAM";
        let result: StepResult = serde_json::from_str(&load_program(source)).unwrap();
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
        load_program(source);
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
        load_program(source);

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
        load_program(source);

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
        load_program(source);

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
        load_program(source);
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
        let result: CompileResult = serde_json::from_str(&compile(source)).unwrap();
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
        let result: CompileResult = serde_json::from_str(&compile(source)).unwrap();
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
        let result: CompileResult = serde_json::from_str(&compile(source)).unwrap();
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
        load_program(source_a);
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
        load_program(source_b);
        let r2: StepResult = serde_json::from_str(&step(1)).unwrap();
        assert_eq!(r2.variables[0].value, "20");
        assert_eq!(r2.total_scans, 1);
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
        let result: RunSourceResult = serde_json::from_str(&run_source(source, 1)).unwrap();
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
        let result: RunSourceResult = serde_json::from_str(&run_source(source, 1)).unwrap();
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
        let result: CompileResult = serde_json::from_str(&compile(source)).unwrap();
        assert!(!result.ok);
        assert!(!result.diagnostics.is_empty());
    }
}
