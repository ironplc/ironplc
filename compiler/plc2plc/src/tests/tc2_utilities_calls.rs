//! Round-tripping of user source that calls the `Tc2_Utilities` library
//! function.
//!
//! The compatibility-library mechanism must leave user source untouched
//! (`REQ-CL-plc2plc-001`): calls to `LREAL_TO_FMTSTR` render back under the
//! exact vendor name — including formal (named) argument passing with the
//! vendor's documented parameter names — and no library declaration is ever
//! emitted as user source. See
//! specs/design/library-interfaces/tc2-utilities.md.

use super::common::*;

#[test]
fn write_to_string_when_source_calls_lreal_to_fmtstr_then_round_trips() {
    let source = "
PROGRAM main
VAR
    measured : LREAL;
    display : STRING;
END_VAR
display := LREAL_TO_FMTSTR(measured, 2, TRUE);
display := LREAL_TO_FMTSTR(in := measured, iPrecision := 3, bRound := FALSE);
END_PROGRAM
";
    let rendered = assert_round_trips(source, &CompilerOptions::default());

    // The calls come through unchanged, under the exact vendor name and the
    // vendor's formal parameter names.
    for name in ["LREAL_TO_FMTSTR", "iPrecision", "bRound"] {
        assert!(
            rendered.contains(name),
            "rendered source must keep {name}:\n{rendered}"
        );
    }
    // Nothing of the library implementation leaks into rendered user source.
    assert!(!rendered.contains("__TRUNC"));
    assert!(!rendered.contains("0123456789"));
}
