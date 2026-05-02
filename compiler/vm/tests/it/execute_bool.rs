//! VM-specific edge case tests for boolean opcodes (BOOL_AND, BOOL_OR, BOOL_XOR, BOOL_NOT).
//!
//! Basic correctness is covered by end_to_end_bool.rs.
//! These tests cover non-zero integer coercion to boolean that cannot be expressed in IEC 61131-3 source.

#[test]
fn execute_when_bool_and_nonzero_coercion_then_one() {
    // 5 AND 3 → 1 (both non-zero, coerced to true)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (5)
        0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (3)
        0x78,              // BOOL_AND
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    assert_eq!(crate::common::run_and_read_i32(&bytecode, 1, &[5, 3]), 1);
}

#[test]
fn execute_when_bool_or_nonzero_coercion_then_one() {
    // 5 OR 0 → 1 (5 coerced to true)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (5)
        0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (0)
        0x79,              // BOOL_OR
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    assert_eq!(crate::common::run_and_read_i32(&bytecode, 1, &[5, 0]), 1);
}

#[test]
fn execute_when_bool_xor_nonzero_coercion_then_zero() {
    // 5 XOR 3 → 0 (both coerced to true, same → XOR is 0)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (5)
        0x00, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (3)
        0x7A,              // BOOL_XOR
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    assert_eq!(crate::common::run_and_read_i32(&bytecode, 1, &[5, 3]), 0);
}

#[test]
fn execute_when_bool_not_nonzero_coercion_then_zero() {
    // NOT 5 → 0 (5 coerced to true, NOT true = 0)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (5)
        0x7B,              // BOOL_NOT
        0x10, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0x8C,              // RET_VOID
    ];
    assert_eq!(crate::common::run_and_read_i32(&bytecode, 1, &[5]), 0);
}
