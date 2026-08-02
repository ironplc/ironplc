//! Integration tests for stack overflow detection.

use ironplc_container::opcode;
use ironplc_container::{ContainerBuilder, FunctionId};
use ironplc_vm::error::Trap;
use spec_test_macro::spec_test;

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
        .max_call_depth(1)
        .build();
    let mut b = crate::common::VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    crate::common::assert_trap(&mut vm, Trap::StackOverflow);
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
        .max_call_depth(1)
        .build();
    let mut b = crate::common::VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    crate::common::assert_trap(&mut vm, Trap::StackUnderflow);
}

/// Builds a container whose SCAN function unconditionally calls itself and
/// declares the given per-program `max_call_depth`, then asserts that running
/// it traps cleanly with `Trap::CallStackOverflow`.
///
/// The frame buffer is sized from the container's `max_call_depth`
/// (`VmBuffers::from_container`), so unbounded self-recursion fills exactly
/// that many frames and the next CALL traps. Because the bound comes from the
/// container header — not a VM-wide constant — a small declared depth traps
/// after only a few frames, which is what makes this a per-program contract.
fn assert_self_recursion_traps_at_depth(max_call_depth: u16) {
    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    let scan_id = FunctionId::SCAN.to_le_bytes();
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::CALL, scan_id[0], scan_id[1], 0x00, 0x00, // CALL SCAN, var_offset=0
        opcode::RET_VOID,
    ];
    let c = ContainerBuilder::new()
        .num_variables(1)
        .add_function(FunctionId::INIT, &init_bytecode, 0, 1, 0)
        .add_function(FunctionId::SCAN, &bytecode, 4, 1, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .max_call_depth(max_call_depth)
        .build();
    let mut b = crate::common::VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    crate::common::assert_trap(&mut vm, Trap::CallStackOverflow);
}

/// REQ-RT-vm-001: a CALL that would push a frame beyond the container's
/// declared `max_call_depth` traps with `CALL_DEPTH_EXCEEDED`
/// (`Trap::CallStackOverflow`). Parameterised over several declared depths so
/// the test binds to the per-program contract rather than a hardcoded value:
/// a small declared depth (2) traps after only a few frames — impossible if
/// the guard were still the old VM-wide `MAX_CALL_DEPTH = 32` constant.
#[spec_test(REQ_RT_vm_001)]
fn vm_spec_req_rt_vm_001_call_recursion_exceeds_declared_depth_then_traps() {
    for max_call_depth in [2, 4, 8, 32] {
        assert_self_recursion_traps_at_depth(max_call_depth);
    }
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
        .max_call_depth(1)
        .build();
    let mut b = crate::common::VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(
        vm.read_variable(ironplc_container::VarIndex::new(0))
            .unwrap(),
        15
    );
}
