//! Integration tests for LOAD_TRUE and LOAD_FALSE opcodes.

mod common;

#[test]
fn execute_when_load_true_then_one() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x07,              // LOAD_TRUE
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(common::run_and_read_i32(&bytecode, 1, &[]), 1);
}

#[test]
fn execute_when_load_false_then_zero() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x08,              // LOAD_FALSE
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(common::run_and_read_i32(&bytecode, 1, &[]), 0);
}

#[test]
fn execute_when_load_true_with_bool_not_then_zero() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x07,              // LOAD_TRUE
        0x57,              // BOOL_NOT
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(common::run_and_read_i32(&bytecode, 1, &[]), 0);
}

#[test]
fn execute_when_load_false_with_bool_not_then_one() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x08,              // LOAD_FALSE
        0x57,              // BOOL_NOT
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    assert_eq!(common::run_and_read_i32(&bytecode, 1, &[]), 1);
}
