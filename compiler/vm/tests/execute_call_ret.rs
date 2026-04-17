//! Integration tests for CALL/RET opcodes (user function calls).

mod common;

use common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::{ContainerBuilder, FunctionId, VarIndex};
use ironplc_vm::error::Trap;

/// Helper: builds a container with init, scan, and one or more user functions.
/// The scan function is the entry point (function ID 1).
/// User functions start at function ID 2.
fn call_container(
    scan_bytecode: &[u8],
    user_functions: &[(
        /*bytecode*/ &[u8],
        /*max_stack*/ u16,
        /*num_locals*/ u16,
        /*num_params*/ u16,
    )],
    num_vars: u16,
    i32_constants: &[i32],
) -> ironplc_container::Container {
    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    let mut builder = ContainerBuilder::new().num_variables(num_vars);
    for &c in i32_constants {
        builder = builder.add_i32_constant(c);
    }
    builder = builder
        .add_function(FunctionId::INIT, &init_bytecode, 0, num_vars, 0)
        .add_function(FunctionId::SCAN, scan_bytecode, 16, num_vars, 0);
    for (i, (bytecode, max_stack, num_locals, num_params)) in user_functions.iter().enumerate() {
        builder = builder.add_function(
            FunctionId::new((i + 2) as u16),
            bytecode,
            *max_stack,
            *num_locals,
            *num_params,
        );
    }
    builder
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .build()
}

#[test]
fn execute_when_call_function_with_return_value_then_correct() {
    // Function 2: takes one i32 param at var[2], doubles it, returns via RET.
    // var_offset=2, num_locals=1, num_params=1
    #[rustfmt::skip]
    let func_body: Vec<u8> = vec![
        opcode::LOAD_VAR_I32, 0x02, 0x00,   // load param var[2]
        opcode::LOAD_VAR_I32, 0x02, 0x00,   // load param var[2] again
        opcode::ADD_I32,                     // param + param = 2*param
        opcode::RET,                         // return result on stack
    ];

    // Scan: push arg 21, call function 2, store result to var[0]
    // CALL operands: func_id=2, var_offset=2
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,  // push 21
        opcode::CALL, 0x02, 0x00, 0x02, 0x00, // call func 2, var_offset=2
        opcode::STORE_VAR_I32, 0x00, 0x00,    // store result to var[0]
        opcode::RET_VOID,
    ];

    let c = call_container(&scan_bytecode, &[(&func_body, 4, 1, 1)], 3, &[21]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    // 21 * 2 = 42
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 42);
}

#[test]
fn execute_when_call_function_with_two_params_then_correct() {
    // Function 2: takes two i32 params at var[2] and var[3], returns their sum.
    // var_offset=2, num_locals=2, num_params=2
    #[rustfmt::skip]
    let func_body: Vec<u8> = vec![
        opcode::LOAD_VAR_I32, 0x02, 0x00,   // load param a (var[2])
        opcode::LOAD_VAR_I32, 0x03, 0x00,   // load param b (var[3])
        opcode::ADD_I32,                     // a + b
        opcode::RET,                         // return
    ];

    // Scan: push 30, push 12, call function 2
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,  // push 30
        opcode::LOAD_CONST_I32, 0x01, 0x00,  // push 12
        opcode::CALL, 0x02, 0x00, 0x02, 0x00, // call func 2, var_offset=2
        opcode::STORE_VAR_I32, 0x00, 0x00,    // store result to var[0]
        opcode::RET_VOID,
    ];

    let c = call_container(&scan_bytecode, &[(&func_body, 4, 2, 2)], 4, &[30, 12]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 42);
}

#[test]
fn execute_when_nested_call_then_correct() {
    // Function 2 (inner): takes 1 param at var[4], returns param + 1
    // var_offset=4, num_locals=1, num_params=1
    #[rustfmt::skip]
    let inner_body: Vec<u8> = vec![
        opcode::LOAD_VAR_I32, 0x04, 0x00,   // load param (var[4])
        opcode::LOAD_CONST_I32, 0x01, 0x00,  // push constant 1
        opcode::ADD_I32,                     // param + 1
        opcode::RET,                         // return
    ];

    // Function 3 (outer): takes 1 param at var[2], calls inner(param), returns result * 2
    // var_offset=2, num_locals=1, num_params=1
    #[rustfmt::skip]
    let outer_body: Vec<u8> = vec![
        opcode::LOAD_VAR_I32, 0x02, 0x00,   // load param (var[2])
        opcode::CALL, 0x02, 0x00, 0x04, 0x00, // call inner (func 2), var_offset=4
        // result of inner is now on stack
        opcode::LOAD_VAR_I32, 0x02, 0x00,   // load param (var[2]) again (value still there)
        opcode::CALL, 0x02, 0x00, 0x04, 0x00, // call inner again
        opcode::ADD_I32,                     // inner(param) + inner(param)
        opcode::RET,                         // return
    ];

    // Scan: push 10, call outer, store result
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,  // push 10
        opcode::CALL, 0x03, 0x00, 0x02, 0x00, // call outer (func 3), var_offset=2
        opcode::STORE_VAR_I32, 0x00, 0x00,    // store result to var[0]
        opcode::RET_VOID,
    ];

    let c = call_container(
        &scan_bytecode,
        &[
            (&inner_body, 4, 1, 1), // func 2
            (&outer_body, 8, 1, 1), // func 3
        ],
        5,        // vars: 0=result, 1=unused, 2=outer.param, 3=unused, 4=inner.param
        &[10, 1], // constants: 10, 1
    );
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    // inner(10) = 11, outer(10) = inner(10) + inner(10) = 11 + 11 = 22
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 22);
}

#[test]
fn execute_when_call_invalid_function_id_then_traps() {
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,   // push dummy arg
        opcode::CALL, 0xFF, 0x00, 0x00, 0x00, // call non-existent func 255
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];

    let c = call_container(&scan_bytecode, &[], 1, &[0]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    common::assert_trap(&mut vm, Trap::InvalidFunctionId(FunctionId::new(255)));
}

#[test]
fn execute_when_call_void_function_then_no_return_value() {
    // Function 2: stores constant into var[2], returns void.
    // var_offset=2, num_locals=1, num_params=0
    #[rustfmt::skip]
    let func_body: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x01, 0x00,  // push constant 99
        opcode::STORE_VAR_I32, 0x02, 0x00,   // store to var[2]
        opcode::RET_VOID,
    ];

    // Scan: call function 2 (void), then copy var[2] to var[0]
    #[rustfmt::skip]
    let scan_bytecode: Vec<u8> = vec![
        opcode::CALL, 0x02, 0x00, 0x02, 0x00, // call func 2, var_offset=2
        opcode::LOAD_VAR_I32, 0x02, 0x00,      // load var[2]
        opcode::STORE_VAR_I32, 0x00, 0x00,      // store to var[0]
        opcode::RET_VOID,
    ];

    let c = call_container(&scan_bytecode, &[(&func_body, 4, 1, 0)], 3, &[0, 99]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 99);
}
