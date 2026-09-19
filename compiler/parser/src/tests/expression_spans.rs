//! Every expression records the source text it was written as.
//!
//! An expression is written with tokens that none of its children hold: the
//! operator of `NOT c`, the caret of `p^`, the parentheses of `(a + b)` and
//! of `MAX(a, b)`. `Located for ExprKind` can only join the spans of the
//! children, so a diagnostic labelling any of those underlined too little
//! until the parser started recording the span on `Expr` itself. See
//! https://github.com/ironplc/ironplc/issues/1662.

use super::common::*;
use dsl::core::Located;

/// Parses `FUNCTION_BLOCK fb ... a := <expression>;` and returns both the
/// source and the assigned expression, so a test can slice the source with
/// the span it is checking.
fn parse_assigned_expression(expression: &str) -> (String, Expr) {
    let source = format!(
        "FUNCTION_BLOCK fb
VAR
    a : INT;
    b : INT;
    c : BOOL;
    p : REF_TO INT;
END_VAR
a := {expression};
END_FUNCTION_BLOCK"
    );
    let options = CompilerOptions {
        allow_ref_to: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(&source, &FileId::default(), &options);
    assert!(library.is_ok(), "Parse failed: {:?}", library.err());

    let value = extract_assignment_value(&library.unwrap());
    (source, value)
}

#[rstest]
#[case::variable("a")]
#[case::literal("42")]
#[case::late_bound("undeclared")]
#[case::negation("-a")]
#[case::negated_literal("-42")]
#[case::logical_negation("NOT c")]
#[case::binary("a + b")]
#[case::comparison("a < b")]
#[case::group("(a + b)")]
#[case::group_of_unary("(NOT c)")]
#[case::binary_of_group("(a + b) * 2")]
#[case::binary_of_unary("a + -b")]
#[case::dereference("p^")]
#[case::negated_dereference("-p^")]
#[case::reference("REF(a)")]
#[case::call("MAX(a, b)")]
#[case::negated_call("-MAX(a, b)")]
#[case::null("NULL")]
fn parse_when_expression_then_span_covers_the_expression(#[case] expression: &str) {
    let (source, value) = parse_assigned_expression(expression);

    let span = value.span();
    assert_eq!(
        expression,
        &source[span.start..span.end],
        "span of {value:?} should cover the expression as written"
    );
}

/// The span belongs to each node, not only to the outermost one: a
/// diagnostic about an operand has to underline that operand.
#[test]
fn parse_when_unary_operand_of_binary_then_operand_span_covers_the_operator() {
    let (source, value) = parse_assigned_expression("a + NOT c");

    let binary = cast!(&value.kind, ExprKind::BinaryOp);
    let span = binary.right.span();
    assert_eq!("NOT c", &source[span.start..span.end]);
}

/// A diagnostic about a call's argument underlines the argument, not the
/// whole call.
#[test]
fn parse_when_negated_call_argument_then_argument_span_covers_the_operator() {
    let (source, value) = parse_assigned_expression("MAX(a, -b)");

    let function = cast!(&value.kind, ExprKind::Function);
    let argument = cast!(
        &function.param_assignment[1],
        ParamAssignmentKind::PositionalInput
    );
    let span = argument.expr.span();
    assert_eq!("-b", &source[span.start..span.end]);
}
