//! Integration tests for loop patterns using JMP/JMP_IF_NOT opcodes.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_while_true_three_iterations_then_loops() {
    // WHILE var[0] > 0 DO var[0] := var[0] - 1 END_WHILE
    // var[0] starts at 3, should end at 0.
    // Constants: pool[0]=0, pool[1]=1
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        // LOOP (offset 0):
        0x10, 0x00, 0x00,       // LOAD_VAR_I32 var[0]
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0] (0)
        0x6C,                   // GT_I32
        0xB2, 0x0D, 0x00,       // JMP_IF_NOT +13 -> END (offset 23)
        // body:
        0x10, 0x00, 0x00,       // LOAD_VAR_I32 var[0]
        0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1] (1)
        0x31,                   // SUB_I32
        0x18, 0x00, 0x00,       // STORE_VAR_I32 var[0]
        0xB0, 0xE9, 0xFF,       // JMP -23 -> LOOP (offset 0)
        // END (offset 23):
        0xB5,                   // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0, 1]);
    let mut b = VmBuffers::from_container(&c);
    b.vars[0] = ironplc_vm::Slot::from_i32(3);
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
fn execute_when_while_false_then_skips_body() {
    // WHILE FALSE DO var[0] := 99 END_WHILE
    // var[0] stays 0.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        // LOOP (offset 0):
        0x08,                   // LOAD_FALSE
        0xB2, 0x09, 0x00,       // JMP_IF_NOT +9 -> END (offset 13)
        // body:
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0] (99)
        0x18, 0x00, 0x00,       // STORE_VAR_I32 var[0]
        0xB0, 0xF3, 0xFF,       // JMP -13 -> LOOP (offset 0)
        // END (offset 13):
        0xB5,                   // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[99]);
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
fn execute_when_repeat_until_then_loops_twice() {
    // var[0] starts at 0.
    // REPEAT var[0] := var[0] + 1 UNTIL var[0] >= 2 END_REPEAT
    // Constants: pool[0]=1, pool[1]=2
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        // LOOP (offset 0):
        0x10, 0x00, 0x00,       // LOAD_VAR_I32 var[0]
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0] (1)
        0x30,                   // ADD_I32
        0x18, 0x00, 0x00,       // STORE_VAR_I32 var[0]
        // condition: var[0] >= 2
        0x10, 0x00, 0x00,       // LOAD_VAR_I32 var[0]
        0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1] (2)
        0x6D,                   // GE_I32
        0xB2, 0xEC, 0xFF,       // JMP_IF_NOT -20 -> LOOP (offset 0)
        // END (offset 20):
        0xB5,                   // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[1, 2]);
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
    assert_eq!(vm.read_variable(0).unwrap(), 2);
}

#[test]
fn execute_when_for_loop_then_iterates_correctly() {
    // FOR var[0] := 1 TO 3 BY 1 DO var[1] := var[1] + var[0] END_FOR
    // var[1] = 1+2+3 = 6
    // Constants: pool[0]=1, pool[1]=3
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        // init: var[0] := 1
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0] (1)
        0x18, 0x00, 0x00,       // STORE_VAR_I32 var[0]
        // LOOP (offset 6):
        0x10, 0x00, 0x00,       // LOAD_VAR_I32 var[0]
        0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1] (3)
        0x6C,                   // GT_I32
        0xB2, 0x03, 0x00,       // JMP_IF_NOT +3 -> BODY (offset 19)
        0xB0, 0x17, 0x00,       // JMP +23 -> END (offset 42)
        // BODY (offset 19):
        0x10, 0x01, 0x00,       // LOAD_VAR_I32 var[1]
        0x10, 0x00, 0x00,       // LOAD_VAR_I32 var[0]
        0x30,                   // ADD_I32
        0x18, 0x01, 0x00,       // STORE_VAR_I32 var[1]
        // increment: var[0] := var[0] + 1
        0x10, 0x00, 0x00,       // LOAD_VAR_I32 var[0]
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0] (1)
        0x30,                   // ADD_I32
        0x18, 0x00, 0x00,       // STORE_VAR_I32 var[0]
        0xB0, 0xDC, 0xFF,       // JMP -36 -> LOOP (offset 6)
        // END (offset 42):
        0xB5,                   // RET_VOID
    ];
    let c = single_function_container(&bytecode, 2, &[1, 3]);
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
    assert_eq!(vm.read_variable(0).unwrap(), 4); // control ends at 4 (first value > 3)
    assert_eq!(vm.read_variable(1).unwrap(), 6); // sum = 1+2+3
}

#[test]
fn execute_when_backward_jump_then_loops() {
    // Test backward JMP via a two-iteration loop (REPEAT pattern).
    // var[0]=0 initially. Increment var[0], check if >= 2, if not jump back.
    // Constants: pool[0]=1, pool[1]=2
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        // LOOP (offset 0):
        0x10, 0x00, 0x00,       // LOAD_VAR_I32 var[0]
        0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0] (1)
        0x30,                   // ADD_I32
        0x18, 0x00, 0x00,       // STORE_VAR_I32 var[0]
        // check: var[0] >= 2
        0x10, 0x00, 0x00,       // LOAD_VAR_I32 var[0]
        0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1] (2)
        0x6D,                   // GE_I32
        0xB2, 0xEC, 0xFF,       // JMP_IF_NOT -20 -> LOOP (offset 0)
        // END (offset 20):
        0xB5,                   // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[1, 2]);
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
    assert_eq!(vm.read_variable(0).unwrap(), 2);
}
