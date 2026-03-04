//! Integration tests for math BUILTIN opcodes (F32 variant).

mod common;

use common::{single_function_container_f32, VmBuffers};
use ironplc_vm::Vm;

// --- LN_F32 ---

#[test]
fn execute_when_ln_f32_e_then_one() {
    // LN(e) = 1.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x6C, 0x03,  // BUILTIN LN_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[std::f32::consts::E]);
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
    assert!((result - 1.0).abs() < 1e-4, "expected ~1.0, got {result}");
}

#[test]
fn execute_when_ln_f32_one_then_zero() {
    // LN(1.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x6C, 0x03,  // BUILTIN LN_F32
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

// --- LOG_F32 ---

#[test]
fn execute_when_log_f32_hundred_then_two() {
    // LOG(100.0) = 2.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x6E, 0x03,  // BUILTIN LOG_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[100.0]);
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
    assert!((result - 2.0).abs() < 1e-5, "expected 2.0, got {result}");
}

#[test]
fn execute_when_log_f32_one_then_zero() {
    // LOG(1.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x6E, 0x03,  // BUILTIN LOG_F32
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

// --- EXP_F32 ---

#[test]
fn execute_when_exp_f32_zero_then_one() {
    // EXP(0.0) = 1.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x70, 0x03,  // BUILTIN EXP_F32
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
fn execute_when_exp_f32_one_then_e() {
    // EXP(1.0) = e ≈ 2.718282
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x70, 0x03,  // BUILTIN EXP_F32
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
        (result - std::f32::consts::E).abs() < 1e-4,
        "expected ~e, got {result}"
    );
}
