//! End-to-end tests for the integer `STRING_TO_*` functions other than
//! `STRING_TO_UDINT` under the string-to-number behavior policies
//! (ADR-0049).
//!
//! `STRING_TO_UDINT` was the steel thread (`end_to_end_string_to_udint`);
//! these are the targets that joined it. Each converts under the same two
//! policies through the same scanner, and differs only in its bounds, so the
//! tests are one table of functions run at min, max, one past each bound and
//! the shared invalid inputs, under all six policy combinations. The
//! sub-32-bit targets used to wrap (`STRING_TO_SINT('300')` gave 44); now
//! out of range is a failure like any other.

use ironplc_container::builtin::str_to_num::Target;
use ironplc_parser::options::{
    BehaviorPolicy, CompilerOptions, Dialect, StringToNumFailure, StringToNumNonNumeric,
};
use ironplc_vm::error::Trap;
use ironplc_vm::StringPreview;
use rstest::rstest;

use crate::common::{parse_and_run, parse_and_try_run};

/// A program converting the STRING `input` to `x : <type_name>` (variable 1)
/// with `STRING_TO_<type_name>`.
fn program(type_name: &str, input: &str) -> String {
    format!(
        "
PROGRAM main
  VAR
    s : STRING := '{input}';
    x : {type_name};
  END_VAR
  x := STRING_TO_{type_name}(s);
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

/// Runs the conversion under the given policies and returns `x` as the
/// value the declared type holds, read at `target`'s width and signedness:
/// a signed type's slot is sign-extended, an unsigned type's slot holds the
/// value's low bits.
fn convert(
    type_name: &str,
    target: Target,
    input: &str,
    non_numeric: StringToNumNonNumeric,
    failure: StringToNumFailure,
) -> i128 {
    let (_c, bufs) = parse_and_run(&program(type_name, input), &options(non_numeric, failure));
    let slot = bufs.vars[1];
    match target {
        Target::I8 | Target::I16 | Target::I32 => slot.as_i32() as i128,
        Target::U8 | Target::U16 | Target::U32 => slot.as_i32() as u32 as i128,
        Target::I64 => slot.as_i64() as i128,
        Target::U64 => slot.as_u64() as i128,
    }
}

/// Runs the conversion under the given non-numeric policy with the `trap`
/// failure policy and returns the trap.
fn convert_expecting_trap(
    type_name: &str,
    input: &str,
    non_numeric: StringToNumNonNumeric,
) -> Trap {
    let result = parse_and_try_run(
        &program(type_name, input),
        &options(non_numeric, StringToNumFailure::Trap),
    );
    result.expect_err("expected a trap").trap
}

fn not_convertible(target: Target, input: &str) -> Trap {
    Trap::StringNotConvertible {
        target,
        value: StringPreview::of(input.as_bytes()),
    }
}

// The eleven functions: the type, the block target it encodes, and its
// bounds. The bit-string types share the unsigned target of their width.
#[rstest]
#[case::sint("SINT", Target::I8, -128, 127)]
#[case::int("INT", Target::I16, -32_768, 32_767)]
#[case::dint("DINT", Target::I32, -2_147_483_648, 2_147_483_647)]
#[case::usint("USINT", Target::U8, 0, 255)]
#[case::uint("UINT", Target::U16, 0, 65_535)]
#[case::byte("BYTE", Target::U8, 0, 255)]
#[case::word("WORD", Target::U16, 0, 65_535)]
#[case::dword("DWORD", Target::U32, 0, 4_294_967_295)]
#[case::lint("LINT", Target::I64, -9_223_372_036_854_775_808, 9_223_372_036_854_775_807)]
#[case::ulint("ULINT", Target::U64, 0, 18_446_744_073_709_551_615)]
#[case::lword("LWORD", Target::U64, 0, 18_446_744_073_709_551_615)]
fn string_to_int_when_at_bounds_then_value_under_every_policy(
    #[case] type_name: &str,
    #[case] target: Target,
    #[case] min: i128,
    #[case] max: i128,
) {
    for non_numeric in StringToNumNonNumeric::ALL {
        for failure in StringToNumFailure::ALL {
            let at = |v: i128| convert(type_name, target, &v.to_string(), *non_numeric, *failure);
            assert_eq!(
                at(min),
                min,
                "{type_name} min under {non_numeric:?}/{failure:?}"
            );
            assert_eq!(
                at(max),
                max,
                "{type_name} max under {non_numeric:?}/{failure:?}"
            );
            assert_eq!(
                at(0),
                0,
                "{type_name} zero under {non_numeric:?}/{failure:?}"
            );
        }
    }
}

#[rstest]
#[case::sint("SINT", Target::I8, -128, 127)]
#[case::int("INT", Target::I16, -32_768, 32_767)]
#[case::dint("DINT", Target::I32, -2_147_483_648, 2_147_483_647)]
#[case::usint("USINT", Target::U8, 0, 255)]
#[case::uint("UINT", Target::U16, 0, 65_535)]
#[case::byte("BYTE", Target::U8, 0, 255)]
#[case::word("WORD", Target::U16, 0, 65_535)]
#[case::dword("DWORD", Target::U32, 0, 4_294_967_295)]
#[case::lint("LINT", Target::I64, -9_223_372_036_854_775_808, 9_223_372_036_854_775_807)]
#[case::ulint("ULINT", Target::U64, 0, 18_446_744_073_709_551_615)]
#[case::lword("LWORD", Target::U64, 0, 18_446_744_073_709_551_615)]
fn string_to_int_when_one_past_each_bound_then_failure_under_every_policy(
    #[case] type_name: &str,
    #[case] target: Target,
    #[case] min: i128,
    #[case] max: i128,
) {
    // Out of range is a failure, never a wrap: the failure policy decides
    // between a trap and zero, whatever the non-numeric policy.
    for past in [min - 1, max + 1] {
        let input = past.to_string();
        for non_numeric in StringToNumNonNumeric::ALL {
            assert_eq!(
                convert(
                    type_name,
                    target,
                    &input,
                    *non_numeric,
                    StringToNumFailure::Zero
                ),
                0,
                "{type_name} {input} under {non_numeric:?}/zero"
            );
            assert_eq!(
                convert_expecting_trap(type_name, &input, *non_numeric),
                not_convertible(target, &input),
                "{type_name} {input} under {non_numeric:?}/trap"
            );
        }
    }
}

// The shared invalid inputs: what each non-numeric alternative makes of
// trailing characters, leading characters, nothing numeric at all, and a
// based literal, for every function. `Some(v)` converts to `v` under the
// `zero` failure policy and equally under `trap`; `None` is 0 under `zero`
// and V4006 under `trap`.
#[rstest]
#[case::sint("SINT", Target::I8)]
#[case::int("INT", Target::I16)]
#[case::dint("DINT", Target::I32)]
#[case::usint("USINT", Target::U8)]
#[case::uint("UINT", Target::U16)]
#[case::byte("BYTE", Target::U8)]
#[case::word("WORD", Target::U16)]
#[case::dword("DWORD", Target::U32)]
#[case::lint("LINT", Target::I64)]
#[case::ulint("ULINT", Target::U64)]
#[case::lword("LWORD", Target::U64)]
fn string_to_int_when_shared_invalid_inputs_then_per_non_numeric_policy(
    #[case] type_name: &str,
    #[case] target: Target,
) {
    use StringToNumNonNumeric::{IgnoreSurrounding, IgnoreTrailing, Reject};
    /// The value each non-numeric alternative converts an input to, or
    /// `None` for a failure.
    type PerPolicy = &'static [(StringToNumNonNumeric, Option<i128>)];
    let expectations: &[(&str, PerPolicy)] = &[
        (
            "12abc",
            &[
                (Reject, None),
                (IgnoreTrailing, Some(12)),
                (IgnoreSurrounding, Some(12)),
            ],
        ),
        (
            "abc12",
            &[
                (Reject, None),
                (IgnoreTrailing, None),
                (IgnoreSurrounding, Some(12)),
            ],
        ),
        (
            "abc",
            &[
                (Reject, None),
                (IgnoreTrailing, None),
                (IgnoreSurrounding, None),
            ],
        ),
        (
            "",
            &[
                (Reject, None),
                (IgnoreTrailing, None),
                (IgnoreSurrounding, None),
            ],
        ),
        (
            "  16#7F  ",
            &[
                (Reject, Some(127)),
                (IgnoreTrailing, Some(127)),
                (IgnoreSurrounding, Some(127)),
            ],
        ),
    ];
    for (input, per_policy) in expectations {
        for (non_numeric, expected) in per_policy.iter() {
            let zero = convert(
                type_name,
                target,
                input,
                *non_numeric,
                StringToNumFailure::Zero,
            );
            assert_eq!(
                zero,
                expected.unwrap_or(0),
                "{type_name} {input:?} under {non_numeric:?}/zero"
            );
            let trap = parse_and_try_run(
                &program(type_name, input),
                &options(*non_numeric, StringToNumFailure::Trap),
            )
            .map(|_| ())
            .map_err(|fault| fault.trap);
            let expected_trap = match expected {
                Some(_) => Ok(()),
                None => Err(not_convertible(target, input)),
            };
            assert_eq!(
                trap, expected_trap,
                "{type_name} {input:?} under {non_numeric:?}/trap"
            );
        }
    }
}

#[test]
fn string_to_sint_when_300_then_trap_names_sint_and_never_44() {
    // The value this series exists to stop: '300' used to convert to 300
    // and then truncate to 44.
    let result = parse_and_try_run(&program("SINT", "300"), &CompilerOptions::default());
    let trap = result.expect_err("expected a trap").trap;
    assert_eq!(trap.v_code(), "V4006");
    assert_eq!(trap.to_string(), "string '300' is not convertible to SINT");
    assert_eq!(
        convert(
            "SINT",
            Target::I8,
            "300",
            StringToNumNonNumeric::Reject,
            StringToNumFailure::Zero
        ),
        0
    );
}

#[test]
fn string_to_byte_when_not_convertible_then_trap_names_the_unsigned_type() {
    // BYTE shares USINT's encoding, so the trap names USINT.
    let trap = convert_expecting_trap("BYTE", "300", StringToNumNonNumeric::Reject);
    assert_eq!(trap.to_string(), "string '300' is not convertible to USINT");
}

#[test]
fn string_to_lword_when_not_convertible_then_trap_names_ulint() {
    // LWORD shares ULINT's encoding, so the trap names ULINT; the preview
    // keeps the first sixteen bytes of a 64-bit literal.
    let trap = convert_expecting_trap(
        "LWORD",
        "18446744073709551616",
        StringToNumNonNumeric::Reject,
    );
    assert_eq!(
        trap.to_string(),
        "string '1844674407370955'... is not convertible to ULINT"
    );
}

#[test]
fn string_to_lint_when_based_literal_then_64_bit_value() {
    assert_eq!(
        convert(
            "LINT",
            Target::I64,
            "-16#8000_0000_0000_0000",
            StringToNumNonNumeric::Reject,
            StringToNumFailure::Trap
        ),
        i64::MIN as i128
    );
}

#[rstest]
#[case::negative_one("-1")]
#[case::minus_before_max("-255")]
fn string_to_usint_when_negative_then_failure_not_wrap(#[case] input: &str) {
    // An unsigned target has no negative values; -1 is not 255.
    assert_eq!(
        convert_expecting_trap("USINT", input, StringToNumNonNumeric::Reject),
        not_convertible(Target::U8, input)
    );
    assert_eq!(
        convert(
            "USINT",
            Target::U8,
            input,
            StringToNumNonNumeric::IgnoreSurrounding,
            StringToNumFailure::Zero
        ),
        0
    );
}

#[test]
fn string_to_sint_when_minus_zero_then_zero() {
    assert_eq!(
        convert(
            "SINT",
            Target::I8,
            "-0",
            StringToNumNonNumeric::Reject,
            StringToNumFailure::Trap
        ),
        0
    );
}

#[rstest]
#[case::codesys(Dialect::Codesys)]
#[case::twincat(Dialect::TwinCat)]
fn string_to_sint_when_codesys_family_dialect_then_prefix_and_zero(#[case] dialect: Dialect) {
    let options = CompilerOptions::from_dialect(dialect);
    let (_c, bufs) = parse_and_run(&program("SINT", "12abc"), &options);
    assert_eq!(bufs.vars[1].as_i32(), 12);
    // Out of range is documented as processor-dependent by CODESYS; the
    // preset's failure policy yields zero, and never the wrapped 44.
    let (_c, bufs) = parse_and_run(&program("SINT", "300"), &options);
    assert_eq!(bufs.vars[1].as_i32(), 0);
}

#[test]
fn string_to_int_when_rusty_dialect_then_reject_and_zero() {
    // The dialect binds the two uptime globals ahead of the program's
    // variables, so `x` is variable 3 here.
    let options = CompilerOptions::from_dialect(Dialect::Rusty);
    let (_c, bufs) = parse_and_run(&program("INT", "12abc"), &options);
    assert_eq!(bufs.vars[3].as_i32(), 0);
    let (_c, bufs) = parse_and_run(&program("INT", "-32768"), &options);
    assert_eq!(bufs.vars[3].as_i32(), -32_768);
}

#[test]
fn string_to_int_when_round_trip_from_int_to_string_then_value_preserved() {
    let source = "
PROGRAM main
  VAR
    low : INT := -32768;
    s : STRING;
    back : INT;
  END_VAR
  s := INT_TO_STRING(low);
  back := STRING_TO_INT(s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[2].as_i32(), -32_768);
}
