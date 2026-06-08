//! Integration tests for string opcodes.

use crate::common::VmBuffers;
use ironplc_container::opcode;
use ironplc_container::{ContainerBuilder, FunctionId, VarIndex};
use ironplc_vm::error::Trap;

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

/// Helper: container with i32 and WSTRING (raw UTF-16LE) constants, for the
/// wide (`char_width = 2`) synthetic tests below.
fn wstring_container(
    bytecode: &[u8],
    num_vars: u16,
    i32_constants: &[i32],
    wstr_constants: &[&[u8]],
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
    for w in wstr_constants {
        builder = builder.add_wstr_constant(w);
    }
    builder
        .add_function(FunctionId::INIT, &init_bytecode, 0, num_vars, 0)
        .add_function(FunctionId::SCAN, bytecode, 16, num_vars, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .build()
}

/// Hand-writes a 6-byte string header (max_len, cur_len, char_width — all
/// counts of code units / a width byte) at `off` in a data region, for
/// fixtures that need a wide variable the codegen cannot yet emit.
fn write_str_header(dr: &mut [u8], off: usize, max_len: u16, cur_len: u16, char_width: u16) {
    dr[off..off + 2].copy_from_slice(&max_len.to_le_bytes());
    dr[off + 2..off + 4].copy_from_slice(&cur_len.to_le_bytes());
    dr[off + 4..off + 6].copy_from_slice(&char_width.to_le_bytes());
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
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
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
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
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
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
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
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
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
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
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
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 3);
}

// --- Wide (WSTRING / char_width = 2) synthetic tests ---
//
// Codegen cannot emit wide strings yet (PR D), so these hand-assemble
// bytecode and, where a wide variable is needed, hand-write its v3 header
// into the data region. They prove the VM's data-driven width handling and
// the ADR-0034 encoding-mismatch verification added in PR C1.

#[test]
fn execute_when_wide_const_stored_into_wide_var_then_len_in_code_units() {
    // dest wide var at offset 0 (max_len=10 units, char_width=2).
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_STR, 0x00, 0x00,             // wide "Hi" -> wide temp
        opcode::STR_STORE_VAR, 0x00, 0x00, 0x00, 0x00,  // store into wide var at 0
        opcode::LEN_STR, 0x00, 0x00, 0x00, 0x00,
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    // "Hi" as UTF-16LE: H=0x48, i=0x69.
    let hi: &[u8] = &[0x48, 0x00, 0x69, 0x00];
    let c = wstring_container(&bytecode, 1, &[], &[hi], 64);
    let mut b = VmBuffers::from_container(&c);
    write_str_header(&mut b.data_region, 0, 10, 0, 2);
    let len = {
        let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
        vm.read_variable(VarIndex::new(0)).unwrap()
    };
    // LEN reports code units, not bytes.
    assert_eq!(len, 2);
    // The stored data is the two UTF-16LE code units of "Hi".
    assert_eq!(&b.data_region[6..10], &[0x48, 0x00, 0x69, 0x00]);
    // The destination header kept char_width = 2.
    assert_eq!(u16::from_le_bytes([b.data_region[4], b.data_region[5]]), 2);
}

#[test]
fn execute_when_wide_const_stored_into_narrow_var_then_encoding_mismatch() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::STR_INIT, 0x00, 0x00, 0x00, 0x00, 0x14, 0x00, // narrow var at 0, max=20
        opcode::LOAD_CONST_STR, 0x00, 0x00,                   // wide "Hi"
        opcode::STR_STORE_VAR, 0x00, 0x00, 0x00, 0x00,        // wide src -> narrow dest
        opcode::RET_VOID,
    ];
    let hi: &[u8] = &[0x48, 0x00, 0x69, 0x00];
    let c = wstring_container(&bytecode, 1, &[], &[hi], 64);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    let err = vm.run_round(0).unwrap_err().trap;

    assert_eq!(
        err,
        Trap::EncodingMismatch {
            expected: 1,
            actual: 2,
        }
    );
}

#[test]
fn execute_when_load_var_with_invalid_char_width_then_trap() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::STR_LOAD_VAR, 0x00, 0x00, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = wstring_container(&bytecode, 1, &[], &[], 64);
    let mut b = VmBuffers::from_container(&c);
    write_str_header(&mut b.data_region, 0, 10, 2, 0); // char_width = 0 is invalid
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    let err = vm.run_round(0).unwrap_err().trap;

    assert_eq!(err, Trap::InvalidCharWidth(0));
}

#[test]
fn execute_when_wide_var_roundtrip_then_preserves_utf16le_bytes() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::STR_LOAD_VAR, 0x00, 0x00, 0x00, 0x00,  // load wide var at 0 -> wide temp
        opcode::STR_STORE_VAR, 0x20, 0x00, 0x00, 0x00, // store into wide var at 32
        opcode::LEN_STR, 0x20, 0x00, 0x00, 0x00,       // len of var at 32
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = wstring_container(&bytecode, 1, &[], &[], 64);
    let mut b = VmBuffers::from_container(&c);
    // Source wide var "ABC" at offset 0.
    write_str_header(&mut b.data_region, 0, 10, 3, 2);
    b.data_region[6..12].copy_from_slice(&[0x41, 0x00, 0x42, 0x00, 0x43, 0x00]);
    // Dest wide var at offset 32.
    write_str_header(&mut b.data_region, 32, 10, 0, 2);
    let len = {
        let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
        vm.read_variable(VarIndex::new(0)).unwrap()
    };
    assert_eq!(len, 3);
    // Bytes survive the wide load -> temp -> wide store round trip.
    assert_eq!(
        &b.data_region[38..44],
        &[0x41, 0x00, 0x42, 0x00, 0x43, 0x00]
    );
}

#[test]
fn execute_when_cmp_wide_equal_then_zero() {
    let cmp = opcode::builtin::CMP_STR.to_le_bytes();
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,  // left_offset = i32 pool[0] = 0
        opcode::LOAD_CONST_I32, 0x01, 0x00,  // right_offset = i32 pool[1] = 32
        opcode::BUILTIN, cmp[0], cmp[1],
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = wstring_container(&bytecode, 1, &[0, 32], &[], 64);
    let mut b = VmBuffers::from_container(&c);
    // Two equal wide "AB" strings.
    write_str_header(&mut b.data_region, 0, 10, 2, 2);
    b.data_region[6..10].copy_from_slice(&[0x41, 0x00, 0x42, 0x00]);
    write_str_header(&mut b.data_region, 32, 10, 2, 2);
    b.data_region[38..42].copy_from_slice(&[0x41, 0x00, 0x42, 0x00]);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 0);
}

#[test]
fn execute_when_cmp_mixed_encoding_then_trap() {
    let cmp = opcode::builtin::CMP_STR.to_le_bytes();
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,
        opcode::LOAD_CONST_I32, 0x01, 0x00,
        opcode::BUILTIN, cmp[0], cmp[1],
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = wstring_container(&bytecode, 1, &[0, 32], &[], 64);
    let mut b = VmBuffers::from_container(&c);
    write_str_header(&mut b.data_region, 0, 10, 2, 2); // left wide
    write_str_header(&mut b.data_region, 32, 10, 2, 1); // right narrow
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    let err = vm.run_round(0).unwrap_err().trap;

    assert_eq!(
        err,
        Trap::EncodingMismatch {
            expected: 2,
            actual: 1,
        }
    );
}

// --- Wide string-function tests (PR C2) ---

#[test]
fn execute_when_concat_wide_then_joins_code_units() {
    // Wide "AB" at 0, wide "CD" at 16, concat -> wide dest at 32.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::CONCAT_STR, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00,
        opcode::STR_STORE_VAR, 0x20, 0x00, 0x00, 0x00,
        opcode::LEN_STR, 0x20, 0x00, 0x00, 0x00,
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = wstring_container(&bytecode, 1, &[], &[], 64);
    let mut b = VmBuffers::from_container(&c);
    write_str_header(&mut b.data_region, 0, 5, 2, 2);
    b.data_region[6..10].copy_from_slice(&[0x41, 0x00, 0x42, 0x00]); // "AB"
    write_str_header(&mut b.data_region, 16, 5, 2, 2);
    b.data_region[22..26].copy_from_slice(&[0x43, 0x00, 0x44, 0x00]); // "CD"
    write_str_header(&mut b.data_region, 32, 5, 0, 2);
    let len = {
        let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
        vm.read_variable(VarIndex::new(0)).unwrap()
    };
    assert_eq!(len, 4);
    assert_eq!(
        &b.data_region[38..46],
        &[0x41, 0x00, 0x42, 0x00, 0x43, 0x00, 0x44, 0x00] // "ABCD"
    );
}

#[test]
fn execute_when_concat_mixed_encoding_then_trap() {
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::CONCAT_STR, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = wstring_container(&bytecode, 1, &[], &[], 64);
    let mut b = VmBuffers::from_container(&c);
    write_str_header(&mut b.data_region, 0, 5, 2, 2); // wide
    write_str_header(&mut b.data_region, 16, 5, 2, 1); // narrow
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    let err = vm.run_round(0).unwrap_err().trap;

    assert_eq!(
        err,
        Trap::EncodingMismatch {
            expected: 2,
            actual: 1,
        }
    );
}

#[test]
fn execute_when_left_wide_then_returns_leading_code_units() {
    // LEFT("ABCDE", 3) over a wide source -> "ABC".
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,            // L = i32 pool[0] = 3
        opcode::LEFT_STR, 0x00, 0x00, 0x00, 0x00,
        opcode::STR_STORE_VAR, 0x20, 0x00, 0x00, 0x00,
        opcode::LEN_STR, 0x20, 0x00, 0x00, 0x00,
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = wstring_container(&bytecode, 1, &[3], &[], 64);
    let mut b = VmBuffers::from_container(&c);
    write_str_header(&mut b.data_region, 0, 8, 5, 2);
    b.data_region[6..16].copy_from_slice(&[
        0x41, 0x00, 0x42, 0x00, 0x43, 0x00, 0x44, 0x00, 0x45, 0x00, // "ABCDE"
    ]);
    write_str_header(&mut b.data_region, 32, 8, 0, 2);
    let len = {
        let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
        vm.read_variable(VarIndex::new(0)).unwrap()
    };
    assert_eq!(len, 3);
    assert_eq!(
        &b.data_region[38..44],
        &[0x41, 0x00, 0x42, 0x00, 0x43, 0x00] // "ABC"
    );
}

#[test]
fn execute_when_mid_wide_then_returns_middle_code_units() {
    // MID("ABCDE", P=2, L=3) over a wide source -> "BCD".
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,            // L = i32 pool[0] = 3 (pushed first)
        opcode::LOAD_CONST_I32, 0x01, 0x00,            // P = i32 pool[1] = 2 (top)
        opcode::MID_STR, 0x00, 0x00, 0x00, 0x00,
        opcode::STR_STORE_VAR, 0x20, 0x00, 0x00, 0x00,
        opcode::LEN_STR, 0x20, 0x00, 0x00, 0x00,
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = wstring_container(&bytecode, 1, &[3, 2], &[], 64);
    let mut b = VmBuffers::from_container(&c);
    write_str_header(&mut b.data_region, 0, 8, 5, 2);
    b.data_region[6..16].copy_from_slice(&[
        0x41, 0x00, 0x42, 0x00, 0x43, 0x00, 0x44, 0x00, 0x45, 0x00, // "ABCDE"
    ]);
    write_str_header(&mut b.data_region, 32, 8, 0, 2);
    let len = {
        let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
        vm.read_variable(VarIndex::new(0)).unwrap()
    };
    assert_eq!(len, 3);
    assert_eq!(
        &b.data_region[38..44],
        &[0x42, 0x00, 0x43, 0x00, 0x44, 0x00] // "BCD"
    );
}

#[test]
fn execute_when_find_wide_then_returns_code_unit_position() {
    // FIND("LL" in "HELLO") over wide sources -> 3 (1-based code units).
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::FIND_STR, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00,
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let c = wstring_container(&bytecode, 1, &[], &[], 64);
    let mut b = VmBuffers::from_container(&c);
    write_str_header(&mut b.data_region, 0, 8, 5, 2);
    b.data_region[6..16].copy_from_slice(&[
        0x48, 0x00, 0x45, 0x00, 0x4C, 0x00, 0x4C, 0x00, 0x4F, 0x00, // "HELLO"
    ]);
    write_str_header(&mut b.data_region, 32, 4, 2, 2);
    b.data_region[38..42].copy_from_slice(&[0x4C, 0x00, 0x4C, 0x00]); // "LL"
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).unwrap();

    assert_eq!(vm.read_variable(VarIndex::new(0)).unwrap(), 3);
}

// --- Wide string-array tests (PR C3) ---

/// Helper: container with a string array variable. var[0] holds the array
/// base offset (0, set by INIT); var[1] is an i32 result. The constant pool
/// is `i32_constants`, then `wstr_constants`, then a trailing i32(0) used to
/// initialize the base offset.
fn wstr_array_container(
    bytecode: &[u8],
    total_elements: u32,
    max_str_len: u16,
    i32_constants: &[i32],
    wstr_constants: &[&[u8]],
    data_region_bytes: u32,
) -> ironplc_container::Container {
    let base_const_index = (i32_constants.len() + wstr_constants.len()) as u16;
    let bci = base_const_index.to_le_bytes();
    #[rustfmt::skip]
    let init_bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, bci[0], bci[1],
        opcode::STORE_VAR_I32, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let mut builder = ContainerBuilder::new()
        .num_variables(2)
        .data_region_bytes(data_region_bytes)
        .num_temp_bufs(4)
        .max_temp_buf_bytes(64);
    for &c in i32_constants {
        builder = builder.add_i32_constant(c);
    }
    for w in wstr_constants {
        builder = builder.add_wstr_constant(w);
    }
    builder = builder.add_i32_constant(0); // array base offset
    builder.add_array_descriptor(0, total_elements, max_str_len);
    builder
        .add_function(FunctionId::new(0), &init_bytecode, 2, 2, 0)
        .add_function(FunctionId::new(1), bytecode, 16, 2, 0)
        .init_function_id(FunctionId::new(0))
        .entry_function_id(FunctionId::new(1))
        .build()
}

#[test]
fn execute_when_store_wide_into_narrow_array_elem_then_encoding_mismatch() {
    // STR_INIT_ARRAY makes narrow elements; storing a wide temp traps.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::STR_INIT_ARRAY, 0x00, 0x00, 0x00, 0x00,
        opcode::LOAD_CONST_STR, 0x01, 0x00,            // wide "Hi" pool[1] -> wide temp
        opcode::LOAD_CONST_I32, 0x00, 0x00,            // index = i32 pool[0] = 0
        opcode::STR_STORE_ARRAY_ELEM, 0x00, 0x00, 0x00, 0x00,
        opcode::RET_VOID,
    ];
    let hi: &[u8] = &[0x48, 0x00, 0x69, 0x00];
    let c = wstr_array_container(&bytecode, 2, 5, &[0], &[hi], 64);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    let err = vm.run_round(0).unwrap_err().trap;

    assert_eq!(
        err,
        Trap::EncodingMismatch {
            expected: 1,
            actual: 2,
        }
    );
}

#[test]
fn execute_when_wide_array_elem_roundtrip_then_preserves_bytes() {
    // Hand-written wide elements. The descriptor stride is narrow
    // (6 + max_str_len = 16), chosen large enough that wide data fits without
    // overlapping the next element: element 0 at offset 0, element 1 at 16.
    // Load element 0, store into element 1, check.
    #[rustfmt::skip]
    let bytecode: Vec<u8> = vec![
        opcode::LOAD_CONST_I32, 0x00, 0x00,            // index 0
        opcode::STR_LOAD_ARRAY_ELEM, 0x00, 0x00, 0x00, 0x00, // -> wide temp
        opcode::LOAD_CONST_I32, 0x01, 0x00,            // index 1
        opcode::STR_STORE_ARRAY_ELEM, 0x00, 0x00, 0x00, 0x00,
        opcode::LEN_STR, 0x10, 0x00, 0x00, 0x00,       // len of element 1 (offset 16)
        opcode::STORE_VAR_I32, 0x01, 0x00,             // -> var[1]
        opcode::RET_VOID,
    ];
    let c = wstr_array_container(&bytecode, 2, 10, &[0, 1], &[], 64);
    let mut b = VmBuffers::from_container(&c);
    // Element 0 (offset 0): wide "ABC".
    write_str_header(&mut b.data_region, 0, 10, 3, 2);
    b.data_region[6..12].copy_from_slice(&[0x41, 0x00, 0x42, 0x00, 0x43, 0x00]);
    // Element 1 (offset 16): empty wide.
    write_str_header(&mut b.data_region, 16, 10, 0, 2);
    let len = {
        let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
        vm.run_round(0).unwrap();
        vm.read_variable(VarIndex::new(1)).unwrap()
    };
    assert_eq!(len, 3);
    // Element 1 data starts at 16 + 6 = 22.
    assert_eq!(
        &b.data_region[22..28],
        &[0x41, 0x00, 0x42, 0x00, 0x43, 0x00]
    );
}
