//! Integration tests for the BUILTIN EXPT_I32 opcode.

mod common;

use common::{assert_trap, single_function_container, VmBuffers};
use ironplc_vm::error::Trap;
use ironplc_vm::Vm;

#[test]
fn execute_when_expt_i32_then_correct_result() {
    // 2 ** 10 = 1024
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (2)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (10)
        0xC4, 0x40, 0x03,  // BUILTIN EXPT_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[2, 10]);
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
    assert_eq!(vm.read_variable(0).unwrap(), 1024);
}

#[test]
fn execute_when_expt_i32_zero_exponent_then_one() {
    // 5 ** 0 = 1
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (5)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (0)
        0xC4, 0x40, 0x03,  // BUILTIN EXPT_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[5, 0]);
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
fn execute_when_expt_i32_negative_exponent_then_trap() {
    // 2 ** -1 → NegativeExponent trap
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (2)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (-1)
        0xC4, 0x40, 0x03,  // BUILTIN EXPT_I32
    ];
    let c = single_function_container(&bytecode, 0, &[2, -1]);
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

#[test]
fn execute_when_expt_i32_overflow_then_wraps() {
    // 2 ** 31 wraps to i32::MIN (-2147483648)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (2)
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (31)
        0xC4, 0x40, 0x03,  // BUILTIN EXPT_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[2, 31]);
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
    assert_eq!(vm.read_variable(0).unwrap(), i32::MIN);
}

#[test]
fn execute_when_invalid_builtin_func_id_then_trap() {
    // Unknown builtin func_id 0xFFFF → InvalidBuiltinFunction trap
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1)
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1)
        0xC4, 0xFF, 0xFF,  // BUILTIN 0xFFFF (unknown)
    ];
    let c = single_function_container(&bytecode, 0, &[1]);
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

    assert_trap(&mut vm, Trap::InvalidBuiltinFunction(0xFFFF));
}
