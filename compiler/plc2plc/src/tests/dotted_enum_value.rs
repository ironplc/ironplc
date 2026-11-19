//! Round-tripping of enumerated values written `Type.value`.
//!
//! The spelling takes no part in AST equality (`Type.value` and `Type#value`
//! are the same value), so the round trip alone cannot show that it
//! survives rendering. These tests check the rendered text as well.

use super::common::*;

fn twincat() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::TwinCat)
}

#[test]
fn write_to_string_when_enumerated_value_is_dotted_then_dot_kept() {
    let source = "
TYPE E_Blink : (off, slow, fast); END_TYPE

PROGRAM main
VAR
    mode : E_Blink := E_Blink.slow;
    out : INT;
END_VAR
CASE mode OF
    E_Blink.fast: out := 1;
END_CASE;
END_PROGRAM
";
    let rendered = assert_round_trips(source, &twincat());

    assert!(rendered.contains("E_Blink.slow"), "{rendered}");
    assert!(rendered.contains("E_Blink.fast"), "{rendered}");
    assert!(!rendered.contains('#'), "TwinCAT rejects `#`:\n{rendered}");
}

#[test]
fn write_to_string_when_enumerated_value_uses_hash_then_hash_kept() {
    let source = "
TYPE E_Blink : (off, slow, fast); END_TYPE

PROGRAM main
VAR
    mode : E_Blink := E_Blink#slow;
END_VAR
END_PROGRAM
";
    let rendered = assert_round_trips(source, &CompilerOptions::default());

    assert!(rendered.contains("E_Blink#slow"), "{rendered}");
}
