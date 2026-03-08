//! Shared test helpers for VM integration tests.

#![allow(dead_code)]

use ironplc_container::{Container, ContainerBuilder};
use ironplc_vm::error::Trap;
use ironplc_vm::{FaultContext, ProgramInstanceState, Slot, TaskState, Vm, VmRunning};

/// Helper struct that allocates Vec-backed buffers for VM usage.
pub struct VmBuffers {
    pub stack: Vec<Slot>,
    pub vars: Vec<Slot>,
    pub data_region: Vec<u8>,
    pub temp_buf: Vec<u8>,
    pub tasks: Vec<TaskState>,
    pub programs: Vec<ProgramInstanceState>,
    pub ready: Vec<usize>,
}

impl VmBuffers {
    pub fn from_container(container: &Container) -> Self {
        let header = &container.header;
        let task_count = container.task_table.tasks.len();
        let program_count = container.task_table.programs.len();
        let temp_buf_total = header.num_temp_bufs as usize * header.max_temp_buf_bytes as usize;
        VmBuffers {
            stack: vec![Slot::default(); header.max_stack_depth as usize],
            vars: vec![Slot::default(); header.num_variables as usize],
            data_region: vec![0u8; header.data_region_bytes as usize],
            temp_buf: vec![0u8; temp_buf_total],
            tasks: vec![TaskState::default(); task_count],
            programs: vec![ProgramInstanceState::default(); program_count],
            ready: vec![0usize; task_count.max(1)],
        }
    }
}

/// Builds a container with an init function (RET_VOID) and a scan function
/// from the given bytecode, with `num_vars` variables and the given constants.
/// Uses a generous max_stack_depth (16) suitable for most tests.
pub fn single_function_container(bytecode: &[u8], num_vars: u16, constants: &[i32]) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in constants {
        builder = builder.add_i32_constant(c);
    }
    builder
        .add_function(0, &[0xB5], 0, num_vars) // init: RET_VOID
        .add_function(1, bytecode, 16, num_vars) // scan: test bytecode
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}

/// Builds a container with an init function (RET_VOID) and a scan function
/// from the given bytecode, with `num_vars` variables and the given f32 constants.
pub fn single_function_container_f32(
    bytecode: &[u8],
    num_vars: u16,
    constants: &[f32],
) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in constants {
        builder = builder.add_f32_constant(c);
    }
    builder
        .add_function(0, &[0xB5], 0, num_vars)
        .add_function(1, bytecode, 16, num_vars)
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}

/// Builds a container with an init function (RET_VOID) and a scan function
/// from the given bytecode, with `num_vars` variables and the given f64 constants.
pub fn single_function_container_f64(
    bytecode: &[u8],
    num_vars: u16,
    constants: &[f64],
) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in constants {
        builder = builder.add_f64_constant(c);
    }
    builder
        .add_function(0, &[0xB5], 0, num_vars)
        .add_function(1, bytecode, 16, num_vars)
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}

/// Builds a container with an init function (RET_VOID) and a scan function
/// from the given bytecode, with `num_vars` variables and the given i64 constants.
pub fn single_function_container_i64(
    bytecode: &[u8],
    num_vars: u16,
    constants: &[i64],
) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in constants {
        builder = builder.add_i64_constant(c);
    }
    builder
        .add_function(0, &[0xB5], 0, num_vars)
        .add_function(1, bytecode, 16, num_vars)
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}

/// Builds a container with an init function (RET_VOID) and a scan function
/// from the given bytecode, with `num_vars` variables and a mix of i32 then i64 constants.
pub fn single_function_container_i32_i64(
    bytecode: &[u8],
    num_vars: u16,
    i32_constants: &[i32],
    i64_constants: &[i64],
) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in i32_constants {
        builder = builder.add_i32_constant(c);
    }
    for &c in i64_constants {
        builder = builder.add_i64_constant(c);
    }
    builder
        .add_function(0, &[0xB5], 0, num_vars)
        .add_function(1, bytecode, 16, num_vars)
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}

/// Builds a container with an init function (RET_VOID) and a scan function
/// from the given bytecode, with `num_vars` variables and a mix of i32 then f32 constants.
pub fn single_function_container_i32_f32(
    bytecode: &[u8],
    num_vars: u16,
    i32_constants: &[i32],
    f32_constants: &[f32],
) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in i32_constants {
        builder = builder.add_i32_constant(c);
    }
    for &c in f32_constants {
        builder = builder.add_f32_constant(c);
    }
    builder
        .add_function(0, &[0xB5], 0, num_vars)
        .add_function(1, bytecode, 16, num_vars)
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}

/// Builds a container with an init function (RET_VOID) and a scan function
/// from the given bytecode, with `num_vars` variables and a mix of i32 then f64 constants.
pub fn single_function_container_i32_f64(
    bytecode: &[u8],
    num_vars: u16,
    i32_constants: &[i32],
    f64_constants: &[f64],
) -> Container {
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in i32_constants {
        builder = builder.add_i32_constant(c);
    }
    for &c in f64_constants {
        builder = builder.add_f64_constant(c);
    }
    builder
        .add_function(0, &[0xB5], 0, num_vars)
        .add_function(1, bytecode, 16, num_vars)
        .init_function_id(0)
        .entry_function_id(1)
        .build()
}

/// Loads a container into the VM using the given buffers and starts execution.
///
/// This centralizes the `.load()` call so that adding new buffer parameters
/// only requires updating this one function instead of every test file.
pub fn load_and_start<'a>(
    container: &'a Container,
    bufs: &'a mut VmBuffers,
) -> Result<VmRunning<'a>, FaultContext> {
    Vm::new()
        .load(
            container,
            &mut bufs.stack,
            &mut bufs.vars,
            &mut bufs.data_region,
            &mut bufs.temp_buf,
            &mut bufs.tasks,
            &mut bufs.programs,
            &mut bufs.ready,
        )
        .start()
}

/// Asserts that a run_round produces a specific trap.
pub fn assert_trap(vm: &mut VmRunning, expected: Trap) {
    let result = vm.run_round(0);
    assert!(
        result.is_err(),
        "expected trap {expected} but run_round succeeded"
    );
    assert_eq!(result.unwrap_err().trap, expected);
}
