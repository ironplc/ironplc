//! Round-tripping of a `VAR` initializer that names a constant.

use super::common::*;

/// The name renders as written, so a declaration that names a global
/// constant re-parses to the same constant expression. The parser does not
/// consult the dialect for this, so the default options suffice.
#[test]
fn write_to_string_when_initializer_names_constant_then_round_trips() {
    let source = "
VAR_GLOBAL CONSTANT
    C : UDINT := 1;
END_VAR
FUNCTION_BLOCK FB_A
VAR
    x : UDINT := C;
    y : UDINT := C + 1;
END_VAR
END_FUNCTION_BLOCK
";
    assert_round_trips(source, &CompilerOptions::default());
}
