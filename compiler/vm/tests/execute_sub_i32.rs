//! Integration tests for the SUB_I32 opcode.

mod common;

use common::{assert_trap, single_function_container, VmBuffers};
use ironplc_vm::error::Trap;
use ironplc_vm::Vm;

#[test]
fn execute_when_sub_i32_basic_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (10)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (3)
        0x31,              // SUB_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[10, 3]);
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

    assert_eq!(vm.read_variable(0).unwrap(), 7);
}

#[test]
fn execute_when_sub_i32_result_negative_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (3)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (10)
        0x31,              // SUB_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[3, 10]);
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

    assert_eq!(vm.read_variable(0).unwrap(), -7);
}

#[test]
fn execute_when_sub_i32_both_zero_then_zero() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (0)
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (0)
        0x31,              // SUB_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0]);
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

    assert_eq!(vm.read_variable(0).unwrap(), 0);
}

#[test]
fn execute_when_sub_i32_same_value_then_zero() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (42)
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (42)
        0x31,              // SUB_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[42]);
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

    assert_eq!(vm.read_variable(0).unwrap(), 0);
}

// Overflow: i32::MIN - 1 wraps to i32::MAX
#[test]
fn execute_when_sub_i32_wraps_at_min_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MIN)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (1)
        0x31,              // SUB_I32
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
        .start();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(0).unwrap(), i32::MAX);
}

// Overflow: i32::MAX - (-1) wraps to i32::MIN
#[test]
fn execute_when_sub_i32_wraps_at_max_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MAX)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (-1)
        0x31,              // SUB_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[i32::MAX, -1]);
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

    assert_eq!(vm.read_variable(0).unwrap(), i32::MIN);
}

// Overflow: i32::MIN - i32::MAX wraps to 1
#[test]
fn execute_when_sub_i32_min_minus_max_then_wraps_to_one() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MIN)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (i32::MAX)
        0x31,              // SUB_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[i32::MIN, i32::MAX]);
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

    // i32::MIN - i32::MAX = -2147483648 - 2147483647 = wraps to 1
    assert_eq!(vm.read_variable(0).unwrap(), 1);
}

// Overflow: i32::MAX - i32::MIN wraps to -1
#[test]
fn execute_when_sub_i32_max_minus_min_then_wraps_to_neg_one() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MAX)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (i32::MIN)
        0x31,              // SUB_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[i32::MAX, i32::MIN]);
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

    // i32::MAX - i32::MIN = 2147483647 - (-2147483648) = wraps to -1
    assert_eq!(vm.read_variable(0).unwrap(), -1);
}

// Edge: 0 - i32::MIN wraps to i32::MIN (since -i32::MIN overflows)
#[test]
fn execute_when_sub_i32_zero_minus_min_then_wraps() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (0)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (i32::MIN)
        0x31,              // SUB_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0, i32::MIN]);
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

    // 0 - i32::MIN = 0 - (-2147483648) = wraps to i32::MIN
    assert_eq!(vm.read_variable(0).unwrap(), i32::MIN);
}

// Subtraction with negative operands
#[test]
fn execute_when_sub_i32_negative_operands_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (-10)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (-3)
        0x31,              // SUB_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[-10, -3]);
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

    // -10 - (-3) = -7
    assert_eq!(vm.read_variable(0).unwrap(), -7);
}

#[test]
fn execute_when_sub_i32_stack_underflow_then_trap() {
    // SUB_I32 tries to pop 2 values from an empty stack
    let c = single_function_container(&[0x31], 0, &[]);
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

    assert_trap(&mut vm, Trap::StackUnderflow);
}
