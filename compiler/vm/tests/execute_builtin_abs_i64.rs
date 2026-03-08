//! Integration tests for the BUILTIN ABS_I64 opcode.

mod common;

use common::{single_function_container_i64, VmBuffers};
use ironplc_vm::Vm;

#[test]
fn execute_when_abs_i64_positive_then_unchanged() {
    // ABS(7_000_000_000) = 7_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (7_000_000_000)
        0xC4, 0x61, 0x03,  // BUILTIN ABS_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i64(&bytecode, 1, &[7_000_000_000]);
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
    assert_eq!(b.vars[0].as_i64(), 7_000_000_000);
}

#[test]
fn execute_when_abs_i64_negative_then_positive() {
    // ABS(-7_000_000_000) = 7_000_000_000
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (-7_000_000_000)
        0xC4, 0x61, 0x03,  // BUILTIN ABS_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i64(&bytecode, 1, &[-7_000_000_000]);
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
    assert_eq!(b.vars[0].as_i64(), 7_000_000_000);
}

#[test]
fn execute_when_abs_i64_min_then_wraps() {
    // ABS(i64::MIN) wraps to i64::MIN (wrapping_abs)
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x02, 0x00, 0x00,  // LOAD_CONST_I64 pool[0] (i64::MIN)
        0xC4, 0x61, 0x03,  // BUILTIN ABS_I64
        0x19, 0x00, 0x00,  // STORE_VAR_I64 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container_i64(&bytecode, 1, &[i64::MIN]);
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
    assert_eq!(b.vars[0].as_i64(), i64::MIN);
}
