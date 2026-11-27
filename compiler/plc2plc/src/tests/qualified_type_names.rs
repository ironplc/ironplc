//! Round-tripping of type names written with a library prefix.
//!
//! A prefix takes no part in AST equality (a library's names are flat), so
//! the round trip alone cannot show that it survives rendering. These tests
//! check the rendered text as well.

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
        rendered.contains("EXTENDS TcUnit.FB_TestSuite"),
        "{rendered}"
    );
    assert!(rendered.contains("BROTLib.I_Comm"), "{rendered}");
}

#[test]
fn write_to_string_when_variable_types_have_library_prefix_then_prefix_kept() {
    let source = "
FUNCTION_BLOCK FB_Host
VAR
    mode : BROTLib.E_Blink;
    cover : HalfBROT.FB_CoverControl(comm := fbComm);
    fbComm : FB_Comm;
END_VAR
END_FUNCTION_BLOCK
";
    let rendered = assert_round_trips(source, &twincat());

    assert!(rendered.contains("BROTLib.E_Blink"), "{rendered}");
    assert!(rendered.contains("HalfBROT.FB_CoverControl"), "{rendered}");
}

#[test]
fn write_to_string_when_dotted_value_has_library_prefix_then_prefix_and_dots_kept() {
    let source = "
PROGRAM main
VAR
    mode : BROTLib.E_Blink := BROTLib.E_Blink.slow;
END_VAR
CASE mode OF
    BROTLib.E_Blink.off: ;
END_CASE;
END_PROGRAM
";
    let rendered = assert_round_trips(source, &twincat());

    assert!(rendered.contains("BROTLib.E_Blink.slow"), "{rendered}");
    assert!(rendered.contains("BROTLib.E_Blink.off"), "{rendered}");
    assert!(!rendered.contains('#'), "TwinCAT rejects `#`:\n{rendered}");
}
