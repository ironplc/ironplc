//! End-to-end tests for what a value of every elementary type other than a
//! string is once it has been stored: in a variable, a global, a structure
//! field, an array element, a function's result, a function block's field and
//! output, and through a `VAR_IN_OUT` parameter.
//!
//! Each test compiles one program that stores the same expression over the
//! inputs `a` and `b` in each of those places, writes values to the inputs,
//! runs a scan and reads every place back. Two expressions are stored:
//!
//! - `a`, for every type: the value arrives unchanged, whatever its width;
//! - `a + b`, for the integers: the sum keeps the low bits of the type at
//!   every place alike (ADR-0001), so no place stores it wider or narrower
//!   than another.
//!
//! The `rstest` anchors run every type at the ends of its range. The
//! property test adds arbitrary values.

use proptest::prelude::*;
use proptest::sample::select;
use rstest::rstest;

use crate::common::run_scans;
use crate::common::scalar::{
    self, scalars, Mismatch, Num, Scalar, BOOL, BYTE, DATE, DINT, DT, DWORD, INT, INTEGERS, LDATE,
    LDT, LINT, LREAL, LTIME, LTOD, LWORD, REAL, SINT, TIME, TOD, UDINT, UINT, ULINT, USINT, WORD,
};

/// The variables that each read one place back.
const PLACES: [&str; 8] = [
    "in_variable",
    "in_global",
    "in_field",
    "in_element",
    "in_result",
    "in_block_output",
    "in_block_field",
    "in_in_out",
];

/// The program that stores `expr`, an expression over `a` and `b` of type
/// `ty`, in every place and reads each back into one of [`PLACES`].
fn program(ty: Scalar, expr: &str) -> String {
    format!(
        "
TYPE Holder : STRUCT field : {ty}; END_STRUCT; END_TYPE

FUNCTION through_result : {ty}
  VAR_INPUT a : {ty}; b : {ty}; END_VAR
  through_result := {expr};
END_FUNCTION

FUNCTION through_in_out : BOOL
  VAR_IN_OUT target : {ty}; END_VAR
  VAR_INPUT a : {ty}; b : {ty}; END_VAR
  target := {expr};
  through_in_out := TRUE;
END_FUNCTION

FUNCTION_BLOCK Block
  VAR_INPUT a : {ty}; b : {ty}; END_VAR
  VAR_OUTPUT result : {ty}; END_VAR
  VAR kept : {ty}; END_VAR
  kept := {expr};
  result := kept;
END_FUNCTION_BLOCK

CONFIGURATION config
  VAR_GLOBAL shared : {ty}; END_VAR
  RESOURCE resource1 ON PLC
    PROGRAM instance : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR_EXTERNAL shared : {ty}; END_VAR
  VAR
    a : {ty}; b : {ty};
    holder : Holder; elements : ARRAY[0..2] OF {ty}; block : Block; done : BOOL;
    in_variable : {ty}; in_global : {ty}; in_field : {ty}; in_element : {ty};
    in_result : {ty}; in_block_output : {ty}; in_block_field : {ty}; in_in_out : {ty};
  END_VAR
  in_variable := {expr};
  shared := {expr};
  in_global := shared;
  holder.field := {expr};
  in_field := holder.field;
  elements[1] := {expr};
  in_element := elements[1];
  in_result := through_result(a, b);
  block(a := a, b := b, result => in_block_output);
  in_block_field := block.result;
  done := through_in_out(in_in_out, a, b);
END_PROGRAM
",
        ty = ty.name
    )
}

/// Stores `expr` in every place once for each `(a, b)` of `inputs`, and
/// returns the places that do not hold what `oracle` says.
fn mismatches(
    ty: Scalar,
    expr: &str,
    oracle: fn(Scalar, Num, Num) -> Num,
    inputs: &[(Num, Num)],
) -> Vec<Mismatch> {
    let mut mismatches = Vec::new();
    run_scans(&program(ty, expr), &scalar::options(), |session| {
        for (scan, &(a, b)) in inputs.iter().enumerate() {
            session.write("a", ty.value(a));
            session.write("b", ty.value(b));
            session.scan(scan as u64).unwrap();
            for place in PLACES {
                mismatches.extend(Mismatch::of(
                    || format!("{place} := {expr} of {} with a = {a:?}, b = {b:?}", ty.name),
                    session.read(place),
                    ty.value(oracle(ty, a, b)),
                ));
            }
        }
    });
    mismatches
}

/// `a`, unchanged.
fn unchanged(_: Scalar, a: Num, _: Num) -> Num {
    a
}

/// The sum of two integers, with the low bits of `ty`.
fn sum(ty: Scalar, a: Num, b: Num) -> Num {
    Num::Int(ty.store(a.trunc() + b.trunc()))
}

/// Every pairing of the edge values of `ty`.
fn pairs_of_edges(ty: Scalar) -> Vec<(Num, Num)> {
    let edges = ty.edges();
    edges
        .iter()
        .flat_map(|&a| edges.iter().map(move |&b| (a, b)))
        .collect()
}

/// Any of `types`, with any two values of it.
fn any_type_and_pair(types: &[Scalar]) -> impl Strategy<Value = (Scalar, (Num, Num))> {
    select(types.to_vec()).prop_flat_map(|ty| (Just(ty), (ty.any(), ty.any())))
}

#[rstest]
fn end_to_end_when_value_at_ends_of_range_stored_then_every_place_holds_it(
    #[values(
        BOOL, SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT, REAL, LREAL, BYTE, WORD, DWORD,
        LWORD, TIME, DATE, TOD, DT, LTIME, LDATE, LTOD, LDT
    )]
    ty: Scalar,
) {
    assert_eq!(mismatches(ty, "a", unchanged, &pairs_of_edges(ty)), []);
}

#[rstest]
fn end_to_end_when_sum_at_ends_of_range_stored_then_every_place_keeps_low_bits(
    #[values(SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT)] ty: Scalar,
) {
    assert_eq!(mismatches(ty, "a + b", sum, &pairs_of_edges(ty)), []);
}

proptest! {
    // Each case compiles a program, and the anchors above already run every
    // type at the ends of its range, so a quarter of the default cases is
    // enough.
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn end_to_end_when_arbitrary_value_stored_then_every_place_holds_it(
        (ty, pair) in any_type_and_pair(&scalars()),
    ) {
        prop_assert_eq!(mismatches(ty, "a", unchanged, &[pair]), []);
    }

    #[test]
    fn end_to_end_when_arbitrary_sum_stored_then_every_place_keeps_low_bits(
        (ty, pair) in any_type_and_pair(&INTEGERS),
    ) {
        prop_assert_eq!(mismatches(ty, "a + b", sum, &[pair]), []);
    }
}
