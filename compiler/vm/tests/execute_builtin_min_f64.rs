//! Integration tests for the BUILTIN MIN_F64 opcode.

mod common;

use common::{single_function_container_f64, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_min_f64_then_returns_smaller() {
    // MIN(2.5, 7.0) = 2.5
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] (2.5)
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (7.0)
        0xC4, 0x57, 0x03,  // BUILTIN MIN_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[2.5, 7.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!((result - 2.5).abs() < 1e-12, "expected 2.5, got {result}");
}

#[test]
fn execute_when_min_f64_equal_then_returns_value() {
    // MIN(5.0, 5.0) = 5.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] (5.0)
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] (5.0)
        0xC4, 0x57, 0x03,  // BUILTIN MIN_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[5.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!((result - 5.0).abs() < 1e-12, "expected 5.0, got {result}");
}

#[test]
fn execute_when_min_f64_negative_vs_positive_then_returns_negative() {
    // MIN(-3.0, 7.0) = -3.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0] (-3.0)
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (7.0)
        0xC4, 0x57, 0x03,  // BUILTIN MIN_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[-3.0, 7.0]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_f64();
    assert!(
        (result - (-3.0)).abs() < 1e-12,
        "expected -3.0, got {result}"
    );
}
