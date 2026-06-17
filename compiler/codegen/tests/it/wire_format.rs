//! Wire-format test suite — the canonical guard against accidental
//! changes to the on-disk bytecode encoding.
//!
//! ## Scope
//!
//! Behavioural codegen tests (in `compile_*.rs`) verify that the right
//! opcodes are emitted in the right order. They are *symbolic*: they
//! talk about `LOAD_VAR_I32` by name, not by byte value. That is the
//! right level for testing codegen behaviour, but it would let a
//! silent renumbering of an opcode pass undetected.
//!
//! This file is the **single canonical source of wire-format truth**.
//! It pins:
//!
//! 1. **Every opcode constant's byte value.** A renumber that changes
//!    `LOAD_VAR_I32` from 0x0C to 0x0D fails exactly one test here
//!    with a clear `expected 0x0C, found 0x0D` message — instead of
//!    cascading into ~30 confusing diffs across `compile_*.rs`.
//! 2. **The structured `[op_class:6][type:2]` encoding scheme.** For
//!    op-classes with multiple type variants (e.g. LOAD_VAR_*), the
//!    high 6 bits are constant within the family and the low 2 bits
//!    encode the type tag. A renumber that violates the scheme is
//!    caught here even if every byte value is internally consistent.
//! 3. **Per-shape golden encodings.** A handful of small programs
//!    compile to known-exact byte sequences, guarding operand widths,
//!    little-endian operand encoding, and per-shape layout.
//!
//! See `specs/plans/2026-05-02-codegen-test-wire-format-split.md`.

use ironplc_container::opcode;
use ironplc_parser::options::CompilerOptions;

use crate::common::parse_and_compile;

// ---------------------------------------------------------------------------
// 1. Opcode-byte pinning.
//
// One test per op-class family. Each asserts the **literal byte value**
// of every constant in that family. Grouping by family keeps the diff
// small when a single family is renumbered.
// ---------------------------------------------------------------------------

#[test]
fn opcode_constants_when_load_const_family_then_pinned_bytes() {
    assert_eq!(opcode::LOAD_CONST_I32, 0x00);
    assert_eq!(opcode::LOAD_CONST_I64, 0x01);
    assert_eq!(opcode::LOAD_CONST_F32, 0x02);
    assert_eq!(opcode::LOAD_CONST_F64, 0x03);
}

#[test]
fn opcode_constants_when_load_bool_family_then_pinned_bytes() {
    assert_eq!(opcode::LOAD_FALSE, 0x04);
    assert_eq!(opcode::LOAD_TRUE, 0x05);
}

#[test]
fn opcode_constants_when_load_const_str_then_pinned_byte() {
    assert_eq!(opcode::LOAD_CONST_STR, 0x08);
}

#[test]
fn opcode_constants_when_load_var_family_then_pinned_bytes() {
    assert_eq!(opcode::LOAD_VAR_I32, 0x0C);
    assert_eq!(opcode::LOAD_VAR_I64, 0x0D);
    assert_eq!(opcode::LOAD_VAR_F32, 0x0E);
    assert_eq!(opcode::LOAD_VAR_F64, 0x0F);
}

#[test]
fn opcode_constants_when_store_var_family_then_pinned_bytes() {
    assert_eq!(opcode::STORE_VAR_I32, 0x10);
    assert_eq!(opcode::STORE_VAR_I64, 0x11);
    assert_eq!(opcode::STORE_VAR_F32, 0x12);
    assert_eq!(opcode::STORE_VAR_F64, 0x13);
}

#[test]
fn opcode_constants_when_indirect_then_pinned_bytes() {
    assert_eq!(opcode::LOAD_INDIRECT, 0x14);
    assert_eq!(opcode::STORE_INDIRECT, 0x18);
}

#[test]
fn opcode_constants_when_trunc_family_then_pinned_bytes() {
    assert_eq!(opcode::TRUNC_I8, 0x1C);
    assert_eq!(opcode::TRUNC_U8, 0x1D);
    assert_eq!(opcode::TRUNC_I16, 0x1E);
    assert_eq!(opcode::TRUNC_U16, 0x1F);
}

#[test]
fn opcode_constants_when_arith_family_then_pinned_bytes() {
    // ADD (op_class 0x08).
    assert_eq!(opcode::ADD_I32, 0x20);
    assert_eq!(opcode::ADD_I64, 0x21);
    assert_eq!(opcode::ADD_F32, 0x22);
    assert_eq!(opcode::ADD_F64, 0x23);
    // SUB (0x09).
    assert_eq!(opcode::SUB_I32, 0x24);
    assert_eq!(opcode::SUB_I64, 0x25);
    assert_eq!(opcode::SUB_F32, 0x26);
    assert_eq!(opcode::SUB_F64, 0x27);
    // MUL (0x0A).
    assert_eq!(opcode::MUL_I32, 0x28);
    assert_eq!(opcode::MUL_I64, 0x29);
    assert_eq!(opcode::MUL_F32, 0x2A);
    assert_eq!(opcode::MUL_F64, 0x2B);
    // NEG (0x0B).
    assert_eq!(opcode::NEG_I32, 0x2C);
    assert_eq!(opcode::NEG_I64, 0x2D);
    assert_eq!(opcode::NEG_F32, 0x2E);
    assert_eq!(opcode::NEG_F64, 0x2F);
    // DIV signed (0x0C).
    assert_eq!(opcode::DIV_I32, 0x30);
    assert_eq!(opcode::DIV_I64, 0x31);
    assert_eq!(opcode::DIV_F32, 0x32);
    assert_eq!(opcode::DIV_F64, 0x33);
    // DIV unsigned (0x0D, only int variants).
    assert_eq!(opcode::DIV_U32, 0x34);
    assert_eq!(opcode::DIV_U64, 0x35);
    // MOD signed (0x0E, int only).
    assert_eq!(opcode::MOD_I32, 0x38);
    assert_eq!(opcode::MOD_I64, 0x39);
    // MOD unsigned (0x0F, int only).
    assert_eq!(opcode::MOD_U32, 0x3C);
    assert_eq!(opcode::MOD_U64, 0x3D);
}

#[test]
fn opcode_constants_when_cmp_family_then_pinned_bytes() {
    // EQ (0x10).
    assert_eq!(opcode::EQ_I32, 0x40);
    assert_eq!(opcode::EQ_I64, 0x41);
    assert_eq!(opcode::EQ_F32, 0x42);
    assert_eq!(opcode::EQ_F64, 0x43);
    // NE (0x11).
    assert_eq!(opcode::NE_I32, 0x44);
    assert_eq!(opcode::NE_I64, 0x45);
    assert_eq!(opcode::NE_F32, 0x46);
    assert_eq!(opcode::NE_F64, 0x47);
    // LT signed (0x12).
    assert_eq!(opcode::LT_I32, 0x48);
    assert_eq!(opcode::LT_I64, 0x49);
    assert_eq!(opcode::LT_F32, 0x4A);
    assert_eq!(opcode::LT_F64, 0x4B);
    // LE signed (0x13).
    assert_eq!(opcode::LE_I32, 0x4C);
    assert_eq!(opcode::LE_I64, 0x4D);
    assert_eq!(opcode::LE_F32, 0x4E);
    assert_eq!(opcode::LE_F64, 0x4F);
    // GT signed (0x14).
    assert_eq!(opcode::GT_I32, 0x50);
    assert_eq!(opcode::GT_I64, 0x51);
    assert_eq!(opcode::GT_F32, 0x52);
    assert_eq!(opcode::GT_F64, 0x53);
    // GE signed (0x15).
    assert_eq!(opcode::GE_I32, 0x54);
    assert_eq!(opcode::GE_I64, 0x55);
    assert_eq!(opcode::GE_F32, 0x56);
    assert_eq!(opcode::GE_F64, 0x57);
    // Unsigned LT/LE/GT/GE (0x16..0x19, int only).
    assert_eq!(opcode::LT_U32, 0x58);
    assert_eq!(opcode::LT_U64, 0x59);
    assert_eq!(opcode::LE_U32, 0x5C);
    assert_eq!(opcode::LE_U64, 0x5D);
    assert_eq!(opcode::GT_U32, 0x60);
    assert_eq!(opcode::GT_U64, 0x61);
    assert_eq!(opcode::GE_U32, 0x64);
    assert_eq!(opcode::GE_U64, 0x65);
}

#[test]
fn opcode_constants_when_bitwise_family_then_pinned_bytes() {
    assert_eq!(opcode::BIT_AND_32, 0x68);
    assert_eq!(opcode::BIT_AND_64, 0x69);
    assert_eq!(opcode::BIT_OR_32, 0x6C);
    assert_eq!(opcode::BIT_OR_64, 0x6D);
    assert_eq!(opcode::BIT_XOR_32, 0x70);
    assert_eq!(opcode::BIT_XOR_64, 0x71);
    assert_eq!(opcode::BIT_NOT_32, 0x74);
    assert_eq!(opcode::BIT_NOT_64, 0x75);
}

#[test]
fn opcode_constants_when_bool_family_then_pinned_bytes() {
    // BOOL_OP consolidates AND/OR/XOR/NOT in the type-tag slots
    // (0..3) of op-class 0x1E.
    assert_eq!(opcode::BOOL_AND, 0x78);
    assert_eq!(opcode::BOOL_OR, 0x79);
    assert_eq!(opcode::BOOL_XOR, 0x7A);
    assert_eq!(opcode::BOOL_NOT, 0x7B);
}

#[test]
fn opcode_constants_when_control_flow_then_pinned_bytes() {
    assert_eq!(opcode::JMP, 0x7C);
    assert_eq!(opcode::JMP_IF_NOT, 0x80);
    assert_eq!(opcode::CALL, 0x84);
    assert_eq!(opcode::RET, 0x88);
    assert_eq!(opcode::RET_VOID, 0x8C);
}

#[test]
fn opcode_constants_when_stack_op_family_then_pinned_bytes() {
    // STACK_OP consolidates POP/DUP/SWAP in type-tag slots 0..2.
    assert_eq!(opcode::POP, 0x90);
    assert_eq!(opcode::DUP, 0x91);
    assert_eq!(opcode::SWAP, 0x92);
}

#[test]
fn opcode_constants_when_builtin_then_pinned_byte() {
    assert_eq!(opcode::BUILTIN, 0x94);
}

#[test]
fn opcode_constants_when_fb_family_then_pinned_bytes() {
    assert_eq!(opcode::FB_LOAD_INSTANCE, 0x98);
    assert_eq!(opcode::FB_STORE_PARAM, 0x9C);
    assert_eq!(opcode::FB_LOAD_PARAM, 0xA0);
    assert_eq!(opcode::FB_CALL, 0xA4);
}

#[test]
fn opcode_constants_when_array_family_then_pinned_bytes() {
    assert_eq!(opcode::LOAD_ARRAY, 0xA8);
    assert_eq!(opcode::STORE_ARRAY, 0xAC);
    assert_eq!(opcode::LOAD_ARRAY_DEREF, 0xB0);
    assert_eq!(opcode::STORE_ARRAY_DEREF, 0xB4);
}

#[test]
fn opcode_constants_when_string_family_then_pinned_bytes() {
    assert_eq!(opcode::STR_INIT, 0xB8);
    assert_eq!(opcode::STR_LOAD_VAR, 0xBC);
    assert_eq!(opcode::STR_STORE_VAR, 0xC0);
    assert_eq!(opcode::LEN_STR, 0xC4);
    assert_eq!(opcode::FIND_STR, 0xC8);
    assert_eq!(opcode::REPLACE_STR, 0xCC);
    assert_eq!(opcode::INSERT_STR, 0xD0);
    assert_eq!(opcode::DELETE_STR, 0xD4);
    assert_eq!(opcode::LEFT_STR, 0xD8);
    assert_eq!(opcode::RIGHT_STR, 0xDC);
    assert_eq!(opcode::MID_STR, 0xE0);
    assert_eq!(opcode::CONCAT_STR, 0xE4);
}

#[test]
fn opcode_constants_when_string_array_family_then_pinned_bytes() {
    assert_eq!(opcode::STR_INIT_ARRAY, 0xE8);
    assert_eq!(opcode::STR_LOAD_ARRAY_ELEM, 0xEC);
    assert_eq!(opcode::STR_STORE_ARRAY_ELEM, 0xF0);
}

// ---------------------------------------------------------------------------
// 2. Encoding-scheme tests.
//
// These don't repeat the pinning above; they verify *structural*
// invariants of the `[op_class:6][type:2]` scheme. A renumbering that
// preserved every byte value but broke the scheme would be unusual,
// but a renumbering that broke a single family's invariants is
// plausible — these tests catch that.
// ---------------------------------------------------------------------------

#[test]
fn encoding_when_load_var_family_then_consistent_op_class() {
    // All LOAD_VAR_* share the same high 6 bits.
    let class = opcode::LOAD_VAR_I32 >> 2;
    assert_eq!(opcode::LOAD_VAR_I64 >> 2, class);
    assert_eq!(opcode::LOAD_VAR_F32 >> 2, class);
    assert_eq!(opcode::LOAD_VAR_F64 >> 2, class);
    // Low 2 bits map to type tag.
    assert_eq!(opcode::LOAD_VAR_I32 & 0b11, opcode::T_I32);
    assert_eq!(opcode::LOAD_VAR_I64 & 0b11, opcode::T_I64);
    assert_eq!(opcode::LOAD_VAR_F32 & 0b11, opcode::T_F32);
    assert_eq!(opcode::LOAD_VAR_F64 & 0b11, opcode::T_F64);
}

#[test]
fn encoding_when_store_var_family_then_consistent_op_class() {
    let class = opcode::STORE_VAR_I32 >> 2;
    assert_eq!(opcode::STORE_VAR_I64 >> 2, class);
    assert_eq!(opcode::STORE_VAR_F32 >> 2, class);
    assert_eq!(opcode::STORE_VAR_F64 >> 2, class);
    assert_eq!(opcode::STORE_VAR_I32 & 0b11, opcode::T_I32);
    assert_eq!(opcode::STORE_VAR_I64 & 0b11, opcode::T_I64);
    assert_eq!(opcode::STORE_VAR_F32 & 0b11, opcode::T_F32);
    assert_eq!(opcode::STORE_VAR_F64 & 0b11, opcode::T_F64);
}

#[test]
fn encoding_when_arithmetic_family_then_consistent_op_class() {
    for (i32_op, i64_op, f32_op, f64_op) in [
        (
            opcode::ADD_I32,
            opcode::ADD_I64,
            opcode::ADD_F32,
            opcode::ADD_F64,
        ),
        (
            opcode::SUB_I32,
            opcode::SUB_I64,
            opcode::SUB_F32,
            opcode::SUB_F64,
        ),
        (
            opcode::MUL_I32,
            opcode::MUL_I64,
            opcode::MUL_F32,
            opcode::MUL_F64,
        ),
        (
            opcode::NEG_I32,
            opcode::NEG_I64,
            opcode::NEG_F32,
            opcode::NEG_F64,
        ),
        (
            opcode::DIV_I32,
            opcode::DIV_I64,
            opcode::DIV_F32,
            opcode::DIV_F64,
        ),
    ] {
        let class = i32_op >> 2;
        assert_eq!(i64_op >> 2, class, "i64 vs i32 op_class mismatch");
        assert_eq!(f32_op >> 2, class, "f32 vs i32 op_class mismatch");
        assert_eq!(f64_op >> 2, class, "f64 vs i32 op_class mismatch");
        assert_eq!(i32_op & 0b11, opcode::T_I32);
        assert_eq!(i64_op & 0b11, opcode::T_I64);
        assert_eq!(f32_op & 0b11, opcode::T_F32);
        assert_eq!(f64_op & 0b11, opcode::T_F64);
    }
}

#[test]
fn encoding_when_comparison_family_then_consistent_op_class() {
    for (i32_op, i64_op, f32_op, f64_op) in [
        (
            opcode::EQ_I32,
            opcode::EQ_I64,
            opcode::EQ_F32,
            opcode::EQ_F64,
        ),
        (
            opcode::NE_I32,
            opcode::NE_I64,
            opcode::NE_F32,
            opcode::NE_F64,
        ),
        (
            opcode::LT_I32,
            opcode::LT_I64,
            opcode::LT_F32,
            opcode::LT_F64,
        ),
        (
            opcode::LE_I32,
            opcode::LE_I64,
            opcode::LE_F32,
            opcode::LE_F64,
        ),
        (
            opcode::GT_I32,
            opcode::GT_I64,
            opcode::GT_F32,
            opcode::GT_F64,
        ),
        (
            opcode::GE_I32,
            opcode::GE_I64,
            opcode::GE_F32,
            opcode::GE_F64,
        ),
    ] {
        let class = i32_op >> 2;
        assert_eq!(i64_op >> 2, class);
        assert_eq!(f32_op >> 2, class);
        assert_eq!(f64_op >> 2, class);
        assert_eq!(i32_op & 0b11, opcode::T_I32);
        assert_eq!(i64_op & 0b11, opcode::T_I64);
        assert_eq!(f32_op & 0b11, opcode::T_F32);
        assert_eq!(f64_op & 0b11, opcode::T_F64);
    }
}

#[test]
fn encoding_when_consolidated_op_class_then_type_tag_selects_member() {
    // BOOL_OP: op_class consolidates 4 family members in type-tag slots.
    let bool_class = opcode::BOOL_AND >> 2;
    assert_eq!(opcode::BOOL_OR >> 2, bool_class);
    assert_eq!(opcode::BOOL_XOR >> 2, bool_class);
    assert_eq!(opcode::BOOL_NOT >> 2, bool_class);
    assert_eq!(opcode::BOOL_AND & 0b11, 0);
    assert_eq!(opcode::BOOL_OR & 0b11, 1);
    assert_eq!(opcode::BOOL_XOR & 0b11, 2);
    assert_eq!(opcode::BOOL_NOT & 0b11, 3);

    // STACK_OP: similar consolidation for POP/DUP/SWAP.
    let stack_class = opcode::POP >> 2;
    assert_eq!(opcode::DUP >> 2, stack_class);
    assert_eq!(opcode::SWAP >> 2, stack_class);
    assert_eq!(opcode::POP & 0b11, 0);
    assert_eq!(opcode::DUP & 0b11, 1);
    assert_eq!(opcode::SWAP & 0b11, 2);
}

#[test]
fn encoding_when_decode_opcode_then_round_trips() {
    // The decode/encode helpers are part of the wire-format contract:
    // they let consumers (verifier, disassembler) interpret the byte
    // layout without per-opcode knowledge.
    for op in [
        opcode::LOAD_VAR_I32,
        opcode::STORE_VAR_F64,
        opcode::ADD_I64,
        opcode::EQ_F32,
        opcode::JMP,
        opcode::RET_VOID,
    ] {
        let (class, tag) = opcode::decode_opcode(op);
        assert_eq!(opcode::encode_opcode(class, tag), op);
        assert!(class < 64, "op_class must fit in 6 bits");
        assert!(tag < 4, "type_tag must fit in 2 bits");
    }
}

// ---------------------------------------------------------------------------
// 3. Golden encoding tests — one per shape in `instruction_size`.
//
// Each test compiles a tiny program that exercises a specific
// encoding shape and asserts the **exact** byte sequence. These guard
// operand widths, little-endian operand encoding, and per-shape
// layout. A change to any of these would fail here with a
// byte-for-byte diff.
// ---------------------------------------------------------------------------

/// Returns the bytecode of the program's main entry function.
fn bytecode_of(source: &str) -> Vec<u8> {
    let container = parse_and_compile(source, &CompilerOptions::default());
    container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(1))
        .unwrap()
        .to_vec()
}

#[test]
fn wire_when_one_byte_arith_then_no_operand_bytes() {
    // SUB_I32 is a 1-byte op (opcode only, no operand).
    let bc = bytecode_of(
        "
PROGRAM main
  VAR
    a : DINT;
    b : DINT;
  END_VAR
  a := a - b;
END_PROGRAM
",
    );
    // Last 5 bytes: SUB_I32 (1) + STORE_VAR_I32 (3) + RET_VOID (1).
    let len = bc.len();
    assert_eq!(bc[len - 5], 0x24, "SUB_I32 is one byte at expected offset");
    assert_eq!(
        &bc[len - 4..len - 1],
        &[0x10, 0x00, 0x00],
        "STORE_VAR_I32 var:0"
    );
    assert_eq!(bc[len - 1], 0x8C, "RET_VOID");
}

#[test]
fn wire_when_three_byte_load_var_then_le_u16_operand() {
    // LOAD_VAR_I32 var:1 — opcode + LE u16 operand 0x0001.
    let bc = bytecode_of(
        "
PROGRAM main
  VAR
    a : DINT;
    b : DINT;
  END_VAR
  a := b;
END_PROGRAM
",
    );
    // First 3 bytes: LOAD_VAR_I32 followed by LE u16.
    assert_eq!(
        &bc[0..3],
        &[0x0C, 0x01, 0x00],
        "LOAD_VAR_I32 var:1 (LE u16)"
    );
}

#[test]
fn wire_when_three_byte_load_const_then_le_u16_pool_index() {
    // LOAD_CONST_I32 pool:0 — opcode + LE u16.
    let bc = bytecode_of(
        "
PROGRAM main
  VAR
    a : DINT;
  END_VAR
  a := 42;
END_PROGRAM
",
    );
    assert_eq!(&bc[0..3], &[0x00, 0x00, 0x00], "LOAD_CONST_I32 pool:0");
}

#[test]
fn wire_when_jmp_then_le_i16_signed_offset() {
    // WHILE loop with a non-fusable boolean-variable condition (so it
    // falls back to the LOAD_VAR + JMP_IF_NOT shape rather than the
    // fused CMP_BR_I32 path) validates LE i16 encoding for both
    // JMP_IF_NOT (forward) and JMP (backward).
    let bc = bytecode_of(
        "
PROGRAM main
  VAR
    flag : BOOL;
    x : DINT;
  END_VAR
  WHILE flag DO
    x := x - 1;
  END_WHILE;
END_PROGRAM
",
    );
    // Layout:
    //   0: LOAD_VAR_I32 var:0 (flag)
    //   3: JMP_IF_NOT +13 -> 19
    //   6: LOAD_VAR_I32 var:1 (x)
    //   9: LOAD_CONST_I32 pool:0 (1)
    //  12: SUB_I32
    //  13: STORE_VAR_I32 var:1
    //  16: JMP -19 -> 0
    //  19: RET_VOID
    assert_eq!(bc[3], 0x80, "JMP_IF_NOT");
    assert_eq!(i16::from_le_bytes([bc[4], bc[5]]), 13, "forward LE i16");
    assert_eq!(bc[16], 0x7C, "JMP");
    assert_eq!(i16::from_le_bytes([bc[17], bc[18]]), -19, "backward LE i16");
}

#[test]
fn wire_when_five_byte_call_then_two_le_u16_operands() {
    // CALL emits opcode + LE u16 func_id + LE u16 var_offset.
    // The CALL appears in the program function (the caller), whose
    // FunctionId varies depending on how many functions are declared.
    let container = parse_and_compile(
        "
FUNCTION ADD_ONE : DINT
VAR_INPUT
    x : DINT;
END_VAR
    ADD_ONE := x + 1;
END_FUNCTION

PROGRAM main
VAR
    r : DINT;
END_VAR
    r := ADD_ONE(5);
END_PROGRAM
",
        &CompilerOptions::default(),
    );
    // Walk every function looking for CALL. Any function may contain
    // it; we just need one occurrence to validate the wire shape.
    let (call_bc, call_pos) = container
        .code
        .functions
        .iter()
        .find_map(|entry| {
            let bc = container.code.get_function_bytecode(entry.function_id)?;
            bc.iter().position(|&b| b == opcode::CALL).map(|p| (bc, p))
        })
        .expect("CALL opcode present in some function");
    assert_eq!(opcode::instruction_size(opcode::CALL), 5);
    assert!(
        call_pos + 5 <= call_bc.len(),
        "CALL has 4 trailing operand bytes"
    );
    // Decode trailing operands: two LE u16 values. We don't assert
    // exact values (codegen may renumber functions/vars) but they
    // must decode without panic.
    let _func_id = u16::from_le_bytes([call_bc[call_pos + 1], call_bc[call_pos + 2]]);
    let _var_offset = u16::from_le_bytes([call_bc[call_pos + 3], call_bc[call_pos + 4]]);
}

#[test]
fn wire_when_eight_byte_str_init_then_u32_plus_u16_plus_u8_operands() {
    // STR_INIT is an 8-byte shape (op + u32 data_offset + u16 max_len + u8
    // char_width). It appears in the program-init function (FunctionId 0),
    // not main.
    let container = parse_and_compile(
        "
PROGRAM main
  VAR
    s : STRING[10];
  END_VAR
END_PROGRAM
",
        &CompilerOptions::default(),
    );
    let init_bc = container
        .code
        .get_function_bytecode(ironplc_container::FunctionId::new(0))
        .unwrap();
    // Find STR_INIT (0xB8). Validate the 7 trailing operand bytes
    // decode as LE u32 + LE u16 + u8.
    let pos = init_bc
        .iter()
        .position(|&b| b == 0xB8)
        .expect("STR_INIT opcode present in program init");
    assert_eq!(opcode::instruction_size(opcode::STR_INIT), 8);
    assert!(
        pos + 8 <= init_bc.len(),
        "STR_INIT has 7 trailing operand bytes"
    );
    let _data_offset = u32::from_le_bytes([
        init_bc[pos + 1],
        init_bc[pos + 2],
        init_bc[pos + 3],
        init_bc[pos + 4],
    ]);
    let max_len = u16::from_le_bytes([init_bc[pos + 5], init_bc[pos + 6]]);
    assert_eq!(max_len, 10, "STRING[10] declares max_len = 10");
    assert_eq!(
        init_bc[pos + 7],
        1,
        "STRING declares char_width = 1 (narrow)"
    );
}

#[test]
fn wire_when_five_byte_len_str_then_le_u32_operand() {
    // LEN_STR is a 5-byte shape (op + u32 data_offset).
    let bc = bytecode_of(
        "
PROGRAM main
  VAR
    s : STRING[10];
    n : DINT;
  END_VAR
  n := LEN(s);
END_PROGRAM
",
    );
    let pos = bc
        .iter()
        .position(|&b| b == 0xC4)
        .expect("LEN_STR opcode present");
    assert_eq!(opcode::instruction_size(opcode::LEN_STR), 5);
    assert!(pos + 5 <= bc.len(), "LEN_STR has 4 trailing u32 bytes");
    let _data_offset = u32::from_le_bytes([bc[pos + 1], bc[pos + 2], bc[pos + 3], bc[pos + 4]]);
}

#[test]
fn wire_when_nine_byte_find_str_then_two_le_u32_operands() {
    // FIND_STR / REPLACE_STR / INSERT_STR / CONCAT_STR are 9-byte
    // shapes: opcode + two LE u32 data_offsets.
    let bc = bytecode_of(
        "
PROGRAM main
  VAR
    s1 : STRING[10];
    s2 : STRING[5];
    n : DINT;
  END_VAR
  n := FIND(s1, s2);
END_PROGRAM
",
    );
    let pos = bc
        .iter()
        .position(|&b| b == 0xC8)
        .expect("FIND_STR opcode present");
    assert_eq!(opcode::instruction_size(opcode::FIND_STR), 9);
    assert!(pos + 9 <= bc.len(), "FIND_STR has 8 trailing u32 bytes");
    let _in1 = u32::from_le_bytes([bc[pos + 1], bc[pos + 2], bc[pos + 3], bc[pos + 4]]);
    let _in2 = u32::from_le_bytes([bc[pos + 5], bc[pos + 6], bc[pos + 7], bc[pos + 8]]);
}

#[test]
fn wire_when_ret_void_then_one_byte_ends_program() {
    // Every program ends with RET_VOID (0x8C). Trivial guard, but it
    // pins a wire-level invariant: programs are byte-terminated.
    let bc = bytecode_of(
        "
PROGRAM main
  VAR
    x : DINT;
  END_VAR
  x := 0;
END_PROGRAM
",
    );
    assert_eq!(
        bc.last().copied(),
        Some(0x8C),
        "RET_VOID terminates program"
    );
}
