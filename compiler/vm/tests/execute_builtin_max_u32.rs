//! Integration tests for the BUILTIN MAX_U32 opcode.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_max_u32_large_values_then_unsigned_comparison() {
    // MAX(3_000_000_000_u32, 1_000_000_000_u32) = 3_000_000_000
    // 3_000_000_000_u32 stored as i32 = -1_294_967_296
    // Signed max would wrongly pick 1_000_000_000, but unsigned max picks 3_000_000_000.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (3_000_000_000 as i32)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (1_000_000_000)
        0xC4, 0x67, 0x03,  // BUILTIN MAX_U32
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
    assert_eq!(vm.read_variable(0).unwrap() as u32, 3_000_000_000);
}
