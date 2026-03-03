//! Integration tests for the BUILTIN EXPT_I64 opcode.

mod common;

use common::{assert_trap, single_function_container_i64, VmBuffers};
use ironplc_vm::error::Trap;
use ironplc_vm::Vm;

#[test]
fn execute_when_expt_i64_then_correct() {
    // 2 ** 40 = 1_099_511_627_776
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (2)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (40)
        0xC4, 0x60, 0x03,  // BUILTIN EXPT_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i64(&bytecode, 1, &[2, 40]);
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
    assert_eq!(b.vars[0].as_i64(), 1_099_511_627_776);
}

#[test]
fn execute_when_expt_i64_negative_exponent_then_traps() {
    // 2 ** -1 -> NegativeExponent trap
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (2)
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (-1)
        0xC4, 0x60, 0x03,  // BUILTIN EXPT_I64
    ];
    let c = single_function_container_i64(&bytecode, 0, &[2, -1]);
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
        .start();

    assert_trap(&mut vm, Trap::NegativeExponent);
}
