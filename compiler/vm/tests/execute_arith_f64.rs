//! Integration tests for f64 arithmetic opcodes.

mod common;

#[test]
fn execute_when_add_f64_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]  (1.5)
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1]  (2.25)
        0x4E,              // ADD_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f64(&bytecode, 1, &[1.5, 2.25]);
    assert_eq!(result, 3.75);
}

#[test]
fn execute_when_sub_f64_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]  (100.0)
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1]  (0.001)
        0x4F,              // SUB_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f64(&bytecode, 1, &[100.0, 0.001]);
    assert_eq!(result, 99.999);
}

#[test]
fn execute_when_mul_f64_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]  (3.0)
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1]  (7.0)
        0x50,              // MUL_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f64(&bytecode, 1, &[3.0, 7.0]);
    assert_eq!(result, 21.0);
}

#[test]
fn execute_when_div_f64_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]  (22.0)
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1]  (7.0)
        0x51,              // DIV_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f64(&bytecode, 1, &[22.0, 7.0]);
    assert!((result - 3.142857142857143).abs() < 1e-12);
}

#[test]
fn execute_when_div_f64_by_zero_then_positive_infinity() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]  (1.0)
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1]  (0.0)
        0x51,              // DIV_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f64(&bytecode, 1, &[1.0, 0.0]);
    assert!(result.is_infinite() && result.is_sign_positive());
}

#[test]
fn execute_when_neg_f64_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]  (42.5)
        0x52,              // NEG_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f64(&bytecode, 1, &[42.5]);
    assert_eq!(result, -42.5);
}

#[test]
fn execute_when_mul_f64_large_values_then_correct() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]  (1e200)
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1]  (1e200)
        0x50,              // MUL_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f64(&bytecode, 1, &[1e200, 1e200]);
    assert!(result.is_infinite());
}

#[test]
fn execute_when_sub_f64_nan_then_nan() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]  (NaN)
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1]  (5.0)
        0x4F,              // SUB_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let result = common::run_and_read_f64(&bytecode, 1, &[f64::NAN, 5.0]);
    assert!(result.is_nan());
}
