//! Integration tests for the BUILTIN MAX_F32 opcode.

mod common;

use common::{single_function_container_f32, VmBuffers};

#[test]
fn execute_when_max_f32_then_returns_larger() {
    // MAX(10.5, 3.0) = 10.5
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] (10.5)
        0x03, 0x01, 0x00,  // LOAD_CONST_F32 pool[1] (3.0)
        0xC4, 0x58, 0x03,  // BUILTIN MAX_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[10.5, 3.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f32();
    assert!((result - 10.5).abs() < 1e-5, "expected 10.5, got {result}");
}

#[test]
fn execute_when_max_f32_equal_then_returns_value() {
    // MAX(5.0, 5.0) = 5.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] (5.0)
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] (5.0)
        0xC4, 0x58, 0x03,  // BUILTIN MAX_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[5.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f32();
    assert!((result - 5.0).abs() < 1e-5, "expected 5.0, got {result}");
}

#[test]
fn execute_when_max_f32_negative_vs_positive_then_returns_positive() {
    // MAX(-3.0, 7.0) = 7.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] (-3.0)
        0x03, 0x01, 0x00,  // LOAD_CONST_F32 pool[1] (7.0)
        0xC4, 0x58, 0x03,  // BUILTIN MAX_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[-3.0, 7.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f32();
    assert!((result - 7.0).abs() < 1e-5, "expected 7.0, got {result}");
}
