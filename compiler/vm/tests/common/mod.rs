//! Shared test helpers for VM integration tests.

#![allow(dead_code, unused_imports)]

use ironplc_container::{Container, ContainerBuilder, FunctionId, VarIndex};
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
        .add_function(FunctionId::INIT, &[0xB5], 0, num_vars, 0) // init: RET_VOID
        .add_function(FunctionId::SCAN, bytecode, 16, num_vars, 0) // scan: test bytecode
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
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
        .add_function(FunctionId::INIT, &[0xB5], 0, num_vars, 0)
        .add_function(FunctionId::SCAN, bytecode, 16, num_vars, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
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
        .add_function(FunctionId::INIT, &[0xB5], 0, num_vars, 0)
        .add_function(FunctionId::SCAN, bytecode, 16, num_vars, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
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
        .add_function(FunctionId::INIT, &[0xB5], 0, num_vars, 0)
        .add_function(FunctionId::SCAN, bytecode, 16, num_vars, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
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
        .add_function(FunctionId::INIT, &[0xB5], 0, num_vars, 0)
        .add_function(FunctionId::SCAN, bytecode, 16, num_vars, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
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
        .add_function(FunctionId::INIT, &[0xB5], 0, num_vars, 0)
        .add_function(FunctionId::SCAN, bytecode, 16, num_vars, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
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
        .add_function(FunctionId::INIT, &[0xB5], 0, num_vars, 0)
        .add_function(FunctionId::SCAN, bytecode, 16, num_vars, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .build()
}

/// Builds a container for timer FB tests (TON, TOF, etc.).
///
/// The container runs: load fb_ref, store IN, store PT, call FB, load Q, load ET.
///
/// Variable layout:
///   var[0] = fb_ref (offset 0 into data region)
///   var[1] = IN value (set by test via write_variable)
///   var[2] = Q output (read by test)
///   var[3] = ET output (read by test)
/// Constant layout:
///   constant[0] = PT value (i32 milliseconds)
pub fn timer_test_container(pt_ms: i32, fb_type_id: u16) -> Container {
    use ironplc_container::opcode;

    let type_id_bytes = fb_type_id.to_le_bytes();
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FB_LOAD_INSTANCE, 0x00, 0x00,              // push fb_ref from var[0]
        opcode::LOAD_VAR_I32,     0x01, 0x00,              // push IN from var[1]
        opcode::FB_STORE_PARAM,   0x00,                     // store to FB.IN (field 0)
        opcode::LOAD_CONST_I32,   0x00, 0x00,              // push PT constant (i32 ms)
        opcode::FB_STORE_PARAM,   0x01,                     // store to FB.PT (field 1)
        opcode::FB_CALL,          type_id_bytes[0], type_id_bytes[1], // call FB
        opcode::FB_LOAD_PARAM,    0x02,                     // load FB.Q (field 2)
        opcode::STORE_VAR_I32,    0x02, 0x00,               // store Q to var[2]
        opcode::FB_LOAD_PARAM,    0x03,                     // load FB.ET (field 3)
        opcode::STORE_VAR_I32,    0x03, 0x00,               // store ET to var[3]
        opcode::POP,                                        // discard fb_ref
        opcode::RET_VOID,
    ];

    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    ContainerBuilder::new()
        .num_variables(4)
        .data_region_bytes(48) // 6 fields * 8 bytes
        .add_i32_constant(pt_ms)
        .add_function(FunctionId::INIT, &init_bytecode, 0, 4, 0)
        .add_function(FunctionId::SCAN, &bytecode, 16, 4, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
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
    vm.read_variable(VarIndex::new(0)).unwrap()
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

/// Runs bytecode with f32 constants and returns var[0] as f32.
pub fn run_and_read_f32(bytecode: &[u8], num_vars: u16, constants: &[f32]) -> f32 {
    let c = single_function_container_f32(bytecode, num_vars, constants);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    b.vars[0].as_f32()
}

/// Runs bytecode with f64 constants and returns var[0] as f64.
pub fn run_and_read_f64(bytecode: &[u8], num_vars: u16, constants: &[f64]) -> f64 {
    let c = single_function_container_f64(bytecode, num_vars, constants);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    b.vars[0].as_f64()
}

/// Runs bytecode with i32 constants expecting a trap, returns the trap.
pub fn run_and_expect_trap_i32(bytecode: &[u8], num_vars: u16, constants: &[i32]) -> Trap {
    let c = single_function_container(bytecode, num_vars, constants);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap_err().trap
}
