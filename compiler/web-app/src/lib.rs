//! Browser-based compiler and runtime for IronPLC.
//!
//! Exposes three functions to JavaScript:
//! - [`compile`] - Parse IEC 61131-3 source and produce bytecode
//! - [`run`] - Execute pre-compiled bytecode (.iplc)
//! - [`run_source`] - Compile and execute in one step

use std::io::Cursor;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ironplc_codegen::compile as codegen_compile;
use ironplc_container::Container;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::ParseOptions;
use ironplc_parser::parse_program;
use ironplc_vm::{ProgramInstanceState, Slot, TaskState, Vm};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

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
#[derive(Serialize, Deserialize)]
struct DiagnosticInfo {
    code: String,
    message: String,
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
    value: i32,
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
        format!(r#"{{"ok":false,"diagnostics":[{{"code":"INTERNAL","message":"Serialization error: {e}","start":0,"end":0}}]}}"#)
    })
}

fn compile_inner(source: &str) -> CompileResult {
    let library = match parse_program(source, &FileId::default(), &ParseOptions::default()) {
        Ok(lib) => lib,
        Err(diag) => {
            return CompileResult {
                ok: false,
                bytecode: None,
                diagnostics: vec![DiagnosticInfo {
                    code: diag.code.clone(),
                    message: diag.description(),
                    start: diag.primary.location.start,
                    end: diag.primary.location.end,
                }],
            };
        }
    };

    let container = match codegen_compile(&library) {
        Ok(c) => c,
        Err(diag) => {
            return CompileResult {
                ok: false,
                bytecode: None,
                diagnostics: vec![DiagnosticInfo {
                    code: diag.code.clone(),
                    message: diag.description(),
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
    let task_count = container.task_table.tasks.len();
    let program_count = container.task_table.programs.len();
    let mut task_states = vec![TaskState::default(); task_count];
    let mut program_instances = vec![ProgramInstanceState::default(); program_count];
    let mut ready_buf = vec![0usize; task_count.max(1)];

    let mut running = Vm::new()
        .load(
            &container,
            &mut stack_buf,
            &mut var_buf,
            &mut task_states,
            &mut program_instances,
            &mut ready_buf,
        )
        .start();

    for round in 0..scans {
        let current_us = (round as u64) * 1000;
        if let Err(ctx) = running.run_round(current_us) {
            let faulted = running.fault(ctx);
            let variables = read_all_variables_faulted(&faulted);
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
    let variables = read_all_variables_running(&running, num_vars);
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

fn read_all_variables_running(vm: &ironplc_vm::VmRunning, num_vars: u16) -> Vec<VariableInfo> {
    (0..num_vars)
        .filter_map(|i| {
            vm.read_variable(i)
                .ok()
                .map(|value| VariableInfo { index: i, value })
        })
        .collect()
}

fn read_all_variables_faulted(vm: &ironplc_vm::VmFaulted) -> Vec<VariableInfo> {
    let num_vars = vm.num_variables();
    (0..num_vars)
        .filter_map(|i| {
            vm.read_variable(i)
                .ok()
                .map(|value| VariableInfo { index: i, value })
        })
        .collect()
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
        assert_eq!(result.variables[0].value, 42);
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
        assert_eq!(result.variables[0].value, 10);
        assert_eq!(result.variables[1].value, 42);
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
        assert_eq!(result.variables[0].value, 99);
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
}
