//! OOP extension: METHOD declarations and instance.Method(args) calls,
//! round-trip. See ADR-0041
//! (specs/adrs/0041-staged-method-and-interface-dispatch.md).

use super::common::*;
use rstest::rstest;

/// Each case parses source under `allow_fb_inheritance`, renders it back
/// to text, and re-parses the rendering to confirm it produces the same
/// AST as the original. Collapses the individually near-identical
/// parse/render/re-parse/assert round-trip tests into one table, same
/// shorthand as `corpus.rs`/`reference_to.rs`; each row still runs as an
/// individually-named test.
#[rstest]
#[case::method_no_return_no_params(
    "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
METHOD Start
    bRunning := TRUE;
END_METHOD
END_FUNCTION_BLOCK
"
)]
#[case::multiple_methods(
    "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
METHOD Start
    bRunning := TRUE;
END_METHOD
METHOD Stop
    bRunning := FALSE;
END_METHOD
END_FUNCTION_BLOCK
"
)]
#[case::method_params_and_return(
    "
FUNCTION_BLOCK FB_Motor
VAR
    rSpeed : REAL;
END_VAR
METHOD SetSpeed : BOOL
VAR_INPUT
    rNewSpeed : REAL;
END_VAR
    rSpeed := rNewSpeed;
    SetSpeed := TRUE;
END_METHOD
END_FUNCTION_BLOCK
"
)]
#[case::method_call_statement(
    "
FUNCTION_BLOCK FB_Motor
VAR
    rSpeed : REAL;
END_VAR
METHOD SetSpeed : BOOL
VAR_INPUT
    rNewSpeed : REAL;
END_VAR
    rSpeed := rNewSpeed;
    SetSpeed := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
m.SetSpeed(1.5);
END_PROGRAM
"
)]
fn write_to_string_when_method_source_then_round_trips(#[case] source: &'static str) {
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    assert_round_trips(source, &options);
}
