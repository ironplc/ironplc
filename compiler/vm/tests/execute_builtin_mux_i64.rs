//! Integration tests for the BUILTIN MUX_I64 opcodes.

mod common;

use common::{single_function_container_i32_i64, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_mux_i64_k0_2_inputs_then_returns_in0() {
    // MUX(K:=0, IN0:=5_000_000_000, IN1:=10_000_000_000) = 5_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0)                K
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (5_000_000_000)   IN0
        0x02, 0x02, 0x00,  // LOAD_CONST_I64 pool[2] (10_000_000_000)  IN1
        0xC4, 0x22, 0x04,  // BUILTIN MUX_I64(2) = 0x0422
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_i64(&bytecode, 1, &[0], &[5_000_000_000, 10_000_000_000]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i64(), 5_000_000_000);
}

#[test]
fn execute_when_mux_i64_k1_2_inputs_then_returns_in1() {
    // MUX(K:=1, IN0:=5_000_000_000, IN1:=10_000_000_000) = 10_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1)                K
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (5_000_000_000)   IN0
        0x02, 0x02, 0x00,  // LOAD_CONST_I64 pool[2] (10_000_000_000)  IN1
        0xC4, 0x22, 0x04,  // BUILTIN MUX_I64(2) = 0x0422
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_i64(&bytecode, 1, &[1], &[5_000_000_000, 10_000_000_000]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i64(), 10_000_000_000);
}

#[test]
fn execute_when_mux_i64_k2_3_inputs_then_returns_in2() {
    // MUX(K:=2, IN0:=100, IN1:=200, IN2:=300) = 300
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (2)    K
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (100)  IN0
        0x02, 0x02, 0x00,  // LOAD_CONST_I64 pool[2] (200)  IN1
        0x02, 0x03, 0x00,  // LOAD_CONST_I64 pool[3] (300)  IN2
        0xC4, 0x23, 0x04,  // BUILTIN MUX_I64(3) = 0x0423
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_i64(&bytecode, 1, &[2], &[100, 200, 300]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i64(), 300);
}

#[test]
fn execute_when_mux_i64_k_out_of_range_then_clamps_to_last() {
    // MUX(K:=10, IN0:=100, IN1:=200, IN2:=300) = 300 (clamped)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (10)   K
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (100)  IN0
        0x02, 0x02, 0x00,  // LOAD_CONST_I64 pool[2] (200)  IN1
        0x02, 0x03, 0x00,  // LOAD_CONST_I64 pool[3] (300)  IN2
        0xC4, 0x23, 0x04,  // BUILTIN MUX_I64(3) = 0x0423
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_i64(&bytecode, 1, &[10], &[100, 200, 300]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i64(), 300);
}

#[test]
fn execute_when_mux_i64_k_negative_then_clamps_to_first() {
    // MUX(K:=-1, IN0:=100, IN1:=200) = 100 (clamped)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (-1)   K
        0x02, 0x01, 0x00,  // LOAD_CONST_I64 pool[1] (100)  IN0
        0x02, 0x02, 0x00,  // LOAD_CONST_I64 pool[2] (200)  IN1
        0xC4, 0x22, 0x04,  // BUILTIN MUX_I64(2) = 0x0422
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i32_i64(&bytecode, 1, &[-1], &[100, 200]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = Vm::new()
            .load(
                &c,
                &mut b.stack,
                &mut b.vars,
                &mut b.data_region,
                &mut b.temp_buf,
                &mut b.tasks,
                &mut b.programs,
                &mut b.ready,
            )
            .start()
            .unwrap();
        vm.run_round(0).unwrap();
    }
    assert_eq!(b.vars[0].as_i64(), 100);
}
