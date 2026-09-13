//! End-to-end tests for `STRING_TO_UDINT` under the string-to-number behavior
//! policies (ADR-0049).
//!
//! `STRING_TO_UDINT` is the steel thread for compile-time behavior policies:
//! the two policies select the builtin the call compiles to, and the VM does
//! what that builtin says. These tests run the same source under every
//! alternative and pin the result for each, including the upper half of the
//! UDINT range that used to come back as 0 (#1592).

use ironplc_container::FunctionId;
use ironplc_parser::options::{
    BehaviorPolicy, CompilerOptions, Dialect, StringToNumFailure, StringToNumNonNumeric,
};
use ironplc_vm::error::Trap;
use ironplc_vm::StringPreview;
use rstest::rstest;

use crate::common::{parse_and_compile, parse_and_run, parse_and_try_run};

/// A program converting the STRING `input` to the UDINT `x` (variable 1).
fn program(input: &str) -> String {
    format!(
        "
PROGRAM main
  VAR
    s : STRING := '{input}';
    x : UDINT;
  END_VAR
  x := STRING_TO_UDINT(s);
END_PROGRAM
"
    )
}

fn options(non_numeric: StringToNumNonNumeric, failure: StringToNumFailure) -> CompilerOptions {
    CompilerOptions {
        policy_string_to_num_non_numeric: non_numeric,
        policy_string_to_num_failure: failure,
        ..CompilerOptions::default()
    }
}

/// Runs `program(input)` under the given policies and returns `x`.
fn convert(input: &str, non_numeric: StringToNumNonNumeric, failure: StringToNumFailure) -> u32 {
    let (_c, bufs) = parse_and_run(&program(input), &options(non_numeric, failure));
    bufs.vars[1].as_i32() as u32
}

/// Runs `program(input)` under the given non-numeric policy with the `trap`
/// failure policy and returns the trap.
fn convert_expecting_trap(input: &str, non_numeric: StringToNumNonNumeric) -> Trap {
    let result = parse_and_try_run(
        &program(input),
        &options(non_numeric, StringToNumFailure::Trap),
    );
    result.expect_err("expected a trap").trap
}

// The #1592 inputs and the rest of the whole-literal syntax: every policy
// converts these to the same value, so a test per policy would be noise.
#[rstest]
#[case::control("123", 123)]
#[case::i32_max("2147483647", 2_147_483_647)]
#[case::above_i32_max("2147483648", 2_147_483_648)]
#[case::udint_max("4294967295", u32::MAX)]
#[case::hex("16#FFFF_FFFF", u32::MAX)]
#[case::binary("2#1010", 10)]
#[case::whitespace("  42  ", 42)]
fn string_to_udint_when_whole_literal_then_value_under_every_policy(
    #[case] input: &str,
    #[case] expected: u32,
) {
    for non_numeric in StringToNumNonNumeric::ALL {
        for failure in StringToNumFailure::ALL {
            assert_eq!(
                convert(input, *non_numeric, *failure),
                expected,
                "{input:?} under {non_numeric:?}/{failure:?}"
            );
        }
    }
}

// `12abc`: what each non-numeric alternative makes of trailing characters.
#[rstest]
#[case::reject(StringToNumNonNumeric::Reject, None)]
#[case::ignore_trailing(StringToNumNonNumeric::IgnoreTrailing, Some(12))]
#[case::ignore_surrounding(StringToNumNonNumeric::IgnoreSurrounding, Some(12))]
fn string_to_udint_when_trailing_characters_then_per_non_numeric_policy(
    #[case] non_numeric: StringToNumNonNumeric,
    #[case] expected: Option<u32>,
) {
    assert_eq!(
        convert("12abc", non_numeric, StringToNumFailure::Zero),
        expected.unwrap_or(0)
    );
    if expected.is_none() {
        assert!(matches!(
            convert_expecting_trap("12abc", non_numeric),
            Trap::StringNotConvertible { .. }
        ));
    }
}

// `abc12`: only `ignore-surrounding` skips leading characters.
#[rstest]
#[case::reject(StringToNumNonNumeric::Reject, None)]
#[case::ignore_trailing(StringToNumNonNumeric::IgnoreTrailing, None)]
#[case::ignore_surrounding(StringToNumNonNumeric::IgnoreSurrounding, Some(12))]
fn string_to_udint_when_leading_characters_then_per_non_numeric_policy(
    #[case] non_numeric: StringToNumNonNumeric,
    #[case] expected: Option<u32>,
) {
    assert_eq!(
        convert("abc12", non_numeric, StringToNumFailure::Zero),
        expected.unwrap_or(0)
    );
    if expected.is_none() {
        assert!(matches!(
            convert_expecting_trap("abc12", non_numeric),
            Trap::StringNotConvertible { .. }
        ));
    }
}

// Out of range is a failure under every non-numeric alternative: the
// failure policy decides between a trap and zero, and nothing wraps.
#[rstest]
#[case::reject(StringToNumNonNumeric::Reject)]
#[case::ignore_trailing(StringToNumNonNumeric::IgnoreTrailing)]
#[case::ignore_surrounding(StringToNumNonNumeric::IgnoreSurrounding)]
fn string_to_udint_when_out_of_range_then_failure_under_every_policy(
    #[case] non_numeric: StringToNumNonNumeric,
) {
    assert_eq!(
        convert("4294967296", non_numeric, StringToNumFailure::Zero),
        0
    );
    assert_eq!(
        convert_expecting_trap("4294967296", non_numeric),
        Trap::StringNotConvertible {
            target: ironplc_container::builtin::str_to_num::Target::U32,
            value: StringPreview::of(b"4294967296"),
        }
    );
}

#[test]
fn string_to_udint_when_default_options_then_reject_and_trap() {
    // The strict default: the standard leaves the result to the
    // implementer, so a string that is not a literal is an error, and an
    // error is a trap (ADR-0049 rule 4).
    let result = parse_and_try_run(&program("12abc"), &CompilerOptions::default());
    let trap = result.expect_err("expected a trap").trap;
    assert_eq!(trap.v_code(), "V4006");
    assert_eq!(
        trap.to_string(),
        "string '12abc' is not convertible to UDINT"
    );
}

#[rstest]
#[case::codesys(Dialect::Codesys)]
#[case::twincat(Dialect::TwinCat)]
fn string_to_udint_when_codesys_family_dialect_then_prefix_and_zero(#[case] dialect: Dialect) {
    // TwinCAT (observed) stops at the first invalid character, and documents
    // 0 for a string that is not valid in the target type.
    let options = CompilerOptions::from_dialect(dialect);
    let (_c, bufs) = parse_and_run(&program("12abc"), &options);
    assert_eq!(bufs.vars[1].as_i32() as u32, 12);
    let (_c, bufs) = parse_and_run(&program("abc"), &options);
    assert_eq!(bufs.vars[1].as_i32() as u32, 0);
    // Out of range is documented as processor-dependent; the preset's
    // failure policy yields zero rather than emulating something undefined.
    let (_c, bufs) = parse_and_run(&program("4294967296"), &options);
    assert_eq!(bufs.vars[1].as_i32() as u32, 0);
}

#[test]
fn string_to_udint_when_rusty_dialect_then_reject_and_zero() {
    // RuSTy rejects trailing characters and never faults. The dialect binds
    // the two uptime globals ahead of the program's variables, so `x` is
    // variable 3 here.
    let options = CompilerOptions::from_dialect(Dialect::Rusty);
    let (_c, bufs) = parse_and_run(&program("12abc"), &options);
    assert_eq!(bufs.vars[3].as_i32() as u32, 0);
    let (_c, bufs) = parse_and_run(&program("4294967295"), &options);
    assert_eq!(bufs.vars[3].as_i32() as u32, u32::MAX);
}

#[test]
fn string_to_udint_when_policies_differ_then_bytecode_differs_only_in_func_id() {
    // ADR-0049 confirmation 1: the same source under two policy selections
    // produces bytecode that differs only in the encoded alternative.
    let strict = parse_and_compile(&program("1"), &CompilerOptions::default());
    let lenient = parse_and_compile(
        &program("1"),
        &options(
            StringToNumNonNumeric::IgnoreTrailing,
            StringToNumFailure::Zero,
        ),
    );
    let strict_body = strict
        .code
        .get_function_bytecode(FunctionId::new(1))
        .unwrap();
    let lenient_body = lenient
        .code
        .get_function_bytecode(FunctionId::new(1))
        .unwrap();
    assert_eq!(strict_body.len(), lenient_body.len());
    let differing: Vec<usize> = strict_body
        .iter()
        .zip(lenient_body)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    // The func_id is a u16 operand; the two selections differ in its low
    // byte only (0x0480 versus 0x0483).
    assert_eq!(differing.len(), 1);
    let at = differing[0];
    assert_eq!(strict_body[at], 0x80);
    assert_eq!(lenient_body[at], 0x83);
}

#[test]
fn string_to_udint_when_round_trip_from_udint_to_string_then_value_preserved() {
    // The #1592 round-trip control: UDINT_TO_STRING(16#FFFFFFFF) fed straight
    // back through STRING_TO_UDINT.
    let source = "
PROGRAM main
  VAR
    big : UDINT := 16#FFFFFFFF;
    s : STRING;
    back : UDINT;
  END_VAR
  s := UDINT_TO_STRING(big);
  back := STRING_TO_UDINT(s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[2].as_i32() as u32, u32::MAX);
}
