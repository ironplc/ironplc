//! Round-tripping of names written with a library or namespace prefix.
//!
//! A qualified name's prefix takes no part in AST equality (compatibility
//! library symbols are flat), so the round trip alone cannot show that it
//! survives rendering. These tests check the rendered text as well.

use super::common::*;

fn twincat() -> CompilerOptions {
    CompilerOptions::from_dialect(Dialect::TwinCat)
}

#[test]
fn write_to_string_when_extends_and_implements_have_library_prefix_then_prefix_kept() {
    let source = "
FUNCTION_BLOCK FB_Suite EXTENDS TcUnit.FB_TestSuite IMPLEMENTS BROTLib.I_Comm, I_Other
END_FUNCTION_BLOCK
";
    let rendered = assert_round_trips(source, &twincat());

    assert!(
        rendered.contains("TcUnit.FB_TestSuite"),
        "the EXTENDS prefix must be kept:\n{rendered}"
    );
    assert!(
        rendered.contains("BROTLib.I_Comm"),
        "the IMPLEMENTS prefix must be kept:\n{rendered}"
    );
}

#[test]
fn write_to_string_when_variable_type_has_library_prefix_then_prefix_kept() {
    let source = "
FUNCTION_BLOCK FB_Host
VAR
    eMode : BROTLib.E_Blink;
    sensor : MONETcommon.FB_Sensor;
END_VAR
END_FUNCTION_BLOCK
";
    let rendered = assert_round_trips(source, &twincat());

    assert!(rendered.contains("BROTLib.E_Blink"), "{rendered}");
    assert!(rendered.contains("MONETcommon.FB_Sensor"), "{rendered}");
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
