//! VM session management for the LSP server.
//!
//! Provides compile-and-run functionality so the VS Code extension can
//! execute IEC 61131-3 programs and display variable values. The session
//! persists variable state across step calls, matching the playground's
//! stepping model.

use std::collections::HashMap;
use std::io::Cursor;

use ironplc_analyzer::stages::analyze;
use ironplc_codegen::compile as codegen_compile;
use ironplc_container::debug_section::iec_type_tag;
use ironplc_container::Container;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_sources::{parse_source, FileType};
use ironplc_vm::{Slot, Vm, VmBuffers};
use serde::{Deserialize, Serialize};

/// A variable value read from the VM after execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableInfo {
    pub index: u16,
    pub value: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub type_name: String,
}

/// Result of a run/step operation.
#[derive(Debug, Serialize, Deserialize)]
pub struct RunResult {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<VariableInfo>,
    pub total_scans: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Persistent state for step-through execution.
///
/// Stores compiled bytecode and variable buffer so variables persist
/// across calls to [`VmRunner::step`].
pub struct VmRunner {
    container_bytes: Vec<u8>,
    var_buf: Vec<Slot>,
    data_region: Vec<u8>,
    scan_count: u64,
    cycle_time_us: u64,
    faulted: bool,
}

impl VmRunner {
    /// Compile source code and create a new runner session.
    ///
    /// Runs the init function once so initial values are applied to the
    /// variable buffer. Returns the runner and initial variable metadata.
    pub fn load(
        source: &str,
        cycle_time_us: u64,
        options: &CompilerOptions,
    ) -> Result<(Self, RunResult), RunResult> {
        let container_bytes = compile_to_bytes(source, options)?;

        let container =
            Container::read_from(&mut Cursor::new(&container_bytes)).map_err(|e| RunResult {
                ok: false,
                variables: vec![],
                total_scans: 0,
                error: Some(format!("Failed to load bytecode: {e}")),
            })?;

        // Run init to apply initial values
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
                return Err(RunResult {
                    ok: false,
                    variables: vec![],
                    total_scans: 0,
                    error: Some(format!("VM init trap: {}", ctx.trap)),
                });
            }
        }

        let runner = VmRunner {
            container_bytes,
            var_buf: bufs.vars,
            data_region: bufs.data_region,
            scan_count: 0,
            cycle_time_us,
            faulted: false,
        };

        let result = RunResult {
            ok: true,
            variables: vec![],
            total_scans: 0,
            error: None,
        };

        Ok((runner, result))
    }

    /// Execute N scan cycles within the session.
    ///
    /// Variable values persist between calls. Uses `resume()` to skip
    /// re-initialization.
    pub fn step(&mut self, scans: u32) -> RunResult {
        if self.faulted {
            return RunResult {
                ok: false,
                variables: vec![],
                total_scans: self.scan_count,
                error: Some("Session is faulted. Load a new program to restart.".to_string()),
            };
        }

        let container = match Container::read_from(&mut Cursor::new(&self.container_bytes)) {
            Ok(c) => c,
            Err(e) => {
                return RunResult {
                    ok: false,
                    variables: vec![],
                    total_scans: self.scan_count,
                    error: Some(format!("Failed to load bytecode: {e}")),
                };
            }
        };

        let mut bufs = VmBuffers::from_container(&container);

        let mut running = Vm::new()
            .load(
                &container,
                &mut bufs.stack,
                &mut self.var_buf,
                &mut self.data_region,
                &mut bufs.temp_buf,
                &mut bufs.tasks,
                &mut bufs.programs,
                &mut bufs.ready,
            )
            .resume(self.scan_count);

        for _ in 0..scans {
            let current_us = running.scan_count() * self.cycle_time_us;
            if let Err(ctx) = running.run_round(current_us) {
                let total_scans = running.scan_count();
                let faulted = running.fault(ctx);
                let debug_map = build_var_debug_map(&container);
                let variables = read_all_variables_faulted(&faulted, &debug_map);
                self.scan_count = total_scans;
                self.faulted = true;
                return RunResult {
                    ok: false,
                    variables,
                    total_scans,
                    error: Some(format!(
                        "VM trap: {} (task {}, instance {})",
                        faulted.trap(),
                        faulted.task_id(),
                        faulted.instance_id()
                    )),
                };
            }
        }

        let debug_map = build_var_debug_map(&container);
        let num_vars = running.num_variables();
        let variables = read_all_variables_running(&running, num_vars, &debug_map);
        let total_scans = running.scan_count();
        running.stop();

        self.scan_count = total_scans;

        RunResult {
            ok: true,
            variables,
            total_scans,
            error: None,
        }
    }
}

/// Compile IEC 61131-3 source to bytecode bytes.
fn compile_to_bytes(source: &str, options: &CompilerOptions) -> Result<Vec<u8>, RunResult> {
    let file_type = FileType::from_content(source);
    let library =
        parse_source(file_type, source, &FileId::default(), options).map_err(|diag| RunResult {
            ok: false,
            variables: vec![],
            total_scans: 0,
            error: Some(diag.description()),
        })?;

    let (library, context) = analyze(&[&library], options).map_err(|diagnostics| RunResult {
        ok: false,
        variables: vec![],
        total_scans: 0,
        error: Some(
            diagnostics
                .iter()
                .map(|d| d.description())
                .collect::<Vec<_>>()
                .join("; "),
        ),
    })?;

    if context.has_diagnostics() {
        return Err(RunResult {
            ok: false,
            variables: vec![],
            total_scans: 0,
            error: Some(
                context
                    .diagnostics()
                    .iter()
                    .map(|d| d.description())
                    .collect::<Vec<_>>()
                    .join("; "),
            ),
        });
    }

    let container = codegen_compile(&library, &context).map_err(|diag| RunResult {
        ok: false,
        variables: vec![],
        total_scans: 0,
        error: Some(diag.description()),
    })?;

    let mut buf = Vec::new();
    container.write_to(&mut buf).map_err(|e| RunResult {
        ok: false,
        variables: vec![],
        total_scans: 0,
        error: Some(format!("Failed to serialize bytecode: {e}")),
    })?;

    Ok(buf)
}

/// Debug metadata for a variable.
struct VarDebugInfo {
    name: String,
    type_name: String,
    iec_type_tag: u8,
}

/// Builds a lookup map from var_index to debug info.
fn build_var_debug_map(container: &Container) -> HashMap<u16, VarDebugInfo> {
    let mut map = HashMap::new();
    if let Some(debug) = &container.debug_section {
        for entry in &debug.var_names {
            map.insert(
                entry.var_index.raw(),
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
        _ => format!("{}", raw as i32),
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

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_PROGRAM: &str = "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 42;
END_PROGRAM
";

    #[test]
    fn load_when_valid_source_then_creates_session() {
        let options = CompilerOptions::default();
        let result = VmRunner::load(SIMPLE_PROGRAM, 100_000, &options);
        assert!(result.is_ok());
        let (_, run_result) = result.unwrap();
        assert!(run_result.ok);
        assert_eq!(run_result.total_scans, 0);
    }

    #[test]
    fn load_when_invalid_source_then_returns_error() {
        let options = CompilerOptions::default();
        let result = VmRunner::load("INVALID CODE", 100_000, &options);
        assert!(result.is_err());
        match result {
            Err(err) => {
                assert!(!err.ok);
                assert!(err.error.is_some());
            }
            Ok(_) => unreachable!(),
        }
    }

    #[test]
    fn step_when_one_scan_then_returns_variables() {
        let options = CompilerOptions::default();
        let (mut runner, _) = VmRunner::load(SIMPLE_PROGRAM, 100_000, &options).unwrap();
        let result = runner.step(1);
        assert!(result.ok);
        assert_eq!(result.total_scans, 1);
        assert!(!result.variables.is_empty());
        assert_eq!(result.variables[0].name, "x");
        assert_eq!(result.variables[0].value, "42");
    }

    #[test]
    fn step_when_multiple_scans_then_accumulates_count() {
        let options = CompilerOptions::default();
        let (mut runner, _) = VmRunner::load(SIMPLE_PROGRAM, 100_000, &options).unwrap();
        runner.step(5);
        let result = runner.step(3);
        assert!(result.ok);
        assert_eq!(result.total_scans, 8);
    }

    #[test]
    fn format_variable_value_when_bool_true_then_returns_true() {
        assert_eq!(format_variable_value(1, iec_type_tag::BOOL), "TRUE");
    }

    #[test]
    fn format_variable_value_when_bool_false_then_returns_false() {
        assert_eq!(format_variable_value(0, iec_type_tag::BOOL), "FALSE");
    }

    #[test]
    fn format_variable_value_when_dint_then_returns_decimal() {
        assert_eq!(format_variable_value(42, iec_type_tag::DINT), "42");
    }

    #[test]
    fn format_variable_value_when_real_then_returns_float() {
        let raw = f32::to_bits(3.14) as u64;
        let result = format_variable_value(raw, iec_type_tag::REAL);
        assert!(result.starts_with("3.14"));
    }
}
