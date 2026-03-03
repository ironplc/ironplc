//! Integration tests for the BUILTIN SEL_I64 opcode.

mod common;

use common::{single_function_container_i32_i64, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_sel_i64_false_then_returns_in0() {
    // SEL(G:=0, IN0:=5_000_000_000, IN1:=10_000_000_000) = 5_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0)                G
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (5_000_000_000)   IN0
        0x02, 0x02, 0x00,  // LOAD_CONST_I64 pool[2] (10_000_000_000)  IN1
        0xC4, 0x65, 0x03,  // BUILTIN SEL_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_i64(&bytecode, 1, &[0], &[5_000_000_000, 10_000_000_000]);
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
    assert_eq!(b.vars[0].as_i64(), 5_000_000_000);
}

#[test]
fn execute_when_sel_i64_true_then_returns_in1() {
    // SEL(G:=1, IN0:=5_000_000_000, IN1:=10_000_000_000) = 10_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1)                G
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (5_000_000_000)   IN0
        0x02, 0x02, 0x00,  // LOAD_CONST_I64 pool[2] (10_000_000_000)  IN1
        0xC4, 0x65, 0x03,  // BUILTIN SEL_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_i64(&bytecode, 1, &[1], &[5_000_000_000, 10_000_000_000]);
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
