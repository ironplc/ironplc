//! Implements the command line behavior.

use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::vm::Vm;

/// Loads a container file, executes one scheduling round, and optionally dumps variables.
///
/// On success with no `dump_vars`, produces no output.
/// When `dump_vars` is `Some(path)`, writes all variable values to that file.
pub fn run(path: &Path, dump_vars: Option<&Path>) -> Result<(), String> {
    let mut file =
        File::open(path).map_err(|e| format!("Unable to open {}: {}", path.display(), e))?;

    let container = ironplc_container::Container::read_from(&mut file)
        .map_err(|e| format!("Unable to read container {}: {e}", path.display()))?;

    let mut vm = Vm::new().load(container).start();

    vm.run_round()
        .map_err(|e| format!("VM trap during execution: {e}"))?;

    if let Some(dump_path) = dump_vars {
        let num_vars = vm.num_variables();
        let mut out = File::create(dump_path)
            .map_err(|e| format!("Unable to create dump file {}: {e}", dump_path.display()))?;
        for i in 0..num_vars {
            let value = vm
                .read_variable(i)
                .map_err(|e| format!("Unable to read variable {i}: {e}"))?;
            writeln!(out, "var[{i}]: {value}")
                .map_err(|e| format!("Unable to write dump file: {e}"))?;
        }
    }

    Ok(())
}
