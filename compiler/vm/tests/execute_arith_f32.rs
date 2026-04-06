//! Integration tests for f32 arithmetic opcodes.

mod common;

#[test]
fn execute_when_add_f32_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]  (1.5)
        0x03, 0x01, 0x00,  // LOAD_CONST_F32 pool[1]  (2.25)
        0x48,              // ADD_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f32(&bytecode, 1, &[1.5, 2.25]);
    assert_eq!(result, 3.75);
}

#[test]
fn execute_when_sub_f32_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]  (10.0)
        0x03, 0x01, 0x00,  // LOAD_CONST_F32 pool[1]  (3.5)
        0x49,              // SUB_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f32(&bytecode, 1, &[10.0, 3.5]);
    assert_eq!(result, 6.5);
}

#[test]
fn execute_when_mul_f32_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]  (4.0)
        0x03, 0x01, 0x00,  // LOAD_CONST_F32 pool[1]  (2.5)
        0x4A,              // MUL_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f32(&bytecode, 1, &[4.0, 2.5]);
    assert_eq!(result, 10.0);
}

#[test]
fn execute_when_div_f32_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]  (7.5)
        0x03, 0x01, 0x00,  // LOAD_CONST_F32 pool[1]  (2.5)
        0x4B,              // DIV_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f32(&bytecode, 1, &[7.5, 2.5]);
    assert_eq!(result, 3.0);
}

#[test]
fn execute_when_div_f32_by_zero_then_positive_infinity() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]  (1.0)
        0x03, 0x01, 0x00,  // LOAD_CONST_F32 pool[1]  (0.0)
        0x4B,              // DIV_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f32(&bytecode, 1, &[1.0, 0.0]);
    assert!(result.is_infinite() && result.is_sign_positive());
}

#[test]
fn execute_when_neg_f32_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]  (5.5)
        0x4C,              // NEG_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f32(&bytecode, 1, &[5.5]);
    assert_eq!(result, -5.5);
}

#[test]
fn execute_when_neg_f32_negative_then_positive() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]  (-3.25)
        0x4C,              // NEG_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f32(&bytecode, 1, &[-3.25]);
    assert_eq!(result, 3.25);
}

#[test]
fn execute_when_add_f32_nan_then_nan() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]  (NaN)
        0x03, 0x01, 0x00,  // LOAD_CONST_F32 pool[1]  (1.0)
        0x48,              // ADD_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f32(&bytecode, 1, &[f32::NAN, 1.0]);
    assert!(result.is_nan());
}
