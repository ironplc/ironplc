//! Integration tests for the BUILTIN shift/rotate opcodes
//! (SHL, SHR, ROL, ROR) at the VM level.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_container::ContainerBuilder;
use ironplc_vm::Vm;

// --- SHL_I32 ---

#[test]
fn execute_when_shl_i32_then_shifts_left() {
    // SHL(0x0F, 4) = 0xF0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0x0F)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (4)
        0xC4, 0x48, 0x03,  // BUILTIN SHL_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0x0F, 4]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0xF0);
}

// --- SHL_I64 ---

#[test]
fn execute_when_shl_i64_then_shifts_left() {
    // SHL(0x01_i64, 32) = 0x1_0000_0000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (1)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (32)
        0xC4, 0x49, 0x03,  // BUILTIN SHL_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(1)
        .add_i64_constant(32)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), 0x1_0000_0000);
}

// --- SHR_I32 ---

#[test]
fn execute_when_shr_i32_then_shifts_right_logical() {
    // SHR(0xF0, 4) = 0x0F
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0xF0)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (4)
        0xC4, 0x4A, 0x03,  // BUILTIN SHR_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0xF0, 4]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0x0F);
}

#[test]
fn execute_when_shr_i32_high_bit_set_then_logical_shift() {
    // SHR(0x80000000, 1) = 0x40000000 (logical, not arithmetic)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0x80000000 as i32 = i32::MIN)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (1)
        0xC4, 0x4A, 0x03,  // BUILTIN SHR_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[i32::MIN, 1]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0x40000000);
}

// --- SHR_I64 ---

#[test]
fn execute_when_shr_i64_then_shifts_right() {
    // SHR(0xFF00_i64, 8) = 0xFF
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (0xFF00)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (8)
        0xC4, 0x4B, 0x03,  // BUILTIN SHR_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(0xFF00)
        .add_i64_constant(8)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), 0xFF);
}

// --- ROL_I32 ---

#[test]
fn execute_when_rol_i32_then_rotates_left() {
    // ROL(0x80000001_u32, 1) = 0x00000003
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0x80000001 as i32)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (1)
        0xC4, 0x4C, 0x03,  // BUILTIN ROL_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0x80000001_u32 as i32, 1]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0x00000003);
}

// --- ROL_I64 ---

#[test]
fn execute_when_rol_i64_then_rotates_left() {
    // ROL(0x8000_0000_0000_0001_u64, 1) = 0x0000_0000_0000_0003
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0]
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (1)
        0xC4, 0x4D, 0x03,  // BUILTIN ROL_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(0x8000_0000_0000_0001_u64 as i64)
        .add_i64_constant(1)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), 0x0000_0000_0000_0003);
}

// --- ROR_I32 ---

#[test]
fn execute_when_ror_i32_then_rotates_right() {
    // ROR(0x80000001_u32, 1) = 0xC0000000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0x80000001 as i32)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (1)
        0xC4, 0x4E, 0x03,  // BUILTIN ROR_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0x80000001_u32 as i32, 1]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0xC0000000_u32 as i32);
}

// --- ROR_I64 ---

#[test]
fn execute_when_ror_i64_then_rotates_right() {
    // ROR(0x0000_0000_0000_0001_u64, 1) = 0x8000_0000_0000_0000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (1)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (1)
        0xC4, 0x4F, 0x03,  // BUILTIN ROR_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(1)
        .add_i64_constant(1)
        .add_function(0, &bytecode, 16, 1)
        .build();
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    vm.stop();
    assert_eq!(b.vars[0].as_i64(), i64::MIN); // 0x8000_0000_0000_0000
}

// --- ROL_U8 (narrow 8-bit rotate) ---

#[test]
fn execute_when_rol_u8_then_rotates_within_8_bits() {
    // ROL(0x81_u8, 1) = 0x03 (bit 7 wraps to bit 0)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0x81)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (1)
        0xC4, 0x50, 0x03,  // BUILTIN ROL_U8
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0x81, 1]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0x03);
}

// --- ROL_U16 (narrow 16-bit rotate) ---

#[test]
fn execute_when_rol_u16_then_rotates_within_16_bits() {
    // ROL(0x8001_u16, 1) = 0x0003
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0x8001)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (1)
        0xC4, 0x51, 0x03,  // BUILTIN ROL_U16
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0x8001, 1]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0x0003);
}

// --- ROR_U8 (narrow 8-bit rotate) ---

#[test]
fn execute_when_ror_u8_then_rotates_within_8_bits() {
    // ROR(0x81_u8, 1) = 0xC0 (bit 0 wraps to bit 7)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0x81)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (1)
        0xC4, 0x52, 0x03,  // BUILTIN ROR_U8
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0x81, 1]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0xC0);
}

// --- ROR_U16 (narrow 16-bit rotate) ---

#[test]
fn execute_when_ror_u16_then_rotates_within_16_bits() {
    // ROR(0x8001_u16, 1) = 0xC000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0x8001)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (1)
        0xC4, 0x53, 0x03,  // BUILTIN ROR_U16
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0x8001, 1]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = Vm::new()
        .load(
            &c,
            &mut b.stack,
            &mut b.vars,
            &mut b.tasks,
            &mut b.programs,
            &mut b.ready,
        )
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0xC000);
}
