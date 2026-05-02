//! Integration tests for the DUP and SWAP opcodes.

mod common;

use ironplc_container::VarIndex;

#[test]
fn execute_when_dup_then_duplicates_top_value() {
    // push 42, DUP, ADD → 42 + 42 = 84
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (42)
        0x91,              // DUP
        0x20,              // ADD_I32
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    assert_eq!(common::run_and_read_i32(&bytecode, 1, &[42]), 84);
}

#[test]
fn execute_when_dup_then_both_copies_independent() {
    // push 10, DUP, store var[0], store var[1] → both 10
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (10)
        0x91,              // DUP
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x10, 0x01, 0x00,  // STORE_VAR_I32 var[1]
        0x8C,              // RET_VOID
    ];
    let c = common::single_function_container(&bytecode, 2, &[10]);
    let mut b = ironplc_vm::VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 10);
    assert_eq!(vm.read_variable(VarIndex::new(1)).unwrap(), 10);
}

#[test]
fn execute_when_swap_then_reverses_top_two() {
    // push 10, push 3, SWAP, SUB → 3 - 10 = -7
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (10)
        0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (3)
        0x92,              // SWAP
        0x24,              // SUB_I32
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    assert_eq!(common::run_and_read_i32(&bytecode, 1, &[10, 3]), -7);
}

#[test]
fn execute_when_dup_and_swap_combined_then_correct() {
    // push 5, DUP, push 20, SWAP, SUB → 20 - 5 = 15
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]  (5)
        0x91,              // DUP
        0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1]  (20)
        0x92,              // SWAP
        0x24,              // SUB_I32
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    assert_eq!(common::run_and_read_i32(&bytecode, 1, &[5, 20]), 15);
}
