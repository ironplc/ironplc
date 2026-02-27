//! Implements the command line behavior.

use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use ironplc_vm::{ProgramInstanceState, Slot, TaskState, Vm};

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
