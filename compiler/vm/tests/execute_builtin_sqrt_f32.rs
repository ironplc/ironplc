//! Integration tests for the BUILTIN SQRT_F32 opcode.

mod common;

use common::{single_function_container_f32, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_sqrt_f32_perfect_square_then_exact() {
    // SQRT(9.0) = 3.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] (9.0)
        0xC4, 0x5E, 0x03,  // BUILTIN SQRT_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[9.0]);
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
    let result = b.vars[0].as_f32();
    assert!((result - 3.0).abs() < 1e-5, "expected 3.0, got {result}");
}

#[test]
fn execute_when_sqrt_f32_zero_then_zero() {
    // SQRT(0.0) = 0.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] (0.0)
        0xC4, 0x5E, 0x03,  // BUILTIN SQRT_F32
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
    let result = b.vars[0].as_f32();
    assert!((result - 0.0).abs() < 1e-5, "expected 0.0, got {result}");
}

#[test]
fn execute_when_sqrt_f32_negative_then_nan() {
    // SQRT(-1.0) = NaN
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0] (-1.0)
        0xC4, 0x5E, 0x03,  // BUILTIN SQRT_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[-1.0]);
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
    let result = b.vars[0].as_f32();
    assert!(result.is_nan(), "expected NaN, got {result}");
}
