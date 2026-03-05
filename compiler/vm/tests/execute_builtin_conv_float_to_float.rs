//! Integration tests for type conversion BUILTIN opcodes: float to float.

mod common;

use common::{single_function_container_f32, single_function_container_f64, VmBuffers};
use ironplc_vm::Vm;

// --- CONV_F32_TO_F64 ---

#[test]
fn execute_when_conv_f32_to_f64_then_correct() {
    // CONV_F32_TO_F64(1.5) ≈ 1.5
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x8E, 0x03,  // BUILTIN CONV_F32_TO_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[1.5]);
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
    // f32 1.5 promotes to f64 exactly
    assert!((result - 1.5).abs() < 1e-5, "expected ~1.5, got {result}");
}

// --- CONV_F64_TO_F32 ---

#[test]
fn execute_when_conv_f64_to_f32_then_correct() {
    // CONV_F64_TO_F32(9.876543210) ≈ 9.876543
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x8F, 0x03,  // BUILTIN CONV_F64_TO_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[9.876543210]);
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
        (result - 9.876543).abs() < 1e-4,
        "expected ~9.876543, got {result}"
    );
}
