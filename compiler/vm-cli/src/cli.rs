//! Implements the command line behavior.

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use ironplc_container::debug_format::{build_var_debug_map, format_variable_value, VarDebugInfo};
use ironplc_container::debug_section::var_section;
use ironplc_container::{Container, FunctionId};
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
pub fn run(
    path: &Path,
    dump_vars: Option<&Path>,
    scans: Option<u64>,
    group_by_scope: bool,
) -> Result<(), VmError> {
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
                dump_variables_faulted(&faulted, &container, dump_path, group_by_scope)?;
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
        dump_variables_stopped(&stopped, &container, dump_path, group_by_scope)?;
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

/// Writes a single variable line to the writer.
///
/// Uses debug info when available: `Buzzer: TRUE`
/// Falls back to indexed format: `var[0]: 1`
fn write_variable_line(
    out: &mut dyn Write,
    index: u16,
    raw: u64,
    debug_map: &HashMap<u16, VarDebugInfo>,
) -> Result<(), VmError> {
    let line = if let Some(info) = debug_map.get(&index) {
        format!(
            "{}: {}",
            info.name,
            format_variable_value(raw, info.iec_type_tag)
        )
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

/// A variable row in the scoped dump: its slot metadata plus current value.
struct ScopeRow {
    var_index: u16,
    name: String,
    type_name: String,
    iec_type_tag: u8,
    var_section: u8,
    raw: u64,
}

/// A group of variables in the scoped dump, owned by a single POU. The
/// `[Globals]` group uses [`FunctionId::GLOBAL_SCOPE`].
struct ScopeGroup {
    function_id: FunctionId,
    label: String,
    rows: Vec<ScopeRow>,
}

/// Returns the IEC 61131-3 section name for a `var_section` code.
fn var_section_name(section: u8) -> &'static str {
    match section {
        var_section::VAR => "VAR",
        var_section::VAR_TEMP => "VAR_TEMP",
        var_section::VAR_INPUT => "VAR_INPUT",
        var_section::VAR_OUTPUT => "VAR_OUTPUT",
        var_section::VAR_IN_OUT => "VAR_IN_OUT",
        var_section::VAR_EXTERNAL => "VAR_EXTERNAL",
        var_section::VAR_GLOBAL => "VAR_GLOBAL",
        _ => "VAR",
    }
}

/// Builds the scoped dump groups from the container's debug section and the
/// per-index variable values. `values[i]` is the raw value of variable slot
/// `i`. Returns an empty vector when the container carries no named
/// variables (the caller then falls back to the flat dump).
///
/// Globals (`function_id == GLOBAL_SCOPE`) come first, then each owning POU
/// in ascending `function_id` order. Within a group, rows are ordered by
/// `var_index`, which matches declaration order (params, locals, return).
fn build_scope_groups(container: &Container, values: &[u64]) -> Vec<ScopeGroup> {
    let Some(debug) = &container.debug_section else {
        return Vec::new();
    };
    if debug.var_names.is_empty() {
        return Vec::new();
    }

    // function_id (raw) -> POU name, for group labels.
    let mut func_names: HashMap<u16, &str> = HashMap::new();
    for f in &debug.func_names {
        func_names.insert(f.function_id.raw(), f.name.as_str());
    }

    // Collect rows per function_id (raw), skipping any entry whose slot is
    // out of range for the values we read.
    let mut by_function: HashMap<u16, Vec<ScopeRow>> = HashMap::new();
    for entry in &debug.var_names {
        let idx = entry.var_index.raw();
        let Some(&raw) = values.get(idx as usize) else {
            continue;
        };
        by_function
            .entry(entry.function_id.raw())
            .or_default()
            .push(ScopeRow {
                var_index: idx,
                name: entry.name.clone(),
                type_name: entry.type_name.clone(),
                iec_type_tag: entry.iec_type_tag,
                var_section: entry.var_section,
                raw,
            });
    }

    let global_raw = FunctionId::GLOBAL_SCOPE.raw();
    let mut ordered_ids: Vec<u16> = by_function.keys().copied().collect();
    // Globals first, then ascending by function id.
    ordered_ids.sort_by_key(|&id| (id != global_raw, id));

    let mut groups = Vec::with_capacity(ordered_ids.len());
    for id in ordered_ids {
        let mut rows = by_function.remove(&id).unwrap_or_default();
        rows.sort_by_key(|r| r.var_index);
        let label = if id == global_raw {
            "Globals".to_string()
        } else if let Some(name) = func_names.get(&id) {
            (*name).to_string()
        } else {
            format!("function {id}")
        };
        groups.push(ScopeGroup {
            function_id: FunctionId::new(id),
            label,
            rows,
        });
    }
    groups
}

/// Writes one scoped group: a `[label]` header followed by one indented line
/// per variable. Non-global variables carry an IEC section annotation;
/// globals omit it (the group already implies global scope).
fn write_scoped_group(out: &mut dyn Write, group: &ScopeGroup) -> Result<(), VmError> {
    let is_global = group.function_id == FunctionId::GLOBAL_SCOPE;
    let mut text = format!("[{}]\n", group.label);
    for row in &group.rows {
        let value = format_variable_value(row.raw, row.iec_type_tag);
        let typed = if row.type_name.is_empty() {
            row.name.clone()
        } else {
            format!("{} : {}", row.name, row.type_name)
        };
        if is_global {
            text.push_str(&format!("  {typed} = {value}\n"));
        } else {
            text.push_str(&format!(
                "  {typed} = {value}  ({})\n",
                var_section_name(row.var_section)
            ));
        }
    }
    out.write_all(text.as_bytes()).map_err(|e| {
        VmError::io(
            error::DUMP_WRITE,
            format!("Unable to write dump output: {e}"),
        )
    })
}

/// Writes the variable dump. When `group_by_scope` is set and the container
/// has named variables, emits the scoped layout; otherwise emits the flat,
/// one-line-per-variable format (REQ-VC-005/008/009).
fn write_dump(
    out: &mut dyn Write,
    container: &Container,
    values: &[u64],
    group_by_scope: bool,
) -> Result<(), VmError> {
    if group_by_scope {
        let groups = build_scope_groups(container, values);
        if !groups.is_empty() {
            for group in &groups {
                write_scoped_group(out, group)?;
            }
            return Ok(());
        }
        // No debug info to group by — fall back to the flat dump.
    }

    let debug_map = build_var_debug_map(container);
    for (i, &raw) in values.iter().enumerate() {
        write_variable_line(out, i as u16, raw, &debug_map)?;
    }
    Ok(())
}

fn dump_variables_stopped(
    stopped: &ironplc_vm::VmStopped,
    container: &Container,
    dump_path: &Path,
    group_by_scope: bool,
) -> Result<(), VmError> {
    let num_vars = stopped.num_variables();
    let mut values = Vec::with_capacity(num_vars as usize);
    for i in 0..num_vars {
        values.push(
            stopped
                .read_variable_raw(ironplc_container::VarIndex::new(i))
                .map_err(|e| {
                    VmError::io(error::VAR_READ, format!("Unable to read variable {i}: {e}"))
                })?,
        );
    }
    let mut out = open_dump_output(dump_path)?;
    write_dump(&mut *out, container, &values, group_by_scope)
}

fn dump_variables_faulted(
    faulted: &ironplc_vm::VmFaulted,
    container: &Container,
    dump_path: &Path,
    group_by_scope: bool,
) -> Result<(), VmError> {
    let num_vars = faulted.num_variables();
    let mut values = Vec::with_capacity(num_vars as usize);
    for i in 0..num_vars {
        values.push(
            faulted
                .read_variable_raw(ironplc_container::VarIndex::new(i))
                .map_err(|e| {
                    VmError::io(error::VAR_READ, format!("Unable to read variable {i}: {e}"))
                })?,
        );
    }
    let mut out = open_dump_output(dump_path)?;
    write_dump(&mut *out, container, &values, group_by_scope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_container::debug_section::iec_type_tag;
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
        let debug_map: HashMap<u16, VarDebugInfo> = HashMap::new();
        let mut buf = Vec::new();
        assert!(write_variable_line(&mut buf, 3, 0xFFFF_FFFF, &debug_map).is_ok());
        assert_eq!(std::str::from_utf8(&buf).unwrap(), "var[3]: -1\n");
    }

    /// REQ-VC-008: with debug info, lines use `name: <typed value>`.
    #[spec_test(REQ_VC_008)]
    fn write_variable_line_when_debug_then_named_and_typed() {
        let mut debug_map: HashMap<u16, VarDebugInfo> = HashMap::new();
        debug_map.insert(
            0,
            VarDebugInfo {
                name: "Counter".into(),
                type_name: "DINT".into(),
                iec_type_tag: iec_type_tag::DINT,
            },
        );
        let mut buf = Vec::new();
        assert!(write_variable_line(&mut buf, 0, 42_u64, &debug_map).is_ok());
        assert_eq!(std::str::from_utf8(&buf).unwrap(), "Counter: 42\n");
    }

    /// A Write impl that always fails, used to cover the write-error branch in
    /// `write_variable_line`.
    struct FailingWriter;
    impl Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("simulated write failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    /// REQ-VC-005: dump write failures produce a V6006 error.
    #[spec_test(REQ_VC_005)]
    fn write_variable_line_when_writer_errors_then_v6006() {
        let debug_map: HashMap<u16, VarDebugInfo> = HashMap::new();
        let mut sink = FailingWriter;
        let err = write_variable_line(&mut sink, 0, 0, &debug_map)
            .expect_err("writer failure should surface as VmError");
        assert!(
            err.to_string().starts_with("V6006"),
            "expected V6006 (dump write), got {err}"
        );
    }

    use ironplc_container::debug_section::{FuncNameEntry, VarNameEntry};
    use ironplc_container::{ContainerBuilder, VarIndex};

    fn var(
        index: u16,
        owner: FunctionId,
        section: u8,
        tag: u8,
        name: &str,
        type_name: &str,
    ) -> VarNameEntry {
        VarNameEntry {
            var_index: VarIndex::new(index),
            function_id: owner,
            var_section: section,
            iec_type_tag: tag,
            name: name.into(),
            type_name: type_name.into(),
        }
    }

    fn container_with(vars: Vec<VarNameEntry>, funcs: Vec<FuncNameEntry>) -> Container {
        let mut builder = ContainerBuilder::new();
        for v in vars {
            builder = builder.add_var_name(v);
        }
        for f in funcs {
            builder = builder.add_func_name(f);
        }
        builder.build()
    }

    #[test]
    fn var_section_name_when_known_section_then_returns_iec_name() {
        assert_eq!(var_section_name(var_section::VAR), "VAR");
        assert_eq!(var_section_name(var_section::VAR_INPUT), "VAR_INPUT");
        assert_eq!(var_section_name(var_section::VAR_OUTPUT), "VAR_OUTPUT");
        assert_eq!(var_section_name(var_section::VAR_IN_OUT), "VAR_IN_OUT");
    }

    /// REQ-VC-018: scoped dump groups variables, with the `[Globals]` group
    /// first, then per-POU groups in ascending function-id order.
    #[spec_test(REQ_VC_018)]
    fn build_scope_groups_when_globals_and_functions_then_globals_first_then_by_function_id() {
        let container = container_with(
            vec![
                var(
                    0,
                    FunctionId::GLOBAL_SCOPE,
                    var_section::VAR,
                    iec_type_tag::DINT,
                    "g",
                    "DINT",
                ),
                var(
                    2,
                    FunctionId::new(3),
                    var_section::VAR_INPUT,
                    iec_type_tag::DINT,
                    "n3",
                    "DINT",
                ),
                var(
                    1,
                    FunctionId::new(2),
                    var_section::VAR_INPUT,
                    iec_type_tag::DINT,
                    "n2",
                    "DINT",
                ),
            ],
            vec![
                FuncNameEntry {
                    function_id: FunctionId::new(2),
                    name: "alpha".into(),
                },
                FuncNameEntry {
                    function_id: FunctionId::new(3),
                    name: "beta".into(),
                },
            ],
        );
        let groups = build_scope_groups(&container, &[0, 0, 0]);
        let labels: Vec<&str> = groups.iter().map(|g| g.label.as_str()).collect();
        assert_eq!(labels, vec!["Globals", "alpha", "beta"]);
    }

    #[test]
    fn build_scope_groups_when_rows_then_ordered_by_var_index_within_group() {
        let container = container_with(
            vec![
                var(
                    2,
                    FunctionId::new(2),
                    var_section::VAR,
                    iec_type_tag::DINT,
                    "c",
                    "DINT",
                ),
                var(
                    0,
                    FunctionId::new(2),
                    var_section::VAR_INPUT,
                    iec_type_tag::DINT,
                    "a",
                    "DINT",
                ),
                var(
                    1,
                    FunctionId::new(2),
                    var_section::VAR,
                    iec_type_tag::DINT,
                    "b",
                    "DINT",
                ),
            ],
            vec![FuncNameEntry {
                function_id: FunctionId::new(2),
                name: "f".into(),
            }],
        );
        let groups = build_scope_groups(&container, &[0, 0, 0]);
        assert_eq!(groups.len(), 1);
        let names: Vec<&str> = groups[0].rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    /// REQ-VC-019: non-global lines carry an IEC section annotation; the
    /// `[Globals]` group omits it.
    #[spec_test(REQ_VC_019)]
    fn write_scoped_group_when_function_local_then_includes_section_annotation() {
        let global = ScopeGroup {
            function_id: FunctionId::GLOBAL_SCOPE,
            label: "Globals".into(),
            rows: vec![ScopeRow {
                var_index: 0,
                name: "counter".into(),
                type_name: "DINT".into(),
                iec_type_tag: iec_type_tag::DINT,
                var_section: var_section::VAR_GLOBAL,
                raw: 3,
            }],
        };
        let mut buf = Vec::new();
        assert!(write_scoped_group(&mut buf, &global).is_ok());
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "[Globals]\n  counter : DINT = 3\n"
        );

        let func = ScopeGroup {
            function_id: FunctionId::new(2),
            label: "add_offset".into(),
            rows: vec![ScopeRow {
                var_index: 5,
                name: "n".into(),
                type_name: "DINT".into(),
                iec_type_tag: iec_type_tag::DINT,
                var_section: var_section::VAR_INPUT,
                raw: 7,
            }],
        };
        let mut buf = Vec::new();
        assert!(write_scoped_group(&mut buf, &func).is_ok());
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "[add_offset]\n  n : DINT = 7  (VAR_INPUT)\n"
        );
    }

    /// REQ-VC-020: with no debug section, `--group-by-scope` falls back to the
    /// flat dump format.
    #[spec_test(REQ_VC_020)]
    fn write_dump_when_no_debug_section_then_falls_back_to_flat() {
        let container = ContainerBuilder::new().build();
        let mut buf = Vec::new();
        assert!(write_dump(&mut buf, &container, &[10, 42], true).is_ok());
        assert_eq!(String::from_utf8(buf).unwrap(), "var[0]: 10\nvar[1]: 42\n");
    }

    #[test]
    fn write_dump_when_group_by_scope_and_debug_then_emits_grouped_layout() {
        let container = container_with(
            vec![
                var(
                    0,
                    FunctionId::GLOBAL_SCOPE,
                    var_section::VAR,
                    iec_type_tag::DINT,
                    "counter",
                    "DINT",
                ),
                var(
                    1,
                    FunctionId::new(2),
                    var_section::VAR_INPUT,
                    iec_type_tag::DINT,
                    "step",
                    "DINT",
                ),
            ],
            vec![FuncNameEntry {
                function_id: FunctionId::new(2),
                name: "acc".into(),
            }],
        );
        let mut buf = Vec::new();
        assert!(write_dump(&mut buf, &container, &[3, 9], true).is_ok());
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "[Globals]\n  counter : DINT = 3\n[acc]\n  step : DINT = 9  (VAR_INPUT)\n"
        );
    }
}
