//! TIME function declarations and SIZEOF rendering.

use super::common::*;

#[test]
fn write_to_string_when_time_function_decl_then_round_trips() {
    let options = CompilerOptions {
        allow_time_as_function_name: true,
        ..CompilerOptions::default()
    };
    assert_resource_renders_to(
        "time_function_decl.st",
        "time_function_decl_rendered.st",
        &options,
    );
}

#[test]
fn write_to_string_sizeof() {
    let options = CompilerOptions {
        allow_sizeof: true,
        ..CompilerOptions::default()
    };
    assert_resource_renders_to("sizeof.st", "sizeof_rendered.st", &options);
}

/// A 64-bit temporal literal keeps its own prefix on round trip. Writing the
/// 32-bit prefix for every literal narrowed the type — `LTIME#30d` came back
/// as `TIME#2592000000ms` — and since that count does not fit a `TIME`, the
/// rendering no longer compiled.
#[rstest::rstest]
#[case::ltime("t : LTIME := LTIME#30d;", "LTIME#")]
#[case::ldate("d : LDATE := LDATE#2200-01-01;", "LDATE#")]
#[case::ldt("d : LDT := LDT#2200-01-01-00:00:00;", "LDATE_AND_TIME#")]
#[case::ltod("t : LTIME_OF_DAY := LTOD#12:30:00;", "LTIME_OF_DAY#")]
#[case::time("t : TIME := T#1d;", "TIME#")]
#[case::date("d : DATE := D#2024-01-01;", "DATE#")]
#[case::dt("d : DT := DT#2024-01-01-12:30:00;", "DATE_AND_TIME#")]
#[case::tod("t : TIME_OF_DAY := TOD#12:30:00;", "TIME_OF_DAY#")]
fn write_to_string_when_temporal_literal_then_keeps_its_own_prefix(
    #[case] declaration: &str,
    #[case] expected_prefix: &str,
) {
    let source = format!("PROGRAM main\nVAR\n  {declaration}\nEND_VAR\nEND_PROGRAM");
    let rendered = assert_round_trips(&source, &edition3());
    assert!(
        rendered.contains(expected_prefix),
        "expected {expected_prefix} in the rendering:\n{rendered}"
    );
}
