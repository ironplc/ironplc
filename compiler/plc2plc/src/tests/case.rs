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
