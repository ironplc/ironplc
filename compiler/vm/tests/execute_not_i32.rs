//! Integration tests for the NOT_I32 opcode.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_not_i32_zero_then_all_ones() {
    // NOT 0 = -1 (all bits set)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0)
        0x40,              // NOT_I32
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
    assert_eq!(vm.read_variable(0).unwrap(), -1);
}

#[test]
fn execute_when_not_i32_all_ones_then_zero() {
    // NOT -1 = 0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (-1)
        0x40,              // NOT_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[-1]);
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
fn execute_when_not_i32_positive_then_complement() {
    // NOT 1 = -2 (bitwise complement)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1)
        0x40,              // NOT_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[1]);
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
    assert_eq!(vm.read_variable(0).unwrap(), -2);
}

#[test]
fn execute_when_not_i32_double_then_identity() {
    // NOT (NOT x) = x
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (42)
        0x40,              // NOT_I32
        0x40,              // NOT_I32
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
    assert_eq!(vm.read_variable(0).unwrap(), 42);
}
