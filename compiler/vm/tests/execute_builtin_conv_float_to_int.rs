//! Integration tests for type conversion BUILTIN opcodes: float to integer.

mod common;

use common::{single_function_container_f32, single_function_container_f64, VmBuffers};
use ironplc_vm::Vm;

// --- CONV_F32_TO_I32 ---

#[test]
fn execute_when_conv_f32_to_i32_truncation_then_correct() {
    // CONV_F32_TO_I32(3.7) = 3 (truncation toward zero)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x86, 0x03,  // BUILTIN CONV_F32_TO_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[3.7]);
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
    let result = b.vars[0].as_i32();
    assert_eq!(result, 3, "expected 3, got {result}");
}

#[test]
fn execute_when_conv_f32_to_i32_negative_then_correct() {
    // CONV_F32_TO_I32(-7.9) = -7 (truncation toward zero)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x86, 0x03,  // BUILTIN CONV_F32_TO_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[-7.9]);
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
    let result = b.vars[0].as_i32();
    assert_eq!(result, -7, "expected -7, got {result}");
}

// --- CONV_F64_TO_I32 ---

#[test]
fn execute_when_conv_f64_to_i32_then_correct() {
    // CONV_F64_TO_I32(99.99) = 99
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x88, 0x03,  // BUILTIN CONV_F64_TO_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[99.99]);
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
    let result = b.vars[0].as_i32();
    assert_eq!(result, 99, "expected 99, got {result}");
}

// --- CONV_F64_TO_I64 ---

#[test]
fn execute_when_conv_f64_to_i64_then_correct() {
    // CONV_F64_TO_I64(5_000_000_000.7) = 5_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x04, 0x00, 0x00,  // LOAD_CONST_F64 pool[0]
        0xC4, 0x89, 0x03,  // BUILTIN CONV_F64_TO_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f64(&bytecode, 1, &[5_000_000_000.7]);
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
    let result = b.vars[0].as_i64();
    assert_eq!(result, 5_000_000_000, "expected 5000000000, got {result}");
}

// --- CONV_F32_TO_U32 ---

#[test]
fn execute_when_conv_f32_to_u32_then_correct() {
    // CONV_F32_TO_U32(100.5) = 100 (truncation)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x03, 0x00, 0x00,  // LOAD_CONST_F32 pool[0]
        0xC4, 0x8A, 0x03,  // BUILTIN CONV_F32_TO_U32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_f32(&bytecode, 1, &[100.5]);
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
    let result = b.vars[0].as_i32() as u32;
    assert_eq!(result, 100, "expected 100, got {result}");
}
