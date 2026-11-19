//! Renderer tests for a `;` after a POU header
//! (`--allow-pou-header-semicolon`).
//!
//! The AST does not record the `;`, so the renderer omits it: the rendering
//! is the standard spelling.

use super::common::*;

#[test]
fn write_to_string_when_pou_header_semicolons_then_round_trips_without_them() {
    let rendered = assert_round_trips(
        "
FUNCTION F : INT;
VAR_INPUT
    a : INT;
END_VAR
    F := a;
END_FUNCTION

FUNCTION_BLOCK FB;
VAR_INPUT
    a : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM main;
VAR
    x : INT;
END_VAR
    x := F(a := 1);
END_PROGRAM
",
        &CompilerOptions {
            allow_pou_header_semicolon: true,
            ..CompilerOptions::default()
        },
    );
    // The rendering needs no flag.
    assert!(
        parse_program(&rendered, &FileId::default(), &CompilerOptions::default()).is_ok(),
        "{rendered}"
    );
}
