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

/// The destination check stops at a function call, whose arguments are
/// the parameters' business. The literal's own type travels with it.
#[test]
fn apply_when_prefixed_literal_is_function_argument_then_err() {
    let codes = out_of_range_count(&program_with(
        "x : DINT;\n",
        "x := INT_TO_DINT(INT#40000);\n",
    ));

    assert_eq!(codes, 1);
}

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
    let codes =
        real_out_of_range_count(&program_with("x : REAL;\n", "x := 1.0E30 * 1.0E30;\n"));

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
