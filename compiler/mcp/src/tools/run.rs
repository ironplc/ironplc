//! The `run` MCP tool.
//!
//! Loads a compiled `.iplc` container from the process cache and executes
//! it in the IronPLC VM, returning a time-ordered trace of observed
//! variable values and a summary of task cycles completed.
//!
//! This implements Phase 10 of the MCP server plan (see
//! `specs/plans/2026-04-23-mcp-run-tool.md`). Phase 11 features
//! (stimuli, non-default trace modes, `tasks` filter, `container_base64`
//! ingestion, full IEC value codec for STRING/DATE/struct/array) return
//! `ok: false` with a diagnostic directing the caller to the follow-up.
//!
//! Design references: `specs/design/mcp-server.md` §`run`
//! (REQ-TOL-040..048), §Variable Naming (REQ-ARC-020..021), §VM
//! Sandboxing (REQ-ARC-030..035).

use std::sync::Mutex;

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::cache::{ContainerCache, ResolvedVar, VariableSymbolMap};
use crate::runner::{self, EffectiveLimits, RunOutcome, TerminatedReason};
use crate::tools::common::{serialize_diagnostic, serialize_diagnostics};

/// Combined input accepted by `run`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunInput {
    /// Opaque handle returned by a prior `compile` call. Exactly one of
    /// `container_id` and `container_base64` must be supplied.
    #[serde(default)]
    pub container_id: Option<String>,
    /// Inline `.iplc` bytes. Phase 11 feature — rejected in MVP.
    #[serde(default)]
    pub container_base64: Option<String>,
    /// Simulated time to run, in milliseconds.
    pub duration_ms: u64,
    /// Fully-qualified variable names to include in the trace.
    #[serde(default)]
    pub variables: Vec<String>,
    /// When `true`, the server auto-expands the trace set to include every
    /// observable output in the container.
    #[serde(default)]
    pub trace_outputs: bool,
    /// Time-ordered writes applied to drivable inputs. Phase 11 feature.
    #[serde(default)]
    #[schemars(schema_with = "stimuli_schema")]
    pub stimuli: Vec<Value>,
    /// Controls trace sampling mode and cap.
    #[serde(default)]
    pub trace: Option<TraceOptions>,
    /// Per-call limit tighteners. May not loosen server defaults (REQ-ARC-031).
    #[serde(default)]
    pub limits: Option<LimitOverrides>,
    /// Restrict trace emission to cycles from named tasks. Phase 11 feature.
    #[serde(default)]
    pub tasks: Option<Vec<String>>,
}

/// schemars schema function for the free-form `stimuli` array.
///
/// The element type is `serde_json::Value`, which schemars renders as the
/// boolean schema `true` for the array's `items`. Some MCP clients reject a
/// boolean schema in that position, so emit an explicit object schema for the
/// items instead. Stimuli are a Phase 11 feature, validated/rejected at runtime.
fn stimuli_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "array",
        "items": { "type": "object" },
        "description": "Time-ordered writes applied to drivable inputs. Phase 11 feature."
    })
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct TraceOptions {
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub interval_ms: Option<u64>,
    #[serde(default)]
    pub max_samples: Option<usize>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct LimitOverrides {
    #[serde(default)]
    pub max_duration_ms: Option<u64>,
    #[serde(default)]
    pub max_fuel: Option<u64>,
    #[serde(default)]
    pub max_wall_clock_ms: Option<u64>,
    #[serde(default)]
    pub max_samples: Option<usize>,
    #[serde(default)]
    pub max_variables_per_run: Option<usize>,
}

/// Top-level response for the `run` tool.
#[derive(Debug, Serialize)]
pub struct RunResponse {
    pub ok: bool,
    pub trace: Vec<TraceEntry>,
    pub truncated: bool,
    pub terminated_reason: String,
    pub summary: RunSummary,
    pub diagnostics: Vec<Value>,
}

#[derive(Debug, Serialize)]
pub struct TraceEntry {
    pub time_ms: u64,
    pub task: String,
    pub variables: Map<String, Value>,
}

#[derive(Debug, Serialize)]
pub struct RunSummary {
    pub final_values: Map<String, Value>,
    pub completed_cycles: Map<String, Value>,
    pub terminated_reason: String,
}

/// Entry point. Validates input, resolves the trace set, runs the VM,
/// and returns a `RunResponse`. Never returns an MCP-layer error —
/// compiler-surfaced problems always come back as diagnostics
/// (REQ-TOL-024).
pub fn build_response(input: &RunInput, cache: &Mutex<ContainerCache>) -> RunResponse {
    // --- Shape validation (REQ-TOL-040 sentence 1) ---
    match (&input.container_id, &input.container_base64) {
        (None, None) => {
            return fail(vec![validation(
                "`run` requires exactly one of `container_id` or `container_base64`.",
            )]);
        }
        (Some(_), Some(_)) => {
            return fail(vec![validation(
                "`run` accepts at most one of `container_id` or `container_base64`; both were supplied.",
            )]);
        }
        _ => {}
    }

    if input.container_base64.is_some() {
        return fail(vec![validation(
            "`container_base64` ingestion is deferred to a follow-up; pass `container_id` from a prior `compile` call for now.",
        )]);
    }

    // --- Phase 11 feature guards ---
    if !input.stimuli.is_empty() {
        return fail(vec![validation(
            "`stimuli` are not yet implemented; run without stimuli drives the program with declared initial values only.",
        )]);
    }
    if input.tasks.is_some() {
        return fail(vec![validation("`tasks` filter is not yet implemented.")]);
    }
    if let Some(trace) = &input.trace {
        if let Some(mode) = trace.mode.as_deref() {
            if mode != "every_cycle" {
                return fail(vec![validation(&format!(
                    "trace.mode = '{mode}' is not yet implemented; only 'every_cycle' is supported in this milestone."
                ))]);
            }
        }
    }

    // --- Limit override validation (REQ-ARC-031) ---
    let defaults = EffectiveLimits::DEFAULTS;
    let limits = match resolve_limits(defaults, input.limits.as_ref()) {
        Ok(l) => l,
        Err(diags) => return fail(diags),
    };

    // --- Container lookup (REQ-ARC-073) ---
    let container_id = input.container_id.as_deref().unwrap_or_default();
    let mut guard = cache.lock().unwrap();
    let cached = match guard.get(container_id) {
        Some(c) => c,
        None => {
            return fail(vec![validation(&format!(
                "Unknown or evicted container_id '{container_id}'. Re-compile to obtain a fresh handle."
            ))]);
        }
    };

    // REQ-TOL-040 sentence 4: a container with no tasks cannot be scheduled.
    if cached.tasks.is_empty() {
        return fail(vec![validation(
            "Container declares no tasks; declare at least one TASK in a CONFIGURATION to run.",
        )]);
    }

    // --- Trace set resolution (REQ-TOL-041, REQ-ARC-020/021) ---
    let trace_set = match resolve_trace_set(
        &input.variables,
        input.trace_outputs,
        &cached.symbols,
        limits.max_variables_per_run,
    ) {
        Ok(set) => set,
        Err(diags) => return fail(diags),
    };

    // --- Apply a caller-supplied `trace.max_samples` as a tightening cap ---
    let effective_samples = input
        .trace
        .as_ref()
        .and_then(|t| t.max_samples)
        .map(|n| n.min(limits.max_samples))
        .unwrap_or(limits.max_samples);
    let effective_limits = EffectiveLimits {
        max_samples: effective_samples,
        ..limits
    };

    // Clone what we need from the cache so we can release the lock
    // before calling into the VM (VM execution can take up to
    // `max_wall_clock_ms` and must not block other MCP calls).
    let cached_snapshot = (*cached).clone_for_run();
    drop(guard);

    // --- Execute ---
    let outcome = match runner::execute(&cached_snapshot, &trace_set, effective_limits) {
        Ok(o) => o,
        Err(msg) => {
            let diag = Diagnostic::problem(
                Problem::InternalError,
                Label::span(SourceSpan::default(), msg),
            );
            return fail(vec![diag]);
        }
    };

    build_success_response(outcome, input.duration_ms, effective_limits)
}

fn build_success_response(
    outcome: RunOutcome,
    requested_duration_ms: u64,
    _limits: EffectiveLimits,
) -> RunResponse {
    let trace: Vec<TraceEntry> = outcome
        .trace
        .into_iter()
        .map(|s| TraceEntry {
            time_ms: s.time_ms,
            task: s.task,
            variables: s.variables.into_iter().collect(),
        })
        .collect();

    let final_values: Map<String, Value> = outcome.final_values.into_iter().collect();
    let completed_cycles: Map<String, Value> = outcome
        .completed_cycles
        .into_iter()
        .map(|(name, count)| (name, Value::from(count)))
        .collect();

    let mut diagnostics = Vec::new();
    let reason = outcome.terminated_reason;
    if reason != TerminatedReason::Completed {
        let msg = match reason {
            TerminatedReason::Duration => format!(
                "Simulated duration limit reached before the run completed (requested {} ms).",
                requested_duration_ms
            ),
            TerminatedReason::Fuel => {
                "VM fuel budget exhausted (checked between task cycles).".to_string()
            }
            TerminatedReason::WallClock => {
                "Wall-clock limit exceeded before the run completed.".to_string()
            }
            TerminatedReason::SampleCap => {
                "Trace sample cap reached; emitted trace is truncated.".to_string()
            }
            TerminatedReason::Error => outcome
                .error_message
                .clone()
                .unwrap_or_else(|| "VM trap during execution.".to_string()),
            TerminatedReason::Completed => unreachable!(),
        };
        diagnostics.push(serialize_diagnostic(&Diagnostic::problem(
            Problem::McpInputValidation,
            Label::span(SourceSpan::default(), msg),
        )));
    }

    let reason_str = reason.as_str().to_string();
    RunResponse {
        ok: reason == TerminatedReason::Completed,
        trace,
        truncated: outcome.truncated,
        terminated_reason: reason_str.clone(),
        summary: RunSummary {
            final_values,
            completed_cycles,
            terminated_reason: reason_str,
        },
        diagnostics,
    }
}

fn resolve_limits(
    defaults: EffectiveLimits,
    overrides: Option<&LimitOverrides>,
) -> Result<EffectiveLimits, Vec<Diagnostic>> {
    let Some(o) = overrides else {
        return Ok(defaults);
    };
    let mut errors = Vec::new();
    let mut out = defaults;

    if let Some(v) = o.max_duration_ms {
        if v > defaults.max_duration_ms {
            errors.push(validation(&format!(
                "limits.max_duration_ms ({v}) exceeds server default ({}); overrides may only tighten.",
                defaults.max_duration_ms
            )));
        } else {
            out.max_duration_ms = v;
        }
    }
    if let Some(v) = o.max_fuel {
        if v > defaults.max_fuel {
            errors.push(validation(&format!(
                "limits.max_fuel ({v}) exceeds server default ({}); overrides may only tighten.",
                defaults.max_fuel
            )));
        } else {
            out.max_fuel = v;
        }
    }
    if let Some(v) = o.max_wall_clock_ms {
        if v > defaults.max_wall_clock_ms {
            errors.push(validation(&format!(
                "limits.max_wall_clock_ms ({v}) exceeds server default ({}); overrides may only tighten.",
                defaults.max_wall_clock_ms
            )));
        } else {
            out.max_wall_clock_ms = v;
        }
    }
    if let Some(v) = o.max_samples {
        if v > defaults.max_samples {
            errors.push(validation(&format!(
                "limits.max_samples ({v}) exceeds server default ({}); overrides may only tighten.",
                defaults.max_samples
            )));
        } else {
            out.max_samples = v;
        }
    }
    if let Some(v) = o.max_variables_per_run {
        if v > defaults.max_variables_per_run {
            errors.push(validation(&format!(
                "limits.max_variables_per_run ({v}) exceeds server default ({}); overrides may only tighten.",
                defaults.max_variables_per_run
            )));
        } else {
            out.max_variables_per_run = v;
        }
    }

    if errors.is_empty() {
        Ok(out)
    } else {
        Err(errors)
    }
}

/// Resolves every `requested` name into a `ResolvedVar`, expanding
/// `trace_outputs` to every observable output and enforcing the
/// `max_variables_per_run` cap.
fn resolve_trace_set(
    requested: &[String],
    trace_outputs: bool,
    symbols: &VariableSymbolMap,
    max_variables: usize,
) -> Result<Vec<ResolvedVar>, Vec<Diagnostic>> {
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut out: Vec<ResolvedVar> = Vec::new();
    let mut errors: Vec<Diagnostic> = Vec::new();

    for name in requested {
        if name.contains('*') {
            errors.push(validation(&format!(
                "Wildcard names are not supported ('{name}'); enumerate variables or set `trace_outputs: true`."
            )));
            continue;
        }
        match resolve_name(symbols, name) {
            Ok(resolved) => {
                if seen.insert(resolved.canonical_name.clone()) {
                    out.push(resolved);
                }
            }
            Err(NameError::Unresolved) => {
                errors.push(validation(&format!(
                    "Variable '{name}' does not resolve against the loaded container."
                )));
            }
            Err(NameError::Ambiguous(candidates)) => {
                errors.push(validation(&format!(
                    "Variable '{name}' is ambiguous; candidates: [{}]. Qualify the name (e.g. 'Program.{name}').",
                    candidates.join(", ")
                )));
            }
        }
    }

    if trace_outputs {
        for v in symbols.iter() {
            if is_observable_output(v) && seen.insert(v.canonical_name.clone()) {
                out.push(v.clone());
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    if out.len() > max_variables {
        return Err(vec![validation(&format!(
            "Trace set has {} variables, exceeding the server-configured limit of {}.",
            out.len(),
            max_variables
        ))]);
    }

    Ok(out)
}

/// Mirrors the `project_io::classify` output rules (REQ-TOL-211) without
/// re-running analysis — operates on the pre-built symbol map.
fn is_observable_output(v: &ResolvedVar) -> bool {
    use ironplc_container::debug_section::var_section;
    let addr = v.address.as_deref();
    let is_hw_output = addr.is_some_and(|a| a.starts_with("%Q"));
    matches!(
        v.var_section,
        var_section::VAR_OUTPUT | var_section::VAR_IN_OUT
    ) || is_hw_output
        || (v.program.is_none() && addr.is_none() && v.var_section == var_section::VAR_GLOBAL)
}

#[derive(Debug)]
enum NameError {
    Unresolved,
    Ambiguous(Vec<String>),
}

/// Resolves a fully-qualified or bare name per REQ-ARC-020.
fn resolve_name(symbols: &VariableSymbolMap, requested: &str) -> Result<ResolvedVar, NameError> {
    // Qualified lookup first.
    if requested.contains('.') {
        return symbols
            .by_qualified(requested)
            .cloned()
            .ok_or(NameError::Unresolved);
    }

    // Bare name: prefer globals, then single match, else ambiguous.
    let candidates = symbols.by_bare(requested);
    if candidates.is_empty() {
        return Err(NameError::Unresolved);
    }
    let globals: Vec<&ResolvedVar> = candidates.iter().filter(|c| c.program.is_none()).collect();
    if globals.len() == 1 {
        return Ok(globals[0].clone());
    }
    if candidates.len() == 1 {
        return Ok(candidates[0].clone());
    }
    let names: Vec<String> = candidates
        .iter()
        .map(|c| c.canonical_name.clone())
        .collect();
    Err(NameError::Ambiguous(names))
}

fn validation(message: &str) -> Diagnostic {
    Diagnostic::problem(
        Problem::McpInputValidation,
        Label::span(SourceSpan::default(), message),
    )
}

fn fail(diags: Vec<Diagnostic>) -> RunResponse {
    RunResponse {
        ok: false,
        trace: vec![],
        truncated: false,
        terminated_reason: "error".into(),
        summary: RunSummary {
            final_values: Map::new(),
            completed_cycles: Map::new(),
            terminated_reason: "error".into(),
        },
        diagnostics: serialize_diagnostics(&diags),
    }
}

// Allow `run` to snapshot the cached container under the lock so it can
// release the lock before calling into the VM. Implemented as a trait-
// style inherent extension on `CachedContainer` to avoid making the
// full struct `Clone`-able (bytes can be large).
impl crate::cache::CachedContainer {
    fn clone_for_run(&self) -> Self {
        crate::cache::CachedContainer::new(
            self.iplc_bytes.clone(),
            self.tasks.clone(),
            self.programs.clone(),
            self.symbols.clone(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::{CachedContainer, ContainerCache};
    use crate::tools::common::SourceInput;

    fn make_cache() -> Mutex<ContainerCache> {
        Mutex::new(ContainerCache::new(64, 64 * 1024 * 1024))
    }

    fn ed2_options() -> Value {
        serde_json::json!({ "dialect": "iec61131-3-ed2" })
    }

    /// Compile a program via the `compile` tool to populate the cache with
    /// a fresh container and symbol map; returns the container_id.
    fn compile_into(cache: &Mutex<ContainerCache>, source: &str) -> String {
        let sources = vec![SourceInput {
            name: "main.st".into(),
            content: source.into(),
        }];
        let resp = crate::tools::compile::build_response(&sources, &ed2_options(), false, cache);
        assert!(resp.ok, "compile failed: {:?}", resp.diagnostics);
        resp.container_id
            .expect("compile should return a container_id")
    }

    const COUNTER_PROGRAM: &str = r#"
PROGRAM Main
VAR
  Counter : INT;
END_VAR
  Counter := Counter + 1;
END_PROGRAM

CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM program1 WITH plc_task : Main;
  END_RESOURCE
END_CONFIGURATION
"#;

    fn base_input(container_id: String) -> RunInput {
        RunInput {
            container_id: Some(container_id),
            container_base64: None,
            duration_ms: 500,
            variables: vec!["Main.Counter".into()],
            trace_outputs: false,
            stimuli: vec![],
            trace: None,
            limits: None,
            tasks: None,
        }
    }

    #[test]
    fn build_response_when_unknown_container_id_then_ok_false() {
        let cache = make_cache();
        let mut input = base_input("c_999".into());
        input.variables.clear();
        let resp = build_response(&input, &cache);
        assert!(!resp.ok);
        assert_eq!(resp.terminated_reason, "error");
        assert!(resp
            .diagnostics
            .iter()
            .any(|d| d["message"].as_str().unwrap_or("").contains("c_999")));
    }

    #[test]
    fn build_response_when_neither_id_nor_base64_then_ok_false() {
        let cache = make_cache();
        let input = RunInput {
            container_id: None,
            container_base64: None,
            duration_ms: 100,
            variables: vec![],
            trace_outputs: false,
            stimuli: vec![],
            trace: None,
            limits: None,
            tasks: None,
        };
        let resp = build_response(&input, &cache);
        assert!(!resp.ok);
        assert!(!resp.diagnostics.is_empty());
    }

    #[test]
    fn build_response_when_both_id_and_base64_then_ok_false() {
        let cache = make_cache();
        let input = RunInput {
            container_id: Some("c_0".into()),
            container_base64: Some("ignored".into()),
            duration_ms: 100,
            variables: vec![],
            trace_outputs: false,
            stimuli: vec![],
            trace: None,
            limits: None,
            tasks: None,
        };
        let resp = build_response(&input, &cache);
        assert!(!resp.ok);
    }

    #[test]
    fn build_response_when_container_base64_only_then_mvp_not_implemented() {
        let cache = make_cache();
        let input = RunInput {
            container_id: None,
            container_base64: Some("abc".into()),
            duration_ms: 100,
            variables: vec![],
            trace_outputs: false,
            stimuli: vec![],
            trace: None,
            limits: None,
            tasks: None,
        };
        let resp = build_response(&input, &cache);
        assert!(!resp.ok);
        assert!(resp.diagnostics.iter().any(|d| d["message"]
            .as_str()
            .unwrap_or("")
            .contains("container_base64")));
    }

    #[test]
    fn build_response_when_stimuli_supplied_then_phase11_guard_fires() {
        let cache = make_cache();
        let id = compile_into(&cache, COUNTER_PROGRAM);
        let mut input = base_input(id);
        input.stimuli = vec![serde_json::json!({"time_ms": 0, "set": {}})];
        let resp = build_response(&input, &cache);
        assert!(!resp.ok);
        assert!(resp
            .diagnostics
            .iter()
            .any(|d| d["message"].as_str().unwrap_or("").contains("stimuli")));
    }

    #[test]
    fn build_response_when_non_every_cycle_trace_mode_then_phase11_guard_fires() {
        let cache = make_cache();
        let id = compile_into(&cache, COUNTER_PROGRAM);
        let mut input = base_input(id);
        input.trace = Some(TraceOptions {
            mode: Some("on_change".into()),
            interval_ms: None,
            max_samples: None,
        });
        let resp = build_response(&input, &cache);
        assert!(!resp.ok);
    }

    #[test]
    fn build_response_when_tasks_filter_supplied_then_phase11_guard_fires() {
        let cache = make_cache();
        let id = compile_into(&cache, COUNTER_PROGRAM);
        let mut input = base_input(id);
        input.tasks = Some(vec!["plc_task".into()]);
        let resp = build_response(&input, &cache);
        assert!(!resp.ok);
    }

    #[test]
    fn build_response_when_limits_loosen_duration_then_ok_false() {
        let cache = make_cache();
        let id = compile_into(&cache, COUNTER_PROGRAM);
        let mut input = base_input(id);
        input.limits = Some(LimitOverrides {
            max_duration_ms: Some(EffectiveLimits::DEFAULTS.max_duration_ms + 1),
            ..Default::default()
        });
        let resp = build_response(&input, &cache);
        assert!(!resp.ok);
        assert!(resp.diagnostics.iter().any(|d| d["message"]
            .as_str()
            .unwrap_or("")
            .contains("max_duration_ms")));
    }

    #[test]
    fn build_response_when_wildcard_in_variable_then_ok_false() {
        let cache = make_cache();
        let id = compile_into(&cache, COUNTER_PROGRAM);
        let mut input = base_input(id);
        input.variables = vec!["Main.*".into()];
        let resp = build_response(&input, &cache);
        assert!(!resp.ok);
        assert!(resp
            .diagnostics
            .iter()
            .any(|d| d["message"].as_str().unwrap_or("").contains("Wildcard")));
    }

    #[test]
    fn build_response_when_unresolved_variable_then_diagnostic_names_var() {
        let cache = make_cache();
        let id = compile_into(&cache, COUNTER_PROGRAM);
        let mut input = base_input(id);
        input.variables = vec!["Main.NoSuchVar".into()];
        let resp = build_response(&input, &cache);
        assert!(!resp.ok);
        assert!(resp
            .diagnostics
            .iter()
            .any(|d| d["message"].as_str().unwrap_or("").contains("NoSuchVar")));
    }

    #[test]
    fn build_response_when_too_many_variables_then_ok_false() {
        let cache = make_cache();
        let id = compile_into(&cache, COUNTER_PROGRAM);
        let mut input = base_input(id);
        input.limits = Some(LimitOverrides {
            max_variables_per_run: Some(0),
            ..Default::default()
        });
        input.variables = vec!["Main.Counter".into()];
        let resp = build_response(&input, &cache);
        assert!(!resp.ok);
    }

    #[test]
    fn build_response_when_valid_counter_program_then_trace_shows_increment() {
        let cache = make_cache();
        let id = compile_into(&cache, COUNTER_PROGRAM);
        let input = base_input(id);
        let resp = build_response(&input, &cache);
        assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
        assert_eq!(resp.terminated_reason, "completed");
        assert!(!resp.trace.is_empty(), "expected at least one trace sample");
        // Counter should increase monotonically across trace samples.
        let values: Vec<i64> = resp
            .trace
            .iter()
            .filter_map(|e| e.variables.get("Main.Counter"))
            .filter_map(|v| v.as_i64())
            .collect();
        for pair in values.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "Counter should be non-decreasing: {values:?}"
            );
        }
    }

    #[test]
    fn build_response_when_duration_zero_then_completed_empty_trace() {
        let cache = make_cache();
        let id = compile_into(&cache, COUNTER_PROGRAM);
        let mut input = base_input(id);
        input.duration_ms = 0;
        input.limits = Some(LimitOverrides {
            max_duration_ms: Some(0),
            ..Default::default()
        });
        let resp = build_response(&input, &cache);
        assert_eq!(resp.terminated_reason, "duration");
        assert!(resp.trace.is_empty());
    }

    #[test]
    fn build_response_when_container_has_no_tasks_then_ok_false() {
        // Construct a CachedContainer directly to exercise the no-task guard
        // without round-tripping through compile (which synthesizes a
        // default task).
        let cache = make_cache();
        let id = {
            let mut guard = cache.lock().unwrap();
            guard
                .insert(CachedContainer::new(
                    vec![0u8; 64],
                    vec![],
                    vec![],
                    VariableSymbolMap::new(),
                ))
                .unwrap()
        };
        let input = base_input(id);
        let resp = build_response(&input, &cache);
        assert!(!resp.ok);
        assert!(resp
            .diagnostics
            .iter()
            .any(|d| d["message"].as_str().unwrap_or("").contains("no tasks")));
    }

    #[test]
    fn resolve_limits_when_all_within_defaults_then_applies_override() {
        let defaults = EffectiveLimits::DEFAULTS;
        let o = LimitOverrides {
            max_duration_ms: Some(100),
            max_fuel: Some(10_000),
            ..Default::default()
        };
        let r = resolve_limits(defaults, Some(&o)).unwrap();
        assert_eq!(r.max_duration_ms, 100);
        assert_eq!(r.max_fuel, 10_000);
        // Unchanged fields stay at default.
        assert_eq!(r.max_wall_clock_ms, defaults.max_wall_clock_ms);
    }
}
