//! Integration tests for string opcodes.

mod common;

use common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::{ContainerBuilder, FunctionId, VarIndex};

/// Helper: builds a container configured for string operations.
/// Provides a data region, temp buffers, and the given i32 constants.
fn string_container(
    bytecode: &[u8],
    num_vars: u16,
    i32_constants: &[i32],
    str_constants: &[&[u8]],
    data_region_bytes: u32,
) -> ironplc_container::Container {
    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    let mut builder = ContainerBuilder::new()
        .num_variables(num_vars)
        .data_region_bytes(data_region_bytes)
        .num_temp_bufs(4)
        .max_temp_buf_bytes(64);
    for &c in i32_constants {
        builder = builder.add_i32_constant(c);
    }
    for s in str_constants {
        builder = builder.add_str_constant(s);
    }
    builder
        .add_function(FunctionId::INIT, &init_bytecode, 0, num_vars, 0)
        .add_function(FunctionId::SCAN, bytecode, 16, num_vars, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .build()
}

// String opcodes use u32 (4-byte LE) for data offsets.
// STR_INIT: u32 data_offset, u16 max_length
// STR_STORE_VAR / STR_LOAD_VAR / LEN_STR: u32 data_offset
// FIND_STR / CONCAT_STR: u32 in1_offset, u32 in2_offset
// LEFT_STR: u32 in_offset (pops L from stack)

#[test]
fn execute_when_str_init_then_sets_header() {
    // STR_INIT at data_offset 0, max_length 20
    // Then read LEN_STR to verify cur_length is 0
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::STR_INIT, 0x00, 0x00, 0x00, 0x00, 0x14, 0x00,  // STR_INIT offset=0 (u32), max_len=20 (u16)
        opcode::LEN_STR, 0x00, 0x00, 0x00, 0x00,                // LEN_STR offset=0 (u32)
        opcode::STORE_VAR_I32, 0x00, 0x00,                      // store to var[0]
        opcode::RET_VOID,
    ];
    let c = string_container(&bytecode, 1, &[], &[], 32);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    // After init, cur_length should be 0
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 0);
}

#[test]
fn execute_when_str_store_and_load_then_roundtrips() {
    // Init string at offset 0 (max_len=20), load constant string "Hi",
    // store it, then load it back and get its length.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::STR_INIT, 0x00, 0x00, 0x00, 0x00, 0x14, 0x00,  // STR_INIT offset=0, max_len=20
        opcode::LOAD_CONST_STR, 0x00, 0x00,                     // load str constant[0] ("Hi") -> buf_idx
        opcode::STR_STORE_VAR, 0x00, 0x00, 0x00, 0x00,          // store buf to string var at offset 0 (u32)
        opcode::LEN_STR, 0x00, 0x00, 0x00, 0x00,                // LEN_STR offset=0 (u32)
        opcode::STORE_VAR_I32, 0x00, 0x00,                      // store length to var[0]
        opcode::RET_VOID,
    ];
    let c = string_container(&bytecode, 1, &[], &[b"Hi"], 32);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    // "Hi" has length 2
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 2);
}

#[test]
fn execute_when_concat_str_then_correct_length() {
    // Init two strings at offset 0 (max=20) and offset 28 (max=20).
    // Store "AB" at offset 0, "CD" at offset 28, concat them, store
    // result at offset 0, read length.
    // String header is 4 bytes + max_len bytes. For max=20: 4+20=24 bytes.
    // So second string starts at offset 28 (rounded to allow space).
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::STR_INIT, 0x00, 0x00, 0x00, 0x00, 0x14, 0x00,   // STR_INIT offset=0, max_len=20
        opcode::STR_INIT, 0x1C, 0x00, 0x00, 0x00, 0x14, 0x00,   // STR_INIT offset=28, max_len=20
        // Store "AB" at offset 0
        opcode::LOAD_CONST_STR, 0x00, 0x00,
        opcode::STR_STORE_VAR, 0x00, 0x00, 0x00, 0x00,
        // Store "CD" at offset 28
        opcode::LOAD_CONST_STR, 0x01, 0x00,
        opcode::STR_STORE_VAR, 0x1C, 0x00, 0x00, 0x00,
        // Concat offset 0 and offset 28
        opcode::CONCAT_STR, 0x00, 0x00, 0x00, 0x00, 0x1C, 0x00, 0x00, 0x00,
        opcode::STR_STORE_VAR, 0x00, 0x00, 0x00, 0x00,
        // Read length
        opcode::LEN_STR, 0x00, 0x00, 0x00, 0x00,
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = string_container(&bytecode, 1, &[], &[b"AB", b"CD"], 64);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    // "AB" + "CD" = "ABCD", length 4
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 4);
}

#[test]
fn execute_when_find_str_found_then_returns_position() {
    // Init "HELLO" at offset 0, "LL" at offset 28.
    // FIND_STR should return 1-based position of "LL" in "HELLO" = 3.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::STR_INIT, 0x00, 0x00, 0x00, 0x00, 0x14, 0x00,
        opcode::STR_INIT, 0x1C, 0x00, 0x00, 0x00, 0x14, 0x00,
        // Store "HELLO" at offset 0
        opcode::LOAD_CONST_STR, 0x00, 0x00,
        opcode::STR_STORE_VAR, 0x00, 0x00, 0x00, 0x00,
        // Store "LL" at offset 28
        opcode::LOAD_CONST_STR, 0x01, 0x00,
        opcode::STR_STORE_VAR, 0x1C, 0x00, 0x00, 0x00,
        // FIND
        opcode::FIND_STR, 0x00, 0x00, 0x00, 0x00, 0x1C, 0x00, 0x00, 0x00,
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = string_container(&bytecode, 1, &[], &[b"HELLO", b"LL"], 64);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    // "LL" starts at position 3 (1-based)
    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 3);
}

#[test]
fn execute_when_find_str_not_found_then_returns_zero() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::STR_INIT, 0x00, 0x00, 0x00, 0x00, 0x14, 0x00,
        opcode::STR_INIT, 0x1C, 0x00, 0x00, 0x00, 0x14, 0x00,
        opcode::LOAD_CONST_STR, 0x00, 0x00,
        opcode::STR_STORE_VAR, 0x00, 0x00, 0x00, 0x00,
        opcode::LOAD_CONST_STR, 0x01, 0x00,
        opcode::STR_STORE_VAR, 0x1C, 0x00, 0x00, 0x00,
        opcode::FIND_STR, 0x00, 0x00, 0x00, 0x00, 0x1C, 0x00, 0x00, 0x00,
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = string_container(&bytecode, 1, &[], &[b"HELLO", b"XY"], 64);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 0);
}

#[test]
fn execute_when_left_str_then_correct_length() {
    // Init "ABCDE" at offset 0. LEFT_STR with L=3 gives "ABC" (length 3).
    // Constant pool: pool[0] = i32(3), pool[1] = str("ABCDE")
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::STR_INIT, 0x00, 0x00, 0x00, 0x00, 0x14, 0x00,
        opcode::LOAD_CONST_STR, 0x01, 0x00,                     // load str pool[1] ("ABCDE")
        opcode::STR_STORE_VAR, 0x00, 0x00, 0x00, 0x00,
        // LEFT_STR: push L=3, then LEFT_STR offset=0 (u32)
        opcode::LOAD_CONST_I32, 0x00, 0x00,                     // push i32 pool[0] = 3
        opcode::LEFT_STR, 0x00, 0x00, 0x00, 0x00,               // LEFT_STR in=0 (u32), pops L -> buf_idx
        opcode::STR_STORE_VAR, 0x00, 0x00, 0x00, 0x00,          // store result back
        opcode::LEN_STR, 0x00, 0x00, 0x00, 0x00,
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = string_container(&bytecode, 1, &[3], &[b"ABCDE"], 32);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 3);
}
