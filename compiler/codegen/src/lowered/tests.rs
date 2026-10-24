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

/// A declaration lowering does not express yet, a function's string
/// `VAR_IN_OUT`, is a P9999 at the POU that declares it: the bytecode
/// backend lays out only the lowered declarations, so there is no layout to
/// compile any body against.
#[test]
fn layout_when_declaration_not_lowered_then_not_implemented_at_its_pou() {
    let source = "
FUNCTION F : INT
  VAR_IN_OUT s : STRING; END_VAR
  F := 1;
END_FUNCTION
PROGRAM main
  VAR a : INT; b : STRING; END_VAR
  a := F(s := b);
END_PROGRAM
";
    let options = ironplc_parser::options::CompilerOptions::default();
    let library =
        ironplc_parser::parse_program(source, &ironplc_dsl::core::FileId::default(), &options)
            .unwrap();
    let (library, context) = ironplc_analyzer::stages::analyze(&[&library], &options).unwrap();
    let completed = ironplc_analyzer::xform_resolve_initial_values::apply(library, &context);
    let analysis = ironplc_analyzer::CleanAnalysis::new(&completed, &context).unwrap();
    let lowering =
        ironplc_lowering::Lowering::new(&analysis, super::parse_policy(&Default::default()))
            .lower_program()
            .unwrap();

    let error = crate::layout::lowered::Layout::of(
        &lowering,
        &ironplc_dsl::core::Id::from("main"),
        &std::collections::HashMap::new(),
    )
    .err()
    .unwrap();

    let at = &error.primary.location;
    assert_eq!(error.code, "P9999");
    assert_eq!(&source[at.start..at.end], "F");
}
