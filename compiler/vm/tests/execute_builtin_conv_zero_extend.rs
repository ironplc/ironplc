//! Integration tests for type conversion BUILTIN opcodes: zero extension.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

// --- CONV_U32_TO_I64 ---

#[test]
fn execute_when_conv_u32_to_i64_small_then_correct() {
    // CONV_U32_TO_I64(42) = 42
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        0xC4, 0x90, 0x03,  // BUILTIN CONV_U32_TO_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
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
    let result = b.vars[0].as_i64();
    assert_eq!(result, 42, "expected 42, got {result}");
}

#[test]
fn execute_when_conv_u32_to_i64_large_unsigned_then_correct() {
    // CONV_U32_TO_I64(0xFFFFFFFF) = 4294967295
    // 0xFFFFFFFF is stored as -1 in i32
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
        0xC4, 0x90, 0x03,  // BUILTIN CONV_U32_TO_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
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
    let result = b.vars[0].as_i64();
    assert_eq!(result, 4_294_967_295, "expected 4294967295, got {result}");
}
