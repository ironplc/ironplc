//! Integration tests for the BUILTIN BCD_TO_INT and INT_TO_BCD opcodes.

mod common;

use common::{single_function_container, VmBuffers};

#[test]
fn execute_when_bcd_to_int_8_then_decodes() {
    // BCD_TO_INT_8(0x42) = 42
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0x42)
        0xC4, 0x91, 0x03,  // BUILTIN BCD_TO_INT_8
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0x42]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_i32() as u8;
    assert_eq!(result, 42, "expected 42, got {result}");
}

#[test]
fn execute_when_bcd_to_int_16_then_decodes() {
    // BCD_TO_INT_16(0x1234) = 1234
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0x1234)
        0xC4, 0x92, 0x03,  // BUILTIN BCD_TO_INT_16
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0x1234]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_i32() as u16;
    assert_eq!(result, 1234, "expected 1234, got {result}");
}

#[test]
fn execute_when_bcd_to_int_8_zero_then_zero() {
    // BCD_TO_INT_8(0x00) = 0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (0x00)
        0xC4, 0x91, 0x03,  // BUILTIN BCD_TO_INT_8
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[0x00]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_i32();
    assert_eq!(result, 0, "expected 0, got {result}");
}

#[test]
fn execute_when_int_to_bcd_8_then_encodes() {
    // INT_TO_BCD_8(42) = 0x42
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (42)
        0xC4, 0x95, 0x03,  // BUILTIN INT_TO_BCD_8
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[42]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_i32() as u8;
    assert_eq!(result, 0x42, "expected 0x42, got 0x{result:02X}");
}

#[test]
fn execute_when_int_to_bcd_16_then_encodes() {
    // INT_TO_BCD_16(1234) = 0x1234
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (1234)
        0xC4, 0x96, 0x03,  // BUILTIN INT_TO_BCD_16
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[1234]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_i32() as u16;
    assert_eq!(result, 0x1234, "expected 0x1234, got 0x{result:04X}");
}

#[test]
fn execute_when_bcd_roundtrip_then_matches() {
    // INT_TO_BCD_8(73) then BCD_TO_INT_8 => 73
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0] (73)
        0xC4, 0x95, 0x03,  // BUILTIN INT_TO_BCD_8
        0xC4, 0x91, 0x03,  // BUILTIN BCD_TO_INT_8
        0x18, 0x00, 0x00,  // STORE_VAR_I32 var[0]
        0xB5,              // RET_VOID
    ];
    let c = single_function_container(&bytecode, 1, &[73]);
    let mut b = VmBuffers::from_container(&c);
    {
        let mut vm = common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
    }
    let result = b.vars[0].as_i32();
    assert_eq!(result, 73, "expected 73, got {result}");
}
