//! Integration tests for the BUILTIN SEL_I32 opcode.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_sel_i32_g_zero_then_returns_in0() {
    // SEL(G:=0, IN0:=10, IN1:=20) = 10
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0)   G
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (10)  IN0
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (20)  IN1
        0xC4, 0x47, 0x03,  // BUILTIN SEL_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
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
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 10);
}

#[test]
fn execute_when_sel_i32_g_one_then_returns_in1() {
    // SEL(G:=1, IN0:=10, IN1:=20) = 20
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1)   G
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (10)  IN0
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (20)  IN1
        0xC4, 0x47, 0x03,  // BUILTIN SEL_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
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
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 20);
}

#[test]
fn execute_when_sel_i32_g_nonzero_then_returns_in1() {
    // SEL(G:=42, IN0:=10, IN1:=20) = 20 (any nonzero selects IN1)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (42)  G
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (10)  IN0
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (20)  IN1
        0xC4, 0x47, 0x03,  // BUILTIN SEL_I32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[42, 10, 20]);
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
        .start()
        .unwrap();

    vm.run_round(0).unwrap();
    assert_eq!(vm.read_variable(0).unwrap(), 20);
}
