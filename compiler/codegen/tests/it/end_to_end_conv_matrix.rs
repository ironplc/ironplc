//! End-to-end tests for every conversion function between two elementary
//! types other than a string: all 430 `<source>_TO_<target>` functions.
//!
//! Each test compiles one program that converts an input `x` of one source
//! type to every type it converts to, writes values to `x`, runs a scan and
//! compares every result with a Rust oracle. The oracle states the rules a
//! conversion follows whatever backend computes it:
//!
//! - to an integer, a bit string, a duration, a date or a time, an integer
//!   keeps its low bits, sign extended into a signed type;
//! - to a `BOOL`, any value other than zero is `TRUE`;
//! - to a real, an integer rounds to nearest;
//! - from a real, the fraction is dropped and the value saturates at the
//!   range of the operation width, then keeps the low bits of a narrower type.
//!
//! A duration, a date or a time converts as the count it holds. A result of
//! one of those types is read back as that count, through the conversion to
//! the integer type that holds it unchanged.
//!
//! The `rstest` anchors run every source type at the ends of its range, so
//! every function is called on every run. The property test adds arbitrary
//! values.

use proptest::prelude::*;
use proptest::sample::select;
use rstest::rstest;

use crate::common::run_scans;
use crate::common::scalar::{
    self, scalars, Mismatch, Num, Repr, Scalar, BOOL, BYTE, DATE, DINT, DT, DWORD, INT, LDATE, LDT,
    LINT, LREAL, LTIME, LTOD, LWORD, REAL, SINT, TIME, TOD, UDINT, UINT, ULINT, USINT, WORD,
};

/// Whether `<source>_TO_<target>` is a standard function. Every pair of
/// different types is, except two durations, dates or times, and a `BOOL`
/// with anything but an integer or a bit string.
fn converts(source: Scalar, target: Scalar) -> bool {
    let counts = |ty: Scalar| matches!(ty.repr, Repr::Int { .. }) && !ty.is_temporal();
    let with_bool = source == BOOL || target == BOOL;
    source != target
        && !(source.is_temporal() && target.is_temporal())
        && (!with_bool || counts(source) || counts(target))
}

/// The types `source` converts to.
fn targets(source: Scalar) -> Vec<Scalar> {
    scalars()
        .into_iter()
        .filter(|&target| converts(source, target))
        .collect()
}

/// The type of the variable that holds a result of type `target`: the type
/// itself, or the integer that holds the count of a duration, date or time.
fn observed(target: Scalar) -> Scalar {
    if target.is_temporal() {
        target.carrier()
    } else {
        target
    }
}

/// The expression that converts `x` to `target`, as a value of
/// [`observed`]`(target)`.
fn conversion(source: Scalar, target: Scalar) -> String {
    let convert = format!("{}_TO_{}(x)", source.name, target.name);
    if target.is_temporal() {
        format!("{}_TO_{}({convert})", target.name, observed(target).name)
    } else {
        convert
    }
}

/// The program that converts `x` to every target of `source`.
fn program(source: Scalar) -> String {
    let targets = targets(source);
    let results: String = targets
        .iter()
        .map(|target| format!("y_{} : {}; ", target.name, observed(*target).name))
        .collect();
    let body: String = targets
        .iter()
        .map(|target| format!("  y_{} := {};\n", target.name, conversion(source, *target)))
        .collect();
    format!(
        "PROGRAM main\n  VAR x : {}; {results}END_VAR\n{body}END_PROGRAM\n",
        source.name
    )
}

/// What `number` converts to as a value of `target`.
fn convert(number: Num, target: Scalar) -> Num {
    match (number, target.repr) {
        (Num::Int(count), Repr::Bool) => Num::Int((count != 0).into()),
        (Num::Int(count), Repr::Int { .. }) => Num::Int(target.store(count)),
        (Num::Int(count), Repr::Real { bits: 32 }) => Num::Real((count as f32).into()),
        (Num::Int(count), Repr::Real { .. }) => Num::Real(count as f64),
        (Num::Real(real), Repr::Real { bits: 32 }) => Num::Real((real as f32).into()),
        (Num::Real(real), Repr::Real { .. }) => Num::Real(real),
        // `as` drops the fraction, saturates and takes a NaN to zero, which
        // is what a conversion does at the operation width.
        (Num::Real(real), Repr::Int { signed, .. }) => {
            let top = 1i128 << (target.operation_bits() - 1);
            let (lowest, highest) = if signed {
                (-top, top - 1)
            } else {
                (0, 2 * top - 1)
            };
            Num::Int(target.store((real as i128).clamp(lowest, highest)))
        }
        // No real converts to a `BOOL`, so nothing asks for this.
        (Num::Real(real), Repr::Bool) => Num::Int((real != 0.0).into()),
    }
}

/// Converts each of `inputs` from `source` to every type it converts to, and
/// returns the results that differ from what the oracle says.
fn mismatches(source: Scalar, inputs: &[Num]) -> Vec<Mismatch> {
    let mut mismatches = Vec::new();
    run_scans(&program(source), &scalar::options(), |session| {
        for (scan, &input) in inputs.iter().enumerate() {
            session.write("x", source.value(input));
            session.scan(scan as u64).unwrap();
            for target in targets(source) {
                mismatches.extend(Mismatch::of(
                    || format!("{}_TO_{} of {input:?}", source.name, target.name),
                    session.read(&format!("y_{}", target.name)),
                    observed(target).value(convert(input, target)),
                ));
            }
        }
    });
    mismatches
}

/// Any source type, with any value of it.
fn any_source_and_value() -> impl Strategy<Value = (Scalar, Num)> {
    select(scalars()).prop_flat_map(|source| (Just(source), source.any()))
}

#[rstest]
fn end_to_end_when_source_at_ends_of_range_then_every_conversion_matches_oracle(
    #[values(
        BOOL, SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT, REAL, LREAL, BYTE, WORD, DWORD,
        LWORD, TIME, DATE, TOD, DT, LTIME, LDATE, LTOD, LDT
    )]
    source: Scalar,
) {
    assert_eq!(mismatches(source, &source.edges()), []);
}

proptest! {
    // Each case compiles a program, and the anchors above already run every
    // type at the ends of its range, so a quarter of the default cases is
    // enough.
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn end_to_end_when_source_arbitrary_then_every_conversion_matches_oracle(
        (source, value) in any_source_and_value(),
    ) {
        prop_assert_eq!(mismatches(source, &[value]), []);
    }
}

/// Every pair `converts` accepts is compiled by the tests above, which fail
/// on a function that does not exist. This pins the count, so a pair dropped
/// from `converts` by mistake is noticed too.
#[test]
fn converts_when_every_pair_counted_then_430_functions() {
    let functions: usize = scalars()
        .into_iter()
        .map(|source| targets(source).len())
        .sum();

    assert_eq!(functions, 430);
}
