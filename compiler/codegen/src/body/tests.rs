//! Tests of the bytecode backend as a whole.

/// REQ-LOW-lowering-082: the bytecode backend denies wildcard arms over the
/// lowered program's enums. A test cannot observe a lint that is not
/// configured, so it checks the crate root.
#[test]
fn crate_root_when_read_then_denies_wildcard_enum_match_arm() {
    assert!(include_str!("../lib.rs").contains("#![deny(clippy::wildcard_enum_match_arm)]"));
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
/// `VAR_IN_OUT`, is a P9999 at the POU that declares it: there is no whole
/// lowered program to compile.
#[test]
fn compile_when_declaration_not_lowered_then_not_implemented_at_its_pou() {
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
        ironplc_parser::parse_program(source, &ironplc_ir::FileId::default(), &options).unwrap();
    let (library, context) = ironplc_analyzer::stages::analyze(&[&library], &options).unwrap();

    let error = crate::test_support::compile_analyzed(&library, &context)
        .err()
        .unwrap();

    let at = &error.primary.location;
    assert_eq!(error.code, "P9999");
    assert_eq!(&source[at.start..at.end], "F");
}

/// REQ-LOW-codegen-003: the public entry point takes a lowered program, and
/// the source bytes its debug information hashes.
#[test]
fn compile_when_named_then_takes_lowered_program() {
    let entry: fn(
        &ironplc_ir::lowered::Program,
        &dyn ironplc_ir::source::SourceLookup,
    ) -> Result<ironplc_container::Container, ironplc_ir::Diagnostic> = crate::compile;

    let _ = entry;
}
