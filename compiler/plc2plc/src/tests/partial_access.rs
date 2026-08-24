//! Partial-access (`%X`) round-tripping.

use super::common::*;

fn partial_access_options() -> CompilerOptions {
    CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    }
}

/// REQ-PAB-060: plc2plc normalizes `.%Xn` to `.n`. The round trip proves the
/// normalization is semantics-preserving: re-parsing the rendered output
/// yields the same AST as parsing the original source.
#[test]
fn plc2plc_spec_req_pab_060_percent_x_round_trips_through_short_form() {
    let rendered = assert_resource_renders_to(
        "partial_access_bit.st",
        "partial_access_bit_rendered.st",
        &partial_access_options(),
    );

    // The short form needs no flag, so the rendering is valid input under the
    // strict grammar too -- not just under the flag its source required.
    parse_program(&rendered, &FileId::default(), &CompilerOptions::default())
        .expect("normalized `.n` form must parse without allow_partial_access_syntax");
}

#[test]
fn plc2plc_when_partial_access_multi_then_round_trips() {
    // Byte/word/dword access keeps its `%B0` spelling (only `%Xn` has a short
    // form), so the rendering still needs the flag to re-parse.
    assert_resource_renders_to(
        "partial_access_multi.st",
        "partial_access_multi_rendered.st",
        &partial_access_options(),
    );
}
