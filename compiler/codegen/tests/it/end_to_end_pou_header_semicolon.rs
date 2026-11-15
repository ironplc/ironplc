//! End-to-end test for a `;` after a POU header
//! (`--allow-pou-header-semicolon`): the VAR blocks after the `;` are the
//! POU's declarations, so a function's inputs and a program's variables
//! behave as if the `;` were absent.

use ironplc_parser::options::CompilerOptions;

e2e_i32_with!(
    end_to_end_when_pou_header_semicolon_then_var_blocks_declare_variables,
    CompilerOptions {
        allow_pou_header_semicolon: true,
        ..CompilerOptions::default()
    },
    "FUNCTION DOUBLE : DINT;
VAR_INPUT
    a : DINT;
END_VAR
DOUBLE := a * 2;
END_FUNCTION

PROGRAM main;
VAR
    x : DINT;
END_VAR
x := DOUBLE(a := 21);
END_PROGRAM",
    &[("x", 42)],
);
