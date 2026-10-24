//! Tests of the lowered bridge as a whole.

/// REQ-LOW-lowering-082: the lowered bridge denies wildcard arms over the
/// lowered program's enums. A test cannot observe a lint that is not
/// configured, so it checks the module root.
#[test]
fn module_root_when_read_then_denies_wildcard_enum_match_arm() {
    assert!(include_str!("mod.rs").contains("#![deny(clippy::wildcard_enum_match_arm)]"));
}

/// A declaration whose initializer reaches code generation without a
/// complete value is an internal error, not given a default: the starting
/// values `init` and each prologue store are read through
/// `initial_value::read`.
#[spec_test_macro::spec_test(REQ_IV_codegen_001)]
fn read_when_initializer_has_no_value_then_internal_error() {
    let int = ironplc_dsl::type_id::TypeId::from_raw(1);
    let mut decl = ironplc_dsl::common::VarDecl::simple("x", "INT");
    decl.type_id = Some(int);
    let types = std::collections::HashMap::from([(
        int,
        ironplc_analyzer::SemanticType::Int {
            size: ironplc_analyzer::semantic_type::ByteSized::B16,
        },
    )]);

    let result =
        ironplc_lowering::initial_value::read(&decl, &types, &std::collections::HashMap::new());

    assert_eq!(result.unwrap_err().code, "P9998");
}
