//! Integration tests for the ADD_I32 opcode.

mod common;

use common::{single_function_container, VmBuffers};

#[test]
fn execute_when_add_i32_wraps_at_max_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MAX)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (1)
        0x30,              // ADD_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[i32::MAX, 1]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(0).unwrap(), i32::MIN);
}

#[test]
fn execute_when_add_i32_wraps_at_min_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (i32::MIN)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (-1)
        0x30,              // ADD_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[i32::MIN, -1]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(0).unwrap(), i32::MAX);
}
