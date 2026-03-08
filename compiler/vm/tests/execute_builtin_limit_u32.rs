//! Integration tests for the BUILTIN LIMIT_U32 opcode.

mod common;

use common::{single_function_container, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_limit_u32_below_min_then_clamped() {
    // LIMIT(MN:=1_000_000_000, IN:=500_000_000, MX:=3_000_000_000) = 1_000_000_000
    // 500_000_000 is below MN in unsigned comparison.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1_000_000_000)    MN
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (500_000_000)      IN
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (3_000_000_000 as i32)  MX
        0xC4, 0x68, 0x03,  // BUILTIN LIMIT_U32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(
        &bytecode,
        1,
        &[1_000_000_000, 500_000_000, 3_000_000_000_u32 as i32],
    );
    let mut b = VmBuffers::from_container(&c);
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
    assert_eq!(vm.read_variable(0).unwrap() as u32, 1_000_000_000);
}

#[test]
fn execute_when_limit_u32_in_range_then_unchanged() {
    // LIMIT(MN:=1_000_000_000, IN:=2_000_000_000, MX:=3_000_000_000) = 2_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1_000_000_000)    MN
        0x01, 0x01, 0x00,  // LOAD_CONST_I32 pool[1] (2_000_000_000)    IN
        0x01, 0x02, 0x00,  // LOAD_CONST_I32 pool[2] (3_000_000_000 as i32)  MX
        0xC4, 0x68, 0x03,  // BUILTIN LIMIT_U32
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(
        &bytecode,
        1,
        &[1_000_000_000, 2_000_000_000, 3_000_000_000_u32 as i32],
    );
    let mut b = VmBuffers::from_container(&c);
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
    assert_eq!(vm.read_variable(0).unwrap() as u32, 2_000_000_000);
}
