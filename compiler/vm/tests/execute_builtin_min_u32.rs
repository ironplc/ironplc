//! Integration tests for the BUILTIN MIN_U32 opcode.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_min_u32_large_values_then_unsigned_comparison() {
    // MIN(3_000_000_000_u32, 1_000_000_000_u32) = 1_000_000_000
    // 3_000_000_000_u32 stored as i32 = -1_294_967_296
    // Signed min would wrongly pick -1_294_967_296, but unsigned min picks 1_000_000_000.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (3_000_000_000 as i32)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (1_000_000_000)
        0xC4, 0x66, 0x03,  // BUILTIN MIN_U32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[3_000_000_000_u32 as i32, 1_000_000_000]);
    let mut b = VmBuffers::from_container(&c);
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
    assert_eq!(vm.read_variable(0).unwrap() as u32, 1_000_000_000);
}

#[test]
fn execute_when_min_u32_both_large_then_smaller_unsigned() {
    // MIN(4_000_000_000_u32, 3_000_000_000_u32) = 3_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (4_000_000_000 as i32)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (3_000_000_000 as i32)
        0xC4, 0x66, 0x03,  // BUILTIN MIN_U32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(
        &bytecode,
        1,
        &[4_000_000_000_u32 as i32, 3_000_000_000_u32 as i32],
    );
    let mut b = VmBuffers::from_container(&c);
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
    assert_eq!(vm.read_variable(0).unwrap() as u32, 3_000_000_000);
}
