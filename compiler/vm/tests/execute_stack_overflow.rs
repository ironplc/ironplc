//! Integration tests for stack overflow detection.

mod common;

use ironplc_container::opcode;
use ironplc_container::{ContainerBuilder, FunctionId};
use ironplc_vm::error::Trap;

#[test]
fn execute_when_stack_overflow_then_traps() {
    // Build bytecode that pushes more values than the max_stack_depth.
    // Use max_stack_depth=2 and push 3 values.
    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,  // push 1
        opcode::LOAD_CONST_I32, 0x00, 0x00,  // push 1
        opcode::LOAD_CONST_I32, 0x00, 0x00,  // push 1 — overflows stack of depth 2
        opcode::RET_VOID,
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(1)
        .add_function(FunctionId::INIT, &init_bytecode, 0, 1, 0)
        .add_function(FunctionId::SCAN, &bytecode, 2, 1, 0) // max_stack_depth=2
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .build();
    let mut b = common::VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    common::assert_trap(&mut vm, Trap::StackOverflow);
}

#[test]
fn execute_when_stack_underflow_then_traps() {
    // Try to pop from an empty stack by executing ADD_I32 with nothing on the stack.
    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::ADD_I32,  // pop two values from empty stack
        opcode::RET_VOID,
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_function(FunctionId::INIT, &init_bytecode, 0, 1, 0)
        .add_function(FunctionId::SCAN, &bytecode, 4, 1, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .build();
    let mut b = common::VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    common::assert_trap(&mut vm, Trap::StackUnderflow);
}

#[test]
fn execute_when_exactly_at_stack_limit_then_succeeds() {
    // max_stack_depth=2, push exactly 2 values, add them, store result.
    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,  // push 5
        opcode::LOAD_CONST_I32, 0x01, 0x00,  // push 10
        opcode::ADD_I32,                      // 5 + 10 = 15
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_i32_constant(5)
        .add_i32_constant(10)
        .add_function(FunctionId::INIT, &init_bytecode, 0, 1, 0)
        .add_function(FunctionId::SCAN, &bytecode, 2, 1, 0) // max_stack_depth=2
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .build();
    let mut b = common::VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(
        vm.read_variable(ironplc_container::VarIndex::new(0))
            .unwrap(),
        15
    );
}
