//! Integration tests for bitwise opcodes (BIT_AND_32, BIT_OR_32, BIT_XOR_32,
//! BIT_NOT_32, BIT_AND_64, BIT_OR_64, BIT_XOR_64, BIT_NOT_64).

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

// ---------------------------------------------------------------
// BIT_AND_32
// ---------------------------------------------------------------

#[test]
fn execute_when_bit_and_32_then_bitwise_and() {
    // 0xFF AND 0x0F → 0x0F
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0xFF = 255)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (0x0F = 15)
        0x58,              // BIT_AND_32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0xFF, 0x0F]);
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
        .start();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0x0F);
}

// ---------------------------------------------------------------
// BIT_OR_32
// ---------------------------------------------------------------

#[test]
fn execute_when_bit_or_32_then_bitwise_or() {
    // 0xF0 OR 0x0F → 0xFF
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0xF0 = 240)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (0x0F = 15)
        0x59,              // BIT_OR_32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0xF0, 0x0F]);
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
        .start();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0xFF);
}

// ---------------------------------------------------------------
// BIT_XOR_32
// ---------------------------------------------------------------

#[test]
fn execute_when_bit_xor_32_then_bitwise_xor() {
    // 0xFF XOR 0x0F → 0xF0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0xFF = 255)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (0x0F = 15)
        0x5A,              // BIT_XOR_32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0xFF, 0x0F]);
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
        .start();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0xF0);
}

// ---------------------------------------------------------------
// BIT_NOT_32
// ---------------------------------------------------------------

#[test]
fn execute_when_bit_not_32_then_bitwise_not() {
    // NOT 0x0F → 0xFFFFFFF0 (as i32: -16)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0x0F = 15)
        0x5B,              // BIT_NOT_32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0x0F]);
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
        .start();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), !0x0F_i32);
}

// ---------------------------------------------------------------
// BIT_AND_64
// ---------------------------------------------------------------

#[test]
fn execute_when_bit_and_64_then_bitwise_and() {
    // 0xFF_i64 AND 0x0F_i64 → 0x0F_i64
    // Use ContainerBuilder directly for i64 constants.
    use ironplc_container::ContainerBuilder;

    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (0xFF)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (0x0F)
        0x60,              // BIT_AND_64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(0xFF)
        .add_i64_constant(0x0F)
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
        .start();

    vm.run_round(0).unwrap();
    // read_variable returns i32 (lower 32 bits); 0x0F fits in i32.
    let stopped = vm.stop();
    assert_eq!(stopped.read_variable(0).unwrap(), 0x0F);
}

// ---------------------------------------------------------------
// BIT_OR_64
// ---------------------------------------------------------------

#[test]
fn execute_when_bit_or_64_then_bitwise_or() {
    use ironplc_container::ContainerBuilder;

    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (0xF0)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (0x0F)
        0x61,              // BIT_OR_64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(0xF0)
        .add_i64_constant(0x0F)
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
        .start();

    vm.run_round(0).unwrap();
    let stopped = vm.stop();
    assert_eq!(stopped.read_variable(0).unwrap(), 0xFF);
}

// ---------------------------------------------------------------
// BIT_XOR_64
// ---------------------------------------------------------------

#[test]
fn execute_when_bit_xor_64_then_bitwise_xor() {
    use ironplc_container::ContainerBuilder;

    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (0xFF)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (0x0F)
        0x62,              // BIT_XOR_64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(0xFF)
        .add_i64_constant(0x0F)
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
        .start();

    vm.run_round(0).unwrap();
    let stopped = vm.stop();
    assert_eq!(stopped.read_variable(0).unwrap(), 0xF0);
}

// ---------------------------------------------------------------
// BIT_NOT_64
// ---------------------------------------------------------------

#[test]
fn execute_when_bit_not_64_then_bitwise_not() {
    use ironplc_container::ContainerBuilder;

    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (0x0F)
        0x63,              // BIT_NOT_64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i64_constant(0x0F)
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
        .start();

    vm.run_round(0).unwrap();
    // !0x0F_i64 = -16, as i32 (lower 32 bits) = -16
    let stopped = vm.stop();
    assert_eq!(stopped.read_variable(0).unwrap(), -16);
}
