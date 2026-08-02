//! `REFERENCE TO` / `REF=` binding, dereference and NULL parsing.

use super::common::*;

// --- REF= disambiguation against other uses of `=` -----------------------

/// A variable *named* `REF` assigned with `:=` is a normal assignment, not a
/// `REF=` binding — even with `--allow-reference-to` active.
#[test]
fn ref_bind_when_variable_named_ref_assigned_then_normal_assignment() {
    let lib = parse_text_reference_to(
        "PROGRAM main
VAR
    REF : INT;
END_VAR
    REF := 5;
END_PROGRAM",
    );
    let assignment = cast!(only_statement(&lib), StmtKind::Assignment);
    assert!(
        !assignment.ref_bind,
        "must not be treated as a REF= binding"
    );
    assert_eq!(assignment.target.to_string(), "REF");
}

/// An `=` equality comparison in a condition still parses (the REF= rule
/// only fires on `REF` immediately followed by `=`).
#[test]
fn ref_bind_when_equality_in_condition_then_parses_as_comparison() {
    let lib = parse_text_reference_to(
        "PROGRAM main
VAR
    a : INT;
    b : INT;
    c : INT;
END_VAR
    IF a = b THEN
        c := 1;
    END_IF;
END_PROGRAM",
    );
    // The single top-level statement is the IF, not an assignment.
    cast!(only_statement(&lib), StmtKind::If);
}

/// An `=` comparison on the right-hand side of `:=` still parses as a normal
/// assignment whose value is the comparison.
#[test]
fn ref_bind_when_equality_in_rhs_then_normal_assignment() {
    let lib = parse_text_reference_to(
        "PROGRAM main
VAR
    a : INT;
    c : INT;
    result : BOOL;
END_VAR
    result := a = c;
END_PROGRAM",
    );
    let assignment = cast!(only_statement(&lib), StmtKind::Assignment);
    assert!(!assignment.ref_bind);
}

/// `REF=` binds when `REF` is the keyword token (both `--allow-ref-to` and
/// `--allow-reference-to` active, e.g. the `codesys` dialect).
#[test]
fn ref_bind_when_ref_is_keyword_token_then_binds() {
    let options = CompilerOptions {
        allow_ref_to: true,
        allow_reference_to: true,
        ..CompilerOptions::default()
    };
    let source = "PROGRAM main
VAR
    x : INT;
    r : REFERENCE TO INT;
END_VAR
    r REF= x;
END_PROGRAM";
    let lib = parse_program(source, &FileId::default(), &options).unwrap();
    let assignment = cast!(only_statement(&lib), StmtKind::Assignment);
    assert!(assignment.ref_bind);
    let referent = cast!(&assignment.value.kind, ExprKind::Ref);
    assert_eq!(referent.to_string(), "x");
}

/// The `REF=` operator is case-insensitive (`ref=` binds too).
#[test]
fn ref_bind_when_lowercase_operator_then_binds() {
    let lib = parse_text_reference_to(
        "PROGRAM main
VAR
    x : INT;
    r : REFERENCE TO INT;
END_VAR
    r ref= x;
END_PROGRAM",
    );
    let assignment = cast!(only_statement(&lib), StmtKind::Assignment);
    assert!(assignment.ref_bind);
}

/// A space between `REF` and `=` is not the binding operator: `REF` and `=`
/// must be adjacent, so `x REF = y` is rejected.
#[test]
fn ref_bind_when_space_between_ref_and_equals_then_error() {
    let options = CompilerOptions {
        allow_reference_to: true,
        ..CompilerOptions::default()
    };
    let source = "PROGRAM main
VAR
    x : REFERENCE TO INT;
    y : INT;
END_VAR
    x REF = y;
END_PROGRAM";
    let result = parse_program(source, &FileId::default(), &options);
    assert!(result.is_err(), "REF = with a space must be rejected");
}

/// The right-hand side of `REF=` must be a variable (address-of), matching
/// `REF()` semantics — a literal is rejected rather than silently accepted.
#[test]
fn ref_bind_when_rhs_is_literal_then_error() {
    let options = CompilerOptions {
        allow_reference_to: true,
        ..CompilerOptions::default()
    };
    let source = "PROGRAM main
VAR
    r : REFERENCE TO INT;
END_VAR
    r REF= 5;
END_PROGRAM";
    let result = parse_program(source, &FileId::default(), &options);
    assert!(result.is_err(), "REF= to a literal must be rejected");
}

/// A referent *named* `REF` can be bound: `x REF= REF` binds `x` to the
/// variable `REF`.
#[test]
fn ref_bind_when_referent_named_ref_then_binds() {
    let lib = parse_text_reference_to(
        "PROGRAM main
VAR
    REF : INT;
    x : REFERENCE TO INT;
END_VAR
    x REF= REF;
END_PROGRAM",
    );
    let assignment = cast!(only_statement(&lib), StmtKind::Assignment);
    assert!(assignment.ref_bind);
    let referent = cast!(&assignment.value.kind, ExprKind::Ref);
    assert_eq!(referent.to_string(), "REF");
}

#[test]
fn parse_when_ref_to_int_type_decl_then_ok() {
    let lib = parse_text_edition3("TYPE IntRef : REF_TO INT; END_TYPE");
    assert_eq!(lib.elements.len(), 1);
    let dt = cast!(&lib.elements[0], LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(dt, DataTypeDeclarationKind::Reference);
    assert_eq!(decl.type_name.to_string(), "IntRef");
    assert_eq!(decl.target.type_name().unwrap().to_string(), "INT");
}

#[test]
fn parse_when_ref_to_var_decl_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    x : REF_TO INT;
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    assert_eq!(prog.variables.len(), 1);
    assert!(matches!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Reference(_)
    ));
}

#[test]
fn parse_when_ref_to_var_decl_with_null_init_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    x : REF_TO INT := NULL;
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let ref_init = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Reference
    );
    assert!(matches!(
        ref_init.initial_value,
        Some(dsl::common::ReferenceInitialValue::Null(_))
    ));
}

#[test]
fn parse_when_ref_to_var_decl_with_ref_init_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    counter : INT;
    x : REF_TO INT := REF(counter);
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let ref_init = cast!(
        &prog.variables[1].initializer,
        InitialValueAssignmentKind::Reference
    );
    assert!(matches!(
        ref_init.initial_value,
        Some(dsl::common::ReferenceInitialValue::Ref(_))
    ));
}

#[test]
fn parse_when_ref_to_array_var_decl_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    x : REF_TO ARRAY[1..10] OF INT;
END_VAR
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    assert_eq!(prog.variables.len(), 1);
    let ref_init = cast!(
        &prog.variables[0].initializer,
        InitialValueAssignmentKind::Reference
    );
    assert!(matches!(ref_init.target, ReferenceTarget::Array(_)));
}

#[test]
fn parse_when_ref_to_array_type_decl_then_ok() {
    let lib = parse_text_edition3("TYPE ArrRef : REF_TO ARRAY[0..3] OF BYTE; END_TYPE");
    assert_eq!(lib.elements.len(), 1);
    let dt = cast!(&lib.elements[0], LibraryElementKind::DataTypeDeclaration);
    let decl = cast!(dt, DataTypeDeclarationKind::Reference);
    assert_eq!(decl.type_name.to_string(), "ArrRef");
    assert!(matches!(decl.target, ReferenceTarget::Array(_)));
}

#[test]
fn parse_when_ref_operator_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    counter : INT;
    x : REF_TO INT;
END_VAR
    x := REF(counter);
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let s = cast!(&prog.body, FunctionBlockBodyKind::Statements);
    assert!(!s.body.is_empty());
}

#[test]
fn parse_when_deref_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    myRef : REF_TO INT;
    value : INT;
END_VAR
    value := myRef^;
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let s = cast!(&prog.body, FunctionBlockBodyKind::Statements);
    assert_eq!(s.body.len(), 1);
}

#[test]
fn parse_when_deref_assign_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    myRef : REF_TO INT;
END_VAR
    myRef^ := 42;
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let s = cast!(&prog.body, FunctionBlockBodyKind::Statements);
    assert_eq!(s.body.len(), 1);
    let assignment = cast!(&s.body[0], StmtKind::Assignment);
    assert!(assignment.deref);
}

#[test]
fn parse_when_null_literal_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    myRef : REF_TO INT;
END_VAR
    myRef := NULL;
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let s = cast!(&prog.body, FunctionBlockBodyKind::Statements);
    assert_eq!(s.body.len(), 1);
}

#[test]
fn parse_when_null_comparison_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    myRef : REF_TO INT;
    x : INT;
END_VAR
    IF myRef <> NULL THEN
        x := 1;
    END_IF;
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let s = cast!(&prog.body, FunctionBlockBodyKind::Statements);
    assert_eq!(s.body.len(), 1);
}

#[test]
fn parse_when_xor_still_works_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    a : BOOL;
    b : BOOL;
    result : BOOL;
END_VAR
    result := a XOR b;
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let s = cast!(&prog.body, FunctionBlockBodyKind::Statements);
    assert_eq!(s.body.len(), 1);
}

#[test]
fn parse_when_deref_then_xor_then_ok() {
    let lib = parse_text_edition3(
        "PROGRAM main
VAR
    myRef : REF_TO BOOL;
    b : BOOL;
    result : BOOL;
END_VAR
    result := myRef^ XOR b;
END_PROGRAM",
    );
    let prog = cast!(&lib.elements[0], LibraryElementKind::ProgramDeclaration);
    let s = cast!(&prog.body, FunctionBlockBodyKind::Statements);
    assert_eq!(s.body.len(), 1);
}

#[test]
fn parse_when_deref_array_subscript_then_ok() {
    let lib = parse_text_edition3(
        "FUNCTION my_func : INT
VAR_INPUT
    PT : REF_TO ARRAY[0..10] OF BYTE;
END_VAR
    my_func := 0;
    PT^[0] := BYTE#0;
END_FUNCTION

PROGRAM test_main
VAR
    result : INT;
END_VAR
    result := 0;
END_PROGRAM",
    );
    let func = cast!(&lib.elements[0], LibraryElementKind::FunctionDeclaration);
    let stmts = &func.body;
    assert_eq!(stmts.len(), 2);
    let assignment = cast!(&stmts[1], StmtKind::Assignment);
    // Target should be Array(Deref(Named(PT)), [0])
    let sym = cast!(&assignment.target, Variable::Symbolic);
    let arr = cast!(sym, SymbolicVariableKind::Array);
    let d = cast!(
        arr.subscripted_variable.as_ref(),
        SymbolicVariableKind::Deref
    );
    let n = cast!(d.variable.as_ref(), SymbolicVariableKind::Named);
    assert_eq!(n.name.to_string(), "PT");
}

#[test]
fn parse_when_deref_array_subscript_in_expression_then_ok() {
    let lib = parse_text_edition3(
        "FUNCTION my_func : BYTE
VAR_INPUT
    PT : REF_TO ARRAY[0..10] OF BYTE;
END_VAR
    my_func := PT^[0];
END_FUNCTION

PROGRAM test_main
VAR
    result : BYTE;
END_VAR
    result := BYTE#0;
END_PROGRAM",
    );
    let func = cast!(&lib.elements[0], LibraryElementKind::FunctionDeclaration);
    let stmts = &func.body;
    assert_eq!(stmts.len(), 1);
    let assignment = cast!(&stmts[0], StmtKind::Assignment);
    // Value should be Variable(Array(Deref(Named(PT)), [0]))
    let var = cast!(&assignment.value.kind, ExprKind::Variable);
    let sym = cast!(var, Variable::Symbolic);
    let arr = cast!(sym, SymbolicVariableKind::Array);
    let d = cast!(
        arr.subscripted_variable.as_ref(),
        SymbolicVariableKind::Deref
    );
    let n = cast!(d.variable.as_ref(), SymbolicVariableKind::Named);
    assert_eq!(n.name.to_string(), "PT");
}
