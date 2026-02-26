//! Implements the command line behavior.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::vm::Vm;

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

    let mut running = Vm::new().load(container).start();

    // Install signal handler for clean shutdown
    let handle = running.stop_handle();
    ctrlc::set_handler(move || handle.request_stop())
        .map_err(|e| format!("Failed to set signal handler: {e}"))?;

    let mut rounds = 0u64;
    loop {
        if running.stop_requested() {
            break;
        }
        if let Some(max) = scans {
            if rounds >= max {
                break;
            }
        }
        if let Err(ctx) = running.run_round() {
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

fn dump_variables_stopped(stopped: &crate::vm::VmStopped, dump_path: &Path) -> Result<(), String> {
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

fn dump_variables_faulted(faulted: &crate::vm::VmFaulted, dump_path: &Path) -> Result<(), String> {
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
