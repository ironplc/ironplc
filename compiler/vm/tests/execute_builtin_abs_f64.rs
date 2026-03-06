//! Integration tests for the BUILTIN ABS_F64 opcode.

mod common;

use common::{single_function_container_f64, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_abs_f64_positive_then_unchanged() {
    // ABS(3.5) = 3.5
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] (3.5)
        0xC4, 0x55, 0x03,  // BUILTIN ABS_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[3.5]);
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
            .start()
            .unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!((result - 3.5).abs() < 1e-12, "expected 3.5, got {result}");
}

#[test]
fn execute_when_abs_f64_negative_then_positive() {
    // ABS(-7.25) = 7.25
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] (-7.25)
        0xC4, 0x55, 0x03,  // BUILTIN ABS_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[-7.25]);
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
            .start()
            .unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!((result - 7.25).abs() < 1e-12, "expected 7.25, got {result}");
}

#[test]
fn execute_when_abs_f64_zero_then_zero() {
    // ABS(0.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] (0.0)
        0xC4, 0x55, 0x03,  // BUILTIN ABS_F64
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
            .start()
            .unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!((result - 0.0).abs() < 1e-12, "expected 0.0, got {result}");
}
