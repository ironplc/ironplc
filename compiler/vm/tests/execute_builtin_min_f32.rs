//! Integration tests for the BUILTIN MIN_F32 opcode.

mod common;

use common::{single_function_container_f32, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_min_f32_then_returns_smaller() {
    // MIN(2.5, 7.0) = 2.5
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] (2.5)
        0x03, 0x01, 0x00,  // LOAD_CONST_F32 pool[1] (7.0)
        0xC4, 0x56, 0x03,  // BUILTIN MIN_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[2.5, 7.0]);
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
    assert!((result - 2.5).abs() < 1e-5, "expected 2.5, got {result}");
}

#[test]
fn execute_when_min_f32_equal_then_returns_value() {
    // MIN(5.0, 5.0) = 5.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] (5.0)
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] (5.0)
        0xC4, 0x56, 0x03,  // BUILTIN MIN_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[5.0]);
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
    assert!((result - 5.0).abs() < 1e-5, "expected 5.0, got {result}");
}

#[test]
fn execute_when_min_f32_negative_vs_positive_then_returns_negative() {
    // MIN(-3.0, 7.0) = -3.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] (-3.0)
        0x03, 0x01, 0x00,  // LOAD_CONST_F32 pool[1] (7.0)
        0xC4, 0x56, 0x03,  // BUILTIN MIN_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[-3.0, 7.0]);
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
        (result - (-3.0)).abs() < 1e-5,
        "expected -3.0, got {result}"
    );
}
