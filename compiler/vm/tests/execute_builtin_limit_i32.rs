//! Integration tests for the BUILTIN LIMIT_I32 opcode.

mod common;

use common::{single_function_container, VmBuffers};

#[test]
fn execute_when_limit_i32_in_range_then_unchanged() {
    // LIMIT(MN:=0, IN:=5, MX:=10) = 5
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0)   MN
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (5)   IN
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (10)  MX
        0xC4, 0x46, 0x03,  // BUILTIN LIMIT_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0, 5, 10]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 5);
}

#[test]
fn execute_when_limit_i32_below_min_then_clamped() {
    // LIMIT(MN:=0, IN:=-5, MX:=10) = 0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0)   MN
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (-5)  IN
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (10)  MX
        0xC4, 0x46, 0x03,  // BUILTIN LIMIT_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0, -5, 10]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0);
}

#[test]
fn execute_when_limit_i32_above_max_then_clamped() {
    // LIMIT(MN:=0, IN:=15, MX:=10) = 10
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0)   MN
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (15)  IN
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (10)  MX
        0xC4, 0x46, 0x03,  // BUILTIN LIMIT_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0, 15, 10]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 10);
}

#[test]
fn execute_when_limit_i32_at_boundary_then_unchanged() {
    // LIMIT(MN:=0, IN:=0, MX:=10) = 0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0)   MN
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0)   IN
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (10)  MX
        0xC4, 0x46, 0x03,  // BUILTIN LIMIT_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0, 10]);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 0);
}
