//! Subrange bounds written in hex, binary or octal round-trip.
//!
//! A radix bound is held as the same literal a decimal bound is, so it
//! renders in decimal, as every radix literal does, and re-parses to the
//! same AST. The decimal rendering is standard syntax, so it also parses
//! without the dialect flags the source needed.

use super::common::*;

#[test]
fn write_to_string_when_subrange_bounds_are_radix_then_round_trips_as_decimal() {
    let source = "
TYPE
    R : INT (16#00..16#FF);
END_TYPE

FUNCTION_BLOCK FB_Example
VAR
    a : ARRAY[2#0..2#11, 8#1..8#10] OF INT;
    x : DINT;
    y : INT;
END_VAR
CASE x OF
    16#01..16#0F: y := 1;
END_CASE;
END_FUNCTION_BLOCK
";
    let options = CompilerOptions::from_dialect(Dialect::TwinCat);
    let rendered = assert_round_trips(source, &options);

    parse_program(&rendered, &FileId::default(), &CompilerOptions::default())
        .expect("decimal subrange bounds must parse in the strict dialect");
}
