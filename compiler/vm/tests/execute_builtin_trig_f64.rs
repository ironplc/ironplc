//! Integration tests for trig BUILTIN opcodes (F64 variant).

mod common;

use common::{single_function_container_f64, VmBuffers};
use ironplc_vm::Vm;

// --- SIN_F64 ---

#[test]
fn execute_when_sin_f64_zero_then_zero() {
    // SIN(0.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x73, 0x03,  // BUILTIN SIN_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[0.0]);
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
    assert!((result - 0.0).abs() < 1e-12, "expected 0.0, got {result}");
}

#[test]
fn execute_when_sin_f64_pi_over_2_then_one() {
    // SIN(PI/2) = 1.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x73, 0x03,  // BUILTIN SIN_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[std::f64::consts::FRAC_PI_2]);
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

// --- COS_F64 ---

#[test]
fn execute_when_cos_f64_zero_then_one() {
    // COS(0.0) = 1.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x75, 0x03,  // BUILTIN COS_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[0.0]);
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
fn execute_when_cos_f64_pi_then_neg_one() {
    // COS(PI) = -1.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x75, 0x03,  // BUILTIN COS_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[std::f64::consts::PI]);
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
    assert!(
        (result - (-1.0)).abs() < 1e-12,
        "expected -1.0, got {result}"
    );
}

// --- TAN_F64 ---

#[test]
fn execute_when_tan_f64_zero_then_zero() {
    // TAN(0.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x77, 0x03,  // BUILTIN TAN_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[0.0]);
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
    assert!((result - 0.0).abs() < 1e-12, "expected 0.0, got {result}");
}

#[test]
fn execute_when_tan_f64_pi_over_4_then_one() {
    // TAN(PI/4) = 1.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x77, 0x03,  // BUILTIN TAN_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[std::f64::consts::FRAC_PI_4]);
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

// --- ASIN_F64 ---

#[test]
fn execute_when_asin_f64_zero_then_zero() {
    // ASIN(0.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x79, 0x03,  // BUILTIN ASIN_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[0.0]);
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
    assert!((result - 0.0).abs() < 1e-12, "expected 0.0, got {result}");
}

#[test]
fn execute_when_asin_f64_one_then_pi_over_2() {
    // ASIN(1.0) = FRAC_PI_2
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x79, 0x03,  // BUILTIN ASIN_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[1.0]);
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
    assert!(
        (result - std::f64::consts::FRAC_PI_2).abs() < 1e-12,
        "expected FRAC_PI_2, got {result}"
    );
}

// --- ACOS_F64 ---

#[test]
fn execute_when_acos_f64_one_then_zero() {
    // ACOS(1.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x7B, 0x03,  // BUILTIN ACOS_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[1.0]);
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
    assert!((result - 0.0).abs() < 1e-12, "expected 0.0, got {result}");
}

#[test]
fn execute_when_acos_f64_zero_then_pi_over_2() {
    // ACOS(0.0) = FRAC_PI_2
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x7B, 0x03,  // BUILTIN ACOS_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[0.0]);
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
    assert!(
        (result - std::f64::consts::FRAC_PI_2).abs() < 1e-12,
        "expected FRAC_PI_2, got {result}"
    );
}

// --- ATAN_F64 ---

#[test]
fn execute_when_atan_f64_zero_then_zero() {
    // ATAN(0.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x7D, 0x03,  // BUILTIN ATAN_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[0.0]);
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
    assert!((result - 0.0).abs() < 1e-12, "expected 0.0, got {result}");
}

#[test]
fn execute_when_atan_f64_one_then_pi_over_4() {
    // ATAN(1.0) = FRAC_PI_4
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x7D, 0x03,  // BUILTIN ATAN_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[1.0]);
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
    assert!(
        (result - std::f64::consts::FRAC_PI_4).abs() < 1e-12,
        "expected FRAC_PI_4, got {result}"
    );
}
