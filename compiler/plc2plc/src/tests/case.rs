//! CASE statement round-tripping.

use super::common::*;

#[test]
fn write_to_string_when_case_branch_empty_then_round_trips() {
    // The source's dropped `;` needs `--allow-missing-semicolon` to
    // parse at all, but the renderer always writes an explicit `(*
    // empty *) ;` for an empty branch, so the re-parse of the
    // rendered output is already strict-grammar-valid and needs no
    // flag.
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
    let options = CompilerOptions {
        allow_missing_semicolon: true,
        ..CompilerOptions::default()
    };
    let library_original = parse_program(source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    let library_rendered =
        parse_program(&rendered, &FileId::default(), &CompilerOptions::default())
            .expect("rendered output must parse");
    assert_eq!(library_original, library_rendered);
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
    let library_original =
        parse_program(source, &FileId::default(), &CompilerOptions::default()).unwrap();
    let rendered = write_to_string(&library_original).unwrap();

    assert!(rendered.contains("53266"));
    assert!(rendered.contains("10"));

    let library_rendered =
        parse_program(&rendered, &FileId::default(), &CompilerOptions::default())
            .expect("rendered output must parse");
    let rendered_again = write_to_string(&library_rendered).unwrap();
    assert_eq!(rendered, rendered_again);
}
