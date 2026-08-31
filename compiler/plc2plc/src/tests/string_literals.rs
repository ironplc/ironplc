//! Character string literals in statement bodies.
//!
//! A declaration spells its own width (`STRING` / `WSTRING`), so the renderer
//! can pick the delimiter from the declaration. A literal in a statement body
//! has no such keyword — the delimiter *is* the width — so the literal has to
//! carry it. See issue #1550.

use super::common::*;

fn assignment_program(declaration: &str, literal: &str) -> String {
    format!("PROGRAM main\nVAR\n    v : {declaration};\nEND_VAR\nv := {literal};\nEND_PROGRAM\n")
}

#[test]
fn write_to_string_when_narrow_literal_in_body_then_single_quoted() {
    let source = assignment_program("STRING[10]", "'abc'");
    let rendered = assert_round_trips(&source, &CompilerOptions::default());
    assert!(rendered.contains("v := 'abc'"), "rendered:\n{rendered}");
}

#[test]
fn write_to_string_when_wide_literal_in_body_then_double_quoted() {
    let source = assignment_program("WSTRING[10]", "\"abc\"");
    let rendered = assert_round_trips(&source, &CompilerOptions::default());
    assert!(rendered.contains("v := \"abc\""), "rendered:\n{rendered}");
}

#[test]
fn write_to_string_when_wide_literal_contains_single_quote_then_not_escaped() {
    // Only the delimiter in force needs a `$` escape. Escaping a single quote
    // inside a WSTRING would change the value, because nothing unescapes it
    // on the way back in.
    let source = assignment_program("WSTRING[10]", "\"it's\"");
    let rendered = assert_round_trips(&source, &CompilerOptions::default());
    assert!(rendered.contains("v := \"it's\""), "rendered:\n{rendered}");
}

#[test]
fn write_to_string_when_narrow_literal_contains_double_quote_then_not_escaped() {
    let source = assignment_program("STRING[10]", "'say \"hi\"'");
    let rendered = assert_round_trips(&source, &CompilerOptions::default());
    assert!(
        rendered.contains("v := 'say \"hi\"'"),
        "rendered:\n{rendered}"
    );
}

#[test]
fn write_to_string_when_literal_in_function_call_argument_then_keeps_width() {
    let source = "PROGRAM main
VAR
    a : WSTRING[10];
    c : WSTRING[20];
END_VAR
c := CONCAT(a, \"tail\");
END_PROGRAM
";
    let rendered = assert_round_trips(source, &CompilerOptions::default());
    assert!(rendered.contains("\"tail\""), "rendered:\n{rendered}");
}
