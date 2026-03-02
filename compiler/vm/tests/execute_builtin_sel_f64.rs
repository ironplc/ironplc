//! Integration tests for the BUILTIN SEL_F64 opcode.

mod common;

use common::{single_function_container_i32_f64, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_sel_f64_g_zero_then_returns_in0() {
    // SEL(G:=0, IN0:=10.5, IN1:=20.5) = 10.5
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0)     G
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (10.5)  IN0
        0x04, 0x02, 0x00,  // LOAD_CONST_F64 pool[2] (20.5)  IN1
        0xC4, 0x5D, 0x03,  // BUILTIN SEL_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_f64(&bytecode, 1, &[0], &[10.5, 20.5]);
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
    let result = b.vars[0].as_f64();
    assert!((result - 10.5).abs() < 1e-12, "expected 10.5, got {result}");
}

#[test]
fn execute_when_sel_f64_g_one_then_returns_in1() {
    // SEL(G:=1, IN0:=10.5, IN1:=20.5) = 20.5
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1)     G
        0x04, 0x01, 0x00,  // LOAD_CONST_F64 pool[1] (10.5)  IN0
        0x04, 0x02, 0x00,  // LOAD_CONST_F64 pool[2] (20.5)  IN1
        0xC4, 0x5D, 0x03,  // BUILTIN SEL_F64
        0x1B, 0x00, 0x00,  // STORE_VAR_F64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_f64(&bytecode, 1, &[1], &[10.5, 20.5]);
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
    let result = b.vars[0].as_f64();
    assert!((result - 20.5).abs() < 1e-12, "expected 20.5, got {result}");
}
