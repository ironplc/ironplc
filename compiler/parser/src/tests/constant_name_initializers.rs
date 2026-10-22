//! A `VAR` initializer that is a constant's name (`x : UDINT := C;`).
//!
//! After an elementary type the name can only be a constant, so the parser
//! keeps it as a constant expression for the analyzer to fold or diagnose.
//! After a user type name it may as well be an enumeration value, so it is
//! recorded for the type resolver instead; `late_resolved_initializers.rs`
//! covers that side.
use super::common::*;

fn first_var_initializer(source: &str, options: &CompilerOptions) -> InitialValueAssignmentKind {
    let library = parse_program(source, &FileId::default(), options).unwrap();
    let fb = cast!(
        &library.elements[0],
        LibraryElementKind::FunctionBlockDeclaration
    );
    fb.variables[0].initializer.clone()
}

/// Whether the name is a constant, and whether the dialect allows a
/// constant expression here, are the analyzer's questions: the parser
/// produces the same shape with or without the flag.
#[rstest]
#[case::default_dialect(CompilerOptions::default())]
#[case::twincat(CompilerOptions::from_dialect(Dialect::TwinCat))]
fn parse_when_elementary_type_initialized_from_name_then_simple_expr_naming_it(
    #[case] options: CompilerOptions,
) {
    let init = first_var_initializer(
        "
FUNCTION_BLOCK fb
VAR
    x : UDINT := C;
END_VAR
END_FUNCTION_BLOCK",
        &options,
    );
    let expr = cast!(&init, InitialValueAssignmentKind::SimpleExpr);
    assert_eq!(TypeName::from("UDINT"), expr.type_name);
    assert!(matches!(
        &expr.initial_value.kind,
        ExprKind::LateBound(late) if late.value == Id::from("C")
    ));
}

#[test]
fn parse_when_structure_element_initialized_from_name_then_simple_expr_naming_it() {
    let library = parse_text(
        "
TYPE
    Pt : STRUCT
        f : UDINT := C;
    END_STRUCT;
END_TYPE",
    );
    let data_type = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );
    let structure = cast!(data_type, DataTypeDeclarationKind::Structure);
    let expr = cast!(
        &structure.elements[0].init,
        InitialValueAssignmentKind::SimpleExpr
    );
    assert_eq!(TypeName::from("UDINT"), expr.type_name);
    assert!(matches!(
        &expr.initial_value.kind,
        ExprKind::LateBound(late) if late.value == Id::from("C")
    ));
}
