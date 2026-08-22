//! OOP extension: `THIS^` / `SUPER^` round-trip. See
//! specs/plans/2026-08-22-oop-this-super-parsing.md.

use super::common::*;
use rstest::rstest;

/// Each case parses source under `allow_fb_inheritance`, renders it back
/// to text, and re-parses the rendering to confirm it produces the same
/// AST as the original. The re-parse is what proves the caret is rendered
/// tight against its keyword and against whatever follows it -- a stray
/// space either side fails to parse.
///
/// No array-subscript case: `THIS^.values[2]` renders as
/// `THIS^.values [ 2 ]`, which does not re-parse. That is a pre-existing
/// renderer bug affecting every subscript, `THIS^` or not (issue #1404);
/// the AST shape for that chain is asserted in the parser tests instead.
#[rstest]
#[case::this_field_write(
    "
FUNCTION_BLOCK FB_Motor
VAR
    count : INT;
END_VAR
METHOD Run
    THIS^.count := 1;
END_METHOD
END_FUNCTION_BLOCK
"
)]
#[case::super_field_read(
    "
FUNCTION_BLOCK FB_Motor
VAR
    count : INT;
END_VAR
METHOD Run
    count := SUPER^.count;
END_METHOD
END_FUNCTION_BLOCK
"
)]
#[case::this_method_call(
    "
FUNCTION_BLOCK FB_Motor
VAR
    count : INT;
END_VAR
METHOD Start
    count := 1;
END_METHOD
METHOD Run
    THIS^.Start();
END_METHOD
END_FUNCTION_BLOCK
"
)]
#[case::super_method_call_with_args(
    "
FUNCTION_BLOCK FB_Motor
VAR
    count : INT;
END_VAR
METHOD Run
    SUPER^.SetSpeed(rNewSpeed := 1.5);
END_METHOD
END_FUNCTION_BLOCK
"
)]
fn write_to_string_when_self_ref_source_then_round_trips(#[case] source: &'static str) {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let library_original = parse_program(source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library_original).unwrap();
    let library_rendered = parse_program(&rendered, &FileId::default(), &options).unwrap();
    assert_eq!(library_original, library_rendered);
}

/// Whitespace written between the keyword and its caret is not preserved:
/// the caret belongs to the node, so every spelling renders canonically.
#[test]
fn write_to_string_when_space_before_caret_then_renders_tight() {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    let source = "
FUNCTION_BLOCK FB_Motor
VAR
    count : INT;
END_VAR
METHOD Run
    THIS ^.count := 1;
END_METHOD
END_FUNCTION_BLOCK
";
    let library = parse_program(source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library).unwrap();
    assert!(
        rendered.contains("THIS^.count"),
        "Expected THIS^.count in output, got: {rendered}"
    );
}
