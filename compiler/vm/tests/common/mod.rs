//! Shared test helpers for VM integration tests.

#![allow(dead_code)]

use ironplc_container::{Container, ContainerBuilder};
use ironplc_vm::error::Trap;
pub use ironplc_vm::VmBuffers;
use ironplc_vm::{FaultContext, Vm, VmRunning};

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
