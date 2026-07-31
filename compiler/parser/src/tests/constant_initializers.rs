//! Constant-expression VAR initializers.

use super::common::*;

#[test]
fn parse_when_negative_integer_initializer_then_parses_as_simple_not_simple_expr() {
    // Regression test: switching the initializer grammar from
    // constant() to expression() must not turn ordinary negative
    // literals into the vendor-extension SimpleExpr shape, since
    // expression() routes a leading '-' through its own unary-operator
    // handling rather than constant()'s built-in signed-literal
    // parsing. This must parse identically with or without the flag.
    let source = "PROGRAM main VAR x : INT := -123; END_VAR END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).expect(
        "negative literal initializers must not require allow_constant_initializer_expressions",
    );

    let program = cast!(&library.elements[0], LibraryElementKind::ProgramDeclaration);
    let simple = cast!(
        &program.variables[0].initializer,
        InitialValueAssignmentKind::Simple
    );
    let lit = cast!(
        simple.initial_value.as_ref().unwrap(),
        ConstantKind::IntegerLiteral
    );
    assert!(lit.value.is_neg);
    assert_eq!(lit.value.value.value, 123);
}

#[test]
fn parse_when_negative_real_initializer_then_parses_as_simple() {
    let source = "PROGRAM main VAR x : LREAL := -3.5; END_VAR END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &CompilerOptions::default()).expect(
        "negative real initializers must not require allow_constant_initializer_expressions",
    );

    let program = cast!(&library.elements[0], LibraryElementKind::ProgramDeclaration);
    let simple = cast!(
        &program.variables[0].initializer,
        InitialValueAssignmentKind::Simple
    );
    let lit = cast!(
        simple.initial_value.as_ref().unwrap(),
        ConstantKind::RealLiteral
    );
    assert_eq!(lit.value, -3.5);
}

#[test]
fn parse_when_var_enum_default_then_still_resolves_as_enumerated_not_simple_expr() {
    // Regression test: identifier := identifier is ambiguous between a
    // simple-typed constant-expression initializer (referencing a
    // variable) and an enum-typed variable with an enum-value default.
    // The enum interpretation must still win, matching pre-existing
    // disambiguation behavior.
    let source = "
TYPE
    MotorState : (STOPPED, RUNNING);
END_TYPE
FUNCTION_BLOCK fb1
VAR
    State : MotorState := STOPPED;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(
        source,
        &FileId::default(),
        &opts_with_constant_initializer_expressions(),
    )
    .unwrap();

    let fb = cast!(
        &library.elements[1],
        LibraryElementKind::FunctionBlockDeclaration
    );
    assert!(matches!(
        &fb.variables[0].initializer,
        InitialValueAssignmentKind::EnumeratedType(_)
    ));
}

#[test]
fn parse_when_arithmetic_initializer_and_flag_disabled_then_still_parses() {
    // Grammar accepts the broader form unconditionally (parse
    // permissively, validate later) — only the semantic fold pass
    // diagnoses a disabled flag, not the parser.
    let source = "PROGRAM main VAR d2r : LREAL := 4.25/180.0; END_VAR END_PROGRAM";
    let result = parse_program(source, &FileId::default(), &CompilerOptions::default());
    assert!(result.is_ok());
}

#[test]
fn parse_when_arithmetic_initializer_then_parses_as_simple_expr() {
    let source = "PROGRAM main VAR d2r : LREAL := 4.25/180.0; END_VAR END_PROGRAM";
    let library = parse_program(
        source,
        &FileId::default(),
        &opts_with_constant_initializer_expressions(),
    )
    .unwrap();

    let program = cast!(&library.elements[0], LibraryElementKind::ProgramDeclaration);
    assert!(matches!(
        &program.variables[0].initializer,
        InitialValueAssignmentKind::SimpleExpr(_)
    ));
}
