//! Integration tests for type conversion BUILTIN opcodes: integer to boolean.

mod common;

use common::{single_function_container, single_function_container_i64, VmBuffers};

// --- CONV_I32_TO_BOOL ---

#[test]
fn execute_when_conv_i32_to_bool_nonzero_then_returns_1() {
    // CONV_I32_TO_BOOL(42) = 1 (TRUE)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        0xC4, 0x99, 0x03,  // BUILTIN CONV_I32_TO_BOOL
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[42]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_i32();
    assert_eq!(result, 1, "expected 1, got {result}");
}

#[test]
fn execute_when_conv_i32_to_bool_zero_then_returns_0() {
    // CONV_I32_TO_BOOL(0) = 0 (FALSE)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        0xC4, 0x99, 0x03,  // BUILTIN CONV_I32_TO_BOOL
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_i32();
    assert_eq!(result, 0, "expected 0, got {result}");
}

#[test]
fn execute_when_conv_i32_to_bool_negative_then_returns_1() {
    // CONV_I32_TO_BOOL(-1) = 1 (TRUE)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        0xC4, 0x99, 0x03,  // BUILTIN CONV_I32_TO_BOOL
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[-1]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_i32();
    assert_eq!(result, 1, "expected 1, got {result}");
}

#[test]
fn execute_when_conv_i32_to_bool_value_2_then_returns_1() {
    // CONV_I32_TO_BOOL(2) = 1 (TRUE) — ensures we test non-zero, not just bit 0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        0xC4, 0x99, 0x03,  // BUILTIN CONV_I32_TO_BOOL
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[2]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_i32();
    assert_eq!(result, 1, "expected 1, got {result}");
}

// --- CONV_I64_TO_BOOL ---

#[test]
fn execute_when_conv_i64_to_bool_nonzero_then_returns_1() {
    // CONV_I64_TO_BOOL(100000) = 1 (TRUE)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0]
        0xC4, 0x9A, 0x03,  // BUILTIN CONV_I64_TO_BOOL
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i64(&bytecode, 1, &[100000]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_i32();
    assert_eq!(result, 1, "expected 1, got {result}");
}

#[test]
fn execute_when_conv_i64_to_bool_zero_then_returns_0() {
    // CONV_I64_TO_BOOL(0) = 0 (FALSE)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0]
        0xC4, 0x9A, 0x03,  // BUILTIN CONV_I64_TO_BOOL
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i64(&bytecode, 1, &[0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_i32();
    assert_eq!(result, 0, "expected 0, got {result}");
}
