//! Implements the command line behavior.

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use ironplc_container::debug_section::iec_type_tag;
use ironplc_container::Container;
use ironplc_vm::{Vm, VmBuffers};
use serde_json::json;

use crate::error::{self, VmError};

const BUILD_OPT_LEVEL: &str = env!("BUILD_OPT_LEVEL");

/// Loads a container file and executes it.
///
/// When `scans` is `Some(n)`, runs exactly `n` scheduling rounds.
/// When `scans` is `None`, runs continuously until Ctrl+C.
/// When `dump_vars` is `Some(path)`, writes variable values after stopping.
/// A path of "-" writes to stdout; any other path writes to a file.
pub fn run(path: &Path, dump_vars: Option<&Path>, scans: Option<u64>) -> Result<(), VmError> {
    let mut file = File::open(path).map_err(|e| {
        VmError::io(
            error::FILE_OPEN,
            format!("Unable to open {}: {}", path.display(), e),
        )
    })?;

    let container = ironplc_container::Container::read_from(&mut file).map_err(|e| {
        VmError::io(
            error::CONTAINER_READ,
            format!("Unable to read container {}: {e}", path.display()),
        )
    })?;

    let mut bufs = VmBuffers::from_container(&container);

    let mut running = Vm::new()
        .load(&container, &mut bufs)
        .start()
        .map_err(|ctx| VmError::from_trap(&ctx.trap, ctx.task_id, ctx.instance_id))?;

    // Install signal handler for clean shutdown
    let stop_flag = Arc::new(AtomicBool::new(false));
    let handle = stop_flag.clone();
    ctrlc::set_handler(move || handle.store(true, Ordering::Relaxed)).map_err(|e| {
        VmError::io(
            error::SIGNAL_HANDLER,
            format!("Failed to set signal handler: {e}"),
        )
    })?;

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
            let err = VmError::from_trap(faulted.trap(), faulted.task_id(), faulted.instance_id());
            if let Some(dump_path) = dump_vars {
                dump_variables_faulted(&faulted, &container, dump_path)?;
            }
            return Err(err);
        }
        rounds += 1;
    }

    let stopped = running.stop();

    if let Some(dump_path) = dump_vars {
        dump_variables_stopped(&stopped, &container, dump_path)?;
    }

    Ok(())
}

/// Benchmarks a bytecode container by running it for `cycles` scan rounds,
/// preceded by `warmup` unmeasured rounds, then prints JSON timing statistics.
pub fn benchmark(path: &Path, cycles: u64, warmup: u64) -> Result<(), VmError> {
    let mut file = File::open(path).map_err(|e| {
        VmError::io(
            error::FILE_OPEN,
            format!("Unable to open {}: {}", path.display(), e),
        )
    })?;

    let container = ironplc_container::Container::read_from(&mut file).map_err(|e| {
        VmError::io(
            error::CONTAINER_READ,
            format!("Unable to read container {}: {e}", path.display()),
        )
    })?;

    let mut bufs = VmBuffers::from_container(&container);

    let mut running = Vm::new()
        .load(&container, &mut bufs)
        .start()
        .map_err(|ctx| VmError::from_trap(&ctx.trap, ctx.task_id, ctx.instance_id))?;

    let clock = Instant::now();

    // Warmup phase (unmeasured)
    for _ in 0..warmup {
        let current_us = clock.elapsed().as_micros() as u64;
        if let Err(ctx) = running.run_round(current_us) {
            let faulted = running.fault(ctx);
            return Err(VmError::from_trap(
                faulted.trap(),
                faulted.task_id(),
                faulted.instance_id(),
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
            return Err(VmError::from_trap(
                faulted.trap(),
                faulted.task_id(),
                faulted.instance_id(),
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
    let tasks_json: Vec<serde_json::Value> = bufs
        .tasks
        .iter()
        .map(|ts| {
            let mut task = json!({
                "task_id": ts.task_id.raw(),
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

/// Builds a lookup from variable index to (name, iec_type_tag) from the container's debug section.
fn build_var_debug_map(container: &Container) -> HashMap<u16, (&str, u8)> {
    let mut map = HashMap::new();
    if let Some(ref debug) = container.debug_section {
        for entry in &debug.var_names {
            map.insert(entry.var_index.raw(), (entry.name.as_str(), entry.iec_type_tag));
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
        iec_type_tag::ULINT => format!("{raw}"),
        iec_type_tag::REAL => format!("{}", f32::from_bits(raw as u32)),
        iec_type_tag::LREAL => format!("{}", f64::from_bits(raw)),
        iec_type_tag::BYTE => format!("16#{:02X}", raw as u8),
        iec_type_tag::WORD => format!("16#{:04X}", raw as u16),
        iec_type_tag::DWORD => format!("16#{:08X}", raw as u32),
        iec_type_tag::LWORD => format!("16#{:016X}", raw),
        iec_type_tag::TIME => format!("T#{}ms", raw as i32),
        iec_type_tag::LTIME => format!("LTIME#{}ms", raw as i64),
        _ => format!("{}", raw as i32),
    }
}

/// Writes a single variable line to the writer.
///
/// Uses debug info when available: `Buzzer: TRUE`
/// Falls back to indexed format: `var[0]: 1`
fn write_variable_line(
    out: &mut dyn Write,
    index: u16,
    raw: u64,
    debug_map: &HashMap<u16, (&str, u8)>,
) -> Result<(), VmError> {
    let line = if let Some(&(name, tag)) = debug_map.get(&index) {
        format!("{}: {}", name, format_variable_value(raw, tag))
    } else {
        format!("var[{index}]: {}", raw as i32)
    };
    writeln!(out, "{line}").map_err(|e| {
        VmError::io(error::DUMP_WRITE, format!("Unable to write dump output: {e}"))
    })
}

/// Opens the dump output destination: stdout for "-", otherwise a file.
fn open_dump_output(dump_path: &Path) -> Result<Box<dyn Write>, VmError> {
    if dump_path == Path::new("-") {
        Ok(Box::new(std::io::stdout().lock()))
    } else {
        let file = File::create(dump_path).map_err(|e| {
            VmError::io(
                error::DUMP_CREATE,
                format!("Unable to create dump file {}: {e}", dump_path.display()),
            )
        })?;
        Ok(Box::new(file))
    }
}

fn dump_variables_stopped(
    stopped: &ironplc_vm::VmStopped,
    container: &Container,
    dump_path: &Path,
) -> Result<(), VmError> {
    let debug_map = build_var_debug_map(container);
    let num_vars = stopped.num_variables();
    let mut out = open_dump_output(dump_path)?;
    for i in 0..num_vars {
        let raw = stopped
            .read_variable_raw(ironplc_container::VarIndex::new(i))
            .map_err(|e| {
                VmError::io(error::VAR_READ, format!("Unable to read variable {i}: {e}"))
            })?;
        write_variable_line(&mut *out, i, raw, &debug_map)?;
    }
    Ok(())
}

fn dump_variables_faulted(
    faulted: &ironplc_vm::VmFaulted,
    container: &Container,
    dump_path: &Path,
) -> Result<(), VmError> {
    let debug_map = build_var_debug_map(container);
    let num_vars = faulted.num_variables();
    let mut out = open_dump_output(dump_path)?;
    for i in 0..num_vars {
        let raw = faulted
            .read_variable_raw(ironplc_container::VarIndex::new(i))
            .map_err(|e| {
                VmError::io(error::VAR_READ, format!("Unable to read variable {i}: {e}"))
            })?;
        write_variable_line(&mut *out, i, raw, &debug_map)?;
    }
    Ok(())
}
