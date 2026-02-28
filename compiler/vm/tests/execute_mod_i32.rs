//! Integration tests for the MOD_I32 opcode.

mod common;

use common::{assert_trap, single_function_container, VmBuffers};
use ironplc_vm::error::Trap;
use ironplc_vm::Vm;

#[test]
fn execute_when_mod_i32_then_correct_result() {
    // 10 % 3 = 1
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (10)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (3)
        0x34,              // MOD_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[10, 3]);
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

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 1);
}

#[test]
fn execute_when_mod_i32_negative_then_truncates_toward_zero() {
    // -7 % 2 = -1 (truncates toward zero, not 1)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (-7)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (2)
        0x34,              // MOD_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[-7, 2]);
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

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), -1);
}

#[test]
fn execute_when_mod_i32_by_zero_then_trap() {
    // 10 % 0 → DivideByZero trap
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (10)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (0)
        0x34,              // MOD_I32
    ];
    let c = single_function_container(&bytecode, 0, &[10, 0]);
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

    assert_trap(&mut vm, Trap::DivideByZero);
}

#[test]
fn execute_when_mod_i32_min_by_neg_one_then_wraps() {
    // i32::MIN % -1 wraps to 0 (wrapping_rem behavior)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (i32::MIN)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (-1)
        0x34,              // MOD_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[i32::MIN, -1]);
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

    vm.run_round(0).unwrap();
    // wrapping_rem: i32::MIN % -1 wraps to 0
    assert_eq!(vm.read_variable(0).unwrap(), 0);
}
