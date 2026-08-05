//! Partial-access (`%X`) round-tripping.

use super::common::*;

/// REQ-PAB-060: plc2plc normalizes `.%Xn` to `.n`. Re-parsing the rendered
/// output must yield the same AST (structural equivalence).
#[test]
fn plc2plc_spec_req_pab_060_percent_x_round_trips_through_short_form() {
    let rendered = parse_and_render_resource_with_partial_access("partial_access_bit.st");
    let expected = read_resource("partial_access_bit_rendered.st");
    assert_eq!(rendered, expected);

    // Confirm re-parsing the rendered output yields the same AST as parsing
    // the original source, proving the normalization is semantics-preserving.
    let original = read_shared_resource("partial_access_bit.st");
    let options = CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    };
    let library_original = parse_program(&original, &FileId::default(), &options).unwrap();
    let library_rendered =
        parse_program(&rendered, &FileId::default(), &CompilerOptions::default()).unwrap();
    assert_eq!(library_original, library_rendered);
}

#[test]
fn plc2plc_when_partial_access_multi_then_round_trips() {
    let rendered = parse_and_render_resource_with_partial_access("partial_access_multi.st");
    let expected = read_resource("partial_access_multi_rendered.st");
    assert_eq!(rendered, expected);
}
