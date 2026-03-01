//! Implements the command line behavior.

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use ironplc_vm::{ProgramInstanceState, Slot, TaskState, Vm};
use serde_json::json;

const BUILD_OPT_LEVEL: &str = env!("BUILD_OPT_LEVEL");

/// Loads a container file and executes it.
///
/// When `scans` is `Some(n)`, runs exactly `n` scheduling rounds.
/// When `scans` is `None`, runs continuously until Ctrl+C.
/// When `dump_vars` is `Some(path)`, writes all variable values after stopping.
pub fn run(path: &Path, dump_vars: Option<&Path>, scans: Option<u64>) -> Result<(), String> {
    let mut file =
        File::open(path).map_err(|e| format!("Unable to open {}: {}", path.display(), e))?;

    let container = ironplc_container::Container::read_from(&mut file)
        .map_err(|e| format!("Unable to read container {}: {e}", path.display()))?;

    // Allocate buffers from header sizes
    let h = &container.header;
    let mut stack_buf = vec![Slot::default(); h.max_stack_depth as usize];
    let mut var_buf = vec![Slot::default(); h.num_variables as usize];
    let mut task_states = vec![TaskState::default(); container.task_table.tasks.len()];
    let mut program_instances =
        vec![ProgramInstanceState::default(); container.task_table.programs.len()];
    let mut ready_buf = vec![0usize; container.task_table.tasks.len()];

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

    // Install signal handler for clean shutdown
    let stop_flag = Arc::new(AtomicBool::new(false));
    let handle = stop_flag.clone();
    ctrlc::set_handler(move || handle.store(true, Ordering::Relaxed))
        .map_err(|e| format!("Failed to set signal handler: {e}"))?;

    let start = Instant::now();
    let mut rounds = 0u64;
    loop {
        if stop_flag.load(Ordering::Relaxed) {
            running.request_stop();
        }
        if running.stop_requested() {
            break;
        }
        if let Some(max) = scans {
            if rounds >= max {
                break;
            }
        }

        let current_us = start.elapsed().as_micros() as u64;
        if let Err(ctx) = running.run_round(current_us) {
            let faulted = running.fault(ctx);
            let err_msg = format!(
                "VM trap: {} (task {}, instance {})",
                faulted.trap(),
                faulted.task_id(),
                faulted.instance_id()
            );
            if let Some(dump_path) = dump_vars {
                dump_variables_faulted(&faulted, dump_path)?;
            }
            return Err(err_msg);
        }
        rounds += 1;
    }

    let stopped = running.stop();

    if let Some(dump_path) = dump_vars {
        dump_variables_stopped(&stopped, dump_path)?;
    }

    Ok(())
}

/// Benchmarks a bytecode container by running it for `cycles` scan rounds,
/// preceded by `warmup` unmeasured rounds, then prints JSON timing statistics.
pub fn benchmark(path: &Path, cycles: u64, warmup: u64) -> Result<(), String> {
    let mut file =
        File::open(path).map_err(|e| format!("Unable to open {}: {}", path.display(), e))?;

    let container = ironplc_container::Container::read_from(&mut file)
        .map_err(|e| format!("Unable to read container {}: {e}", path.display()))?;

    let h = &container.header;
    let mut stack_buf = vec![Slot::default(); h.max_stack_depth as usize];
    let mut var_buf = vec![Slot::default(); h.num_variables as usize];
    let mut task_states = vec![TaskState::default(); container.task_table.tasks.len()];
    let mut program_instances =
        vec![ProgramInstanceState::default(); container.task_table.programs.len()];
    let mut ready_buf = vec![0usize; container.task_table.tasks.len()];

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

    let clock = Instant::now();

    // Warmup phase (unmeasured)
    for _ in 0..warmup {
        let current_us = clock.elapsed().as_micros() as u64;
        if let Err(ctx) = running.run_round(current_us) {
            let faulted = running.fault(ctx);
            return Err(format!(
                "VM trap during warmup: {} (task {}, instance {})",
                faulted.trap(),
                faulted.task_id(),
                faulted.instance_id()
            ));
        }
    }

    // Measured phase — record each round's duration
    let mut durations_us = Vec::with_capacity(cycles as usize);
    for _ in 0..cycles {
        let round_start = Instant::now();
        let current_us = clock.elapsed().as_micros() as u64;
        if let Err(ctx) = running.run_round(current_us) {
            let faulted = running.fault(ctx);
            return Err(format!(
                "VM trap during benchmark: {} (task {}, instance {})",
                faulted.trap(),
                faulted.task_id(),
                faulted.instance_id()
            ));
        }
        let elapsed = round_start.elapsed().as_nanos() as f64 / 1000.0;
        durations_us.push(elapsed);
    }

    running.stop();

    // Compute statistics
    durations_us.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let count = durations_us.len() as f64;
    let mean = durations_us.iter().sum::<f64>() / count;
    let variance = durations_us.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / count;
    let stddev = variance.sqrt();
    let max = durations_us.last().copied().unwrap_or(0.0);
    let p99 = percentile(&durations_us, 99.0);

    let program_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();

    // Build per-task metadata from task_states (borrow released by stop()).
    let tasks_json: Vec<serde_json::Value> = task_states
        .iter()
        .map(|ts| {
            let mut task = json!({
                "task_id": ts.task_id,
                "task_type": ts.task_type.as_str(),
                "interval_us": ts.interval_us,
                "scan_count": ts.scan_count,
                "overruns": ts.overrun_count,
            });
            if ts.task_type == ironplc_container::TaskType::Cyclic && ts.interval_us > 0 {
                let interval = ts.interval_us as f64;
                task["budget_pct"] = json!({
                    "mean": round3(mean / interval * 100.0),
                    "p99": round3(p99 / interval * 100.0),
                    "max": round3(max / interval * 100.0),
                });
            }
            task
        })
        .collect();

    let result = json!({
        "program": program_name,
        "opt_level": BUILD_OPT_LEVEL,
        "cycles": cycles,
        "warmup": warmup,
        "scan_us": {
            "mean": round3(mean),
            "stddev": round3(stddev),
            "p99": round3(p99),
            "max": round3(max),
        },
        "tasks": tasks_json,
    });

    println!(
        "{}",
        serde_json::to_string_pretty(&result).unwrap_or_default()
    );
    Ok(())
}

/// Returns the value at the given percentile (0–100) using nearest-rank.
fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (pct / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.saturating_sub(1).min(sorted.len() - 1)]
}

/// Rounds to 3 decimal places.
fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn dump_variables_stopped(stopped: &ironplc_vm::VmStopped, dump_path: &Path) -> Result<(), String> {
    let num_vars = stopped.num_variables();
    let mut out = File::create(dump_path)
        .map_err(|e| format!("Unable to create dump file {}: {e}", dump_path.display()))?;
    for i in 0..num_vars {
        let value = stopped
            .read_variable(i)
            .map_err(|e| format!("Unable to read variable {i}: {e}"))?;
        writeln!(out, "var[{i}]: {value}")
            .map_err(|e| format!("Unable to write dump file: {e}"))?;
    }
    Ok(())
}

fn dump_variables_faulted(faulted: &ironplc_vm::VmFaulted, dump_path: &Path) -> Result<(), String> {
    let num_vars = faulted.num_variables();
    let mut out = File::create(dump_path)
        .map_err(|e| format!("Unable to create dump file {}: {e}", dump_path.display()))?;
    for i in 0..num_vars {
        let value = faulted
            .read_variable(i)
            .map_err(|e| format!("Unable to read variable {i}: {e}"))?;
        writeln!(out, "var[{i}]: {value}")
            .map_err(|e| format!("Unable to write dump file: {e}"))?;
    }
    Ok(())
}
