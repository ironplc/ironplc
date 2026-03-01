//! Integration tests for the JMP and JMP_IF_NOT opcodes.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_jmp_then_skips_instruction() {
    // JMP skips over a STORE_VAR_I32, so var[0] stays 0.
    // var[1] gets set to 99 after the jump target.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0xB0, 0x03, 0x00,       // JMP offset:+3 (skip next instruction)
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0] (99) -- skipped
        // jump target (offset 6):
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0] (99)
        0x18, 0x01, 0x00,       // STORE_VAR_I32 var[1]
        0xB5,                   // RET_VOID
    ];
    let c = single_function_container(&bytecode, 2, &[99]);
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
    assert_eq!(vm.read_variable(0).unwrap(), 0); // untouched
    assert_eq!(vm.read_variable(1).unwrap(), 99);
}

#[test]
fn execute_when_jmp_if_not_true_then_no_jump() {
    // Condition is 1 (true), so JMP_IF_NOT does NOT jump.
    // Falls through to store 42 into var[0].
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0] (1) -- condition
        0xB2, 0x06, 0x00,       // JMP_IF_NOT offset:+6 (skip to RET_VOID)
        0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1] (42)
        0x18, 0x00, 0x00,       // STORE_VAR_I32 var[0]
        0xB5,                   // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[1, 42]);
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
    assert_eq!(vm.read_variable(0).unwrap(), 42);
}

#[test]
fn execute_when_jmp_if_not_false_then_jumps() {
    // Condition is 0 (false), so JMP_IF_NOT jumps past the store.
    // var[0] stays 0.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0] (0) -- condition
        0xB2, 0x06, 0x00,       // JMP_IF_NOT offset:+6 (skip to RET_VOID)
        0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1] (42) -- skipped
        0x18, 0x00, 0x00,       // STORE_VAR_I32 var[0]       -- skipped
        0xB5,                   // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0, 42]);
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
    assert_eq!(vm.read_variable(0).unwrap(), 0);
}

#[test]
fn execute_when_if_else_true_then_takes_then_branch() {
    // IF (1) THEN var[0] := 10 ELSE var[0] := 20 END_IF
    // Condition is true, so var[0] = 10.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        // IF condition:
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0] (1)
        0xB2, 0x09, 0x00,       // JMP_IF_NOT offset:+9 -> else branch (offset 15)
        // THEN body:
        0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1] (10)
        0x18, 0x00, 0x00,       // STORE_VAR_I32 var[0]
        0xB0, 0x06, 0x00,       // JMP offset:+6 -> end (offset 21)
        // ELSE body (offset 15):
        0x01, 0x02, 0x00,       // LOAD_CONST_I32 pool[2] (20)
        0x18, 0x00, 0x00,       // STORE_VAR_I32 var[0]
        // END (offset 21):
        0xB5,                   // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[1, 10, 20]);
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
    assert_eq!(vm.read_variable(0).unwrap(), 10);
}

#[test]
fn execute_when_if_else_false_then_takes_else_branch() {
    // IF (0) THEN var[0] := 10 ELSE var[0] := 20 END_IF
    // Condition is false, so var[0] = 20.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        // IF condition:
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0] (0)
        0xB2, 0x09, 0x00,       // JMP_IF_NOT offset:+9 -> else branch (offset 15)
        // THEN body:
        0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1] (10)
        0x18, 0x00, 0x00,       // STORE_VAR_I32 var[0]
        0xB0, 0x06, 0x00,       // JMP offset:+6 -> end (offset 21)
        // ELSE body (offset 15):
        0x01, 0x02, 0x00,       // LOAD_CONST_I32 pool[2] (20)
        0x18, 0x00, 0x00,       // STORE_VAR_I32 var[0]
        // END (offset 21):
        0xB5,                   // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0, 10, 20]);
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
    assert_eq!(vm.read_variable(0).unwrap(), 20);
}
