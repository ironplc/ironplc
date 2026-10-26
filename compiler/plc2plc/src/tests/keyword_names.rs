//! Renderer tests for keywords used as names (`--allow-keywords-as-names`).
//!
//! The parser accepts `ON`, `STEP`, `R_EDGE` and `F_EDGE` as names whatever
//! the flag says (a semantic rule enforces it), so the round trip parses under
//! the default options.

use super::common::*;

#[test]
fn write_to_string_when_keyword_names_enum_member_and_variables_then_round_trips() {
    let rendered = assert_round_trips(
        "
TYPE
M : (Off, On);
END_TYPE

FUNCTION_BLOCK fb
VAR_INPUT
    Step : DINT;
    R_EDGE : BOOL;
    F_EDGE : BOOL;
END_VAR
VAR
    c : M;
END_VAR
    c := On;
END_FUNCTION_BLOCK
",
        &CompilerOptions::default(),
    );
    // `Id` compares case-insensitively, so the round trip alone cannot see
    // the spelling.
    assert!(rendered.contains("( Off , On )"), "{rendered}");
}
