//! Integration tests for the BUILTIN LIMIT_U64 opcode.

mod common;

use common::{single_function_container_i64, VmBuffers};

#[test]
fn execute_when_limit_u64_in_range_then_unchanged() {
    // LIMIT(MN:=1_000_000_000, IN:=5_000_000_000, MX:=10_000_000_000_000_000_000) = 5_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (1_000_000_000)     MN
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (5_000_000_000)     IN
        0x02, 0x02, 0x00,  // LOAD_CONST_I64 pool[2] (10e18 as i64)      MX
        0xC4, 0x6B, 0x03,  // BUILTIN LIMIT_U64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i64(
        &bytecode,
        1,
        &[
            1_000_000_000,
            5_000_000_000,
            10_000_000_000_000_000_000_u64 as i64,
        ],
    );
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i64() as u64, 5_000_000_000);
}
