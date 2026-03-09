//! Shared test helpers for VM integration tests.

#![allow(dead_code, unused_imports)]

use ironplc_container::{Container, ContainerBuilder};
use ironplc_vm::error::Trap;
pub use ironplc_vm::test_support::{assert_trap, load_and_start};
pub use ironplc_vm::VmBuffers;

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

/// Runs bytecode with i32 constants and returns var[0] as i32.
///
/// Shorthand for the common pattern: build container, allocate buffers,
/// load VM, execute one round, read variable 0.
pub fn run_and_read_i32(bytecode: &[u8], num_vars: u16, constants: &[i32]) -> i32 {
    let c = single_function_container(bytecode, num_vars, constants);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();
    vm.read_variable(0).unwrap()
}

/// Runs bytecode with i64 constants and returns var[0] as i64.
pub fn run_and_read_i64(bytecode: &[u8], num_vars: u16, constants: &[i64]) -> i64 {
    let c = single_function_container_i64(bytecode, num_vars, constants);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    b.vars[0].as_i64()
}

/// Runs bytecode with i32 constants expecting a trap, returns the trap.
pub fn run_and_expect_trap_i32(bytecode: &[u8], num_vars: u16, constants: &[i32]) -> Trap {
    let c = single_function_container(bytecode, num_vars, constants);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap_err().trap
}
