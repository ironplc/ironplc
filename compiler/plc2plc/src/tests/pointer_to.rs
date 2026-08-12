//! `POINTER TO` declaration and dereference round-tripping.

use super::common::*;

/// `POINTER TO` binding uses `REF()`/`NULL` (from `allow_ref_to`) until the
/// `ADR()` operator lands, so the fixture enables both flags — the same
/// combination the `codesys` dialect provides.
#[test]
fn write_to_string_when_pointer_to_then_round_trips() {
    let source = read_shared_resource("pointer_to.st");
    let options = CompilerOptions {
        allow_pointer_to: true,
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(&source, &FileId::default(), &options).unwrap();
    let rendered = write_to_string(&library).unwrap();
    let expected = read_resource("pointer_to_rendered.st");
    assert_eq!(rendered, expected);
}
