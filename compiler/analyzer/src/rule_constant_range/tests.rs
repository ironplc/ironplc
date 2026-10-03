use crate::test_helpers::diagnostic_codes;
use crate::test_helpers::{codes, rule_codes, rule_diagnostics};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use rstest::rstest;

/// No problems.
const OK: &[Problem] = &[];
/// One constant outside its type's range.
const OVERFLOW: &[Problem] = &[Problem::ConstantOverflow];
const OVERFLOW_TWICE: &[Problem] = &[Problem::ConstantOverflow, Problem::ConstantOverflow];
/// One real outside its type's range.
const REAL: &[Problem] = &[Problem::RealLiteralOutOfRange];
const OVERFLOW_AND_REAL: &[Problem] = &[Problem::ConstantOverflow, Problem::RealLiteralOutOfRange];

/// The problem codes this rule reports for `program`, in order.
fn problems_of(program: &str) -> Vec<String> {
    rule_codes(super::apply, program, &CompilerOptions::default())
}

/// The diagnostics this rule reports for `program` under default options.
fn diagnostics_of(program: &str) -> Vec<Diagnostic> {
    rule_diagnostics(super::apply, program, &CompilerOptions::default())
}

fn program_with(declarations: &str, body: &str) -> String {
    format!("PROGRAM main\nVAR\n{declarations}END_VAR\n{body}END_PROGRAM\n")
}

// --- Every integer type's boundaries ---
//
// For each type: the extremes it can hold are accepted, and one step
// beyond either is reported.

#[rstest]
#[case::sint_low("SINT", "-128", OK)]
#[case::sint_high("SINT", "127", OK)]
#[case::sint_below("SINT", "-129", OVERFLOW)]
#[case::sint_above("SINT", "128", OVERFLOW)]
#[case::int_high("INT", "32767", OK)]
#[case::int_above("INT", "32768", OVERFLOW)]
#[case::dint_high("DINT", "2147483647", OK)]
#[case::dint_above("DINT", "2147483648", OVERFLOW)]
#[case::lint_high("LINT", "9223372036854775807", OK)]
#[case::lint_above("LINT", "9223372036854775808", OVERFLOW)]
#[case::usint_low("USINT", "0", OK)]
#[case::usint_high("USINT", "255", OK)]
#[case::usint_below("USINT", "-1", OVERFLOW)]
#[case::usint_above("USINT", "256", OVERFLOW)]
#[case::uint_above("UINT", "65536", OVERFLOW)]
#[case::udint_above("UDINT", "4294967296", OVERFLOW)]
#[case::ulint_high("ULINT", "18446744073709551615", OK)]
#[case::ulint_above("ULINT", "18446744073709551616", OVERFLOW)]
fn apply_when_initializer_at_boundary_then_ok_or_err(
    #[case] declared_type: &str,
    #[case] value: &str,
    #[case] expected: &[Problem],
) {
    let program = program_with(&format!("x : {declared_type} := {value};\n"), "");

    assert_eq!(problems_of(&program), codes(expected));
}

// --- The contexts a constant is checked in ---

#[test]
fn apply_when_assignment_out_of_range_then_err() {
    assert_eq!(
        problems_of(&program_with("x : USINT;\n", "x := 300;\n")),
        codes(OVERFLOW)
    );
}

/// The operator does not widen the type, so a folded constant is checked
/// exactly as a written one is.
#[test]
fn apply_when_folded_operand_out_of_range_then_err() {
    assert_eq!(
        problems_of(&program_with("x : USINT;\n", "x := 255 + 1;\n")),
        codes(OVERFLOW)
    );
}

/// A comparison happens at the variable's type, so a literal it can never
/// equal is a mistake rather than a false condition.
#[test]
fn apply_when_comparison_constant_out_of_range_then_err() {
    assert_eq!(
        problems_of(&program_with(
            "x : SINT;\ny : DINT;\n",
            "IF x = 200 THEN y := 0; END_IF;\n",
        )),
        codes(OVERFLOW)
    );
}

/// A `CASE` label the selector can never equal selects a group that can
/// never run.
#[test]
fn apply_when_case_label_out_of_range_then_err() {
    assert_eq!(
        problems_of(&program_with(
            "x : SINT;\ny : DINT;\n",
            "CASE x OF\n200: y := 1;\nEND_CASE;\n",
        )),
        codes(OVERFLOW)
    );
}

/// How a label is spelled makes no difference: each label is a value, and
/// the value is checked against the selector's type. A radix label is not a
/// bit pattern to be reinterpreted at the selector's width, so
/// `16#FFFFFFFF` is 4294967295 and no `DINT`, rather than a `DINT` -1.
#[rstest]
#[case::decimal_in_range("DINT", "2147483647", OK)]
#[case::decimal_above("DINT", "4294967295", OVERFLOW)]
#[case::decimal_below("DINT", "-2147483649", OVERFLOW)]
#[case::hex_in_range("DINT", "16#7FFFFFFF", OK)]
#[case::hex_above("DINT", "16#FFFFFFFF", OVERFLOW)]
#[case::binary_above("SINT", "2#11111111", OVERFLOW)]
#[case::octal_above("SINT", "8#377", OVERFLOW)]
#[case::hex_fills_unsigned("UDINT", "16#FFFFFFFF", OK)]
#[case::decimal_fills_unsigned("UDINT", "4294967295", OK)]
#[case::hex_above_unsigned("UDINT", "16#100000000", OVERFLOW)]
#[case::hex_fills_unsigned_64("ULINT", "16#FFFFFFFFFFFFFFFF", OK)]
#[case::hex_above_signed_64("LINT", "16#FFFFFFFFFFFFFFFF", OVERFLOW)]
#[case::negative_unsigned("USINT", "-1", OVERFLOW)]
#[case::subrange_in_range("SINT", "-128..127", OK)]
#[case::subrange_end_above("SINT", "100..300", OVERFLOW)]
#[case::subrange_start_below("SINT", "-300..0", OVERFLOW)]
#[case::subrange_both_outside("USINT", "-1..256", OVERFLOW_TWICE)]
#[case::subrange_unsigned_32("UDINT", "3000000000..4294967295", OK)]
fn apply_when_case_label_then_checked_against_selector_type(
    #[case] selector_type: &str,
    #[case] label: &str,
    #[case] expected: &[Problem],
) {
    assert_eq!(
        problems_of(&program_with(
            &format!("x : {selector_type};\ny : DINT;\n"),
            &format!("CASE x OF\n{label}: y := 1;\nEND_CASE;\n"),
        )),
        codes(expected)
    );
}

/// A selector of a subrange type can hold only the values the subrange
/// states, so a label beyond them selects a group that can never run.
#[rstest]
#[case::decimal("20")]
#[case::hex("16#14")]
#[case::subrange_bound("5..20")]
fn apply_when_case_label_outside_subrange_selector_then_err(#[case] label: &str) {
    assert_eq!(
        problems_of(&format!(
            "TYPE
Ratio : INT(0..10);
END_TYPE

PROGRAM main
VAR
    x : Ratio;
    y : DINT;
END_VAR
CASE x OF
{label}: y := 1;
END_CASE;
END_PROGRAM"
        )),
        codes(OVERFLOW)
    );
}

/// A label too large for any integer type is reported by the value the
/// source spelled, with its own sign.
#[test]
fn apply_when_case_label_beyond_every_type_then_reported_with_its_sign() {
    let program = program_with(
        "x : DINT;\ny : DINT;\n",
        "CASE x OF\n16#FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF: y := 1;\nEND_CASE;\n",
    );

    let diagnostics = diagnostics_of(&program);

    assert_eq!(
        diagnostic_codes(&diagnostics),
        [Problem::ConstantOverflow.code()]
    );
    assert!(diagnostics[0]
        .described
        .contains(&"value=340282366920938463463374607431768211455".to_owned()));
}

#[test]
fn apply_when_struct_field_out_of_range_then_err() {
    assert_eq!(
        problems_of(
            "TYPE
Counts : STRUCT
    small : USINT;
END_STRUCT;
END_TYPE

PROGRAM main
VAR
    counts : Counts;
END_VAR
    counts.small := 300;
END_PROGRAM",
        ),
        codes(OVERFLOW)
    );
}

#[test]
fn apply_when_array_element_out_of_range_then_err() {
    assert_eq!(
        problems_of(&program_with(
            "readings : ARRAY[1..2] OF USINT;\ni : DINT;\n",
            "readings[i] := 300;\n",
        )),
        codes(OVERFLOW)
    );
}

#[test]
fn apply_when_global_out_of_range_then_err() {
    assert_eq!(
        problems_of(
            "PROGRAM main
    g := 300;
END_PROGRAM

CONFIGURATION config
VAR_GLOBAL
    g : USINT;
END_VAR
RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM inst WITH plc_task : main;
END_RESOURCE
END_CONFIGURATION",
        ),
        codes(OVERFLOW)
    );
}

// --- A subrange states its own range ---

#[test]
fn apply_when_subrange_initializer_out_of_range_then_err() {
    assert_eq!(
        problems_of(
            "TYPE
Ratio : INT(0..10);
END_TYPE

PROGRAM main
VAR
    r : Ratio := 20;
END_VAR
END_PROGRAM",
        ),
        codes(OVERFLOW)
    );
}

#[test]
fn apply_when_subrange_initializer_in_range_then_ok() {
    assert_eq!(
        problems_of(
            "TYPE
Ratio : INT(0..10);
END_TYPE

PROGRAM main
VAR
    r : Ratio := 10;
END_VAR
END_PROGRAM",
        ),
        codes(OK)
    );
}

// --- A prefixed literal states its own type ---
//
// `INT#40000` is not an `INT` whatever it is stored into. Every case
// stores into a `LINT`, which holds all of these values, so the
// destination check stays silent and only the prefix decides.

#[rstest]
#[case::sint_low("SINT#-128", OK)]
#[case::sint_below("SINT#-129", OVERFLOW)]
#[case::int_high("INT#32767", OK)]
#[case::int_above("INT#32768", OVERFLOW)]
#[case::dint_above("DINT#2147483648", OVERFLOW)]
#[case::usint_high("USINT#255", OK)]
#[case::usint_negative("USINT#-1", OVERFLOW)]
#[case::udint_above("UDINT#4294967296", OVERFLOW)]
#[case::radix_high("INT#16#7FFF", OK)]
#[case::radix_above("INT#16#FFFF", OVERFLOW)]
fn apply_when_prefixed_literal_at_own_boundary_then_ok_or_err(
    #[case] literal: &str,
    #[case] expected: &[Problem],
) {
    let program = program_with(&format!("x : LINT := {literal};\n"), "");

    assert_eq!(problems_of(&program), codes(expected));
}

#[test]
fn apply_when_prefixed_literal_in_assignment_then_err() {
    assert_eq!(
        problems_of(&program_with("x : DINT;\n", "x := INT#40000;\n")),
        codes(OVERFLOW)
    );
}

#[test]
fn apply_when_prefixed_literal_in_comparison_then_err() {
    assert_eq!(
        problems_of(&program_with(
            "x : DINT;\ny : DINT;\n",
            "IF x = INT#40000 THEN y := 0; END_IF;\n",
        )),
        codes(OVERFLOW)
    );
}

/// The literal's own type travels with it into a function argument. The
/// parameter is a `LINT`, which holds 40000, so only the prefix decides.
#[test]
fn apply_when_prefixed_literal_is_function_argument_then_err() {
    assert_eq!(
        problems_of(&format!(
            "{WIDE_FUNCTION}{}",
            program_with("x : LINT;\n", "x := WIDE(INT#40000);\n")
        )),
        codes(OVERFLOW)
    );
}

const WIDE_FUNCTION: &str = "FUNCTION WIDE : LINT
VAR_INPUT p : LINT; END_VAR
WIDE := p;
END_FUNCTION
";

/// A literal that is a valid `INT` but not a valid `SINT` is the
/// destination's problem alone.
#[test]
fn apply_when_prefixed_literal_fits_own_type_only_then_one_err() {
    assert_eq!(
        problems_of(&program_with("x : SINT := INT#200;\n", "")),
        codes(OVERFLOW)
    );
}

/// The two checks answer different questions -- is this an `INT`, and
/// does it fit a `SINT` -- so a literal that fails both is reported for
/// each.
#[test]
fn apply_when_prefixed_literal_fits_neither_then_err_for_each() {
    assert_eq!(
        problems_of(&program_with("x : SINT := INT#40000;\n", "")),
        codes(OVERFLOW_TWICE)
    );
}

// --- What is deliberately not checked ---
//
// A bit string is a pattern rather than a magnitude, so wrapping one is
// a legitimate thing to want. The type decides that, not how the literal
// was spelled.

#[rstest]
#[case::byte("BYTE", "300")]
#[case::word("WORD", "70000")]
#[case::dword("DWORD", "5000000000")]
fn apply_when_bit_string_overflows_then_ok(#[case] declared_type: &str, #[case] value: &str) {
    let program = program_with(
        &format!("x : {declared_type};\n"),
        &format!("x := {value};\n"),
    );

    assert_eq!(problems_of(&program), codes(OK));
}

/// A radix does not change a value: `16#1FF` is 511, which no `USINT`
/// can hold.
#[test]
fn apply_when_radix_literal_out_of_range_then_err() {
    assert_eq!(
        problems_of(&program_with("x : USINT;\n", "x := 16#1FF;\n")),
        codes(OVERFLOW)
    );
}

/// The same literal against a type that can hold it stays silent, so the
/// check is about the value rather than the spelling.
#[test]
fn apply_when_radix_literal_in_range_then_ok() {
    assert_eq!(
        problems_of(&program_with("x : UINT;\n", "x := 16#1FF;\n")),
        codes(OK)
    );
}

// --- An untyped real literal takes the type it is stored into ---

#[rstest]
#[case::real_high("REAL", "3.4028235E38", OK)]
#[case::real_low("REAL", "-3.4028235E38", OK)]
#[case::real_above("REAL", "3.5E38", REAL)]
#[case::real_below("REAL", "-3.5E38", REAL)]
#[case::lreal_holds_it("LREAL", "1.0E300", OK)]
fn apply_when_real_initializer_at_boundary_then_ok_or_err(
    #[case] declared_type: &str,
    #[case] value: &str,
    #[case] expected: &[Problem],
) {
    let program = program_with(&format!("x : {declared_type} := {value};\n"), "");

    assert_eq!(problems_of(&program), codes(expected));
}

#[test]
fn apply_when_real_assignment_out_of_range_then_err() {
    assert_eq!(
        problems_of(&program_with("x : REAL;\n", "x := 1.0E300;\n")),
        codes(REAL)
    );
}

/// Each factor is a valid `REAL`, but their product is not.
#[test]
fn apply_when_folded_real_out_of_range_then_err() {
    assert_eq!(
        problems_of(&program_with("x : REAL;\n", "x := 1.0E30 * 1.0E30;\n")),
        codes(REAL)
    );
}

/// The operator computes at `REAL`, so the operand is a `REAL` too.
#[test]
fn apply_when_real_operand_out_of_range_then_err() {
    assert_eq!(
        problems_of(&program_with(
            "x : REAL;\ny : REAL;\n",
            "x := y + 1.0E300;\n",
        )),
        codes(REAL)
    );
}

/// A literal beyond every real type, or one that names its own type, is
/// checked by `rule_real_literal_range`, so this rule stays silent rather
/// than report it a second time.
#[rstest]
#[case::beyond_lreal("1.0E400")]
#[case::prefixed("REAL#1.0E40")]
fn apply_when_real_literal_reported_by_own_rule_then_not_reported_here(#[case] value: &str) {
    let program = program_with("x : REAL;\n", &format!("x := {value};\n"));

    assert_eq!(problems_of(&program), codes(OK));
}

#[rstest]
#[case::real("REAL", "3.4")]
#[case::time("TIME", "T#1s")]
fn apply_when_not_integer_storage_then_ok(#[case] declared_type: &str, #[case] value: &str) {
    let program = program_with(&format!("x : {declared_type} := {value};\n"), "");

    assert_eq!(problems_of(&program), codes(OK));
}

// --- Initializers, type defaults and call arguments ---
//
// Each context is checked for an integer (P2026) and an untyped real
// stored into a `REAL` (P2040), with a value that fits alongside to show
// the check is about the value.

const SMALL_TYPES: &str = "TYPE
Small : STRUCT
    i : USINT;
    r : REAL;
    a : ARRAY[1..2] OF USINT;
END_STRUCT;
Outer : STRUCT
    inner : Small;
END_STRUCT;
END_TYPE

FUNCTION TAKE : REAL
VAR_INPUT i : USINT; r : REAL; END_VAR
TAKE := r;
END_FUNCTION

FUNCTION_BLOCK HOLD
VAR_INPUT i : USINT; r : REAL; END_VAR
VAR_IN_OUT io : USINT; END_VAR
END_FUNCTION_BLOCK
";

/// The problem codes this rule reports for `SMALL_TYPES` and a program with
/// `declarations` and `body`.
fn problems_with_types(declarations: &str, body: &str) -> Vec<String> {
    problems_of(&format!(
        "{SMALL_TYPES}{}",
        program_with(declarations, body)
    ))
}

#[rstest]
#[case::array_integer("a : ARRAY[1..3] OF USINT := [1, 300, 255];\n", OVERFLOW)]
#[case::array_real("a : ARRAY[1..2] OF REAL := [1.0E300, 1.0];\n", REAL)]
#[case::array_in_range("a : ARRAY[1..2] OF USINT := [0, 255];\n", OK)]
#[case::array_repeated("a : ARRAY[1..4] OF USINT := [2(300), 2(1)];\n", OVERFLOW)]
#[case::array_multi_dimension("a : ARRAY[1..2, 1..2] OF USINT := [1, 2, 3, 300];\n", OVERFLOW)]
#[case::struct_integer("s : Small := (i := 300);\n", OVERFLOW)]
#[case::struct_real("s : Small := (r := 1.0E300);\n", REAL)]
#[case::struct_in_range("s : Small := (i := 255, r := 1.0);\n", OK)]
#[case::struct_array_field("s : Small := (a := [1, 300]);\n", OVERFLOW)]
#[case::struct_nested("o : Outer := (inner := (i := 300));\n", OVERFLOW)]
#[case::function_block_instance("h : HOLD := (i := 300, r := 1.0E300);\n", OVERFLOW_AND_REAL)]
fn apply_when_initializer_out_of_range_then_err(
    #[case] declarations: &str,
    #[case] expected: &[Problem],
) {
    assert_eq!(problems_with_types(declarations, ""), codes(expected));
}

#[rstest]
#[case::named_integer("x := TAKE(i := 300, r := 1.0);\n", OVERFLOW)]
#[case::named_real("x := TAKE(i := 1, r := 1.0E300);\n", REAL)]
#[case::positional("x := TAKE(300, 1.0E300);\n", OVERFLOW_AND_REAL)]
#[case::in_range("x := TAKE(255, 1.0);\n", OK)]
#[case::folded("x := TAKE(255 + 1, 1.0);\n", OVERFLOW)]
#[case::fb_named("h(i := 300, r := 1.0E300);\n", OVERFLOW_AND_REAL)]
#[case::fb_positional("h(300, 1.0E300);\n", OVERFLOW_AND_REAL)]
#[case::fb_in_range("h(i := 255, r := 1.0);\n", OK)]
#[case::fb_named_in_out("h(io := y);\n", OK)]
fn apply_when_call_argument_out_of_range_then_err(
    #[case] body: &str,
    #[case] expected: &[Problem],
) {
    assert_eq!(
        problems_with_types("x : REAL;\nh : HOLD;\ny : USINT;\n", body),
        codes(expected)
    );
}

/// A generic parameter states no range, so a standard function's `ANY_NUM`
/// argument is not checked; a concrete parameter of a conversion function is.
#[rstest]
#[case::generic("x : DINT;\n", "x := ADD(300, 1);\n", OK)]
#[case::conversion("x : INT;\n", "x := USINT_TO_INT(300);\n", OVERFLOW)]
fn apply_when_standard_function_argument_then_checked_by_parameter_type(
    #[case] declarations: &str,
    #[case] body: &str,
    #[case] expected: &[Problem],
) {
    assert_eq!(
        problems_of(&program_with(declarations, body)),
        codes(expected)
    );
}

#[rstest]
#[case::struct_field_integer("S : STRUCT f : USINT := 300; END_STRUCT;", OVERFLOW)]
#[case::struct_field_real("S : STRUCT f : REAL := 1.0E300; END_STRUCT;", REAL)]
#[case::struct_field_array(
    "S : STRUCT f : ARRAY[1..2] OF USINT := [1, 300]; END_STRUCT;",
    OVERFLOW
)]
#[case::alias_integer("Small : USINT := 300;", OVERFLOW)]
#[case::alias_real("R : REAL := 1.0E300;", REAL)]
#[case::alias_in_range("Small : USINT := 255;", OK)]
#[case::array_type("A : ARRAY[1..2] OF USINT := [2(300)];", OVERFLOW)]
fn apply_when_type_default_out_of_range_then_err(
    #[case] declarations: &str,
    #[case] expected: &[Problem],
) {
    let program = format!(
        "TYPE\n{declarations}\nEND_TYPE\n{}",
        program_with("x : DINT;\n", "")
    );

    assert_eq!(problems_of(&program), codes(expected));
}

#[test]
fn apply_when_assignment_to_named_subrange_out_of_range_then_err() {
    let program = format!(
        "TYPE Small : INT(0..10); END_TYPE\n{}",
        program_with("  x : Small;\n", "  x := 20;\n")
    );
    assert_eq!(problems_of(&program), codes(OVERFLOW));
}

#[test]
fn apply_when_assignment_to_named_subrange_in_range_then_ok() {
    let program = format!(
        "TYPE Small : INT(0..10); END_TYPE\n{}",
        program_with("  x : Small;\n", "  x := 10;\n")
    );
    assert_eq!(problems_of(&program), codes(OK));
}
