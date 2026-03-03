//! Integration tests for the BUILTIN MAX_I64 opcode.

mod common;

use common::{single_function_container_i64, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_max_i64_first_larger_then_returns_first() {
    // MAX(10_000_000_000, 5_000_000_000) = 10_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (10_000_000_000)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (5_000_000_000)
        0xC4, 0x63, 0x03,  // BUILTIN MAX_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i64(&bytecode, 1, &[10_000_000_000, 5_000_000_000]);
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
    assert_eq!(b.vars[0].as_i64(), 10_000_000_000);
}
