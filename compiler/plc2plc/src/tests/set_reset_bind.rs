//! `S=` / `R=` set/reset assignment operators (TwinCAT/CODESYS).
//!
//! Not gated by any dialect flag (mirrors `REF=`, which also has none) --
//! see `parser/src/tests/set_reset_bind.rs` for the discussion.

use super::common::*;

#[test]
fn write_to_string_when_set_bind_then_round_trips() {
    let source = "
PROGRAM main
VAR
    bOut : BOOL;
    bCondition : BOOL;
END_VAR
    bOut S= bCondition;
END_PROGRAM
";
    assert_round_trips(source, &CompilerOptions::default());
}

#[test]
fn write_to_string_when_reset_bind_then_round_trips() {
    let source = "
PROGRAM main
VAR
    bOut : BOOL;
    bCondition : BOOL;
END_VAR
    bOut R= bCondition;
END_PROGRAM
";
    assert_round_trips(source, &CompilerOptions::default());
}
