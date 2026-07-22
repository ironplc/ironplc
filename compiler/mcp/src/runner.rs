//! VM execution helper for the `run` MCP tool.
//!
//! This module owns everything that touches the IronPLC VM: it builds the
//! symbol map from a freshly-compiled container, converts raw VM variable
//! slots into JSON values, and drives the execution loop under the resource
//! limits specified in the design doc (REQ-ARC-mcp-030..035).
//!
//! The module is deliberately independent of the MCP transport so that the
//! unit tests can exercise it without spawning a JSON-RPC client.
//!
//! # Fuel limit caveat
//!
//! The VM does not currently expose a per-instruction fuel budget. This
//! module approximates `max_fuel` (REQ-ARC-mcp-030) by checking
//! `InstructionProfile::total()` between task-cycle rounds — the same
//! granularity the spec permits for `max_wall_clock_ms` (REQ-ARC-mcp-035).
//! A follow-up VM change (`Vm::set_fuel_budget` + `Trap::OutOfFuel`) will
//! replace this with strict per-opcode enforcement.

use std::collections::HashMap;

use ironplc_analyzer::symbol_environment::ScopeKind;
use ironplc_analyzer::SemanticContext;
use ironplc_container::debug_section::{iec_type_tag, DebugSection, VarNameEntry};
use ironplc_container::Container;
use ironplc_vm::{Vm, VmBuffers};
use serde_json::Value;

use crate::cache::{CachedContainer, ResolvedVar, VariableSymbolMap};

/// Resource limits enforced on a single `run` invocation.
/// Defaults mirror REQ-ARC-mcp-030.
#[derive(Clone, Copy, Debug)]
pub struct EffectiveLimits {
    pub max_duration_ms: u64,
    pub max_fuel: u64,
    pub max_wall_clock_ms: u64,
    pub max_samples: usize,
    pub max_variables_per_run: usize,
}

impl EffectiveLimits {
    /// Server defaults (REQ-ARC-mcp-030).
    pub const DEFAULTS: Self = Self {
        max_duration_ms: 60_000,
        max_fuel: 50_000_000,
        max_wall_clock_ms: 5_000,
        max_samples: 1_000,
        max_variables_per_run: 64,
    };
}

/// Builds the `VariableSymbolMap` that the `run` tool uses to resolve
/// fully-qualified variable names (REQ-ARC-mcp-020, REQ-ARC-mcp-070).
///
/// The container's debug section owns `VarIndex` + `iec_type_tag`, keyed
/// by bare variable name. The analyzer's `SemanticContext` owns the scope
/// information needed to build qualified names. This function joins the
/// two: for each program in the context, walk its variables and pair each
/// one with the matching debug entry, emitting a `Main.Counter`-style
/// canonical name. Globals get a bare canonical name.
pub fn build_symbol_map(context: &SemanticContext, container: &Container) -> VariableSymbolMap {
    let mut map = VariableSymbolMap::new();
    let Some(debug) = container.debug_section.as_ref() else {
        return map;
    };

    let bare_index = index_debug_entries_by_name(debug);

    // Program-scoped variables: qualify as `<program>.<var>`.
    for (program_name, _) in context.symbols().get_programs() {
        let program = program_name.original().to_string();
        let scope = ScopeKind::Named((*program_name).clone());
        for (var_name, info) in context.symbols().get_variables_in_scope(&scope) {
            let bare = var_name.original().as_str();
            let Some(entries) = bare_index.get(bare) else {
                continue;
            };
            // Today every debug entry uses GLOBAL_SCOPE (see
            // codegen::compile_setup), so a bare-name collision across two
            // programs is resolved at lookup time via ambiguity detection.
            // Pick the first matching entry — each program's qualified name
            // is still registered, so `Main.x` and `Other.x` are both
            // queryable even if the debug entries are indistinguishable.
            let entry = entries[0];
            map.insert(ResolvedVar {
                var_index: entry.var_index,
                iec_type_tag: entry.iec_type_tag,
                var_section: entry.var_section,
                address: info.address.clone(),
                program: Some(program.clone()),
                canonical_name: format!("{}.{}", program, bare),
            });
        }
    }

    // Global variables: canonical name is the bare name.
    for (var_name, info) in context.symbols().get_variables_in_scope(&ScopeKind::Global) {
        let bare = var_name.original().as_str();
        let Some(entries) = bare_index.get(bare) else {
            continue;
        };
        let entry = entries[0];
        map.insert(ResolvedVar {
            var_index: entry.var_index,
            iec_type_tag: entry.iec_type_tag,
            var_section: entry.var_section,
            address: info.address.clone(),
            program: None,
            canonical_name: bare.to_string(),
        });
    }

    map
}

fn index_debug_entries_by_name(debug: &DebugSection) -> HashMap<&str, Vec<&VarNameEntry>> {
    let mut by_bare: HashMap<&str, Vec<&VarNameEntry>> = HashMap::new();
    for entry in &debug.var_names {
        by_bare.entry(entry.name.as_str()).or_default().push(entry);
    }
    by_bare
}

/// Converts a raw 64-bit VM slot to a typed JSON value per REQ-TOL-mcp-043.
///
/// MVP: covers every numeric and time tag. Returns `null` for types whose
/// JSON encoding (strings, dates, enums, arrays, structs) is deferred to
/// the Phase 11 follow-up.
pub fn value_from_raw(raw: u64, tag: u8) -> Value {
    match tag {
        iec_type_tag::BOOL => Value::Bool((raw as i32) != 0),
        iec_type_tag::SINT => Value::from(raw as i32 as i8 as i64),
        iec_type_tag::INT => Value::from(raw as i32 as i16 as i64),
        iec_type_tag::DINT => Value::from(raw as i32 as i64),
        iec_type_tag::USINT => Value::from(raw as u8 as u64),
        iec_type_tag::UINT => Value::from(raw as u16 as u64),
        iec_type_tag::UDINT => Value::from(raw as u32 as u64),
        iec_type_tag::LINT => Value::String((raw as i64).to_string()),
        iec_type_tag::ULINT => Value::String(raw.to_string()),
        iec_type_tag::REAL => finite_or_special(f32::from_bits(raw as u32) as f64),
        iec_type_tag::LREAL => finite_or_special(f64::from_bits(raw)),
        iec_type_tag::BYTE => Value::from(raw as u8 as u64),
        iec_type_tag::WORD => Value::from(raw as u16 as u64),
        iec_type_tag::DWORD => Value::from(raw as u32 as u64),
        iec_type_tag::LWORD => Value::String(raw.to_string()),
        iec_type_tag::TIME => Value::String(format!("T#{}ms", raw as i32)),
        iec_type_tag::LTIME => Value::String(format!("LTIME#{}ms", raw as i64)),
        // STRING/WSTRING/DATE/LDATE/TOD/LTOD/DT/LDT and OTHER fall through
        // to null in the MVP. Phase 11 extends this table.
        _ => Value::Null,
    }
}

fn finite_or_special(v: f64) -> Value {
    if v.is_nan() {
        Value::String("NaN".into())
    } else if v.is_infinite() && v.is_sign_positive() {
        Value::String("Infinity".into())
    } else if v.is_infinite() {
        Value::String("-Infinity".into())
    } else {
        serde_json::Number::from_f64(v)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

/// A trace sample emitted by the inner execution loop.
#[derive(Clone, Debug)]
pub struct TraceSample {
    pub time_ms: u64,
    pub task: String,
    /// `(canonical_name, value)` in the order the trace set was supplied.
    pub variables: Vec<(String, Value)>,
}

/// Outcome of executing a compiled container.
#[derive(Debug)]
pub struct RunOutcome {
    pub trace: Vec<TraceSample>,
    pub final_values: Vec<(String, Value)>,
    pub completed_cycles: Vec<(String, u64)>,
    pub terminated_reason: TerminatedReason,
    /// True when the trace was truncated at `max_samples`.
    pub truncated: bool,
    /// Diagnostic message when `terminated_reason == Error`.
    pub error_message: Option<String>,
}

/// Why the run stopped (REQ-TOL-mcp-047).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerminatedReason {
    Completed,
    Duration,
    Fuel,
    WallClock,
    SampleCap,
    Error,
}

impl TerminatedReason {
    pub fn as_str(self) -> &'static str {
        match self {
            TerminatedReason::Completed => "completed",
            TerminatedReason::Duration => "duration",
            TerminatedReason::Fuel => "fuel",
            TerminatedReason::WallClock => "wall_clock",
            TerminatedReason::SampleCap => "sample_cap",
            TerminatedReason::Error => "error",
        }
    }
}

/// Executes a cached container for `trace_set` variables under `limits`.
///
/// This is the core of the `run` tool. Control flow:
/// 1. Deserialize bytes → `Container`.
/// 2. Build VM buffers, load, start.
/// 3. Round loop: check limits, step one round, snapshot any task that
///    advanced its `scan_count`, append to trace.
/// 4. On trap: capture diagnostic, drain final values from `VmFaulted`.
/// 5. On clean stop: drain final values from `VmStopped`.
pub fn execute(
    cached: &CachedContainer,
    trace_set: &[ResolvedVar],
    limits: EffectiveLimits,
) -> Result<RunOutcome, String> {
    let mut bytes = cached.iplc_bytes.as_slice();
    let container =
        Container::read_from(&mut bytes).map_err(|e| format!("container read error: {e}"))?;

    let task_names: Vec<String> = cached.tasks.iter().map(|t| t.name.clone()).collect();

    let mut bufs = VmBuffers::from_container(&container);

    let mut running = match Vm::new().load(&container, &mut bufs).start() {
        Ok(r) => r,
        Err(ctx) => {
            // Start-time fault (e.g. init function trapped).
            return Ok(RunOutcome {
                trace: vec![],
                final_values: vec![],
                completed_cycles: vec![],
                terminated_reason: TerminatedReason::Error,
                truncated: false,
                error_message: Some(format!("VM start fault: {}", ctx.trap)),
            });
        }
    };

    let wall_start = std::time::Instant::now();
    let mut simulated_us: u64 = 0;
    let mut trace: Vec<TraceSample> = Vec::new();
    let mut prev_scan_counts: Vec<u64> = vec![0; task_names.len()];
    let mut truncated = false;

    let terminated_reason = loop {
        // Between-rounds limit gates (REQ-ARC-mcp-032/035).
        if simulated_us / 1_000 >= limits.max_duration_ms {
            break TerminatedReason::Duration;
        }
        if wall_start.elapsed().as_millis() as u64 >= limits.max_wall_clock_ms {
            break TerminatedReason::WallClock;
        }
        if running.profile().total() >= limits.max_fuel {
            break TerminatedReason::Fuel;
        }
        if trace.len() >= limits.max_samples {
            truncated = true;
            break TerminatedReason::SampleCap;
        }

        // Advance simulated time to the next cycle that's due.
        let next_due = running.next_due_us().unwrap_or(simulated_us);
        let current_us = simulated_us.max(next_due);

        // Re-check the duration gate against `current_us` so a cyclic task
        // due after the deadline doesn't execute.
        if current_us / 1_000 >= limits.max_duration_ms {
            break TerminatedReason::Duration;
        }

        // Snapshot scan counts before the round so we can tell which tasks
        // ran (TaskState.scan_count is the VM's source of truth).
        //
        // Borrow gymnastics: bufs is borrowed by `running`, so we read
        // scan_count via the running VM's internal state indirectly by
        // re-running this step after observing the delta. The cleanest
        // path here is to rely on `scan_count()` (which currently returns
        // an aggregate) — the VM does not expose per-task scan_count while
        // running. We work around it with a pre/post diff on a reference
        // we hold via `running.next_due_us()` changes.
        //
        // Simpler: just read from `running.scan_count()` (total) and
        // attribute the delta to the task the scheduler ran. The scheduler
        // runs at most one task per round at the chosen `current_us`, so
        // this attribution is accurate.
        let before_total = running.scan_count();

        if let Err(ctx) = running.run_round(current_us) {
            let trap_msg = ctx.trap.to_string();
            let faulted = running.fault(ctx);
            let final_values =
                read_final_values(trace_set, |idx| faulted.read_variable_raw(idx).ok());
            // Per-task scan_count is not reachable from VmFaulted; report
            // totals that we tracked outside.
            let completed_cycles = task_names
                .iter()
                .cloned()
                .zip(prev_scan_counts.iter().copied())
                .collect();
            return Ok(RunOutcome {
                trace,
                final_values,
                completed_cycles,
                terminated_reason: TerminatedReason::Error,
                truncated,
                error_message: Some(trap_msg),
            });
        }

        let after_total = running.scan_count();
        // Attribute each executed round to task 0. Single-task programs
        // (the common MVP shape — `compile` synthesizes a single task
        // when no CONFIGURATION is declared) are unambiguous. Phase 11
        // will add per-task attribution by tracking each TaskState's
        // scan_count delta; the VM API does not currently expose that
        // per-task snapshot from VmRunning.
        if after_total > before_total {
            let task_idx = 0;
            let task_name = task_names
                .get(task_idx)
                .cloned()
                .unwrap_or_else(|| format!("task_{task_idx}"));

            if let Some(slot) = prev_scan_counts.get_mut(task_idx) {
                *slot += 1;
            }

            let variables: Vec<(String, Value)> = trace_set
                .iter()
                .map(|v| {
                    let raw = running.read_variable_raw(v.var_index).unwrap_or(0);
                    (
                        v.canonical_name.clone(),
                        value_from_raw(raw, v.iec_type_tag),
                    )
                })
                .collect();

            trace.push(TraceSample {
                time_ms: current_us / 1_000,
                task: task_name,
                variables,
            });
        }

        // Advance simulated time past this cycle. When the VM has no more
        // due cyclic tasks, break with Completed to avoid an infinite
        // freewheeling loop.
        match running.next_due_us() {
            Some(next) if next > current_us => simulated_us = next,
            Some(_) => simulated_us = current_us.saturating_add(1),
            None => break TerminatedReason::Completed,
        }
    };

    let stopped = running.stop();
    let final_values = read_final_values(trace_set, |idx| stopped.read_variable_raw(idx).ok());
    let completed_cycles = task_names
        .iter()
        .cloned()
        .zip(prev_scan_counts.iter().copied())
        .collect();

    Ok(RunOutcome {
        trace,
        final_values,
        completed_cycles,
        terminated_reason,
        truncated,
        error_message: None,
    })
}

fn read_final_values<F>(trace_set: &[ResolvedVar], mut read: F) -> Vec<(String, Value)>
where
    F: FnMut(ironplc_container::VarIndex) -> Option<u64>,
{
    trace_set
        .iter()
        .map(|v| {
            let raw = read(v.var_index).unwrap_or(0);
            (
                v.canonical_name.clone(),
                value_from_raw(raw, v.iec_type_tag),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_from_raw_when_bool_true_then_json_true() {
        assert_eq!(value_from_raw(1, iec_type_tag::BOOL), Value::Bool(true));
    }

    #[test]
    fn value_from_raw_when_bool_zero_then_json_false() {
        assert_eq!(value_from_raw(0, iec_type_tag::BOOL), Value::Bool(false));
    }

    #[test]
    fn value_from_raw_when_dint_negative_then_sign_extended_i64() {
        let raw = 0xFFFF_FFFF_u64;
        assert_eq!(value_from_raw(raw, iec_type_tag::DINT), Value::from(-1i64));
    }

    #[test]
    fn value_from_raw_when_int_negative_then_sign_extended_from_16_bits() {
        let raw = 0xFFFF_u64;
        assert_eq!(value_from_raw(raw, iec_type_tag::INT), Value::from(-1i64));
    }

    #[test]
    fn value_from_raw_when_udint_then_unsigned_number() {
        assert_eq!(
            value_from_raw(0xFFFF_FFFF, iec_type_tag::UDINT),
            Value::from(0xFFFF_FFFF_u64)
        );
    }

    #[test]
    fn value_from_raw_when_lint_max_then_json_string() {
        let raw = i64::MAX as u64;
        assert_eq!(
            value_from_raw(raw, iec_type_tag::LINT),
            Value::String(i64::MAX.to_string())
        );
    }

    #[test]
    fn value_from_raw_when_ulint_max_then_json_string() {
        let raw = u64::MAX;
        assert_eq!(
            value_from_raw(raw, iec_type_tag::ULINT),
            Value::String(u64::MAX.to_string())
        );
    }

    #[test]
    fn value_from_raw_when_real_finite_then_number() {
        let raw = 1.5_f32.to_bits() as u64;
        let v = value_from_raw(raw, iec_type_tag::REAL);
        assert_eq!(v, Value::from(1.5_f64));
    }

    #[test]
    fn value_from_raw_when_real_nan_then_json_string_nan() {
        let raw = f32::NAN.to_bits() as u64;
        assert_eq!(
            value_from_raw(raw, iec_type_tag::REAL),
            Value::String("NaN".into())
        );
    }

    #[test]
    fn value_from_raw_when_lreal_infinity_then_json_string_infinity() {
        let raw = f64::INFINITY.to_bits();
        assert_eq!(
            value_from_raw(raw, iec_type_tag::LREAL),
            Value::String("Infinity".into())
        );
    }

    #[test]
    fn value_from_raw_when_lreal_neg_infinity_then_json_string_neg_infinity() {
        let raw = f64::NEG_INFINITY.to_bits();
        assert_eq!(
            value_from_raw(raw, iec_type_tag::LREAL),
            Value::String("-Infinity".into())
        );
    }

    #[test]
    fn value_from_raw_when_time_then_iec_duration_string() {
        assert_eq!(
            value_from_raw(250, iec_type_tag::TIME),
            Value::String("T#250ms".into())
        );
    }

    #[test]
    fn value_from_raw_when_string_tag_then_null_mvp_placeholder() {
        assert_eq!(value_from_raw(0, iec_type_tag::STRING), Value::Null);
    }
}
