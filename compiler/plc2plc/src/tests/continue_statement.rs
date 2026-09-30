//! `CONTINUE` statement (IEC 61131-3 Edition 3).

use super::common::*;

#[test]
fn write_to_string_when_continue_in_each_loop_then_round_trips() {
    let source = "
PROGRAM main
VAR
    i : INT;
    b : BOOL;
END_VAR
FOR i := 1 TO 3 DO
    IF i = 2 THEN
        CONTINUE;
    END_IF;
END_FOR;
WHILE b DO
    CONTINUE;
END_WHILE;
REPEAT
    CONTINUE;
UNTIL b
END_REPEAT;
END_PROGRAM
";
    let options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
    assert_round_trips(source, &options);
}
