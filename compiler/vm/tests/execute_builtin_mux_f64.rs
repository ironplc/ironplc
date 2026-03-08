//! Integration tests for the BUILTIN MUX_F64 opcodes.

mod common;

use common::{single_function_container_i32_f64, VmBuffers};

#[test]
fn execute_when_mux_f64_k0_2_inputs_then_returns_in0() {
    // MUX(K:=0, IN0:=10.5, IN1:=20.5) = 10.5
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0)     K
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (10.5)  IN0
        0x04, 0x02, 0x00,  // LOAD_CONST_F64 pool[2] (20.5)  IN1
        0xC4, 0x62, 0x04,  // BUILTIN MUX_F64(2) = 0x0462
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_f64(&bytecode, 1, &[0], &[10.5, 20.5]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!((result - 10.5).abs() < 1e-12, "expected 10.5, got {result}");
}

#[test]
fn execute_when_mux_f64_k1_2_inputs_then_returns_in1() {
    // MUX(K:=1, IN0:=10.5, IN1:=20.5) = 20.5
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1)     K
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (10.5)  IN0
        0x04, 0x02, 0x00,  // LOAD_CONST_F64 pool[2] (20.5)  IN1
        0xC4, 0x62, 0x04,  // BUILTIN MUX_F64(2) = 0x0462
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_f64(&bytecode, 1, &[1], &[10.5, 20.5]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!((result - 20.5).abs() < 1e-12, "expected 20.5, got {result}");
}

#[test]
fn execute_when_mux_f64_k2_3_inputs_then_returns_in2() {
    // MUX(K:=2, IN0:=1.0, IN1:=2.0, IN2:=3.0) = 3.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (2)    K
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (1.0)  IN0
        0x04, 0x02, 0x00,  // LOAD_CONST_F64 pool[2] (2.0)  IN1
        0x04, 0x03, 0x00,  // LOAD_CONST_F64 pool[3] (3.0)  IN2
        0xC4, 0x63, 0x04,  // BUILTIN MUX_F64(3) = 0x0463
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_f64(&bytecode, 1, &[2], &[1.0, 2.0, 3.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!((result - 3.0).abs() < 1e-12, "expected 3.0, got {result}");
}

#[test]
fn execute_when_mux_f64_k_out_of_range_then_clamps_to_last() {
    // MUX(K:=10, IN0:=1.0, IN1:=2.0, IN2:=3.0) = 3.0 (clamped)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (10)   K
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (1.0)  IN0
        0x04, 0x02, 0x00,  // LOAD_CONST_F64 pool[2] (2.0)  IN1
        0x04, 0x03, 0x00,  // LOAD_CONST_F64 pool[3] (3.0)  IN2
        0xC4, 0x63, 0x04,  // BUILTIN MUX_F64(3) = 0x0463
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_f64(&bytecode, 1, &[10], &[1.0, 2.0, 3.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!((result - 3.0).abs() < 1e-12, "expected 3.0, got {result}");
}

#[test]
fn execute_when_mux_f64_k_negative_then_clamps_to_first() {
    // MUX(K:=-1, IN0:=10.5, IN1:=20.5) = 10.5 (clamped)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (-1)    K
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (10.5)  IN0
        0x04, 0x02, 0x00,  // LOAD_CONST_F64 pool[2] (20.5)  IN1
        0xC4, 0x62, 0x04,  // BUILTIN MUX_F64(2) = 0x0462
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_f64(&bytecode, 1, &[-1], &[10.5, 20.5]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!((result - 10.5).abs() < 1e-12, "expected 10.5, got {result}");
}
