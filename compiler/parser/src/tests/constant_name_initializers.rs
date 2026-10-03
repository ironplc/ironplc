//! A variable initializer that is a bare name (`x : UDINT := C;`).
//!
//! After an elementary type the name can only denote a constant, so it is a
//! constant-expression initializer. After a user type name it may as well be
//! an enumeration value, so it is recorded for the type resolver (ADR-0050).
use super::common::*;
use dsl::common::SimpleExprInitializer;

/// The late-bound name an initializer expression is exactly, if it is one.
fn bare_name(init: &InitialValueAssignmentKind) -> Option<(&TypeName, &Id)> {
    match init {
        InitialValueAssignmentKind::SimpleExpr(SimpleExprInitializer {
            type_name,
            initial_value:
                Expr {
                    kind: ExprKind::LateBound(late_bound),
                    ..
                },
        }) => Some((type_name, &late_bound.value)),
        _ => None,
    }
}

#[rstest]
#[case::default(CompilerOptions::default())]
#[case::twincat(CompilerOptions::from_dialect(Dialect::TwinCat))]
fn parse_when_elementary_type_with_bare_name_then_simple_expr_naming_it(
    #[case] options: CompilerOptions,
) {
    let source = "
FUNCTION_BLOCK FB_A
VAR
    x : UDINT := C;
END_VAR
END_FUNCTION_BLOCK";
    let library = parse_program(source, &FileId::default(), &options).unwrap();

    let fb = cast!(
        &library.elements[0],
        LibraryElementKind::FunctionBlockDeclaration
    );
    assert_eq!(
        Some((&TypeName::from("UDINT"), &Id::from("C"))),
        bare_name(&fb.variables[0].initializer)
    );
}

#[test]
fn parse_when_located_elementary_type_with_bare_name_then_simple_expr_naming_it() {
    let options = CompilerOptions::from_dialect(Dialect::TwinCat);
    let source = "
PROGRAM main
VAR
    x AT %MD0 : UDINT := C;
END_VAR
END_PROGRAM";
    let library = parse_program(source, &FileId::default(), &options).unwrap();

    let program = cast!(&library.elements[0], LibraryElementKind::ProgramDeclaration);
    assert_eq!(
        Some((&TypeName::from("UDINT"), &Id::from("C"))),
        bare_name(&program.variables[0].initializer)
    );
}

#[test]
fn parse_when_structure_field_with_bare_name_then_simple_expr_naming_it() {
    let library = parse_text(
        "
TYPE
    S : STRUCT
        f : UDINT := C;
    END_STRUCT;
END_TYPE",
    );

    let decl = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );
    let structure = cast!(decl, DataTypeDeclarationKind::Structure);
    assert_eq!(
        Some((&TypeName::from("UDINT"), &Id::from("C"))),
        bare_name(&structure.elements[0].init)
    );
}

#[test]
fn parse_when_global_user_type_with_bare_name_then_late_resolved_value() {
    // A global declaration used to take the name as an expression, which
    // reads an enumeration value as a variable; like any other declaration
    // against a user type it is now left to the type resolver.
    let library = parse_text(
        "
VAR_GLOBAL
    state : Color := Green;
END_VAR",
    );

    let globals = cast!(
        &library.elements[0],
        LibraryElementKind::GlobalVarDeclarations
    );
    let late = cast!(
        &globals[0].initializer,
        InitialValueAssignmentKind::LateResolvedType
    );
    assert_eq!(TypeName::from("Color"), late.type_name);
    assert_eq!(
        Some(LateResolvedInitialValue::Value(Id::from("Green"))),
        late.initial_value
    );
}

#[test]
fn parse_when_global_elementary_type_with_bare_name_then_simple_expr_naming_it() {
    let library = parse_text(
        "
VAR_GLOBAL CONSTANT
    D : UDINT := C;
END_VAR",
    );

    let globals = cast!(
        &library.elements[0],
        LibraryElementKind::GlobalVarDeclarations
    );
    assert_eq!(
        Some((&TypeName::from("UDINT"), &Id::from("C"))),
        bare_name(&globals[0].initializer)
    );
}

#[test]
fn parse_when_global_user_type_with_qualified_value_then_enumerated_type() {
    let library = parse_text(
        "
VAR_GLOBAL
    state : Color := Color#Green;
END_VAR",
    );

    let globals = cast!(
        &library.elements[0],
        LibraryElementKind::GlobalVarDeclarations
    );
    let enumerated = cast!(
        &globals[0].initializer,
        InitialValueAssignmentKind::EnumeratedType
    );
    assert_eq!(TypeName::from("Color"), enumerated.type_name);
}
