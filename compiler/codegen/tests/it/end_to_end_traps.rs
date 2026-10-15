//! End-to-end tests for the run-time traps a program can raise, and for what
//! a trap leaves behind.
//!
//! A trap is named by its problem code (`V4001` for a division by zero,
//! `V4002` for a negative exponent, `V4005` for a subscript out of bounds),
//! which is what any backend reports. Every case also checks that the
//! statements before the trap took effect and that none after it ran.
//!
//! A null dereference (`V4004`) is covered beside references, in
//! `end_to_end_ref.rs`, and a string that does not convert (`V4006`) beside
//! the conversions, in `end_to_end_string_to_int.rs`.

use ironplc_parser::options::CompilerOptions;
use proptest::prelude::*;
use proptest::sample::select;
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::common::scalar::{Num, Scalar, INTEGERS};
use crate::common::Snapshot;

/// Runs `statement` between an assignment to `before` and one to `after`,
/// and returns the code of the trap it raised with what the two hold.
fn trap_between(variables: &str, statement: &str) -> (Option<&'static str>, bool, bool) {
    let source = format!(
        "PROGRAM main\n  VAR {variables} before : BOOL; after : BOOL; END_VAR\n  \
         before := TRUE;\n  {statement}\n  after := TRUE;\nEND_PROGRAM\n"
    );
    let (snapshot, fault) = Snapshot::try_run(&source, &CompilerOptions::default());
    (
        fault.map(|fault| fault.trap.v_code()),
        snapshot.read_as::<i32>("before") == 1,
        snapshot.read_as::<i32>("after") == 1,
    )
}

/// What [`trap_between`] returns for a statement that traps with `code`:
/// `before` was assigned and `after` was not.
fn trapped(code: &'static str) -> (Option<&'static str>, bool, bool) {
    (Some(code), true, false)
}

#[rstest]
fn end_to_end_when_integer_divisor_is_zero_then_traps_at_every_type(
    #[values("SINT", "INT", "DINT", "LINT", "USINT", "UINT", "UDINT", "ULINT")] ty: &str,
    #[values("a / zero", "a MOD zero", "DIV(a, zero)", "MOD(a, zero)")] expr: &str,
) {
    let variables = format!("a : {ty} := 7; zero : {ty}; result : {ty};");

    assert_eq!(
        trap_between(&variables, &format!("result := {expr};")),
        trapped("V4001")
    );
}

#[rstest]
// An integer raised to a negative power has no integer result.
#[case::operator_on_dint(
    "base : DINT := 2; power : DINT := -1; r : DINT;",
    "r := base ** power;"
)]
#[case::operator_on_lint(
    "base : LINT := 2; power : LINT := -1; r : LINT;",
    "r := base ** power;"
)]
#[case::function_on_dint(
    "base : DINT := 2; power : DINT := -1; r : DINT;",
    "r := EXPT(base, power);"
)]
fn end_to_end_when_integer_exponent_is_negative_then_traps(
    #[case] variables: &str,
    #[case] statement: &str,
) {
    assert_eq!(trap_between(variables, statement), trapped("V4002"));
}

#[rstest]
#[case::read_above_upper_bound(
    "cells : ARRAY[1..3] OF DINT; i : DINT := 4; r : DINT;",
    "r := cells[i];"
)]
#[case::write_below_lower_bound("cells : ARRAY[1..3] OF DINT; i : DINT := 0;", "cells[i] := 5;")]
#[case::read_below_negative_lower_bound(
    "cells : ARRAY[-2..2] OF DINT; i : DINT := -3; r : DINT;",
    "r := cells[i];"
)]
// Analysis checks a literal subscript, but not one that names a constant.
#[case::subscript_names_a_constant("cells : ARRAY[1..3] OF DINT; r : DINT;", "r := cells[LIMIT];")]
// The check is on the position counted across every dimension (ADR-0023),
// so the last row's fifth column lies past the end of a three by four array.
#[case::position_past_last_dimension(
    "grid : ARRAY[1..3, 1..4] OF DINT; row : DINT := 3; column : DINT := 5; r : DINT;",
    "r := grid[row, column];"
)]
#[case::element_of_narrow_type("cells : ARRAY[1..3] OF SINT; i : DINT := 4;", "cells[i] := 5;")]
fn end_to_end_when_subscript_out_of_bounds_then_traps(
    #[case] variables: &str,
    #[case] statement: &str,
) {
    let variables = format!("CONSTANT LIMIT : DINT := 4; END_VAR VAR {variables}");

    assert_eq!(trap_between(&variables, statement), trapped("V4005"));
}

/// A subscript outside the range of `DINT` traps before the array is
/// reached, rather than wrapping to the position of another element: each
/// of these would otherwise select one.
#[spec_test(REQ_IC_codegen_022)]
#[rstest]
#[case::lint_beyond_32_bits(
    "cells : ARRAY[0..3] OF DINT; i : LINT := 4294967297; r : DINT;",
    "r := cells[i];"
)]
#[case::udint_top_bit_set(
    "cells : ARRAY[-3..100] OF DINT; i : UDINT := 4294967295;",
    "cells[i] := 5;"
)]
#[case::dword_top_bit_set(
    "cells : ARRAY[-3..100] OF DINT; i : DWORD := 16#FFFFFFFF; r : DINT;",
    "r := cells[i];"
)]
#[case::ulint_top_bit_set(
    "cells : ARRAY[-3..100] OF DINT; i : ULINT := 18446744073709551615;",
    "cells[i] := 5;"
)]
#[case::lword_beyond_32_bits(
    "grid : ARRAY[0..1, 0..1] OF DINT; i : LWORD := 16#100000000; r : DINT;",
    "r := grid[0, i];"
)]
fn end_to_end_when_subscript_outside_dint_then_traps(
    #[case] variables: &str,
    #[case] statement: &str,
) {
    assert_eq!(trap_between(variables, statement), trapped("V4007"));
}

/// ADR-0023 checks the position, not each subscript against its own
/// dimension, so a subscript out of range in one dimension that lands inside
/// the array selects the element at that position and does not trap.
#[test]
fn end_to_end_when_subscripts_out_of_dimension_but_inside_array_then_selects_by_position() {
    let source = "
PROGRAM main
  VAR
    grid : ARRAY[1..3, 1..4] OF DINT;
    zero : DINT := 0; one : DINT := 1; five : DINT := 5;
    below_first_row : DINT; past_first_row : DINT;
  END_VAR
  grid[1, 1] := 11;
  grid[2, 1] := 21;
  below_first_row := grid[zero, five];
  past_first_row := grid[one, five];
END_PROGRAM
";
    let (snapshot, fault) = Snapshot::try_run(source, &CompilerOptions::default());

    assert_eq!(fault.map(|fault| fault.trap.v_code()), None);
    assert_eq!(snapshot.read("below_first_row"), 11);
    assert_eq!(snapshot.read("past_first_row"), 21);
}

/// Any integer type, with any value of it.
fn any_subscript() -> impl Strategy<Value = (Scalar, Num)> {
    select(INTEGERS.to_vec()).prop_flat_map(|ty| (Just(ty), ty.any()))
}

/// The lower bound of every array a subscript indexes in these tests:
/// below zero, so that a subscript read as another number, such as an
/// unsigned one whose top bit is set read as a negative one, would select an
/// element.
const LOWER: i128 = -3;

/// What a store to `cells[subscript]` does when the array starts at `LOWER`
/// and ends at 100: reaches the element the subscript names, or traps and
/// reaches none, which leaves `found` at -99. A subscript outside the range
/// of `DINT` traps before the array is reached (V4007), and any other that
/// names no element when the array is (V4005).
fn store_outcome(subscript: i128) -> (Option<&'static str>, i128) {
    if i32::try_from(subscript).is_err() {
        (Some("V4007"), -99)
    } else if (LOWER..=100).contains(&subscript) {
        (None, subscript)
    } else {
        (Some("V4005"), -99)
    }
}

proptest! {
    // A subscript of any integer type selects the element it names, or traps
    // when it names none. The value is not cut to a narrower width first: a
    // subscript of 4294967297, or a UDINT whose top bit is set, names no
    // element of this array. `found` is the subscript of the element the
    // store reached, found by searching with a DINT.
    #[test]
    fn end_to_end_when_subscript_arbitrary_then_selects_its_element_or_traps(
        (ty, subscript) in any_subscript(),
    ) {
        let subscript = subscript.trunc();
        let lower = LOWER;
        let source = format!(
            "
PROGRAM main
  VAR cells : ARRAY[{lower}..100] OF DINT; i : {} := {subscript}; k : DINT; found : DINT := -99; END_VAR
  cells[i] := 7;
  FOR k := {lower} TO 100 DO
    IF cells[k] = 7 THEN found := k; END_IF;
  END_FOR;
END_PROGRAM
",
            ty.name
        );

        let (snapshot, fault) = Snapshot::try_run(&source, &CompilerOptions::default());

        prop_assert_eq!(
            (fault.map(|fault| fault.trap.v_code()), snapshot.read_as::<i128>("found")),
            store_outcome(subscript)
        );
    }
}
