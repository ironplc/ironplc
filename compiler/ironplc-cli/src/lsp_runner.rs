//! VM session management for the LSP server.
//!
//! Provides compile-and-run functionality so the VS Code extension can
//! execute IEC 61131-3 programs and display variable values. The session
//! persists variable state across step calls, matching the playground's
//! stepping model.

use std::io::Cursor;

use ironplc_analyzer::stages::analyze;
use ironplc_codegen::compile as codegen_compile;
use ironplc_container::debug_format::VariableRenderer;
use ironplc_container::Container;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::CompilerOptions;
use ironplc_sources::{parse_source, FileType};
use ironplc_vm::{Slot, VariableView, Vm, VmBuffers};
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
        match Vm::new().load(&container, &mut bufs).start() {
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
        // Swap the session's persistent buffers into VmBuffers so the VM
        // operates on them directly, avoiding a copy.
        std::mem::swap(&mut bufs.vars, &mut self.var_buf);
        std::mem::swap(&mut bufs.data_region, &mut self.data_region);

        let result = run_step_scans(
            &container,
            &mut bufs,
            self.scan_count,
            scans,
            self.cycle_time_us,
        );

        // Swap the (now-updated) persistent buffers back to the session.
        std::mem::swap(&mut bufs.vars, &mut self.var_buf);
        std::mem::swap(&mut bufs.data_region, &mut self.data_region);

        self.scan_count = result.total_scans;
        if !result.ok {
            self.faulted = true;
        }

        result
    }
}

/// Runs scan cycles on an already-prepared [`VmBuffers`], returning a
/// [`RunResult`] with variable snapshots and the total scan count.
fn run_step_scans(
    container: &Container,
    bufs: &mut VmBuffers,
    base_scan_count: u64,
    scans: u32,
    cycle_time_us: u64,
) -> RunResult {
    let mut running = Vm::new().load(container, bufs).resume(base_scan_count);

    for _ in 0..scans {
        let uptime_us = running.scan_count() * cycle_time_us;
        if let Err(ctx) = running.run_round(uptime_us) {
            let total_scans = running.scan_count();
            let faulted = running.fault(ctx);
            let renderer = VariableRenderer::new(container);
            let variables = read_all_variables(&faulted, &renderer);
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

    let renderer = VariableRenderer::new(container);
    let variables = read_all_variables(&running, &renderer);
    let total_scans = running.scan_count();
    running.stop();

    RunResult {
        ok: true,
        variables,
        total_scans,
        error: None,
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

    let codegen_options = ironplc_codegen::CodegenOptions {
        system_uptime_global: options.allow_system_uptime_global,
    };
    let container = codegen_compile(
        &library,
        &context,
        &codegen_options,
        &ironplc_codegen::EmptyLookup,
    )
    .map_err(|diag| RunResult {
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

/// Snapshots every variable slot with its debug name, type and formatted value.
///
/// Takes the VM as a [`VariableView`] so a running and a faulted VM share one
/// body: the caller's lifecycle state does not change how a value is read.
///
/// Rendering goes through [`VariableRenderer`], the one place that formats a
/// variable for display (`specs/design/variable-value-rendering.md`), so the
/// run panel agrees with `--dump-vars`, the debugger and the playground.
fn read_all_variables(vm: &dyn VariableView, renderer: &VariableRenderer) -> Vec<VariableInfo> {
    let data_region = vm.data_region();
    (0..vm.num_variables())
        .filter_map(|i| {
            vm.read_variable_raw(ironplc_container::VarIndex::new(i))
                .ok()
                .map(|raw| VariableInfo {
                    index: i,
                    value: renderer.render(i, raw, data_region).text,
                    name: renderer
                        .var(i)
                        .map(|info| info.name.clone())
                        .unwrap_or_default(),
                    type_name: renderer
                        .var(i)
                        .map(|info| info.type_name.clone())
                        .unwrap_or_default(),
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
}
