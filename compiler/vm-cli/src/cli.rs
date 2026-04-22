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

        // Sleep until the next cyclic task is due to avoid burning CPU.
        // Freewheeling-only programs (next_due_us returns None) run every round.
        if let Some(due_us) = running.next_due_us() {
            let now_us = start.elapsed().as_micros() as u64;
            let sleep_us = due_us.saturating_sub(now_us);
            if sleep_us > 0 {
                std::thread::sleep(std::time::Duration::from_micros(sleep_us));
            }
        }
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
            map.insert(
                entry.var_index.raw(),
                (entry.name.as_str(), entry.iec_type_tag),
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
        VmError::io(
            error::DUMP_WRITE,
            format!("Unable to write dump output: {e}"),
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_container::debug_section::{iec_type_tag, VarNameEntry};
    use ironplc_container::{ContainerBuilder, FunctionId, VarIndex};
    use spec_test_macro::spec_test;

    /// REQ-VC-013: percentile of an empty sample returns 0 so benchmark can
    /// still emit a valid JSON stats object when `--cycles 0` is supplied.
    #[spec_test(REQ_VC_013)]
    fn percentile_when_empty_then_zero() {
        let empty: Vec<f64> = Vec::new();
        assert_eq!(percentile(&empty, 99.0), 0.0);
        assert_eq!(percentile(&empty, 50.0), 0.0);
    }

    #[test]
    fn percentile_when_single_value_then_returns_that_value() {
        let sorted = vec![42.0];
        assert_eq!(percentile(&sorted, 50.0), 42.0);
        assert_eq!(percentile(&sorted, 99.0), 42.0);
    }

    #[test]
    fn percentile_when_many_values_then_nearest_rank() {
        // 10 values 1.0..=10.0; p50 → rank 5 → 5.0, p100 → rank 10 → 10.0
        let sorted: Vec<f64> = (1..=10).map(|v| v as f64).collect();
        assert_eq!(percentile(&sorted, 50.0), 5.0);
        assert_eq!(percentile(&sorted, 100.0), 10.0);
    }

    #[test]
    fn round3_rounds_to_three_decimals() {
        assert_eq!(round3(1.23456), 1.235);
        assert_eq!(round3(0.0005), 0.001);
        assert_eq!(round3(0.0004), 0.0);
    }

    /// REQ-VC-009: BOOL formats as TRUE/FALSE.
    #[spec_test(REQ_VC_009)]
    fn format_variable_value_when_bool_then_true_or_false() {
        assert_eq!(format_variable_value(1, iec_type_tag::BOOL), "TRUE");
        assert_eq!(format_variable_value(0, iec_type_tag::BOOL), "FALSE");
        // Non-zero lower i32 bits → TRUE.
        assert_eq!(
            format_variable_value(0xFFFF_FFFF, iec_type_tag::BOOL),
            "TRUE"
        );
    }

    /// REQ-VC-009: signed IEC integer types decode as signed decimals at their widths.
    #[spec_test(REQ_VC_009)]
    fn format_variable_value_when_signed_int_then_signed_decimal() {
        assert_eq!(
            format_variable_value(0xFF_u64, iec_type_tag::SINT),
            "-1",
            "SINT should sign-extend from 8 bits"
        );
        assert_eq!(
            format_variable_value(0xFFFF_u64, iec_type_tag::INT),
            "-1",
            "INT should sign-extend from 16 bits"
        );
        assert_eq!(
            format_variable_value(0xFFFF_FFFF_u64, iec_type_tag::DINT),
            "-1",
            "DINT should sign-extend from 32 bits"
        );
        assert_eq!(
            format_variable_value(0xFFFF_FFFF_FFFF_FFFF_u64, iec_type_tag::LINT),
            "-1",
            "LINT should interpret as signed 64-bit"
        );
    }

    /// REQ-VC-009: unsigned IEC integer types decode as unsigned decimals at their widths.
    #[spec_test(REQ_VC_009)]
    fn format_variable_value_when_unsigned_int_then_unsigned_decimal() {
        assert_eq!(format_variable_value(0xFF_u64, iec_type_tag::USINT), "255");
        assert_eq!(
            format_variable_value(0xFFFF_u64, iec_type_tag::UINT),
            "65535"
        );
        assert_eq!(
            format_variable_value(0xFFFF_FFFF_u64, iec_type_tag::UDINT),
            "4294967295"
        );
        assert_eq!(
            format_variable_value(0xFFFF_FFFF_FFFF_FFFF_u64, iec_type_tag::ULINT),
            "18446744073709551615"
        );
    }

    /// REQ-VC-009: REAL and LREAL reinterpret the raw bits as float/double.
    #[spec_test(REQ_VC_009)]
    fn format_variable_value_when_real_then_float_decimal() {
        let raw32 = 1.5_f32.to_bits() as u64;
        assert_eq!(format_variable_value(raw32, iec_type_tag::REAL), "1.5");
        let raw64 = 2.25_f64.to_bits();
        assert_eq!(format_variable_value(raw64, iec_type_tag::LREAL), "2.25");
    }

    /// REQ-VC-009: BYTE/WORD/DWORD/LWORD render in IEC `16#...` hex form at their widths.
    #[spec_test(REQ_VC_009)]
    fn format_variable_value_when_bit_string_then_iec_hex() {
        assert_eq!(format_variable_value(0xAB, iec_type_tag::BYTE), "16#AB");
        assert_eq!(format_variable_value(0x0F, iec_type_tag::BYTE), "16#0F");
        assert_eq!(format_variable_value(0xABCD, iec_type_tag::WORD), "16#ABCD");
        assert_eq!(
            format_variable_value(0xDEAD_BEEF, iec_type_tag::DWORD),
            "16#DEADBEEF"
        );
        assert_eq!(
            format_variable_value(0x0000_0000_DEAD_BEEF, iec_type_tag::LWORD),
            "16#00000000DEADBEEF"
        );
    }

    /// REQ-VC-009: TIME/LTIME render with `T#` / `LTIME#` prefixes.
    #[spec_test(REQ_VC_009)]
    fn format_variable_value_when_time_then_iec_duration() {
        assert_eq!(format_variable_value(250, iec_type_tag::TIME), "T#250ms");
        assert_eq!(
            format_variable_value(0xFFFF_FFFF, iec_type_tag::TIME),
            "T#-1ms"
        );
        assert_eq!(
            format_variable_value(10_000, iec_type_tag::LTIME),
            "LTIME#10000ms"
        );
    }

    /// REQ-VC-009: an unknown tag falls back to signed i32 decimal.
    #[spec_test(REQ_VC_009)]
    fn format_variable_value_when_unknown_tag_then_signed_i32_fallback() {
        assert_eq!(format_variable_value(42, iec_type_tag::OTHER), "42");
        assert_eq!(
            format_variable_value(0xFFFF_FFFF, iec_type_tag::OTHER),
            "-1"
        );
    }

    /// REQ-VC-008: without debug info, lines use the `var[i]: <i32>` fallback.
    #[spec_test(REQ_VC_008)]
    fn write_variable_line_when_no_debug_then_indexed_format() {
        let debug_map: HashMap<u16, (&str, u8)> = HashMap::new();
        let mut buf = Vec::new();
        assert!(write_variable_line(&mut buf, 3, 0xFFFF_FFFF, &debug_map).is_ok());
        assert_eq!(std::str::from_utf8(&buf).unwrap(), "var[3]: -1\n");
    }

    /// REQ-VC-008: with debug info, lines use `name: <typed value>`.
    #[spec_test(REQ_VC_008)]
    fn write_variable_line_when_debug_then_named_and_typed() {
        let mut debug_map: HashMap<u16, (&str, u8)> = HashMap::new();
        debug_map.insert(0, ("Counter", iec_type_tag::DINT));
        let mut buf = Vec::new();
        assert!(write_variable_line(&mut buf, 0, 42_u64, &debug_map).is_ok());
        assert_eq!(std::str::from_utf8(&buf).unwrap(), "Counter: 42\n");
    }

    /// A Write impl that always fails, used to cover the write-error branch in
    /// `write_variable_line`.
    struct FailingWriter;
    impl Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "simulated write failure",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// REQ-VC-005: dump write failures produce a V6006 error.
    #[spec_test(REQ_VC_005)]
    fn write_variable_line_when_writer_errors_then_v6006() {
        let debug_map: HashMap<u16, (&str, u8)> = HashMap::new();
        let mut sink = FailingWriter;
        let err = write_variable_line(&mut sink, 0, 0, &debug_map)
            .expect_err("writer failure should surface as VmError");
        assert!(
            err.to_string().starts_with("V6006"),
            "expected V6006 (dump write), got {err}"
        );
    }

    #[test]
    fn build_var_debug_map_when_no_debug_section_then_empty() {
        // A container without any debug entries has no debug section.
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_function(FunctionId::new(0), &[0xB5], 0, 1, 0)
            .build();
        let map = build_var_debug_map(&container);
        assert!(map.is_empty());
    }

    #[test]
    fn build_var_debug_map_when_debug_present_then_maps_by_index() {
        let container = ContainerBuilder::new()
            .num_variables(2)
            .add_function(FunctionId::new(0), &[0xB5], 0, 2, 0)
            .add_var_name(VarNameEntry {
                var_index: VarIndex::new(0),
                function_id: FunctionId::GLOBAL_SCOPE,
                var_section: 0,
                iec_type_tag: iec_type_tag::DINT,
                name: "alpha".into(),
                type_name: "DINT".into(),
            })
            .add_var_name(VarNameEntry {
                var_index: VarIndex::new(1),
                function_id: FunctionId::GLOBAL_SCOPE,
                var_section: 0,
                iec_type_tag: iec_type_tag::BOOL,
                name: "beta".into(),
                type_name: "BOOL".into(),
            })
            .build();
        let map = build_var_debug_map(&container);
        assert_eq!(map.len(), 2);
        assert_eq!(map.get(&0), Some(&("alpha", iec_type_tag::DINT)));
        assert_eq!(map.get(&1), Some(&("beta", iec_type_tag::BOOL)));
    }
}
