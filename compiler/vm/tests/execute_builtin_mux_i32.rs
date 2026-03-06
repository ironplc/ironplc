//! Integration tests for the BUILTIN MUX_I32 opcodes.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_mux_i32_k0_2_inputs_then_returns_in0() {
    // MUX(K:=0, IN0:=10, IN1:=20) = 10
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0)   K
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (10)  IN0
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (20)  IN1
        0xC4, 0x02, 0x04,  // BUILTIN MUX_I32(2) = 0x0402
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
fn execute_when_mux_i32_k1_2_inputs_then_returns_in1() {
    // MUX(K:=1, IN0:=10, IN1:=20) = 20
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1)   K
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (10)  IN0
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (20)  IN1
        0xC4, 0x02, 0x04,  // BUILTIN MUX_I32(2) = 0x0402
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
fn execute_when_mux_i32_k2_3_inputs_then_returns_in2() {
    // MUX(K:=2, IN0:=10, IN1:=20, IN2:=30) = 30
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (2)   K
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (10)  IN0
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (20)  IN1
        0x01, 0x03, 0x00,  // LOAD_CONST_I32 pool[3] (30)  IN2
        0xC4, 0x03, 0x04,  // BUILTIN MUX_I32(3) = 0x0403
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[2, 10, 20, 30]);
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
    assert_eq!(vm.read_variable(0).unwrap(), 30);
}

#[test]
fn execute_when_mux_i32_k_out_of_range_then_clamps_to_last() {
    // MUX(K:=10, IN0:=10, IN1:=20, IN2:=30) = 30 (clamped)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (10)  K
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (10)  IN0
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (20)  IN1
        0x01, 0x03, 0x00,  // LOAD_CONST_I32 pool[3] (30)  IN2
        0xC4, 0x03, 0x04,  // BUILTIN MUX_I32(3) = 0x0403
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[10, 10, 20, 30]);
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
    assert_eq!(vm.read_variable(0).unwrap(), 30);
}

#[test]
fn execute_when_mux_i32_4_inputs_then_works() {
    // MUX(K:=3, IN0:=10, IN1:=20, IN2:=30, IN3:=40) = 40
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (3)   K
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (10)  IN0
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (20)  IN1
        0x01, 0x03, 0x00,  // LOAD_CONST_I32 pool[3] (30)  IN2
        0x01, 0x04, 0x00,  // LOAD_CONST_I32 pool[4] (40)  IN3
        0xC4, 0x04, 0x04,  // BUILTIN MUX_I32(4) = 0x0404
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[3, 10, 20, 30, 40]);
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
    assert_eq!(vm.read_variable(0).unwrap(), 40);
}

#[test]
fn execute_when_mux_i32_k_int_max_then_clamps_to_last() {
    // MUX(K:=i32::MAX, IN0:=10, IN1:=20) = 20 (clamped to last)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (i32::MAX)  K
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (10)        IN0
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (20)        IN1
        0xC4, 0x02, 0x04,  // BUILTIN MUX_I32(2) = 0x0402
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[i32::MAX, 10, 20]);
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
fn execute_when_mux_i32_k_int_min_then_clamps_to_first() {
    // MUX(K:=i32::MIN, IN0:=10, IN1:=20) = 10 (clamped to first)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (i32::MIN)  K
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (10)        IN0
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (20)        IN1
        0xC4, 0x02, 0x04,  // BUILTIN MUX_I32(2) = 0x0402
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[i32::MIN, 10, 20]);
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
fn execute_when_mux_i32_k_negative_then_clamps_to_first() {
    // MUX(K:=-1, IN0:=10, IN1:=20) = 10 (clamped to first)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (-1)  K
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (10)  IN0
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (20)  IN1
        0xC4, 0x02, 0x04,  // BUILTIN MUX_I32(2) = 0x0402
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[-1, 10, 20]);
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
