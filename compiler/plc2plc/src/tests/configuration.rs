//! CONFIGURATION declarations.

use super::common::*;

#[test]
fn write_to_string_when_two_resources_then_round_trips() {
    let source = "
CONFIGURATION config
    RESOURCE r1 ON PLC
        TASK t1(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM a WITH t1 : counter;
    END_RESOURCE
    RESOURCE r2 ON PLC
        TASK t2(INTERVAL := T#20ms, PRIORITY := 2);
        PROGRAM b WITH t2 : counter;
    END_RESOURCE
END_CONFIGURATION
";
    assert_round_trips(source, &CompilerOptions::default());
}
