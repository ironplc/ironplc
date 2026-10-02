//! The fractional seconds of time-of-day and date-and-time literals.
//!
//! The seconds field of a daytime is a fixed-point number, and its fraction is
//! part of the value in every spelling of the prefix (issue #1921).

use super::common::*;

/// Parses `declaration` as the only variable of a program and returns its
/// initial value.
fn initial_value(declaration: &str) -> ConstantKind {
    let program = format!("PROGRAM main\nVAR\n{declaration}\nEND_VAR\nEND_PROGRAM");
    let lib = parse_text_edition3(&program);
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let init = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Simple
    );
    init.initial_value.clone().unwrap()
}

#[rstest]
#[case::time_of_day("t : TIME_OF_DAY := TIME_OF_DAY#10:00:00.250;")]
#[case::tod("t : TIME_OF_DAY := TOD#10:00:00.250;")]
#[case::ltime_of_day("t : LTIME_OF_DAY := LTIME_OF_DAY#10:00:00.250;")]
#[case::ltod("t : LTIME_OF_DAY := LTOD#10:00:00.250;")]
fn parse_program_when_time_of_day_has_fraction_then_literal_keeps_milliseconds(
    #[case] declaration: &str,
) {
    let constant = initial_value(declaration);
    let literal = cast!(constant, ConstantKind::TimeOfDay);
    assert_eq!(literal.whole_milliseconds(), 36_000_250);
}

#[rstest]
#[case::date_and_time("d : DATE_AND_TIME := DATE_AND_TIME#2024-01-02-10:00:00.25;")]
#[case::dt("d : DT := DT#2024-01-02-10:00:00.25;")]
#[case::ldate_and_time("d : LDATE_AND_TIME := LDATE_AND_TIME#2024-01-02-10:00:00.25;")]
#[case::ldt("d : LDT := LDT#2024-01-02-10:00:00.25;")]
fn parse_program_when_date_and_time_has_fraction_then_literal_keeps_fraction(
    #[case] declaration: &str,
) {
    let constant = initial_value(declaration);
    let literal = cast!(constant, ConstantKind::DateAndTime);
    assert_eq!(literal.hmsm(), (10, 0, 0, 250_000));
}

#[test]
fn parse_program_when_fraction_has_nanoseconds_then_literal_keeps_them() {
    let constant = initial_value("t : TIME_OF_DAY := TOD#10:00:00.123456789;");
    let literal = cast!(constant, ConstantKind::TimeOfDay);
    assert_eq!(literal.daytime_text(), "10:00:00.123456789");
}

#[test]
fn parse_program_when_fraction_finer_than_nanoseconds_then_truncated() {
    let constant = initial_value("t : TIME_OF_DAY := TOD#10:00:00.1234567899;");
    let literal = cast!(constant, ConstantKind::TimeOfDay);
    assert_eq!(literal.daytime_text(), "10:00:00.123456789");
}

#[rstest]
#[case::sixty("TOD#10:00:60")]
#[case::wraps_a_byte("TOD#10:00:300")]
fn parse_program_when_second_out_of_range_then_error(#[case] literal: &str) {
    let program = format!("PROGRAM main\nVAR\nt : TIME_OF_DAY := {literal};\nEND_VAR\nEND_PROGRAM");
    let result = parse_program(&program, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_err(), "expected parse error for {literal}");
}
