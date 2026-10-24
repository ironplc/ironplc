//! An initializer that is a constant's name: a `VAR` initializer
//! (`x : UDINT := C;`), a string initializer (`s : STRING[20] := CS;`) and
//! a `TYPE` default (`TYPE MyInt : UDINT := C;`).
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

#[test]
fn parse_when_type_default_names_constant_then_simple_expr_naming_it() {
    let library = parse_text(
        "
TYPE
    MyInt : UDINT := C;
END_TYPE",
    );
    let data_type = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );
    let simple = cast!(data_type, DataTypeDeclarationKind::Simple);
    let expr = cast!(
        &simple.spec_and_init,
        InitialValueAssignmentKind::SimpleExpr
    );
    assert_eq!(TypeName::from("UDINT"), expr.type_name);
    assert!(matches!(
        &expr.initial_value.kind,
        ExprKind::LateBound(late) if late.value == Id::from("C")
    ));
}

/// A literal default keeps the standard shape: the expression alternative
/// collapses a literal, a negative one included, back to `Simple`.
#[test]
fn parse_when_type_default_is_negative_literal_then_simple() {
    let library = parse_text("TYPE MyInt : INT := -5; END_TYPE");
    let data_type = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );
    let simple = cast!(data_type, DataTypeDeclarationKind::Simple);
    let init = cast!(&simple.spec_and_init, InitialValueAssignmentKind::Simple);
    let lit = cast!(
        init.initial_value.as_ref().unwrap(),
        ConstantKind::IntegerLiteral
    );
    assert!(lit.value.is_neg);
    assert_eq!(5, lit.value.value.value);
}

/// The string's width and length are recorded with the expression, so the
/// fold can give the folded literal the declaration it was written for.
#[rstest]
#[case::string("s : STRING := CS;", StringType::String, None)]
#[case::sized_string("s : STRING[20] := CS;", StringType::String, Some(20))]
#[case::wstring("s : WSTRING := CS;", StringType::WString, None)]
#[case::sized_wstring("s : WSTRING[8] := CS;", StringType::WString, Some(8))]
fn parse_when_string_initialized_from_name_then_string_expr_naming_it(
    #[case] declaration: &str,
    #[case] width: StringType,
    #[case] length: Option<u128>,
) {
    let source = format!("FUNCTION_BLOCK fb\nVAR\n    {declaration}\nEND_VAR\nEND_FUNCTION_BLOCK");
    let init = first_var_initializer(&source, &CompilerOptions::default());
    let expr = cast!(&init, InitialValueAssignmentKind::StringExpr);
    assert_eq!(width, expr.width);
    assert_eq!(
        length,
        expr.length
            .as_ref()
            .map(|length| length.as_integer().unwrap().value)
    );
    assert!(matches!(
        &expr.initial_value.kind,
        ExprKind::LateBound(late) if late.value == Id::from("CS")
    ));
}

/// A literal keeps the standard shape, which needs no fold.
#[test]
fn parse_when_string_initialized_from_literal_then_string() {
    let init = first_var_initializer(
        "
FUNCTION_BLOCK fb
VAR
    s : STRING[20] := 'abc';
END_VAR
END_FUNCTION_BLOCK",
        &CompilerOptions::default(),
    );
    let string = cast!(&init, InitialValueAssignmentKind::String);
    assert!(string.initial_value.is_some());
}

/// A literal of another kind is not an expression over constants: it stays
/// the syntax error it was rather than becoming a type error.
#[test]
fn parse_when_string_initialized_from_integer_literal_then_syntax_error() {
    let result = parse_program(
        "
FUNCTION_BLOCK fb
VAR
    s : STRING := 5;
END_VAR
END_FUNCTION_BLOCK",
        &FileId::default(),
        &CompilerOptions::default(),
    );
    assert_eq!("P0002", result.unwrap_err().code);
}

#[test]
fn parse_when_structure_element_string_initialized_from_name_then_string_expr_naming_it() {
    let library = parse_text(
        "
TYPE
    Pt : STRUCT
        name : STRING[10] := CS;
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
        InitialValueAssignmentKind::StringExpr
    );
    assert_eq!(StringType::String, expr.width);
    assert!(matches!(
        &expr.initial_value.kind,
        ExprKind::LateBound(late) if late.value == Id::from("CS")
    ));
}
