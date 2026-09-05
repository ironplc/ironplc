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

// The parser does not decode `$` escapes, so a literal's `value` holds the
// source characters as written. Re-escaping the `$` there changed the value on
// every round trip, and compounded on each pass (`$L`, `$$L`, `$$$$L`).
// `assert_round_trips` compares ASTs and would have caught it -- no test above
// used a literal containing a `$`.

#[test]
fn write_to_string_when_literal_contains_escape_then_escape_is_not_re_escaped() {
    // `$L` is one line feed. Rendering it as `$$L` would make it two
    // characters: a literal dollar and an `L`.
    let source = assignment_program("STRING[20]", "'a$Lb'");
    let rendered = assert_round_trips(&source, &CompilerOptions::default());
    assert!(rendered.contains("v := 'a$Lb'"), "rendered:\n{rendered}");
}

#[test]
fn write_to_string_when_literal_contains_escaped_dollar_then_stays_one_dollar() {
    // `$$` is one dollar sign. It must not become `$$$$`.
    let source = assignment_program("STRING[20]", "'costs $$5'");
    let rendered = assert_round_trips(&source, &CompilerOptions::default());
    assert!(
        rendered.contains("v := 'costs $$5'"),
        "rendered:\n{rendered}"
    );
}

#[test]
fn write_to_string_when_wide_literal_contains_escape_then_escape_is_preserved() {
    let source = assignment_program("WSTRING[20]", "\"tab$There\"");
    let rendered = assert_round_trips(&source, &CompilerOptions::default());
    assert!(
        rendered.contains("v := \"tab$There\""),
        "rendered:\n{rendered}"
    );
}

#[test]
fn write_to_string_when_literal_rendered_twice_then_escapes_are_stable() {
    // The defect compounded: each pass added another `$`. Rendering the
    // re-parsed library must reproduce the same text.
    let source = assignment_program("STRING[20]", "'a$Lb$$c'");
    let rendered = assert_round_trips_idempotently(&source, &CompilerOptions::default());
    assert!(rendered.contains("v := 'a$Lb$$c'"), "rendered:\n{rendered}");
}

#[test]
fn write_to_string_when_literal_contains_raw_control_char_then_passed_through() {
    // The mirror of the same defect: the lexer admits a raw tab inside a
    // literal, and there is no `$T` in the source for it to correspond to, so
    // emitting one turned that single tab into the two characters `$` and `T`.
    // Passing it through merely looks unusual and re-parses as itself.
    let source = assignment_program("STRING[20]", "'a\tb'");
    let rendered = assert_round_trips(&source, &CompilerOptions::default());
    assert!(rendered.contains("v := 'a\tb'"), "rendered:\n{rendered:?}");
}
