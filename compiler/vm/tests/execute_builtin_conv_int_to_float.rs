//! Integration tests for type conversion BUILTIN opcodes: integer to float.

mod common;

use common::{single_function_container, single_function_container_i64, VmBuffers};
use ironplc_vm::Vm;

// --- CONV_I32_TO_F32 ---

#[test]
fn execute_when_conv_i32_to_f32_positive_then_correct() {
    // CONV_I32_TO_F32(42) = 42.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        0xC4, 0x7E, 0x03,  // BUILTIN CONV_I32_TO_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[42]);
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
    assert!((result - 42.0).abs() < 1e-5, "expected 42.0, got {result}");
}

#[test]
fn execute_when_conv_i32_to_f32_negative_then_correct() {
    // CONV_I32_TO_F32(-100) = -100.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        0xC4, 0x7E, 0x03,  // BUILTIN CONV_I32_TO_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[-100]);
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
    assert!(
        (result - (-100.0)).abs() < 1e-5,
        "expected -100.0, got {result}"
    );
}

// --- CONV_I32_TO_F64 ---

#[test]
fn execute_when_conv_i32_to_f64_then_correct() {
    // CONV_I32_TO_F64(1000) = 1000.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        0xC4, 0x7F, 0x03,  // BUILTIN CONV_I32_TO_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[1000]);
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
        (result - 1000.0).abs() < 1e-10,
        "expected 1000.0, got {result}"
    );
}

// --- CONV_I64_TO_F32 ---

#[test]
fn execute_when_conv_i64_to_f32_then_correct() {
    // CONV_I64_TO_F32(5_000_000_000) = 5e9
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0]
        0xC4, 0x80, 0x03,  // BUILTIN CONV_I64_TO_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i64(&bytecode, 1, &[5_000_000_000]);
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
    assert!((result - 5.0e9).abs() < 1e4, "expected ~5e9, got {result}");
}

// --- CONV_I64_TO_F64 ---

#[test]
fn execute_when_conv_i64_to_f64_then_correct() {
    // CONV_I64_TO_F64(5_000_000_000) = 5e9
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0]
        0xC4, 0x81, 0x03,  // BUILTIN CONV_I64_TO_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i64(&bytecode, 1, &[5_000_000_000]);
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
    assert!((result - 5.0e9).abs() < 1e-5, "expected 5e9, got {result}");
}

// --- CONV_U32_TO_F32 ---

#[test]
fn execute_when_conv_u32_to_f32_then_correct() {
    // CONV_U32_TO_F32(1000) = 1000.0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        0xC4, 0x82, 0x03,  // BUILTIN CONV_U32_TO_F32
        0x1A, 0x00, 0x00,  // STORE_VAR_F32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[1000]);
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
    assert!(
        (result - 1000.0).abs() < 1e-5,
        "expected 1000.0, got {result}"
    );
}

// --- CONV_U32_TO_F64 ---

#[test]
fn execute_when_conv_u32_to_f64_large_unsigned_then_correct() {
    // CONV_U32_TO_F64(0xFFFFFFFF) = 4294967295.0
    // 0xFFFFFFFF stored as -1 in i32
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        0xC4, 0x83, 0x03,  // BUILTIN CONV_U32_TO_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[-1_i32]);
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
        (result - 4_294_967_295.0).abs() < 1.0,
        "expected 4294967295.0, got {result}"
    );
}
