//! Keywords the parser also accepts as identifiers.
//!
//! `variable_identifier` admits `ON`, `STEP`, `R_EDGE` and `F_EDGE` as names
//! because real programs use them (`TYPE M : (Off, On); END_TYPE`). Such a
//! name has to behave like any other identifier: it records where it was
//! written, and a bare use of it in an expression is late bound, so it can
//! resolve to an enumerated value. See
//! https://github.com/ironplc/ironplc/issues/1944.

use super::common::*;
use dsl::core::Located;

fn parse_assigned_expression(expression: &str) -> (String, Expr) {
    let source = format!(
        "FUNCTION_BLOCK fb
VAR
    a : INT;
END_VAR
a := {expression};
END_FUNCTION_BLOCK"
    );
    let library = parse_program(&source, &FileId::default(), &CompilerOptions::default());
    assert!(library.is_ok(), "Parse failed: {:?}", library.err());

    let value = extract_assignment_value(&library.unwrap());
    (source, value)
}

#[rstest]
#[case::on("On")]
#[case::step("Step")]
#[case::r_edge("R_EDGE")]
#[case::f_edge("F_EDGE")]
fn parse_when_keyword_identifier_in_expression_then_late_bound(#[case] name: &str) {
    let (_, value) = parse_assigned_expression(name);

    let late_bound = cast!(&value.kind, ExprKind::LateBound);
    assert_eq!(Id::from(name), late_bound.value);
}

#[rstest]
#[case::on("On")]
#[case::step("Step")]
#[case::r_edge("R_EDGE")]
#[case::f_edge("F_EDGE")]
fn parse_when_keyword_identifier_in_expression_then_span_covers_the_name(#[case] name: &str) {
    let (source, value) = parse_assigned_expression(name);

    let span = value.span();
    assert_eq!(name, &source[span.start..span.end]);
}

#[test]
fn parse_when_keyword_identifier_as_enumeration_member_then_span_covers_the_member() {
    let source = "TYPE M : (Off, On); END_TYPE";
    let library = parse_text(source);

    let dt = cast!(
        &library.elements[0],
        LibraryElementKind::DataTypeDeclaration
    );
    let decl = cast!(dt, DataTypeDeclarationKind::Enumeration);
    let values = cast!(&decl.spec_init.spec, SpecificationKind::Inline);
    let span = values.values[1].value.span();
    assert_eq!("On", &source[span.start..span.end]);
}

#[test]
fn parse_when_keyword_identifier_with_member_access_then_variable() {
    let (_, value) = parse_assigned_expression("On.x");

    let variable = cast!(&value.kind, ExprKind::Variable);
    let symbolic = cast!(variable, Variable::Symbolic);
    let structured = cast!(symbolic, SymbolicVariableKind::Structured);
    let record = cast!(structured.record.as_ref(), SymbolicVariableKind::Named);
    assert_eq!(Id::from("On"), record.name);
    assert_eq!(Id::from("x"), structured.field);
}
