//! Tests of the `run` MCP tool: input validation, the trace set, and the
//! simulated run (`specs/design/mcp-server.md` §`run`).

use super::*;
use crate::cache::{CachedContainer, ContainerCache};
use crate::tools::test_support::{ed2_options, source, COUNTER_PROGRAM, COUNTER_PROGRAM_WITH_TASK};

fn make_cache() -> Mutex<ContainerCache> {
    Mutex::new(ContainerCache::new(64, 64 * 1024 * 1024))
}

/// Compile a program via the `compile` tool to populate the cache with
/// a fresh container and symbol map; returns the container_id.
fn compile_into(cache: &Mutex<ContainerCache>, src: &str) -> String {
    let resp = crate::tools::compile::build_response(&source(src), &ed2_options(), false, cache);
    assert!(resp.ok, "compile failed: {:?}", resp.diagnostics);
    resp.container_id.unwrap()
}

fn base_input(container_id: String) -> RunInput {
    RunInput {
        container_id: Some(container_id),
        container_base64: None,
        duration_ms: 500,
        freewheeling_interval_ms: None,
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
        freewheeling_interval_ms: None,
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
        freewheeling_interval_ms: None,
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
        freewheeling_interval_ms: None,
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

/// A freewheeling container with no cycle time cannot run under simulated
/// time. The `run` tool rejects it before reaching the VM; the runner keeps
/// the invariant rather than trusting its caller.
#[test]
fn execute_when_freewheeling_and_no_interval_then_errors() {
    let cache = make_cache();
    let id = compile_into(&cache, COUNTER_PROGRAM);
    let snapshot = {
        let mut guard = cache.lock().unwrap();
        guard.get(&id).unwrap().clone_for_run()
    };

    let result = runner::execute(&snapshot, &[], 500, None, EffectiveLimits::DEFAULTS);

    assert!(result.is_err());
}

#[test]
fn build_response_when_freewheeling_program_then_runs_for_full_duration() {
    let cache = make_cache();
    let id = compile_into(&cache, COUNTER_PROGRAM);
    let mut input = base_input(id);
    input.freewheeling_interval_ms = Some(100.0);

    let resp = build_response(&input, &cache);

    // 500 ms at the supplied 100 ms cycle time. Before #1413 this stopped
    // after a single cycle and still reported `completed`.
    assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
    assert_eq!(resp.summary.completed_cycles["Main"], Value::from(5u64));
    assert_eq!(resp.trace.len(), 5);
}

#[test]
fn build_response_when_freewheeling_program_then_summary_reports_interval_used() {
    let cache = make_cache();
    let id = compile_into(&cache, COUNTER_PROGRAM);
    let mut input = base_input(id);
    input.freewheeling_interval_ms = Some(100.0);

    let resp = build_response(&input, &cache);

    assert_eq!(resp.summary.interval_ms, 100.0);
}

#[test]
fn build_response_when_freewheeling_program_and_no_interval_then_ok_false() {
    // The server does not pick a scan time on the caller's behalf: a trace
    // built on a rate the agent never chose reads as fact.
    let cache = make_cache();
    let id = compile_into(&cache, COUNTER_PROGRAM);
    let input = base_input(id);

    let resp = build_response(&input, &cache);

    assert!(!resp.ok);
    assert!(resp.diagnostics.iter().any(|d| {
        let msg = d["message"].as_str().unwrap_or("");
        msg.contains("freewheeling_interval_ms") && msg.contains("Main")
    }));
}

#[test]
fn build_response_when_cyclic_program_and_no_interval_then_ok_true() {
    // Nothing to supply: every task declares its own INTERVAL.
    let cache = make_cache();
    let id = compile_into(&cache, COUNTER_PROGRAM_WITH_TASK);
    let input = base_input(id);

    let resp = build_response(&input, &cache);

    assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
    assert_eq!(resp.summary.interval_ms, 100.0);
}

#[test]
fn build_response_when_freewheeling_interval_supplied_then_cycle_count_follows_it() {
    let cache = make_cache();
    let id = compile_into(&cache, COUNTER_PROGRAM);
    let mut input = base_input(id);
    input.freewheeling_interval_ms = Some(50.0);

    let resp = build_response(&input, &cache);

    assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
    assert_eq!(resp.summary.completed_cycles["Main"], Value::from(10u64));
    assert_eq!(resp.summary.interval_ms, 50.0);
}

#[test]
fn build_response_when_freewheeling_interval_subms_then_honored_at_microsecond_granularity() {
    let cache = make_cache();
    let id = compile_into(&cache, COUNTER_PROGRAM);
    let mut input = base_input(id);
    input.duration_ms = 1;
    input.freewheeling_interval_ms = Some(0.25);

    let resp = build_response(&input, &cache);

    assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
    assert_eq!(resp.summary.completed_cycles["Main"], Value::from(4u64));
}

#[test]
fn build_response_when_cyclic_program_then_declared_interval_wins_and_nothing_assumed() {
    let cache = make_cache();
    let id = compile_into(&cache, COUNTER_PROGRAM_WITH_TASK);
    let mut input = base_input(id);
    // Supplied but inapplicable: every task declares an INTERVAL.
    input.freewheeling_interval_ms = Some(1.0);

    let resp = build_response(&input, &cache);

    assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
    // 500 ms at the program's own 100 ms INTERVAL, not the supplied 1 ms.
    assert_eq!(resp.summary.completed_cycles["plc_task"], Value::from(5u64));
    // The program's own INTERVAL is what the run used, and what it reports.
    assert_eq!(resp.summary.interval_ms, 100.0);
}

#[rstest::rstest]
#[case(0.0)]
#[case(-1.0)]
#[case(f64::NAN)]
#[case(f64::INFINITY)]
#[case(3_600_001.0)]
fn build_response_when_freewheeling_interval_out_of_range_then_ok_false(#[case] ms: f64) {
    let cache = make_cache();
    let id = compile_into(&cache, COUNTER_PROGRAM);
    let mut input = base_input(id);
    input.freewheeling_interval_ms = Some(ms);

    let resp = build_response(&input, &cache);

    assert!(!resp.ok);
    assert!(resp.diagnostics.iter().any(|d| d["message"]
        .as_str()
        .unwrap_or("")
        .contains("freewheeling_interval_ms")));
}

#[test]
fn build_response_when_stimuli_supplied_then_phase11_guard_fires() {
    let cache = make_cache();
    let id = compile_into(&cache, COUNTER_PROGRAM_WITH_TASK);
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
    let id = compile_into(&cache, COUNTER_PROGRAM_WITH_TASK);
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
    let id = compile_into(&cache, COUNTER_PROGRAM_WITH_TASK);
    let mut input = base_input(id);
    input.tasks = Some(vec!["plc_task".into()]);
    let resp = build_response(&input, &cache);
    assert!(!resp.ok);
}

#[test]
fn build_response_when_limits_loosen_duration_then_ok_false() {
    let cache = make_cache();
    let id = compile_into(&cache, COUNTER_PROGRAM_WITH_TASK);
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
    let id = compile_into(&cache, COUNTER_PROGRAM_WITH_TASK);
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
    let id = compile_into(&cache, COUNTER_PROGRAM_WITH_TASK);
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
    let id = compile_into(&cache, COUNTER_PROGRAM_WITH_TASK);
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
    let id = compile_into(&cache, COUNTER_PROGRAM_WITH_TASK);
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
    let id = compile_into(&cache, COUNTER_PROGRAM_WITH_TASK);
    let mut input = base_input(id);
    input.duration_ms = 0;
    input.limits = Some(LimitOverrides {
        max_duration_ms: Some(0),
        ..Default::default()
    });
    let resp = build_response(&input, &cache);
    // Zero simulated time requested and zero delivered: a clean finish, not
    // a limit violation.
    assert_eq!(resp.terminated_reason, "completed");
    assert!(resp.trace.is_empty());
}

#[test]
fn build_response_when_duration_exceeds_server_limit_then_terminates_on_duration() {
    let cache = make_cache();
    let id = compile_into(&cache, COUNTER_PROGRAM_WITH_TASK);
    let mut input = base_input(id);
    input.duration_ms = 500;
    input.limits = Some(LimitOverrides {
        max_duration_ms: Some(200),
        ..Default::default()
    });
    let resp = build_response(&input, &cache);
    assert!(!resp.ok);
    assert_eq!(resp.terminated_reason, "duration");
}

#[test]
fn build_response_when_cyclic_task_then_cycle_count_follows_interval() {
    // COUNTER_PROGRAM_WITH_TASK declares INTERVAL := T#100ms and the run
    // asks for 500 ms, so the task releases at 0, 100, 200, 300 and 400 ms.
    let cache = make_cache();
    let id = compile_into(&cache, COUNTER_PROGRAM_WITH_TASK);
    let resp = build_response(&base_input(id), &cache);

    assert!(resp.ok, "diagnostics: {:?}", resp.diagnostics);
    assert_eq!(resp.summary.completed_cycles["plc_task"], Value::from(5u64));
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
