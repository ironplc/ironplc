//! Integration tests for the BUILTIN MAX_U64 opcode.

mod common;

use common::{single_function_container_i64, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_max_u64_large_values_then_unsigned_comparison() {
    // MAX(10_000_000_000_000_000_000_u64, 5_000_000_000_u64) = 10_000_000_000_000_000_000
    // 10_000_000_000_000_000_000_u64 stored as i64 = -8_446_744_073_709_551_616
    // Signed max would wrongly pick 5B, but unsigned max picks 10e18.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (10e18 as i64)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (5_000_000_000)
        0xC4, 0x6A, 0x03,  // BUILTIN MAX_U64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i64(
        &bytecode,
        1,
        &[10_000_000_000_000_000_000_u64 as i64, 5_000_000_000],
    );
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
    assert_eq!(b.vars[0].as_i64() as u64, 10_000_000_000_000_000_000);
}
