//! Short-circuit `AND_THEN` round-tripping.

use super::common::*;

#[test]
fn write_to_string_when_and_then_then_round_trips_as_and_then_not_and() {
    let source = "
FUNCTION_BLOCK FB_Example
VAR
    a : BOOL;
    b : BOOL;
    result : BOOL;
END_VAR
result := a AND_THEN b;
END_FUNCTION_BLOCK
";
    let options = CompilerOptions {
        allow_short_circuit_operators: true,
        ..CompilerOptions::default()
    };
    // `AND_THEN` and `AND` are distinct operators, so the round trip is what
    // proves the short-circuit spelling is not normalized away.
    assert_round_trips(source, &options);
}
