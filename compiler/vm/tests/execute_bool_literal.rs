//! Integration tests for LOAD_TRUE and LOAD_FALSE opcodes.

mod common;

use common::{single_function_container, VmBuffers};

#[test]
fn execute_when_load_true_then_one() {
    // LOAD_TRUE → var[0] = 1
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x07,              // LOAD_TRUE
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 1);
}

#[test]
fn execute_when_load_false_then_zero() {
    // LOAD_FALSE → var[0] = 0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x08,              // LOAD_FALSE
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0);
}

#[test]
fn execute_when_load_true_with_bool_not_then_zero() {
    // LOAD_TRUE + BOOL_NOT → var[0] = 0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x07,              // LOAD_TRUE
        0x57,              // BOOL_NOT
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0);
}

#[test]
fn execute_when_load_false_with_bool_not_then_one() {
    // LOAD_FALSE + BOOL_NOT → var[0] = 1
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x08,              // LOAD_FALSE
        0x57,              // BOOL_NOT
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 1);
}
