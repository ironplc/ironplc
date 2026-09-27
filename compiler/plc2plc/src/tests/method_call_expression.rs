//! OOP extension: method calls in expression position, round-trip. The
//! renderer writes the same call as in statement position, without the
//! trailing `;`.

use super::common::*;
use rstest::rstest;

#[rstest]
#[case::assignment("v := m.GetSpeed();")]
#[case::arguments("v := m.Scaled(2.0, offset := 1.0);")]
#[case::binary_operand("v := 1.0 + m.Scaled(2.0);")]
#[case::if_condition("IF m.IsFast() THEN v := 1.0; END_IF;")]
#[case::nested_argument("v := m.Scaled(m.Scaled(2.0));")]
#[case::function_argument("v := ABS(m.GetSpeed());")]
fn write_to_string_when_method_call_in_expression_then_round_trips(#[case] body: &str) {
    let source = format!(
        "
PROGRAM main
VAR
    m : FB_Motor;
    v : REAL;
END_VAR
{body}
END_PROGRAM
"
    );
    let options = CompilerOptions {
        allow_fb_inheritance: true,
        ..CompilerOptions::default()
    };
    assert_round_trips(&source, &options);
}
