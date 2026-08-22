//! `THIS^` / `SUPER^` reach codegen only if analysis lets them through;
//! codegen itself has no execution semantics for them and says so rather
//! than emitting anything. See
//! specs/plans/2026-08-22-oop-this-super-parsing.md.
//!
//! The statements under test live in the function block *body*, not in a
//! `METHOD` body: method bodies are not compiled at all yet (method
//! codegen is its own unstarted slice), so a `THIS^` inside one never
//! reaches these code paths.

use crate::common::try_parse_and_compile;
use ironplc_parser::options::CompilerOptions;
use rstest::rstest;

#[rstest]
#[case::this_field_write("    THIS^.count := 1;")]
#[case::super_field_read("    count := SUPER^.count;")]

fn compile_when_self_ref_then_not_implemented(#[case] body: &str) {
    let source = format!(
        "
FUNCTION_BLOCK FB_Motor
VAR
    count : INT;
END_VAR
{body}
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
    m();
END_PROGRAM
"
    );
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let result = try_parse_and_compile(&source, &options);

    assert!(
        result.is_err(),
        "expected compilation to fail for THIS^/SUPER^"
    );
    assert_eq!(result.unwrap_err().code, "P9999");
}
