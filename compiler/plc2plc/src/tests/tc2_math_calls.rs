//! Round-tripping of user source that calls `Tc2_Math` library functions.
//!
//! The compatibility-library mechanism must leave user source untouched
//! (`REQ-CL-plc2plc-001`): calls to `LTRUNC`, `LMOD`, `MODABS`, and `FRAC`
//! render back under their exact names, and no library declaration is ever
//! emitted as user source. See specs/design/library-interfaces/tc2-math.md.

use super::common::*;

#[test]
fn write_to_string_when_source_calls_tc2_math_functions_then_round_trips() {
    let source = "
PROGRAM main
VAR
    position : LREAL;
    range : LREAL;
    angle : LREAL;
END_VAR
angle := MODABS(position, range);
angle := LMOD(position, range);
angle := LTRUNC(position);
angle := FRAC(position);
END_PROGRAM
";
    let rendered = assert_round_trips(source, &CompilerOptions::default());

    // `Id` equality is case-insensitive, so the round trip alone does not
    // pin the spelling: these check the vendor casing survives verbatim.
    for name in ["MODABS", "LMOD", "LTRUNC", "FRAC"] {
        assert!(
            rendered.contains(name),
            "rendered source must keep the {name} call:\n{rendered}"
        );
    }
    // Nothing of the library implementation leaks into rendered user source.
    assert!(!rendered.contains("__TRUNC"));
    assert!(!rendered.contains("__MOD"));
}
