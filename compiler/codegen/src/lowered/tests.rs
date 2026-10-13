//! Tests of the lowered bridge as a whole.

/// REQ-LOW-lowering-082: the lowered bridge denies wildcard arms over the
/// lowered program's enums. A test cannot observe a lint that is not
/// configured, so it checks the module root.
#[test]
fn module_root_when_read_then_denies_wildcard_enum_match_arm() {
    assert!(include_str!("mod.rs").contains("#![deny(clippy::wildcard_enum_match_arm)]"));
}
