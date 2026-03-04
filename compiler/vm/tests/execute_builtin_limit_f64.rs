//! Integration tests for the BUILTIN LIMIT_F64 opcode.

mod common;

use common::{single_function_container_f64, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_limit_f64_in_range_then_unchanged() {
    // LIMIT(MN:=1.0, IN:=5.0, MX:=10.0) = 5.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] (1.0)  MN
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (5.0)  IN
        0x04, 0x02, 0x00,  // LOAD_CONST_F64 pool[2] (10.0) MX
        0xC4, 0x5B, 0x03,  // BUILTIN LIMIT_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[1.0, 5.0, 10.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!((result - 5.0).abs() < 1e-12, "expected 5.0, got {result}");
}

#[test]
fn execute_when_limit_f64_below_min_then_clamped() {
    // LIMIT(MN:=1.0, IN:=-5.0, MX:=10.0) = 1.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] (1.0)  MN
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (-5.0) IN
        0x04, 0x02, 0x00,  // LOAD_CONST_F64 pool[2] (10.0) MX
        0xC4, 0x5B, 0x03,  // BUILTIN LIMIT_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[1.0, -5.0, 10.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!((result - 1.0).abs() < 1e-12, "expected 1.0, got {result}");
}

#[test]
fn execute_when_limit_f64_above_max_then_clamped() {
    // LIMIT(MN:=1.0, IN:=99.0, MX:=10.0) = 10.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] (1.0)  MN
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (99.0) IN
        0x04, 0x02, 0x00,  // LOAD_CONST_F64 pool[2] (10.0) MX
        0xC4, 0x5B, 0x03,  // BUILTIN LIMIT_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[1.0, 99.0, 10.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!((result - 10.0).abs() < 1e-12, "expected 10.0, got {result}");
}

#[test]
fn execute_when_limit_f64_nan_input_then_nan() {
    // LIMIT(MN:=1.0, IN:=NaN, MX:=10.0) = NaN
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] (1.0)  MN
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (NaN)  IN
        0x04, 0x02, 0x00,  // LOAD_CONST_F64 pool[2] (10.0) MX
        0xC4, 0x5B, 0x03,  // BUILTIN LIMIT_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[1.0, f64::NAN, 10.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!(result.is_nan(), "expected NaN, got {result}");
}

#[test]
fn execute_when_limit_f64_nan_bound_then_nan() {
    // LIMIT(MN:=NaN, IN:=5.0, MX:=10.0) = NaN
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] (NaN)  MN
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (5.0)  IN
        0x04, 0x02, 0x00,  // LOAD_CONST_F64 pool[2] (10.0) MX
        0xC4, 0x5B, 0x03,  // BUILTIN LIMIT_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[f64::NAN, 5.0, 10.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!(result.is_nan(), "expected NaN, got {result}");
}
