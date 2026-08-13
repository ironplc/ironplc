//! `ADR()` operator round-tripping.
//!
//! `ADR` parses as an ordinary function call (no keyword token), so the
//! fixture needs only `allow_pointer_to` for the destination pointer
//! declaration — the call round-trips like any other function call.

use super::common::*;

#[test]
fn write_to_string_when_adr_then_round_trips() {
    let source = read_shared_resource("adr.st");
    let options = CompilerOptions {
        allow_pointer_to: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(&source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library).unwrap();
    let expected = read_resource("adr_rendered.st");
    assert_eq!(rendered, expected);
}
