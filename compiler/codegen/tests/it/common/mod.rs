//! Shared test helpers for codegen integration tests.

#![allow(dead_code)]
#![allow(unused_macros)]
#![allow(clippy::result_large_err)]

use ironplc_analyzer::SemanticContext;
use ironplc_codegen::compile;
use ironplc_container::Container;
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::FaultContext;
pub use ironplc_vm::VmBuffers;

/// Per-instruction bytecode builders.
///
/// Each function returns the encoded byte sequence for one
/// instruction. Use these inside [`assert_bytecode!`] to write
/// expected sequences at the **opcode-name level** without smearing
/// raw byte values across each test. When the wire encoding changes
/// (e.g. an opcode is renumbered, or the instruction format itself
/// migrates), only this module needs to update — every test that
/// uses `assert_bytecode!` continues to work.
///
/// Wire-format guarantees (opcode byte values, operand widths,
/// little-endian encoding) are pinned in `tests/wire_format.rs`. The
/// helpers here just delegate to those constants.
pub mod bc {
    use ironplc_container::opcode;
    use ironplc_container::VarIndex;

    // --- 1-byte instructions (opcode only). -------------------------------

    /// 1-byte instruction with no operand.
    fn op1(op: u8) -> Vec<u8> {
        vec![op]
    }

    pub fn add_i32() -> Vec<u8> {
        op1(opcode::ADD_I32)
    }
    pub fn add_i64() -> Vec<u8> {
        op1(opcode::ADD_I64)
    }
    pub fn add_f32() -> Vec<u8> {
        op1(opcode::ADD_F32)
    }
    pub fn add_f64() -> Vec<u8> {
        op1(opcode::ADD_F64)
    }
    pub fn sub_i32() -> Vec<u8> {
        op1(opcode::SUB_I32)
    }
    pub fn sub_i64() -> Vec<u8> {
        op1(opcode::SUB_I64)
    }
    pub fn sub_f32() -> Vec<u8> {
        op1(opcode::SUB_F32)
    }
    pub fn sub_f64() -> Vec<u8> {
        op1(opcode::SUB_F64)
    }
    pub fn mul_i32() -> Vec<u8> {
        op1(opcode::MUL_I32)
    }
    pub fn mul_i64() -> Vec<u8> {
        op1(opcode::MUL_I64)
    }
    pub fn mul_f32() -> Vec<u8> {
        op1(opcode::MUL_F32)
    }
    pub fn mul_f64() -> Vec<u8> {
        op1(opcode::MUL_F64)
    }
    pub fn div_i32() -> Vec<u8> {
        op1(opcode::DIV_I32)
    }
    pub fn div_i64() -> Vec<u8> {
        op1(opcode::DIV_I64)
    }
    pub fn div_f32() -> Vec<u8> {
        op1(opcode::DIV_F32)
    }
    pub fn div_f64() -> Vec<u8> {
        op1(opcode::DIV_F64)
    }
    pub fn div_u32() -> Vec<u8> {
        op1(opcode::DIV_U32)
    }
    pub fn div_u64() -> Vec<u8> {
        op1(opcode::DIV_U64)
    }
    pub fn mod_i32() -> Vec<u8> {
        op1(opcode::MOD_I32)
    }
    pub fn mod_i64() -> Vec<u8> {
        op1(opcode::MOD_I64)
    }
    pub fn mod_u32() -> Vec<u8> {
        op1(opcode::MOD_U32)
    }
    pub fn mod_u64() -> Vec<u8> {
        op1(opcode::MOD_U64)
    }
    pub fn neg_i32() -> Vec<u8> {
        op1(opcode::NEG_I32)
    }
    pub fn neg_i64() -> Vec<u8> {
        op1(opcode::NEG_I64)
    }
    pub fn neg_f32() -> Vec<u8> {
        op1(opcode::NEG_F32)
    }
    pub fn neg_f64() -> Vec<u8> {
        op1(opcode::NEG_F64)
    }

    pub fn eq_i32() -> Vec<u8> {
        op1(opcode::EQ_I32)
    }
    pub fn eq_i64() -> Vec<u8> {
        op1(opcode::EQ_I64)
    }
    pub fn eq_f32() -> Vec<u8> {
        op1(opcode::EQ_F32)
    }
    pub fn eq_f64() -> Vec<u8> {
        op1(opcode::EQ_F64)
    }
    pub fn ne_i32() -> Vec<u8> {
        op1(opcode::NE_I32)
    }
    pub fn ne_i64() -> Vec<u8> {
        op1(opcode::NE_I64)
    }
    pub fn ne_f32() -> Vec<u8> {
        op1(opcode::NE_F32)
    }
    pub fn ne_f64() -> Vec<u8> {
        op1(opcode::NE_F64)
    }
    pub fn lt_i32() -> Vec<u8> {
        op1(opcode::LT_I32)
    }
    pub fn lt_i64() -> Vec<u8> {
        op1(opcode::LT_I64)
    }
    pub fn lt_f32() -> Vec<u8> {
        op1(opcode::LT_F32)
    }
    pub fn lt_f64() -> Vec<u8> {
        op1(opcode::LT_F64)
    }
    pub fn lt_u32() -> Vec<u8> {
        op1(opcode::LT_U32)
    }
    pub fn lt_u64() -> Vec<u8> {
        op1(opcode::LT_U64)
    }
    pub fn le_i32() -> Vec<u8> {
        op1(opcode::LE_I32)
    }
    pub fn le_i64() -> Vec<u8> {
        op1(opcode::LE_I64)
    }
    pub fn le_f32() -> Vec<u8> {
        op1(opcode::LE_F32)
    }
    pub fn le_f64() -> Vec<u8> {
        op1(opcode::LE_F64)
    }
    pub fn le_u32() -> Vec<u8> {
        op1(opcode::LE_U32)
    }
    pub fn le_u64() -> Vec<u8> {
        op1(opcode::LE_U64)
    }
    pub fn gt_i32() -> Vec<u8> {
        op1(opcode::GT_I32)
    }
    pub fn gt_i64() -> Vec<u8> {
        op1(opcode::GT_I64)
    }
    pub fn gt_f32() -> Vec<u8> {
        op1(opcode::GT_F32)
    }
    pub fn gt_f64() -> Vec<u8> {
        op1(opcode::GT_F64)
    }
    pub fn gt_u32() -> Vec<u8> {
        op1(opcode::GT_U32)
    }
    pub fn gt_u64() -> Vec<u8> {
        op1(opcode::GT_U64)
    }
    pub fn ge_i32() -> Vec<u8> {
        op1(opcode::GE_I32)
    }
    pub fn ge_i64() -> Vec<u8> {
        op1(opcode::GE_I64)
    }
    pub fn ge_f32() -> Vec<u8> {
        op1(opcode::GE_F32)
    }
    pub fn ge_f64() -> Vec<u8> {
        op1(opcode::GE_F64)
    }
    pub fn ge_u32() -> Vec<u8> {
        op1(opcode::GE_U32)
    }
    pub fn ge_u64() -> Vec<u8> {
        op1(opcode::GE_U64)
    }

    pub fn bool_and() -> Vec<u8> {
        op1(opcode::BOOL_AND)
    }
    pub fn bool_or() -> Vec<u8> {
        op1(opcode::BOOL_OR)
    }
    pub fn bool_xor() -> Vec<u8> {
        op1(opcode::BOOL_XOR)
    }
    pub fn bool_not() -> Vec<u8> {
        op1(opcode::BOOL_NOT)
    }

    pub fn bit_and_32() -> Vec<u8> {
        op1(opcode::BIT_AND_32)
    }
    pub fn bit_and_64() -> Vec<u8> {
        op1(opcode::BIT_AND_64)
    }
    pub fn bit_or_32() -> Vec<u8> {
        op1(opcode::BIT_OR_32)
    }
    pub fn bit_or_64() -> Vec<u8> {
        op1(opcode::BIT_OR_64)
    }
    pub fn bit_xor_32() -> Vec<u8> {
        op1(opcode::BIT_XOR_32)
    }
    pub fn bit_xor_64() -> Vec<u8> {
        op1(opcode::BIT_XOR_64)
    }
    pub fn bit_not_32() -> Vec<u8> {
        op1(opcode::BIT_NOT_32)
    }
    pub fn bit_not_64() -> Vec<u8> {
        op1(opcode::BIT_NOT_64)
    }

    pub fn trunc_i8() -> Vec<u8> {
        op1(opcode::TRUNC_I8)
    }
    pub fn trunc_u8() -> Vec<u8> {
        op1(opcode::TRUNC_U8)
    }
    pub fn trunc_i16() -> Vec<u8> {
        op1(opcode::TRUNC_I16)
    }
    pub fn trunc_u16() -> Vec<u8> {
        op1(opcode::TRUNC_U16)
    }

    pub fn load_true() -> Vec<u8> {
        op1(opcode::LOAD_TRUE)
    }
    pub fn load_false() -> Vec<u8> {
        op1(opcode::LOAD_FALSE)
    }
    pub fn load_indirect() -> Vec<u8> {
        op1(opcode::LOAD_INDIRECT)
    }
    pub fn store_indirect() -> Vec<u8> {
        op1(opcode::STORE_INDIRECT)
    }

    pub fn pop() -> Vec<u8> {
        op1(opcode::POP)
    }
    pub fn dup() -> Vec<u8> {
        op1(opcode::DUP)
    }
    pub fn swap() -> Vec<u8> {
        op1(opcode::SWAP)
    }
    pub fn ret() -> Vec<u8> {
        op1(opcode::RET)
    }
    pub fn ret_void() -> Vec<u8> {
        op1(opcode::RET_VOID)
    }

    // --- 3-byte instructions: opcode + LE u16 operand. --------------------

    /// 3-byte instruction with a u16 little-endian operand.
    fn op_u16(op: u8, value: u16) -> Vec<u8> {
        let b = value.to_le_bytes();
        vec![op, b[0], b[1]]
    }

    pub fn load_const_i32(pool_index: u16) -> Vec<u8> {
        op_u16(opcode::LOAD_CONST_I32, pool_index)
    }
    pub fn load_const_i64(pool_index: u16) -> Vec<u8> {
        op_u16(opcode::LOAD_CONST_I64, pool_index)
    }
    pub fn load_const_f32(pool_index: u16) -> Vec<u8> {
        op_u16(opcode::LOAD_CONST_F32, pool_index)
    }
    pub fn load_const_f64(pool_index: u16) -> Vec<u8> {
        op_u16(opcode::LOAD_CONST_F64, pool_index)
    }
    pub fn load_const_str(pool_index: u16) -> Vec<u8> {
        op_u16(opcode::LOAD_CONST_STR, pool_index)
    }

    pub fn load_var_i32(idx: u16) -> Vec<u8> {
        op_u16(opcode::LOAD_VAR_I32, idx)
    }
    pub fn load_var_i64(idx: u16) -> Vec<u8> {
        op_u16(opcode::LOAD_VAR_I64, idx)
    }
    pub fn load_var_f32(idx: u16) -> Vec<u8> {
        op_u16(opcode::LOAD_VAR_F32, idx)
    }
    pub fn load_var_f64(idx: u16) -> Vec<u8> {
        op_u16(opcode::LOAD_VAR_F64, idx)
    }
    pub fn store_var_i32(idx: u16) -> Vec<u8> {
        op_u16(opcode::STORE_VAR_I32, idx)
    }
    pub fn store_var_i64(idx: u16) -> Vec<u8> {
        op_u16(opcode::STORE_VAR_I64, idx)
    }
    pub fn store_var_f32(idx: u16) -> Vec<u8> {
        op_u16(opcode::STORE_VAR_F32, idx)
    }
    pub fn store_var_f64(idx: u16) -> Vec<u8> {
        op_u16(opcode::STORE_VAR_F64, idx)
    }

    pub fn load_var_i32_idx(idx: VarIndex) -> Vec<u8> {
        op_u16(opcode::LOAD_VAR_I32, idx.raw())
    }
    pub fn store_var_i32_idx(idx: VarIndex) -> Vec<u8> {
        op_u16(opcode::STORE_VAR_I32, idx.raw())
    }

    pub fn fb_load_instance(idx: u16) -> Vec<u8> {
        op_u16(opcode::FB_LOAD_INSTANCE, idx)
    }
    pub fn fb_call(type_id: u16) -> Vec<u8> {
        op_u16(opcode::FB_CALL, type_id)
    }
    pub fn builtin(func_id: u16) -> Vec<u8> {
        op_u16(opcode::BUILTIN, func_id)
    }

    /// JMP with signed i16 offset (relative, in bytes, from the byte
    /// after the operand).
    pub fn jmp(offset: i16) -> Vec<u8> {
        let b = offset.to_le_bytes();
        vec![opcode::JMP, b[0], b[1]]
    }

    /// JMP_IF_NOT with signed i16 offset.
    pub fn jmp_if_not(offset: i16) -> Vec<u8> {
        let b = offset.to_le_bytes();
        vec![opcode::JMP_IF_NOT, b[0], b[1]]
    }

    /// CMP_BR_I32 (fused compare-and-branch on 32-bit signed integers).
    /// Operands: cmp_op byte, var index (u16), constant pool index (u16),
    /// signed i16 jump offset.
    pub fn cmp_br_i32(cmp_op_byte: u8, var_idx: u16, const_idx: u16, offset: i16) -> Vec<u8> {
        let v = var_idx.to_le_bytes();
        let c = const_idx.to_le_bytes();
        let o = offset.to_le_bytes();
        vec![
            opcode::CMP_BR_I32,
            cmp_op_byte,
            v[0],
            v[1],
            c[0],
            c[1],
            o[0],
            o[1],
        ]
    }

    /// CMP_BR_I64 (fused compare-and-branch on 64-bit signed integers).
    /// See `cmp_br_i32` for operand layout.
    pub fn cmp_br_i64(cmp_op_byte: u8, var_idx: u16, const_idx: u16, offset: i16) -> Vec<u8> {
        let v = var_idx.to_le_bytes();
        let c = const_idx.to_le_bytes();
        let o = offset.to_le_bytes();
        vec![
            opcode::CMP_BR_I64,
            cmp_op_byte,
            v[0],
            v[1],
            c[0],
            c[1],
            o[0],
            o[1],
        ]
    }

    // --- 2-byte instructions: opcode + u8 operand. ------------------------

    pub fn fb_store_param(field: u8) -> Vec<u8> {
        vec![opcode::FB_STORE_PARAM, field]
    }
    pub fn fb_load_param(field: u8) -> Vec<u8> {
        vec![opcode::FB_LOAD_PARAM, field]
    }

    // --- 5-byte instructions: opcode + u16 + u16. -------------------------

    fn op_u16_u16(op: u8, a: u16, b: u16) -> Vec<u8> {
        let ab = a.to_le_bytes();
        let bb = b.to_le_bytes();
        vec![op, ab[0], ab[1], bb[0], bb[1]]
    }

    pub fn call(func_id: u16, var_offset: u16) -> Vec<u8> {
        op_u16_u16(opcode::CALL, func_id, var_offset)
    }
    pub fn load_array(var_idx: u16, desc_idx: u16) -> Vec<u8> {
        op_u16_u16(opcode::LOAD_ARRAY, var_idx, desc_idx)
    }
    pub fn store_array(var_idx: u16, desc_idx: u16) -> Vec<u8> {
        op_u16_u16(opcode::STORE_ARRAY, var_idx, desc_idx)
    }
    pub fn load_array_deref(ref_idx: u16, desc_idx: u16) -> Vec<u8> {
        op_u16_u16(opcode::LOAD_ARRAY_DEREF, ref_idx, desc_idx)
    }
    pub fn store_array_deref(ref_idx: u16, desc_idx: u16) -> Vec<u8> {
        op_u16_u16(opcode::STORE_ARRAY_DEREF, ref_idx, desc_idx)
    }
    pub fn str_init_array(var_idx: u16, desc_idx: u16) -> Vec<u8> {
        op_u16_u16(opcode::STR_INIT_ARRAY, var_idx, desc_idx)
    }
    pub fn str_load_array_elem(var_idx: u16, desc_idx: u16) -> Vec<u8> {
        op_u16_u16(opcode::STR_LOAD_ARRAY_ELEM, var_idx, desc_idx)
    }
    pub fn str_store_array_elem(var_idx: u16, desc_idx: u16) -> Vec<u8> {
        op_u16_u16(opcode::STR_STORE_ARRAY_ELEM, var_idx, desc_idx)
    }

    // --- 5-byte instructions: opcode + u32. -------------------------------

    fn op_u32(op: u8, value: u32) -> Vec<u8> {
        let b = value.to_le_bytes();
        vec![op, b[0], b[1], b[2], b[3]]
    }

    pub fn str_load_var(data_offset: u32) -> Vec<u8> {
        op_u32(opcode::STR_LOAD_VAR, data_offset)
    }
    pub fn str_store_var(data_offset: u32) -> Vec<u8> {
        op_u32(opcode::STR_STORE_VAR, data_offset)
    }
    pub fn len_str(data_offset: u32) -> Vec<u8> {
        op_u32(opcode::LEN_STR, data_offset)
    }
    pub fn delete_str(data_offset: u32) -> Vec<u8> {
        op_u32(opcode::DELETE_STR, data_offset)
    }
    pub fn left_str(data_offset: u32) -> Vec<u8> {
        op_u32(opcode::LEFT_STR, data_offset)
    }
    pub fn right_str(data_offset: u32) -> Vec<u8> {
        op_u32(opcode::RIGHT_STR, data_offset)
    }
    pub fn mid_str(data_offset: u32) -> Vec<u8> {
        op_u32(opcode::MID_STR, data_offset)
    }

    // --- 8-byte instruction: opcode + u32 + u16 + u8 char_width. ----------

    pub fn str_init(data_offset: u32, max_length: u16, char_width: u8) -> Vec<u8> {
        let d = data_offset.to_le_bytes();
        let m = max_length.to_le_bytes();
        vec![
            opcode::STR_INIT,
            d[0],
            d[1],
            d[2],
            d[3],
            m[0],
            m[1],
            char_width,
        ]
    }

    // --- 9-byte instructions: opcode + u32 + u32. -------------------------

    fn op_u32_u32(op: u8, a: u32, b: u32) -> Vec<u8> {
        let ab = a.to_le_bytes();
        let bb = b.to_le_bytes();
        vec![op, ab[0], ab[1], ab[2], ab[3], bb[0], bb[1], bb[2], bb[3]]
    }

    pub fn find_str(in1: u32, in2: u32) -> Vec<u8> {
        op_u32_u32(opcode::FIND_STR, in1, in2)
    }
    pub fn replace_str(in1: u32, in2: u32) -> Vec<u8> {
        op_u32_u32(opcode::REPLACE_STR, in1, in2)
    }
    pub fn insert_str(in1: u32, in2: u32) -> Vec<u8> {
        op_u32_u32(opcode::INSERT_STR, in1, in2)
    }
    pub fn concat_str(in1: u32, in2: u32) -> Vec<u8> {
        op_u32_u32(opcode::CONCAT_STR, in1, in2)
    }
}

/// Asserts that `actual` matches the given expected sequence of
/// instruction-encoded byte vectors.
///
/// Each item in the list returns a `Vec<u8>` (typically from a
/// helper in [`bc`]); this macro concatenates them and compares
/// with the actual byte slice.
///
/// On mismatch, both sides are formatted as hex byte sequences so
/// the failing offset is visible. Wire-format guarantees (opcode
/// byte values, operand widths, endianness) are pinned in
/// `tests/wire_format.rs` — failures here surface *behavioural*
/// drift (codegen emitted wrong opcodes, wrong order, wrong
/// operands), not encoding drift.
///
/// Example:
///
/// ```ignore
/// use common::bc;
///
/// assert_bytecode!(actual, [
///     bc::load_var_i32(0),
///     bc::load_const_i32(0),
///     bc::gt_i32(),
///     bc::jmp_if_not(13),
///     bc::ret_void(),
/// ]);
/// ```
macro_rules! assert_bytecode {
    ($actual:expr, [ $($expr:expr),* $(,)? ]) => {{
        let mut __expected: Vec<u8> = Vec::new();
        $( __expected.extend($expr); )*
        let __actual: &[u8] = $actual;
        if __actual != __expected.as_slice() {
            panic!(
                "\nbytecode mismatch:\n  actual ({} bytes):   {:02X?}\n  expected ({} bytes): {:02X?}\n",
                __actual.len(), __actual,
                __expected.len(), __expected,
            );
        }
    }};
}

/// Parses an IEC 61131-3 source string and runs type resolution via the analyzer.
///
/// The analyzer populates `Expr.resolved_type` and resolves type aliases in
/// variable declarations, which codegen requires.
pub fn parse(source: &str, options: &CompilerOptions) -> (Library, SemanticContext) {
    let library = parse_program(source, &FileId::default(), options).unwrap();
    let (analyzed, ctx) = ironplc_analyzer::stages::resolve_types(&[&library], options).unwrap();
    (analyzed, ctx)
}

/// Parses, analyzes, and compiles an IEC 61131-3 source string into a Container.
pub fn parse_and_compile(source: &str, options: &CompilerOptions) -> Container {
    try_parse_and_compile(source, options).unwrap()
}

/// Like [`parse_and_compile`], but returns the Result so callers can test error cases.
pub fn try_parse_and_compile(
    source: &str,
    options: &CompilerOptions,
) -> Result<Container, Diagnostic> {
    let (library, context) = parse(source, options);
    let codegen_options = ironplc_codegen::CodegenOptions {
        system_uptime_global: options.allow_system_uptime_global,
    };
    compile(
        &library,
        &context,
        &codegen_options,
        &ironplc_codegen::EmptyLookup,
    )
}

/// Parses, analyzes, compiles, and runs one scan cycle.
/// Returns the container and buffers so callers can inspect variable values.
pub fn parse_and_run(source: &str, options: &CompilerOptions) -> (Container, VmBuffers) {
    let (container, bufs) =
        parse_and_try_run(source, options).expect("VM execution trapped unexpectedly");
    (container, bufs)
}

/// Parses, analyzes, compiles, and runs one scan cycle, returning `Err` on VM trap.
/// Use this to test that certain programs produce runtime traps.
pub fn parse_and_try_run(
    source: &str,
    options: &CompilerOptions,
) -> Result<(Container, VmBuffers), FaultContext> {
    let (library, context) = parse(source, options);
    let codegen_options = ironplc_codegen::CodegenOptions {
        system_uptime_global: options.allow_system_uptime_global,
    };
    let container = compile(
        &library,
        &context,
        &codegen_options,
        &ironplc_codegen::EmptyLookup,
    )
    .unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = load_and_start(&container, &mut bufs)?;
        vm.run_round(0)?;
    }
    Ok((container, bufs))
}

/// Parses, analyzes, compiles, and runs a multi-round test scenario.
///
/// The closure receives a mutable VM reference so it can write variables,
/// run multiple rounds, and read back results.
pub fn parse_and_run_rounds(
    source: &str,
    options: &CompilerOptions,
    f: impl FnOnce(&mut ironplc_vm::VmRunning<'_>),
) {
    let (library, context) = parse(source, options);
    let codegen_options = ironplc_codegen::CodegenOptions {
        system_uptime_global: options.allow_system_uptime_global,
    };
    let container = compile(
        &library,
        &context,
        &codegen_options,
        &ironplc_codegen::EmptyLookup,
    )
    .unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    let mut vm = load_and_start(&container, &mut bufs).unwrap();
    f(&mut vm);
}

/// Runs `source` with default options and asserts each `(var_index, expected)`
/// pair against the corresponding `vars[i].as_i32()` slot after one scan.
///
/// This is the workhorse helper for the `end_to_end_*.rs` integer tests:
/// it collapses the recurring 3-line scaffold (`let source ...; let (_c, bufs)
/// = parse_and_run(...); assert_eq!(...)`) into a single call so that each
/// `#[test] fn` becomes one statement. Reduces duplicate AST mass enough that
/// `cargo dupes` no longer flags the tests as a group.
pub fn assert_run_i32(source: &str, asserts: &[(usize, i32)]) {
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    for (idx, expected) in asserts {
        assert_eq!(bufs.vars[*idx].as_i32(), *expected, "vars[{idx}] mismatch");
    }
}

/// Same as [`assert_run_i32`] but reads slots as i64. Use for LINT/ULINT or
/// any value whose magnitude exceeds 32 bits.
pub fn assert_run_i64(source: &str, asserts: &[(usize, i64)]) {
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    for (idx, expected) in asserts {
        assert_eq!(bufs.vars[*idx].as_i64(), *expected, "vars[{idx}] mismatch");
    }
}

/// Like [`assert_run_i32`] but with explicit [`CompilerOptions`]. Use when a
/// test requires a non-default dialect flag (e.g. `allow_partial_access_syntax`).
pub fn assert_run_i32_with(source: &str, options: &CompilerOptions, asserts: &[(usize, i32)]) {
    let (_c, bufs) = parse_and_run(source, options);
    for (idx, expected) in asserts {
        assert_eq!(bufs.vars[*idx].as_i32(), *expected, "vars[{idx}] mismatch");
    }
}

/// Same as [`assert_run_i32`] but reads slots as f32 (REAL). Uses exact bit
/// equality so tests must choose inputs that produce deterministic results.
pub fn assert_run_f32(source: &str, asserts: &[(usize, f32)]) {
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    for (idx, expected) in asserts {
        assert_eq!(bufs.vars[*idx].as_f32(), *expected, "vars[{idx}] mismatch");
    }
}

/// Same as [`assert_run_i32`] but reads slots as f64 (LREAL). Uses exact bit
/// equality so tests must choose inputs that produce deterministic results.
pub fn assert_run_f64(source: &str, asserts: &[(usize, f64)]) {
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    for (idx, expected) in asserts {
        assert_eq!(bufs.vars[*idx].as_f64(), *expected, "vars[{idx}] mismatch");
    }
}

/// Like [`assert_run_f32`] but asserts each value is within `tolerance` of
/// the expected value. Use when arithmetic (pow, transcendentals) produces
/// values that can't be represented exactly in f32.
pub fn assert_run_f32_near(source: &str, tolerance: f32, asserts: &[(usize, f32)]) {
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    for (idx, expected) in asserts {
        let actual = bufs.vars[*idx].as_f32();
        assert!(
            (actual - *expected).abs() < tolerance,
            "vars[{idx}]: expected {expected}, got {actual}"
        );
    }
}

/// Like [`assert_run_f64`] but asserts each value is within `tolerance` of
/// the expected value.
pub fn assert_run_f64_near(source: &str, tolerance: f64, asserts: &[(usize, f64)]) {
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    for (idx, expected) in asserts {
        let actual = bufs.vars[*idx].as_f64();
        assert!(
            (actual - *expected).abs() < tolerance,
            "vars[{idx}]: expected {expected}, got {actual}"
        );
    }
}

/// Declares a `#[test] fn` that asserts an IEC 61131-3 program produces the
/// given i32 var values.
///
/// The macro form (vs writing the `#[test] fn` body directly as
/// `{ assert_run_i32(...); }`) matters for code duplication: without it,
/// every short 6-line body gets regrouped by `cargo dupes` as a new
/// exact-duplicate set. A macro invocation is opaque to the detector, so
/// each test becomes a single token and no new group forms.
///
/// Any `#[...]` attributes (including `///` docstrings) preceding the
/// macro invocation are forwarded to the generated `fn`.
///
/// These macros are made visible across the `it` test binary by the
/// `#[macro_use] mod common;` declaration in `tests/it.rs`. They reference
/// `$crate::common::...` so they resolve correctly when expanded inside a
/// sibling submodule (e.g. `tests/it/end_to_end_bit_access.rs`).
macro_rules! e2e_i32 {
    ($(#[$meta:meta])* $name:ident, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run_i32($source, $asserts);
        }
    };
}

/// Same as [`e2e_i32`] but reads slots as i64 (LINT/ULINT).
macro_rules! e2e_i64 {
    ($(#[$meta:meta])* $name:ident, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run_i64($source, $asserts);
        }
    };
}

/// Like [`e2e_i32`] but takes a [`CompilerOptions`] expression so the test
/// can enable a non-default dialect flag. The options expression is
/// evaluated once inside the generated test body.
macro_rules! e2e_i32_with {
    ($(#[$meta:meta])* $name:ident, $opts:expr, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run_i32_with($source, &$opts, $asserts);
        }
    };
}

/// Same as [`e2e_i32`] but reads slots as f32 (REAL).
macro_rules! e2e_f32 {
    ($(#[$meta:meta])* $name:ident, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run_f32($source, $asserts);
        }
    };
}

/// Same as [`e2e_i32`] but reads slots as f64 (LREAL).
macro_rules! e2e_f64 {
    ($(#[$meta:meta])* $name:ident, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run_f64($source, $asserts);
        }
    };
}

/// Same as [`e2e_f32`] but takes an explicit tolerance. Use when the expected
/// f32 value cannot be represented exactly (e.g. results of `**`, sqrt, ln).
macro_rules! e2e_f32_near {
    ($(#[$meta:meta])* $name:ident, $tol:expr, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run_f32_near($source, $tol, $asserts);
        }
    };
}

/// Same as [`e2e_f64`] but takes an explicit tolerance.
macro_rules! e2e_f64_near {
    ($(#[$meta:meta])* $name:ident, $tol:expr, $source:literal, $asserts:expr $(,)?) => {
        $(#[$meta])*
        #[test]
        fn $name() {
            $crate::common::assert_run_f64_near($source, $tol, $asserts);
        }
    };
}
