//! `POINTER TO` declaration and dereference round-tripping.

use super::common::*;

/// `POINTER TO` binding uses `REF()`/`NULL` (from `allow_ref_to`) until the
/// `ADR()` operator lands, so the fixture enables both flags — the same
/// combination the `codesys` dialect provides.
#[test]
fn write_to_string_when_pointer_to_then_round_trips() {
    let options = CompilerOptions {
        allow_pointer_to: true,
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    assert_resource_renders_to("pointer_to.st", "pointer_to_rendered.st", &options);
}
