//! `EXIT` and `RETURN` statements.

use super::common::*;

#[test]
fn write_to_string_when_exit_and_return_then_round_trips() {
    let source = "
FUNCTION f : INT
VAR
    i : INT;
END_VAR
FOR i := 1 TO 3 DO
    IF i = 2 THEN
        EXIT;
    END_IF;
END_FOR;
f := i;
RETURN;
END_FUNCTION
";
    assert_round_trips(source, &CompilerOptions::default());
}
