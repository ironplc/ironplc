//! Tests for a call to `MOVE`, `SEL` or `MUX`, whose inputs IEC 61131-3
//! declares as any type (`ANY`): an input of every type but a reference is
//! accepted, as it is for every parameter of `ANY`. And for a call to `MIN`,
//! `MAX` or `LIMIT`, whose inputs it declares as any elementary type
//! (`ANY_ELEMENTARY`): an input of every elementary type is accepted, and one
//! of a derived type is not. Whether the inputs a function selects between
//! have one type to compute at is `rule_function_inputs_common_type`'s.

use ironplc_parser::options::{CompilerOptions, Dialect};
use ironplc_problems::Problem;
use rstest::rstest;
use spec_test_macro::spec_test;

use crate::test_helpers::rule_codes;

/// A program declaring a variable of each kind of type, whose body assigns
/// `value` to `result` of `result_type`.
fn program(result_type: &str, value: &str) -> String {
    format!(
        "TYPE Color : (Red, Green); END_TYPE
         TYPE Point : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE
         PROGRAM main
         VAR result : {result_type}; g : BOOL; k : DINT; b : BOOL; w : WORD; t : TIME;
             d : DATE; tod1 : TOD; dt1 : DT; c : Color; s : STRING; ws : WSTRING;
             p : Point; a : ARRAY[1..2] OF DINT; END_VAR
           result := {value};
         END_PROGRAM"
    )
}

#[spec_test(REQ_IC_analyzer_129)]
#[rstest]
#[case::sel_bool("BOOL", "SEL(g, b, b)")]
#[case::sel_bit_string("WORD", "SEL(g, w, w)")]
#[case::sel_duration("TIME", "SEL(g, t, t)")]
#[case::sel_date("DATE", "SEL(g, d, d)")]
#[case::sel_time_of_day("TOD", "SEL(g, tod1, tod1)")]
#[case::sel_date_and_time("DT", "SEL(g, dt1, dt1)")]
#[case::sel_enumeration("Color", "SEL(g, c, c)")]
#[case::sel_string("STRING", "SEL(g, s, s)")]
#[case::sel_structure("Point", "SEL(g, p, p)")]
#[case::mux_bool("BOOL", "MUX(k, b, b, b)")]
#[case::mux_date_and_time("DT", "MUX(k, dt1, dt1)")]
#[case::mux_wide_string("WSTRING", "MUX(k, ws, ws)")]
#[case::mux_array("ARRAY[1..2] OF DINT", "MUX(k, a, a)")]
#[case::move_bool("BOOL", "MOVE(b)")]
#[case::move_duration("TIME", "MOVE(t)")]
#[case::move_enumeration("Color", "MOVE(c)")]
#[case::move_string("STRING", "MOVE(s)")]
#[case::move_structure("Point", "MOVE(p)")]
#[case::move_array("ARRAY[1..2] OF DINT", "MOVE(a)")]
fn apply_when_move_sel_or_mux_of_any_type_then_ok(#[case] result_type: &str, #[case] value: &str) {
    let codes = rule_codes(
        super::apply,
        &program(result_type, value),
        &CompilerOptions::default(),
    );
    assert!(codes.is_empty(), "{value}: {codes:?}");
}

#[spec_test(REQ_IC_analyzer_137)]
#[rstest]
#[case::min_bool("BOOL", "MIN(b, b)")]
#[case::max_bit_string("WORD", "MAX(w, w)")]
#[case::limit_duration("TIME", "LIMIT(t, t, t)")]
#[case::min_date("DATE", "MIN(d, d)")]
#[case::max_time_of_day("TOD", "MAX(tod1, tod1, tod1)")]
#[case::limit_date_and_time("DT", "LIMIT(dt1, dt1, dt1)")]
#[case::min_string("STRING", "MIN(s, s)")]
#[case::max_wide_string("WSTRING", "MAX(ws, ws)")]
fn apply_when_min_max_or_limit_of_elementary_type_then_ok(
    #[case] result_type: &str,
    #[case] value: &str,
) {
    let codes = rule_codes(
        super::apply,
        &program(result_type, value),
        &CompilerOptions::default(),
    );
    assert!(codes.is_empty(), "{value}: {codes:?}");
}

/// An enumeration, a structure and an array are derived types, which
/// `ANY_ELEMENTARY` does not hold: each input of one is reported.
#[spec_test(REQ_IC_analyzer_137)]
#[rstest]
#[case::min_enumeration("Color", "MIN(c, c)", 2)]
#[case::max_structure("Point", "MAX(p, p)", 2)]
#[case::limit_array("ARRAY[1..2] OF DINT", "LIMIT(a, a, a)", 3)]
fn apply_when_min_max_or_limit_of_derived_type_then_p4026(
    #[case] result_type: &str,
    #[case] value: &str,
    #[case] inputs: usize,
) {
    let codes = rule_codes(
        super::apply,
        &program(result_type, value),
        &CompilerOptions::default(),
    );
    let p4026 = Problem::FunctionCallArgTypeMismatch.code().to_string();
    assert_eq!(codes, vec![p4026; inputs], "{value}");
}

// An input of an extensible call past the ones the function declares is
// checked against the repeating parameter, `ANY` for `MUX`, which a
// reference is not.
rule_err!(
    apply_when_mux_fourth_input_is_reference_then_error,
    "
PROGRAM main
VAR
    a : DINT;
    r : REF_TO DINT;
    result : DINT;
END_VAR
    result := MUX(0, a, a, a, r);
END_PROGRAM",
    [Problem::FunctionCallArgTypeMismatch],
    CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3)
);
