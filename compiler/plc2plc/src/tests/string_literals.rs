//! Character string literals in statement bodies.
//!
//! A declaration spells its own width (`STRING` / `WSTRING`), so the renderer
//! can pick the delimiter from the declaration. A literal in a statement body
//! has no such keyword — the delimiter *is* the width — so the literal has to
//! carry it. See issue #1550.

use super::common::*;
use rstest::rstest;

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

// A literal's `value` holds the decoded characters, so rendering has to
// escape them again, exactly once. An earlier renderer re-escaped undecoded
// source text, which compounded on each pass (`$L`, `$$L`, `$$$$L`).
// `assert_round_trips` compares the decoded ASTs, so it catches either
// mistake.

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
fn write_to_string_when_literal_contains_raw_control_char_then_escaped() {
    // The lexer admits a raw tab inside a literal. It is the same character
    // as `$T`, and renders as the escape.
    let source = assignment_program("STRING[20]", "'a\tb'");
    let rendered = assert_round_trips(&source, &CompilerOptions::default());
    assert!(rendered.contains("v := 'a$Tb'"), "rendered:\n{rendered:?}");
}

#[test]
fn write_to_string_when_narrow_literal_contains_escaped_quote_then_round_trips() {
    // `$'` is the single quote inside a single-quoted literal (#1818).
    let source = assignment_program("STRING[20]", "'it$'s'");
    let rendered = assert_round_trips(&source, &CompilerOptions::default());
    assert!(rendered.contains("v := 'it$'s'"), "rendered:\n{rendered}");
}

#[test]
fn write_to_string_when_wide_literal_contains_escaped_quote_then_round_trips() {
    // `$"` is the double quote inside a double-quoted literal.
    let source = assignment_program("WSTRING[20]", "\"say $\"hi$\"\"");
    let rendered = assert_round_trips(&source, &CompilerOptions::default());
    assert!(
        rendered.contains("v := \"say $\"hi$\"\""),
        "rendered:\n{rendered}"
    );
}

#[rstest]
#[case::hex_narrow("STRING[20]", "'$41$42'", "v := 'AB'")]
#[case::hex_wide("WSTRING[20]", "\"$00E9t$00E9\"", "v := \"été\"")]
#[case::lower_case_named("STRING[20]", "'a$nb'", "v := 'a$Lb'")]
#[case::other_delimiter_escaped("STRING[20]", "'say $\"hi$\"'", "v := 'say \"hi\"'")]
#[case::control_narrow("STRING[20]", "'$01'", "v := '$01'")]
#[case::control_wide("WSTRING[20]", "\"$0001\"", "v := \"$0001\"")]
fn write_to_string_when_literal_has_escape_then_renders_canonical_spelling(
    #[case] declaration: &str,
    #[case] literal: &str,
    #[case] expected: &str,
) {
    // The rendering is the canonical spelling of the same characters, so it
    // re-parses to the same AST even where the text differs.
    let source = assignment_program(declaration, literal);
    let rendered = assert_round_trips(&source, &CompilerOptions::default());
    assert!(rendered.contains(expected), "rendered:\n{rendered}");
}
