//! CASE statement round-tripping.

use super::common::*;

#[test]
fn write_to_string_when_case_branch_empty_then_round_trips() {
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    x : INT;
    y : INT;
END_VAR
CASE x OF
    1: y := 1;
    5: (* no statement here *)
    10: y := 3;
END_CASE;
END_FUNCTION_BLOCK
";
    // The source's dropped `;` needs `--allow-missing-semicolon` to parse at
    // all, so the round trip runs under that flag.
    let options = CompilerOptions {
        allow_missing_semicolon: true,
        ..CompilerOptions::default()
    };
    let rendered = assert_round_trips(source, &options);

    // The renderer always writes an explicit `(* empty *) ;` for an empty
    // branch, so the rendering is strict-grammar-valid and needs no flag.
    parse_program(&rendered, &FileId::default(), &CompilerOptions::default())
        .expect("rendered empty CASE branch must parse without allow_missing_semicolon");
}

#[test]
fn write_to_string_when_case_label_is_hex_and_binary_literal_then_round_trips() {
    // Bit-string literals already render decimalized everywhere in this
    // codebase (confirmed: even an ordinary `x : DWORD := 16#D012;` VAR
    // initializer renders as `53266`, not the original hex spelling) --
    // pre-existing behavior, not something CASE-label support introduces
    // or is expected to fix. A real, likewise pre-existing consequence:
    // re-parsing the decimalized `53266:` label resolves to
    // SignedInteger, not BitStringLiteral again -- same numeric value,
    // different variant. So this asserts render *idempotency* (parse ->
    // render -> reparse -> render again, same text) rather than AST
    // equality, matching the pattern already established for REFERENCE
    // TO/POINTER TO's analogous "normalizes to a different spelling" case.
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    x : DINT;
    y : INT;
END_VAR
CASE x OF
    16#D012: y := 1;
    2#1010: y := 2;
END_CASE;
END_FUNCTION_BLOCK
";
    let rendered = assert_round_trips_idempotently(source, &CompilerOptions::default());

    assert!(rendered.contains("\n      53266 :\n"));
    assert!(rendered.contains("\n      10 :\n"));
}

#[test]
fn write_to_string_when_case_label_list_then_round_trips() {
    let source = "
PROGRAM main
VAR
    n : INT;
END_VAR
CASE n OF
    2, 3: n := 2;
END_CASE;
END_PROGRAM
";
    assert_round_trips(source, &CompilerOptions::default());
}

#[test]
fn write_to_string_when_case_label_list_has_range_and_else_then_round_trips() {
    let source = "
PROGRAM main
VAR
    n : INT;
    hit : INT;
END_VAR
CASE n OF
    1, 3..5: hit := 1;
    7..9, 11: hit := 2;
ELSE
    hit := 3;
END_CASE;
END_PROGRAM
";
    assert_round_trips(source, &CompilerOptions::default());
}

#[test]
fn write_to_string_when_case_label_list_in_dialect_with_empty_arms_then_round_trips() {
    // These dialects accept an arm with no statements, so a label list
    // rendered as one arm per label still parses -- as a different program.
    // Only the AST comparison catches it.
    let source = "
PROGRAM main
VAR
    n : INT;
    hit : INT;
END_VAR
CASE n OF
    2, 3: hit := 1;
END_CASE;
END_PROGRAM
";
    for dialect in [Dialect::Rusty, Dialect::Codesys, Dialect::TwinCat] {
        assert_round_trips(source, &CompilerOptions::from_dialect(dialect));
    }
}
