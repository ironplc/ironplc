//! OOP extension: `THIS^` / `SUPER^` round-trip.

use super::common::*;
use rstest::rstest;

fn inheritance_options() -> CompilerOptions {
    CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    }
}

/// Each case parses source under `allow_fb_inheritance`, renders it back
/// to text, and re-parses the rendering to confirm it produces the same
/// AST as the original. The re-parse is what proves the caret is rendered
/// tight against its keyword and against whatever follows it -- a stray
/// space either side fails to parse.
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
// Subscript after `THIS^.field`: the chain `THIS^ . values [ 2 ]` used to
// render with a space before `[`, which does not re-parse (issue #1407).
#[case::this_field_subscript(
    "
FUNCTION_BLOCK FB_Motor
VAR
    values : ARRAY[0..3] OF INT;
END_VAR
METHOD Run
    THIS^.values[2] := 1;
END_METHOD
END_FUNCTION_BLOCK
"
)]
fn write_to_string_when_self_ref_source_then_round_trips(#[case] source: &'static str) {
    assert_round_trips(source, &inheritance_options());
}

/// Whitespace written between the keyword and its caret is not preserved:
/// the caret belongs to the node, so every spelling renders canonically.
#[test]
fn write_to_string_when_space_before_caret_then_renders_tight() {
    let rendered = assert_round_trips(
        "
FUNCTION_BLOCK FB_Motor
VAR
    count : INT;
END_VAR
METHOD Run
    THIS ^.count := 1;
END_METHOD
END_FUNCTION_BLOCK
",
        &inheritance_options(),
    );
    assert!(
        rendered.contains("THIS^.count"),
        "Expected THIS^.count in output, got: {rendered}"
    );
}
