//! SFC action association qualifiers.

use super::common::*;

fn step_with_association(association: &str) -> String {
    format!(
        "
FUNCTION_BLOCK sfc
  VAR
    t_var : TIME;
  END_VAR

  INITIAL_STEP Start:
    {association}
  END_STEP

  ACTION a1:
  END_ACTION

END_FUNCTION_BLOCK
"
    )
}

/// A timed qualifier echoed as its `Debug` text --
/// `a1 ( SD(Duration(DurationLiteral { ... })) );` -- which does not parse.
#[test]
fn write_to_string_when_timed_qualifier_with_duration_then_round_trips() {
    let rendered = assert_round_trips(
        &step_with_association("a1(SD, T#1s);"),
        &CompilerOptions::default(),
    );
    assert!(rendered.contains("a1 ( SD, TIME#1000ms );"), "{rendered}");
}

#[test]
fn write_to_string_when_pulse_qualifier_with_variable_then_round_trips() {
    let rendered = assert_round_trips(
        &step_with_association("a1(P1, t_var);"),
        &CompilerOptions::default(),
    );
    assert!(rendered.contains("a1 ( P1, t_var );"), "{rendered}");
}
