use crate::stages::analyze;
use ironplc_dsl::core::FileId;
use ironplc_parser::{options::CompilerOptions, parse_program};
use ironplc_problems::Problem;
use rstest::rstest;

/// Analyzes `program`, returning how many out-of-range constants it
/// reported. Naming the problem keeps a diagnostic from another rule
/// from passing for one of ours.
fn out_of_range_count(program: &str) -> usize {
    problem_count(program, Problem::ConstantOverflow)
}

/// Analyzes `program`, returning how many real literals it reported as
/// outside their type's range.
fn real_out_of_range_count(program: &str) -> usize {
    problem_count(program, Problem::RealLiteralOutOfRange)
}

fn problem_count(program: &str, problem: Problem) -> usize {
    let options = CompilerOptions::default();
    let library = parse_program(program, &FileId::default(), &options).unwrap();
    let (_library, context) = analyze(&[&library], &options).unwrap();
    context
        .diagnostics()
        .iter()
        .filter(|d| d.code == problem.code())
        .count()
}

fn program_with(declarations: &str, body: &str) -> String {
    format!("PROGRAM main\nVAR\n{declarations}END_VAR\n{body}END_PROGRAM\n")
}

// --- Every integer type's boundaries ---
//
// For each type: the extremes it can hold are accepted, and one step
// beyond either is reported.

#[rstest]
#[case::sint_low("SINT", "-128", true)]
#[case::sint_high("SINT", "127", true)]
#[case::sint_below("SINT", "-129", false)]
#[case::sint_above("SINT", "128", false)]
#[case::int_high("INT", "32767", true)]
#[case::int_above("INT", "32768", false)]
#[case::dint_high("DINT", "2147483647", true)]
#[case::dint_above("DINT", "2147483648", false)]
#[case::lint_high("LINT", "9223372036854775807", true)]
#[case::lint_above("LINT", "9223372036854775808", false)]
#[case::usint_low("USINT", "0", true)]
#[case::usint_high("USINT", "255", true)]
#[case::usint_below("USINT", "-1", false)]
#[case::usint_above("USINT", "256", false)]
#[case::uint_above("UINT", "65536", false)]
#[case::udint_above("UDINT", "4294967296", false)]
#[case::ulint_high("ULINT", "18446744073709551615", true)]
#[case::ulint_above("ULINT", "18446744073709551616", false)]
fn apply_when_initializer_at_boundary_then_ok_or_err(
    #[case] declared_type: &str,
    #[case] value: &str,
    #[case] expected_ok: bool,
) {
    let program = program_with(&format!("x : {declared_type} := {value};\n"), "");

    assert_eq!(out_of_range_count(&program) == 0, expected_ok);
}

// --- The contexts a constant is checked in ---

#[test]
fn apply_when_assignment_out_of_range_then_err() {
    let codes = out_of_range_count(&program_with("x : USINT;\n", "x := 300;\n"));

    assert_eq!(codes, 1);
}

/// The operator does not widen the type, so a folded constant is checked
/// exactly as a written one is.
#[test]
fn apply_when_folded_operand_out_of_range_then_err() {
    let codes = out_of_range_count(&program_with("x : USINT;\n", "x := 255 + 1;\n"));

    assert_eq!(codes, 1);
}

/// A comparison happens at the variable's type, so a literal it can never
/// equal is a mistake rather than a false condition.
#[test]
fn apply_when_comparison_constant_out_of_range_then_err() {
    let codes = out_of_range_count(&program_with(
        "x : SINT;\ny : DINT;\n",
        "IF x = 200 THEN y := 0; END_IF;\n",
    ));

    assert_eq!(codes, 1);
}

/// A `CASE` label the selector can never equal selects a group that can
/// never run.
#[test]
fn apply_when_case_label_out_of_range_then_err() {
    let codes = out_of_range_count(&program_with(
        "x : SINT;\ny : DINT;\n",
        "CASE x OF\n200: y := 1;\nEND_CASE;\n",
    ));

    assert_eq!(codes, 1);
}

#[test]
fn apply_when_struct_field_out_of_range_then_err() {
    let codes = out_of_range_count(
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
    );

    assert_eq!(codes, 1);
}

#[test]
fn apply_when_array_element_out_of_range_then_err() {
    let codes = out_of_range_count(&program_with(
        "readings : ARRAY[1..2] OF USINT;\ni : DINT;\n",
        "readings[i] := 300;\n",
    ));

    assert_eq!(codes, 1);
}

#[test]
fn apply_when_global_out_of_range_then_err() {
    let codes = out_of_range_count(
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
    );

    assert_eq!(codes, 1);
}

// --- A subrange states its own range ---

#[test]
fn apply_when_subrange_initializer_out_of_range_then_err() {
    let codes = out_of_range_count(
        "TYPE
Ratio : INT(0..10);
END_TYPE

PROGRAM main
VAR
    r : Ratio := 20;
END_VAR
END_PROGRAM",
    );

    assert_eq!(codes, 1);
}

#[test]
fn apply_when_subrange_initializer_in_range_then_ok() {
    let codes = out_of_range_count(
        "TYPE
Ratio : INT(0..10);
END_TYPE

PROGRAM main
VAR
    r : Ratio := 10;
END_VAR
END_PROGRAM",
    );

    assert_eq!(codes, 0);
}

// --- A prefixed literal states its own type ---
//
// `INT#40000` is not an `INT` whatever it is stored into. Every case
// stores into a `LINT`, which holds all of these values, so the
// destination check stays silent and only the prefix decides.

#[rstest]
#[case::sint_low("SINT#-128", true)]
#[case::sint_below("SINT#-129", false)]
#[case::int_high("INT#32767", true)]
#[case::int_above("INT#32768", false)]
#[case::dint_above("DINT#2147483648", false)]
#[case::usint_high("USINT#255", true)]
#[case::usint_negative("USINT#-1", false)]
#[case::udint_above("UDINT#4294967296", false)]
#[case::radix_high("INT#16#7FFF", true)]
#[case::radix_above("INT#16#FFFF", false)]
fn apply_when_prefixed_literal_at_own_boundary_then_ok_or_err(
    #[case] literal: &str,
    #[case] expected_ok: bool,
) {
    let program = program_with(&format!("x : LINT := {literal};\n"), "");

    assert_eq!(out_of_range_count(&program) == 0, expected_ok);
}

#[test]
fn apply_when_prefixed_literal_in_assignment_then_err() {
    let codes = out_of_range_count(&program_with("x : DINT;\n", "x := INT#40000;\n"));

    assert_eq!(codes, 1);
}

#[test]
fn apply_when_prefixed_literal_in_comparison_then_err() {
    let codes = out_of_range_count(&program_with(
        "x : DINT;\ny : DINT;\n",
        "IF x = INT#40000 THEN y := 0; END_IF;\n",
    ));

    assert_eq!(codes, 1);
}

/// The literal's own type travels with it into a function argument. The
/// parameter is a `LINT`, which holds 40000, so only the prefix decides.
#[test]
fn apply_when_prefixed_literal_is_function_argument_then_err() {
    let codes = out_of_range_count(&format!(
        "{WIDE_FUNCTION}{}",
        program_with("x : LINT;\n", "x := WIDE(INT#40000);\n")
    ));

    assert_eq!(codes, 1);
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
    let codes = out_of_range_count(&program_with("x : SINT := INT#200;\n", ""));

    assert_eq!(codes, 1);
}

/// The two checks answer different questions -- is this an `INT`, and
/// does it fit a `SINT` -- so a literal that fails both is reported for
/// each.
#[test]
fn apply_when_prefixed_literal_fits_neither_then_err_for_each() {
    let codes = out_of_range_count(&program_with("x : SINT := INT#40000;\n", ""));

    assert_eq!(codes, 2);
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

    assert_eq!(out_of_range_count(&program), 0);
}

/// A radix does not change a value: `16#1FF` is 511, which no `USINT`
/// can hold.
#[test]
fn apply_when_radix_literal_out_of_range_then_err() {
    let codes = out_of_range_count(&program_with("x : USINT;\n", "x := 16#1FF;\n"));

    assert_eq!(codes, 1);
}

/// The same literal against a type that can hold it stays silent, so the
/// check is about the value rather than the spelling.
#[test]
fn apply_when_radix_literal_in_range_then_ok() {
    let codes = out_of_range_count(&program_with("x : UINT;\n", "x := 16#1FF;\n"));

    assert_eq!(codes, 0);
}

// --- An untyped real literal takes the type it is stored into ---

#[rstest]
#[case::real_high("REAL", "3.4028235E38", true)]
#[case::real_low("REAL", "-3.4028235E38", true)]
#[case::real_above("REAL", "3.5E38", false)]
#[case::real_below("REAL", "-3.5E38", false)]
#[case::lreal_holds_it("LREAL", "1.0E300", true)]
fn apply_when_real_initializer_at_boundary_then_ok_or_err(
    #[case] declared_type: &str,
    #[case] value: &str,
    #[case] expected_ok: bool,
) {
    let program = program_with(&format!("x : {declared_type} := {value};\n"), "");

    assert_eq!(real_out_of_range_count(&program) == 0, expected_ok);
}

#[test]
fn apply_when_real_assignment_out_of_range_then_err() {
    let codes = real_out_of_range_count(&program_with("x : REAL;\n", "x := 1.0E300;\n"));

    assert_eq!(codes, 1);
}

/// Each factor is a valid `REAL`, but their product is not.
#[test]
fn apply_when_folded_real_out_of_range_then_err() {
    let codes = real_out_of_range_count(&program_with("x : REAL;\n", "x := 1.0E30 * 1.0E30;\n"));

    assert_eq!(codes, 1);
}

/// The operator computes at `REAL`, so the operand is a `REAL` too.
#[test]
fn apply_when_real_operand_out_of_range_then_err() {
    let codes = real_out_of_range_count(&program_with(
        "x : REAL;\ny : REAL;\n",
        "x := y + 1.0E300;\n",
    ));

    assert_eq!(codes, 1);
}

/// A literal beyond every real type, or one that names its own type, is
/// checked once, by `rule_real_literal_range`, not again here.
#[rstest]
#[case::beyond_lreal("1.0E400")]
#[case::prefixed("REAL#1.0E40")]
fn apply_when_real_literal_reported_by_own_rule_then_reported_once(#[case] value: &str) {
    let program = program_with("x : REAL;\n", &format!("x := {value};\n"));

    assert_eq!(real_out_of_range_count(&program), 1);
}

#[rstest]
#[case::real("REAL", "3.4")]
#[case::time("TIME", "T#1s")]
fn apply_when_not_integer_storage_then_ok(#[case] declared_type: &str, #[case] value: &str) {
    let program = program_with(&format!("x : {declared_type} := {value};\n"), "");

    assert_eq!(out_of_range_count(&program), 0);
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

/// Analyzes `SMALL_TYPES` and a program with `declarations` and `body`,
/// returning (integer, real) out-of-range counts.
fn counts_with_types(declarations: &str, body: &str) -> (usize, usize) {
    let program = format!("{SMALL_TYPES}{}", program_with(declarations, body));
    (
        out_of_range_count(&program),
        real_out_of_range_count(&program),
    )
}

#[rstest]
#[case::array_integer("a : ARRAY[1..3] OF USINT := [1, 300, 255];\n", (1, 0))]
#[case::array_real("a : ARRAY[1..2] OF REAL := [1.0E300, 1.0];\n", (0, 1))]
#[case::array_in_range("a : ARRAY[1..2] OF USINT := [0, 255];\n", (0, 0))]
#[case::array_repeated("a : ARRAY[1..4] OF USINT := [2(300), 2(1)];\n", (1, 0))]
#[case::array_multi_dimension("a : ARRAY[1..2, 1..2] OF USINT := [1, 2, 3, 300];\n", (1, 0))]
#[case::struct_integer("s : Small := (i := 300);\n", (1, 0))]
#[case::struct_real("s : Small := (r := 1.0E300);\n", (0, 1))]
#[case::struct_in_range("s : Small := (i := 255, r := 1.0);\n", (0, 0))]
#[case::struct_array_field("s : Small := (a := [1, 300]);\n", (1, 0))]
#[case::struct_nested("o : Outer := (inner := (i := 300));\n", (1, 0))]
#[case::function_block_instance("h : HOLD := (i := 300, r := 1.0E300);\n", (1, 1))]
fn apply_when_initializer_out_of_range_then_err(
    #[case] declarations: &str,
    #[case] expected: (usize, usize),
) {
    assert_eq!(counts_with_types(declarations, ""), expected);
}

#[rstest]
#[case::named_integer("x := TAKE(i := 300, r := 1.0);\n", (1, 0))]
#[case::named_real("x := TAKE(i := 1, r := 1.0E300);\n", (0, 1))]
#[case::positional("x := TAKE(300, 1.0E300);\n", (1, 1))]
#[case::in_range("x := TAKE(255, 1.0);\n", (0, 0))]
#[case::folded("x := TAKE(255 + 1, 1.0);\n", (1, 0))]
#[case::fb_named("h(i := 300, r := 1.0E300);\n", (1, 1))]
#[case::fb_positional("h(300, 1.0E300);\n", (1, 1))]
#[case::fb_in_range("h(i := 255, r := 1.0);\n", (0, 0))]
#[case::fb_named_in_out("h(io := y);\n", (0, 0))]
fn apply_when_call_argument_out_of_range_then_err(
    #[case] body: &str,
    #[case] expected: (usize, usize),
) {
    assert_eq!(
        counts_with_types("x : REAL;\nh : HOLD;\ny : USINT;\n", body),
        expected
    );
}

/// A generic parameter states no range, so a standard function's `ANY_NUM`
/// argument is not checked; a concrete parameter of a conversion function is.
#[rstest]
#[case::generic("x : DINT;\n", "x := ADD(300, 1);\n", 0)]
#[case::conversion("x : INT;\n", "x := USINT_TO_INT(300);\n", 1)]
fn apply_when_standard_function_argument_then_checked_by_parameter_type(
    #[case] declarations: &str,
    #[case] body: &str,
    #[case] expected: usize,
) {
    assert_eq!(
        out_of_range_count(&program_with(declarations, body)),
        expected
    );
}

#[rstest]
#[case::struct_field_integer("S : STRUCT f : USINT := 300; END_STRUCT;", (1, 0))]
#[case::struct_field_real("S : STRUCT f : REAL := 1.0E300; END_STRUCT;", (0, 1))]
#[case::struct_field_array("S : STRUCT f : ARRAY[1..2] OF USINT := [1, 300]; END_STRUCT;", (1, 0))]
#[case::alias_integer("Small : USINT := 300;", (1, 0))]
#[case::alias_real("R : REAL := 1.0E300;", (0, 1))]
#[case::alias_in_range("Small : USINT := 255;", (0, 0))]
#[case::array_type("A : ARRAY[1..2] OF USINT := [2(300)];", (1, 0))]
fn apply_when_type_default_out_of_range_then_err(
    #[case] declarations: &str,
    #[case] expected: (usize, usize),
) {
    let program = format!(
        "TYPE\n{declarations}\nEND_TYPE\n{}",
        program_with("x : DINT;\n", "")
    );

    assert_eq!(
        (
            out_of_range_count(&program),
            real_out_of_range_count(&program)
        ),
        expected
    );
}
