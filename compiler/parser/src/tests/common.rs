pub(crate) use dsl::common::{
    next_block_id, ArrayElementType, ConstantKind, DataTypeDeclarationKind, DeclarationQualifier,
    EnumeratedSpecificationInit, EnumerationDeclaration, FunctionBlockBodyKind,
    FunctionBlockDeclaration, FunctionDeclaration, FunctionReturnType, InitialValueAssignmentKind,
    Library, LibraryElementKind, ProgramDeclaration, RealLiteral, ReferenceTarget,
    SimpleInitializer, SpecificationKind, TypeName, TypeReference, VarDecl, VariableIdentifier,
    VariableType,
};
pub(crate) use dsl::configuration::{
    ConfigurationDeclaration, DataSourceKind, ProgramConfiguration, ResourceDeclaration,
    TaskConfiguration,
};
pub(crate) use dsl::core::{FileId, Id, SourceSpan};
pub(crate) use dsl::diagnostic::Diagnostic;
pub(crate) use dsl::sfc::{ActionAssociation, ActionQualifier, ElementKind, Network, Step};
pub(crate) use dsl::textual::*;
pub(crate) use dsl::time::*;
pub(crate) use ironplc_test::cast;
pub(crate) use ironplc_test::read_shared_resource;
pub(crate) use rstest::rstest;
pub(crate) use time::Duration;

pub(crate) use crate::options::{CompilerOptions, Dialect};
pub(crate) use crate::parse_program;

pub(crate) fn parse_resource(name: &'static str) -> Result<Library, Diagnostic> {
    let source = read_shared_resource(name);
    parse_program(&source, &FileId::default(), &CompilerOptions::default())
}

pub(crate) fn parse_text(source: &'static str) -> Library {
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_ok());
    result.unwrap()
}

#[cfg(test)]
pub(crate) fn new_library(element: LibraryElementKind) -> Library {
    Library {
        elements: vec![element],
    }
}

/// Returns options with `allow_missing_semicolon` enabled. Used as a
/// function-pointer `#[case]` value for the parametrized dialect-flag tests.
pub(crate) fn with_missing_semicolon_flag() -> CompilerOptions {
    CompilerOptions {
        allow_missing_semicolon: true,
        ..Default::default()
    }
}

/// Returns options with `allow_empty_var_blocks` enabled.
pub(crate) fn with_empty_var_blocks_flag() -> CompilerOptions {
    CompilerOptions {
        allow_empty_var_blocks: true,
        ..Default::default()
    }
}

pub(crate) fn parse_text_edition3(source: &str) -> Library {
    let options = CompilerOptions {
        allow_iec_61131_3_2013: true,
        ..CompilerOptions::default()
    };
    let result = parse_program(source, &FileId::default(), &options);
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
    result.unwrap()
}

pub(crate) fn parse_text_reference_to(source: &str) -> Library {
    let options = CompilerOptions {
        allow_reference_to: true,
        ..CompilerOptions::default()
    };
    let result = parse_program(source, &FileId::default(), &options);
    assert!(result.is_ok(), "Parse failed: {:?}", result.err());
    result.unwrap()
}

/// The single (non-FB-call) statement in a program body.
pub(crate) fn only_statement(lib: &Library) -> &StmtKind {
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let stmts = cast!(&prog.body, FunctionBlockBodyKind::Statements);
    assert_eq!(stmts.body.len(), 1, "expected exactly one statement");
    &stmts.body[0]
}

// ---------------------------------------------------------------------
// REQ-PAB: IEC 61131-3:2013 partial-access bit syntax (.%Xn).
// See specs/design/partial-access-bit-syntax.md.
// ---------------------------------------------------------------------

pub(crate) fn opts_with_partial_access() -> CompilerOptions {
    CompilerOptions {
        allow_partial_access_syntax: true,
        ..CompilerOptions::default()
    }
}

pub(crate) fn wrap_program(body: &str) -> String {
    format!(
            "PROGRAM main\nVAR\n  b : BYTE;\n  r : BOOL;\n  arr : ARRAY[0..1] OF BYTE;\n  s : MY_STRUCT;\nEND_VAR\n{}\nEND_PROGRAM",
            body
        )
}

// Duration literal conformance tests — see specs/design/time-literals.md.

pub(crate) fn duration_program(literal: &str) -> String {
    format!(
        "FUNCTION fun:TIME\nVAR\n    tv : TIME := {literal};\nEND_VAR\nfun := tv;\nEND_FUNCTION"
    )
}

pub(crate) fn extract_duration(library: &Library) -> &DurationLiteral {
    let func = cast!(
        &library.elements[0],
        LibraryElementKind::FunctionDeclaration
    );
    let initializer = &func.variables[0].initializer;
    let simple = cast!(initializer, InitialValueAssignmentKind::Simple);
    let constant = simple.initial_value.as_ref().expect("initializer");
    cast!(constant, ConstantKind::Duration)
}

// ---------------------------------------------------------------------
// TwinCAT/Siemens `{ ... }` pragma skipping.
// See specs/plans/2026-07-18-twincat-pragma-skipping.md.
// ---------------------------------------------------------------------

pub(crate) fn enum_with_pragma_header() -> String {
    "
        {attribute 'qualified_only'}
        {attribute 'strict'}
        TYPE E_Color :
            (Red, Green, Blue);
        END_TYPE"
        .to_owned()
}

// -----------------------------------------------------------------
// CASE branch with no statements.
// See specs/plans/2026-07-20-twincat-empty-case-branch.md.
// -----------------------------------------------------------------

pub(crate) fn extract_case(library: &Library) -> Case {
    let element = library
        .elements
        .iter()
        .find(|e| matches!(e, LibraryElementKind::FunctionBlockDeclaration(_)))
        .expect("expected a FunctionBlockDeclaration");
    let fb = cast!(element, LibraryElementKind::FunctionBlockDeclaration);
    let stmts = cast!(&fb.body, FunctionBlockBodyKind::Statements);
    cast!(&stmts.body[0], StmtKind::Case).clone()
}

// -----------------------------------------------------------------
// AND_THEN short-circuit boolean operator.
// See specs/plans/2026-07-20-twincat-and-then-operator.md.
// -----------------------------------------------------------------

pub(crate) fn opts_with_short_circuit_operators() -> CompilerOptions {
    CompilerOptions {
        allow_short_circuit_operators: true,
        ..CompilerOptions::default()
    }
}

pub(crate) fn extract_assignment_value(library: &Library) -> Expr {
    let element = library
        .elements
        .iter()
        .find(|e| matches!(e, LibraryElementKind::FunctionBlockDeclaration(_)))
        .expect("expected a FunctionBlockDeclaration");
    let fb = cast!(element, LibraryElementKind::FunctionBlockDeclaration);
    let stmts = cast!(&fb.body, FunctionBlockBodyKind::Statements);
    let assignment = cast!(&stmts.body[0], StmtKind::Assignment);
    assignment.value.clone()
}

// ---------------------------------------------------------------------
// Constant-expression VAR initializers.
// See specs/plans/2026-07-19-twincat-var-initializer-expressions.md.
// ---------------------------------------------------------------------

pub(crate) fn opts_with_constant_initializer_expressions() -> CompilerOptions {
    CompilerOptions {
        allow_constant_initializer_expressions: true,
        ..CompilerOptions::default()
    }
}
