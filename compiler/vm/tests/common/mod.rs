//! Shared test helpers for VM integration tests.

use ironplc_container::{Container, ContainerBuilder};
use ironplc_vm::error::Trap;
use ironplc_vm::{ProgramInstanceState, Slot, TaskState, VmRunning};

/// Helper struct that allocates Vec-backed buffers for VM usage.
pub struct VmBuffers {
    pub stack: Vec<Slot>,
    pub vars: Vec<Slot>,
    pub tasks: Vec<TaskState>,
    pub programs: Vec<ProgramInstanceState>,
    pub ready: Vec<usize>,
}

impl VmBuffers {
    pub fn from_container(container: &Container) -> Self {
        let header = &container.header;
        let task_count = container.task_table.tasks.len();
        let program_count = container.task_table.programs.len();
        VmBuffers {
            stack: vec![Slot::default(); header.max_stack_depth as usize],
            vars: vec![Slot::default(); header.num_variables as usize],
            tasks: vec![TaskState::default(); task_count],
            programs: vec![ProgramInstanceState::default(); program_count],
            ready: vec![0usize; task_count.max(1)],
        }
    }
}

/// Builds a container with one function from the given bytecode,
/// with `num_vars` variables and the given constants.
/// Uses a generous max_stack_depth (16) suitable for most tests.
pub fn single_function_container(bytecode: &[u8], num_vars: u16, constants: &[i32]) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in constants {
        builder = builder.add_i32_constant(c);
    }
    builder.add_function(0, bytecode, 16, num_vars).build()
}

/// Asserts that a run_round produces a specific trap.
#[allow(dead_code)]
pub fn assert_trap(vm: &mut VmRunning, expected: Trap) {
    let result = vm.run_round(0);
    assert!(
        result.is_err(),
        "expected trap {expected} but run_round succeeded"
    );
    assert_eq!(result.unwrap_err().trap, expected);
}
