//! `ADR()` operator round-tripping.
//!
//! `ADR` parses as an ordinary function call (no keyword token), so the
//! fixture needs only `allow_pointer_to` for the destination pointer
//! declaration — the call round-trips like any other function call.

use super::common::*;

#[test]
fn write_to_string_when_adr_then_round_trips() {
    let options = CompilerOptions {
        allow_pointer_to: true,
        ..CompilerOptions::default()
    };
    assert_resource_renders_to("adr.st", "adr_rendered.st", &options);
}
