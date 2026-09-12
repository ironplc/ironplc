//! End-to-end tests for `STRING_TO_REAL` and `STRING_TO_LREAL` under the
//! string-to-number behavior policies (ADR-0049).
//!
//! The real targets are the last on the block. They share the two policies
//! and the failure handling with the integer targets and differ in the
//! literal: a mantissa with an optional decimal point and an optional
//! exponent. `STRING_TO_REAL` used to return 0.0 on any failure whatever the
//! policy said; now the failure policy decides, and a magnitude that rounds
//! to infinity at the target's width is a failure like an integer past its
//! bounds. Nothing produces a NaN.

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

/// Runs the conversion under the given policies and returns `x` as an `f64`
/// (a `REAL` result widened exactly).
fn convert(
    type_name: &str,
    input: &str,
    non_numeric: StringToNumNonNumeric,
    failure: StringToNumFailure,
) -> f64 {
    let (_c, bufs) = parse_and_run(&program(type_name, input), &options(non_numeric, failure));
    match type_name {
        "REAL" => bufs.vars[1].as_f32() as f64,
        _ => bufs.vars[1].as_f64(),
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

// Whole literals convert identically under every policy, at both widths.
// The values are exactly representable at both widths, so the comparison is
// exact.
#[rstest]
#[case::decimal_point("9.5", 9.5)]
#[case::exponent("1.5E3", 1500.0)]
#[case::negative_exponent("25e-2", 0.25)]
#[case::integer_form("5", 5.0)]
#[case::negative("-0.75", -0.75)]
#[case::point_first(".5", 0.5)]
#[case::underscores("1_000.5", 1000.5)]
#[case::whitespace("  2.5  ", 2.5)]
fn string_to_real_when_whole_literal_then_value_under_every_policy(
    #[case] input: &str,
    #[case] expected: f64,
) {
    for type_name in ["REAL", "LREAL"] {
        for non_numeric in StringToNumNonNumeric::ALL {
            for failure in StringToNumFailure::ALL {
                assert_eq!(
                    convert(type_name, input, *non_numeric, *failure),
                    expected,
                    "{type_name} {input:?} under {non_numeric:?}/{failure:?}"
                );
            }
        }
    }
}

#[test]
fn string_to_real_when_codesys_examples_then_the_documented_values() {
    // CODESYS documents '9.876' and '1.2E-34' as convertible floating-point
    // numbers; the values are compared at each width's own rounding.
    assert_eq!(
        convert(
            "REAL",
            "9.876",
            StringToNumNonNumeric::Reject,
            StringToNumFailure::Trap
        ),
        9.876f32 as f64
    );
    assert_eq!(
        convert(
            "LREAL",
            "1.2E-34",
            StringToNumNonNumeric::Reject,
            StringToNumFailure::Trap
        ),
        1.2e-34
    );
}

// The shared invalid inputs, plus the real-specific ones, for both
// functions: `Some(v)` converts to `v` under the `zero` failure policy and
// equally under `trap`; `None` is 0.0 under `zero` and V4006 under `trap`.
#[rstest]
#[case::real("REAL", Target::F32)]
#[case::lreal("LREAL", Target::F64)]
fn string_to_real_when_invalid_inputs_then_per_non_numeric_policy(
    #[case] type_name: &str,
    #[case] target: Target,
) {
    use StringToNumNonNumeric::{IgnoreSurrounding, IgnoreTrailing, Reject};
    /// The value each non-numeric alternative converts an input to, or
    /// `None` for a failure.
    type PerPolicy = &'static [(StringToNumNonNumeric, Option<f64>)];
    let expectations: &[(&str, PerPolicy)] = &[
        (
            "12abc",
            &[
                (Reject, None),
                (IgnoreTrailing, Some(12.0)),
                (IgnoreSurrounding, Some(12.0)),
            ],
        ),
        (
            "abc12",
            &[
                (Reject, None),
                (IgnoreTrailing, None),
                (IgnoreSurrounding, Some(12.0)),
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
            "1.5.5",
            &[
                (Reject, None),
                (IgnoreTrailing, Some(1.5)),
                (IgnoreSurrounding, Some(1.5)),
            ],
        ),
        (
            "1e",
            &[
                (Reject, None),
                (IgnoreTrailing, Some(1.0)),
                (IgnoreSurrounding, Some(1.0)),
            ],
        ),
        (
            "NaN",
            &[
                (Reject, None),
                (IgnoreTrailing, None),
                (IgnoreSurrounding, None),
            ],
        ),
        (
            "inf",
            &[
                (Reject, None),
                (IgnoreTrailing, None),
                (IgnoreSurrounding, None),
            ],
        ),
        (
            "x=.5;",
            &[
                (Reject, None),
                (IgnoreTrailing, None),
                (IgnoreSurrounding, Some(0.5)),
            ],
        ),
    ];
    for (input, per_policy) in expectations {
        for (non_numeric, expected) in per_policy.iter() {
            let zero = convert(type_name, input, *non_numeric, StringToNumFailure::Zero);
            assert_eq!(
                zero,
                expected.unwrap_or(0.0),
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

// Overflow to infinity is a failure at the width that overflows, under
// every non-numeric alternative; the failure policy decides between a trap
// and 0.0, and the value is never an infinity.
#[rstest]
#[case::reject(StringToNumNonNumeric::Reject)]
#[case::ignore_trailing(StringToNumNonNumeric::IgnoreTrailing)]
#[case::ignore_surrounding(StringToNumNonNumeric::IgnoreSurrounding)]
fn string_to_real_when_magnitude_rounds_to_infinity_then_failure_at_that_width(
    #[case] non_numeric: StringToNumNonNumeric,
) {
    assert_eq!(
        convert_expecting_trap("REAL", "1e39", non_numeric),
        not_convertible(Target::F32, "1e39")
    );
    assert_eq!(
        convert("REAL", "1e39", non_numeric, StringToNumFailure::Zero),
        0.0
    );
    assert_eq!(
        convert("LREAL", "1e39", non_numeric, StringToNumFailure::Trap),
        1e39
    );
    assert_eq!(
        convert_expecting_trap("LREAL", "-1e309", non_numeric),
        not_convertible(Target::F64, "-1e309")
    );
    assert_eq!(
        convert("LREAL", "-1e309", non_numeric, StringToNumFailure::Zero),
        0.0
    );
}

#[test]
fn string_to_real_when_magnitude_underflows_then_zero_not_a_failure() {
    assert_eq!(
        convert(
            "REAL",
            "1e-50",
            StringToNumNonNumeric::Reject,
            StringToNumFailure::Trap
        ),
        0.0
    );
    assert_eq!(
        convert(
            "LREAL",
            "1e-50",
            StringToNumNonNumeric::Reject,
            StringToNumFailure::Trap
        ),
        1e-50
    );
}

#[test]
fn string_to_real_when_zero_policy_then_positive_zero() {
    let (_c, bufs) = parse_and_run(
        &program("LREAL", "NaN"),
        &options(StringToNumNonNumeric::Reject, StringToNumFailure::Zero),
    );
    let value = bufs.vars[1].as_f64();
    assert_eq!(value, 0.0);
    assert!(value.is_sign_positive());
}

#[test]
fn string_to_real_when_default_options_then_reject_and_trap() {
    let result = parse_and_try_run(&program("REAL", "xyz"), &CompilerOptions::default());
    let trap = result.expect_err("expected a trap").trap;
    assert_eq!(trap.v_code(), "V4006");
    assert_eq!(trap.to_string(), "string 'xyz' is not convertible to REAL");
}

#[rstest]
#[case::codesys(Dialect::Codesys)]
#[case::twincat(Dialect::TwinCat)]
fn string_to_real_when_codesys_family_dialect_then_prefix_and_zero(#[case] dialect: Dialect) {
    let options = CompilerOptions::from_dialect(dialect);
    let (_c, bufs) = parse_and_run(&program("REAL", "1.5abc"), &options);
    assert_eq!(bufs.vars[1].as_f32(), 1.5);
    let (_c, bufs) = parse_and_run(&program("REAL", "abc"), &options);
    assert_eq!(bufs.vars[1].as_f32(), 0.0);
}

#[test]
fn string_to_lreal_when_rusty_dialect_then_reject_and_zero() {
    // The dialect binds the two uptime globals ahead of the program's
    // variables, so `x` is variable 3 here.
    let options = CompilerOptions::from_dialect(Dialect::Rusty);
    let (_c, bufs) = parse_and_run(&program("LREAL", "1.5abc"), &options);
    assert_eq!(bufs.vars[3].as_f64(), 0.0);
    let (_c, bufs) = parse_and_run(&program("LREAL", "1.5"), &options);
    assert_eq!(bufs.vars[3].as_f64(), 1.5);
}

#[test]
fn string_to_real_when_round_trip_from_real_to_string_then_value_preserved() {
    let source = "
PROGRAM main
  VAR
    v : REAL := 3.5;
    s : STRING;
    back : REAL;
  END_VAR
  s := REAL_TO_STRING(v);
  back := STRING_TO_REAL(s);
END_PROGRAM
";
    let (_c, bufs) = parse_and_run(source, &CompilerOptions::default());
    assert_eq!(bufs.vars[2].as_f32(), 3.5);
}
