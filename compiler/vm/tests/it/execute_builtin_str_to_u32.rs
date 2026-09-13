//! VM tests for the `CONV_STR_TO_U32_*` builtins (ADR-0049), including the
//! VM-owned conformance tests for `specs/design/behavior-policies.md`
//! (`REQ-BP-vm-*`).
//!
//! The scan semantics are pinned here at the instruction level, one builtin
//! per non-numeric alternative, so they are independent of the compiler;
//! codegen's `end_to_end_string_to_udint` covers the same alternatives from
//! source. The rest is what only hand-assembled bytecode reaches: the trap
//! surfacing through `execute()` with its V-code, a wide operand, and an ID
//! inside the policy block that names no conversion.

use ironplc_container::builtin::str_to_num::{self, Target};
use ironplc_container::opcode;
use ironplc_container::policy::{StringToNumFailure, StringToNumNonNumeric};
use ironplc_container::{ContainerBuilder, FunctionId, VarIndex};
use ironplc_vm::error::Trap;
use ironplc_vm::StringPreview;
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::common::VmBuffers;

/// Runs `CONV_STR_TO_U32_*` for the given policies over the narrow string
/// `input`, returning the converted value or the trap.
fn convert(
    input: &[u8],
    non_numeric: StringToNumNonNumeric,
    failure: StringToNumFailure,
) -> Result<u32, Trap> {
    let func_id = str_to_num::func_id(Target::U32, non_numeric, failure);
    let bytecode = convert_bytecode(func_id, 1);
    let c = container(&bytecode, Some(input), None);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    vm.run_round(0).map_err(|fault| fault.trap)?;
    Ok(vm.read_variable(VarIndex::new(0)).unwrap() as u32)
}

fn not_convertible(input: &[u8]) -> Trap {
    Trap::StringNotConvertible {
        target: Target::U32,
        value: StringPreview::of(input),
    }
}

/// REQ-BP-vm-001: the literal grammar -- an optional sign, then decimal
/// digits or a `2#`/`8#`/`16#` based literal, with single `_` separators
/// between digits; no typed prefix.
#[spec_test(REQ_BP_vm_001)]
#[rstest]
#[case::decimal(b"123", Ok(123))]
#[case::plus(b"+123", Ok(123))]
#[case::minus_zero(b"-0", Ok(0))]
#[case::underscores(b"1_000_000", Ok(1_000_000))]
#[case::binary(b"2#1010", Ok(10))]
#[case::octal(b"8#17", Ok(15))]
#[case::hex(b"16#ff", Ok(255))]
#[case::hex_underscore(b"16#FFFF_FFFF", Ok(u32::MAX))]
#[case::typed_prefix(b"UDINT#5", Err(()))]
#[case::double_underscore(b"1__0", Err(()))]
#[case::trailing_underscore(b"10_", Err(()))]
#[case::based_without_digits(b"16#", Err(()))]
#[case::unsupported_base(b"3#12", Err(()))]
fn vm_spec_req_bp_001_literal_grammar(#[case] input: &[u8], #[case] expected: Result<u32, ()>) {
    let result = convert(
        input,
        StringToNumNonNumeric::Reject,
        StringToNumFailure::Trap,
    );
    assert_eq!(result.map_err(|_| ()), expected);
}

/// REQ-BP-vm-002: `reject` converts exactly one literal less surrounding
/// whitespace, and fails on anything else.
#[spec_test(REQ_BP_vm_002)]
#[rstest]
#[case::whole(b"42", Ok(42))]
#[case::surrounding_whitespace(b" \t42\r\n", Ok(42))]
#[case::empty(b"", Err(()))]
#[case::whitespace_only(b"  ", Err(()))]
#[case::trailing(b"42abc", Err(()))]
#[case::leading(b"abc42", Err(()))]
#[case::two_literals(b"4 2", Err(()))]
fn vm_spec_req_bp_002_reject_requires_a_whole_literal(
    #[case] input: &[u8],
    #[case] expected: Result<u32, ()>,
) {
    let result = convert(
        input,
        StringToNumNonNumeric::Reject,
        StringToNumFailure::Trap,
    );
    assert_eq!(result.map_err(|_| ()), expected);
}

/// REQ-BP-vm-003: `ignore-trailing` converts the leading literal after
/// leading whitespace and ignores the rest; no leading literal is a failure.
#[spec_test(REQ_BP_vm_003)]
#[rstest]
#[case::whole(b"42", Ok(42))]
#[case::trailing_letters(b"42abc", Ok(42))]
#[case::trailing_whitespace_then_digits(b"4 2", Ok(4))]
#[case::leading_whitespace(b"  42abc", Ok(42))]
#[case::decimal_point(b"12.5", Ok(12))]
#[case::hex_then_letters(b"16#FFxyz", Ok(255))]
#[case::leading_letters(b"abc42", Err(()))]
#[case::empty(b"", Err(()))]
fn vm_spec_req_bp_003_ignore_trailing_converts_leading_literal(
    #[case] input: &[u8],
    #[case] expected: Result<u32, ()>,
) {
    let result = convert(
        input,
        StringToNumNonNumeric::IgnoreTrailing,
        StringToNumFailure::Trap,
    );
    assert_eq!(result.map_err(|_| ()), expected);
}

/// REQ-BP-vm-004: `ignore-surrounding` skips what cannot start a literal,
/// then converts as `ignore-trailing`; no literal anywhere is a failure.
#[spec_test(REQ_BP_vm_004)]
#[rstest]
#[case::whole(b"42", Ok(42))]
#[case::leading_letters(b"abc42", Ok(42))]
#[case::both_sides(b"x=42;", Ok(42))]
#[case::sign_kept_with_number(b"v+7", Ok(7))]
#[case::minus_kept_with_number(b"a-5", Err(()))]
#[case::sign_not_before_digit(b"a-b5", Ok(5))]
#[case::no_digits(b"abc", Err(()))]
#[case::empty(b"", Err(()))]
fn vm_spec_req_bp_004_ignore_surrounding_skips_to_the_literal(
    #[case] input: &[u8],
    #[case] expected: Result<u32, ()>,
) {
    let result = convert(
        input,
        StringToNumNonNumeric::IgnoreSurrounding,
        StringToNumFailure::Trap,
    );
    assert_eq!(result.map_err(|_| ()), expected);
}

/// REQ-BP-vm-005: out of range is a failure under every non-numeric
/// alternative, and nothing wraps.
#[spec_test(REQ_BP_vm_005)]
#[rstest]
#[case::reject(StringToNumNonNumeric::Reject)]
#[case::ignore_trailing(StringToNumNonNumeric::IgnoreTrailing)]
#[case::ignore_surrounding(StringToNumNonNumeric::IgnoreSurrounding)]
fn vm_spec_req_bp_005_out_of_range_fails_under_every_alternative(
    #[case] non_numeric: StringToNumNonNumeric,
) {
    assert_eq!(
        convert(b"4294967295", non_numeric, StringToNumFailure::Trap),
        Ok(u32::MAX)
    );
    assert_eq!(
        convert(b"4294967296", non_numeric, StringToNumFailure::Trap),
        Err(not_convertible(b"4294967296"))
    );
    assert_eq!(
        convert(b"-1", non_numeric, StringToNumFailure::Trap),
        Err(not_convertible(b"-1"))
    );
    assert_eq!(
        convert(b"4294967296", non_numeric, StringToNumFailure::Zero),
        Ok(0)
    );
}

/// REQ-BP-vm-006: `trap` halts with V4006 naming the string; `zero`
/// produces 0 and continues.
#[spec_test(REQ_BP_vm_006)]
#[test]
fn vm_spec_req_bp_006_failure_traps_v4006_or_yields_zero() {
    let trap = convert(
        b"12abc",
        StringToNumNonNumeric::Reject,
        StringToNumFailure::Trap,
    )
    .unwrap_err();
    assert_eq!(trap, not_convertible(b"12abc"));
    assert_eq!(trap.v_code(), "V4006");
    assert_eq!(trap.exit_code(), 1);
    assert_eq!(
        trap.to_string(),
        "string '12abc' is not convertible to UDINT"
    );

    assert_eq!(
        convert(
            b"12abc",
            StringToNumNonNumeric::Reject,
            StringToNumFailure::Zero
        ),
        Ok(0)
    );
}

/// Bytecode that initialises a string variable at data offset 0 from the
/// string constant (pool index 1, after the i32 data offset at index 0),
/// converts it with the builtin `func_id`, and stores the result in var[0].
/// `char_width` is the string variable's encoding.
fn convert_bytecode(func_id: u16, char_width: u8) -> Vec<u8> {
    let [id_lo, id_hi] = func_id.to_le_bytes();
    #[rustfmt::skip]
    let bytecode = vec![
        opcode::STR_INIT, 0x00, 0x00, 0x00, 0x00, 0x14, 0x00, char_width, // STR_INIT offset=0, max_len=20
        opcode::LOAD_CONST_STR, 0x01, 0x00,                                // load str constant[1] (after the i32)
        opcode::STR_STORE_VAR, 0x00, 0x00, 0x00, 0x00,                     // store to string var at offset 0
        opcode::LOAD_CONST_I32, 0x00, 0x00,                                // data offset 0 (i32 constant[0])
        opcode::BUILTIN, id_lo, id_hi,
        opcode::STORE_VAR_I32, 0x00, 0x00,                                 // var[0]
        opcode::RET_VOID,
    ];
    bytecode
}

fn container(
    bytecode: &[u8],
    narrow: Option<&[u8]>,
    wide: Option<&[u8]>,
) -> ironplc_container::Container {
    let init_bytecode: Vec<u8> = vec![opcode::RET_VOID];
    let mut builder = ContainerBuilder::new()
        .num_variables(1)
        .data_region_bytes(64)
        .num_temp_bufs(4)
        .max_temp_buf_bytes(64)
        .add_i32_constant(0);
    if let Some(s) = narrow {
        builder = builder.add_str_constant(s);
    }
    if let Some(w) = wide {
        builder = builder.add_wstr_constant(w);
    }
    builder
        .add_function(FunctionId::INIT, &init_bytecode, 0, 1, 0)
        .add_function(FunctionId::SCAN, bytecode, 16, 1, 0)
        .init_function_id(FunctionId::INIT)
        .entry_function_id(FunctionId::SCAN)
        .max_call_depth(1)
        .build()
}

/// REQ-BP-vm-007 (first half): a wide operand is a V9014 regardless of
/// policy. The conversions read Latin-1 digits; codegen never emits this
/// (the analyzer rejects it), so it is reachable only from hand-assembled
/// bytecode.
#[spec_test(REQ_BP_vm_007)]
#[test]
fn execute_when_str_to_u32_given_wide_operand_then_encoding_mismatch() {
    let bytecode = convert_bytecode(opcode::builtin::CONV_STR_TO_U32_REJECT_ZERO, 2);
    let c = container(&bytecode, None, Some(&[b'4', 0, b'2', 0]));
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    crate::common::assert_trap(
        &mut vm,
        Trap::EncodingMismatch {
            expected: 1,
            actual: 2,
        },
    );
}

/// REQ-BP-vm-007 (second half): 0x0486 is a spare slot of the U32 stride,
/// inside the block but no conversion. It is an unknown builtin like any
/// other unassigned ID (ADR-0049 rule 6).
#[spec_test(REQ_BP_vm_007)]
#[test]
fn execute_when_id_in_policy_block_names_no_conversion_then_v9007() {
    let bytecode = convert_bytecode(0x0486, 1);
    let c = container(&bytecode, Some(b"1"), None);
    let mut b = VmBuffers::from_container(&c);
    let mut vm = crate::common::load_and_start(&c, &mut b).unwrap();
    crate::common::assert_trap(
        &mut vm,
        Trap::InvalidBuiltinFunction(FunctionId::new(0x0486)),
    );
}
