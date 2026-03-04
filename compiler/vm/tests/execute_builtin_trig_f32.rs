//! Integration tests for trig BUILTIN opcodes (F32 variant).

mod common;

use common::{single_function_container_f32, VmBuffers};
use ironplc_vm::Vm;

// --- SIN_F32 ---

#[test]
fn execute_when_sin_f32_zero_then_zero() {
    // SIN(0.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x72, 0x03,  // BUILTIN SIN_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[0.0]);
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
    let result = b.vars[0].as_f32();
    assert!((result - 0.0).abs() < 1e-5, "expected 0.0, got {result}");
}

#[test]
fn execute_when_sin_f32_pi_over_2_then_one() {
    // SIN(PI/2) = 1.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x72, 0x03,  // BUILTIN SIN_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[1.5707964]);
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
    let result = b.vars[0].as_f32();
    assert!((result - 1.0).abs() < 1e-5, "expected 1.0, got {result}");
}

// --- COS_F32 ---

#[test]
fn execute_when_cos_f32_zero_then_one() {
    // COS(0.0) = 1.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x74, 0x03,  // BUILTIN COS_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[0.0]);
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
    let result = b.vars[0].as_f32();
    assert!((result - 1.0).abs() < 1e-5, "expected 1.0, got {result}");
}

#[test]
fn execute_when_cos_f32_pi_then_neg_one() {
    // COS(PI) = -1.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x74, 0x03,  // BUILTIN COS_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[std::f32::consts::PI]);
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
    let result = b.vars[0].as_f32();
    assert!(
        (result - (-1.0)).abs() < 1e-5,
        "expected -1.0, got {result}"
    );
}

// --- TAN_F32 ---

#[test]
fn execute_when_tan_f32_zero_then_zero() {
    // TAN(0.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x76, 0x03,  // BUILTIN TAN_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[0.0]);
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
    let result = b.vars[0].as_f32();
    assert!((result - 0.0).abs() < 1e-5, "expected 0.0, got {result}");
}

#[test]
fn execute_when_tan_f32_pi_over_4_then_one() {
    // TAN(PI/4) = 1.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x76, 0x03,  // BUILTIN TAN_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[std::f32::consts::FRAC_PI_4]);
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
    let result = b.vars[0].as_f32();
    assert!((result - 1.0).abs() < 1e-4, "expected 1.0, got {result}");
}

// --- ASIN_F32 ---

#[test]
fn execute_when_asin_f32_zero_then_zero() {
    // ASIN(0.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x78, 0x03,  // BUILTIN ASIN_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[0.0]);
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
    let result = b.vars[0].as_f32();
    assert!((result - 0.0).abs() < 1e-5, "expected 0.0, got {result}");
}

#[test]
fn execute_when_asin_f32_one_then_pi_over_2() {
    // ASIN(1.0) = PI/2
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x78, 0x03,  // BUILTIN ASIN_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[1.0]);
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
    let result = b.vars[0].as_f32();
    assert!(
        (result - 1.5707964).abs() < 1e-5,
        "expected ~1.5707964, got {result}"
    );
}

// --- ACOS_F32 ---

#[test]
fn execute_when_acos_f32_one_then_zero() {
    // ACOS(1.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x7A, 0x03,  // BUILTIN ACOS_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[1.0]);
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
    let result = b.vars[0].as_f32();
    assert!((result - 0.0).abs() < 1e-5, "expected 0.0, got {result}");
}

#[test]
fn execute_when_acos_f32_zero_then_pi_over_2() {
    // ACOS(0.0) = PI/2
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x7A, 0x03,  // BUILTIN ACOS_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[0.0]);
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
    let result = b.vars[0].as_f32();
    assert!(
        (result - 1.5707964).abs() < 1e-5,
        "expected ~1.5707964, got {result}"
    );
}

// --- ATAN_F32 ---

#[test]
fn execute_when_atan_f32_zero_then_zero() {
    // ATAN(0.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x7C, 0x03,  // BUILTIN ATAN_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[0.0]);
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
    let result = b.vars[0].as_f32();
    assert!((result - 0.0).abs() < 1e-5, "expected 0.0, got {result}");
}

#[test]
fn execute_when_atan_f32_one_then_pi_over_4() {
    // ATAN(1.0) = PI/4
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x7C, 0x03,  // BUILTIN ATAN_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[1.0]);
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
    let result = b.vars[0].as_f32();
    assert!(
        (result - std::f32::consts::FRAC_PI_4).abs() < 1e-4,
        "expected ~pi/4, got {result}"
    );
}
