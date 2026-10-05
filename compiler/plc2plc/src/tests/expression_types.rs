//! Tests of the annotated rendering: each expression followed by a comment
//! giving the type the analyzer recorded for it.
//!
//! The tests marked `#[spec_test(REQ_ETR_plc2plc_NNN)]` are the conformance
//! tests for "Inspecting the annotation" in
//! `specs/design/expression-type-resolution.md`.

use super::common::*;

use dsl::common::{GenericTypeName, Library};
use dsl::fold::Fold;
use dsl::textual::{Expr, ExprKind, ExprType};
use dsl::type_id::TypeId;
use spec_test_macro::spec_test;

use crate::write_to_string_with_types;

/// A program declaring a variable of each type the tests use, with `body`
/// as its statements.
fn program(body: &str) -> String {
    format!(
        "
TYPE
    Color : (Red, Green, Blue);
END_TYPE

FUNCTION Pass : LREAL
VAR_INPUT
    x : LREAL;
END_VAR
    Pass := x;
END_FUNCTION

PROGRAM main
VAR
    count : INT;
    total : DINT;
    big : LINT;
    out : LREAL;
    flag : BOOL;
    c : Color;
    v : INT;
    r : REF_TO INT;
END_VAR
{body}
END_PROGRAM"
    )
}

/// The annotated rendering of the statements of `program(body)`, one per line.
fn statements(body: &str) -> Vec<String> {
    let (_, rendered) = analyze_and_render_with_types(&program(body), &edition3());
    rendered
        .lines()
        .skip_while(|line| !line.starts_with("PROGRAM main"))
        .filter(|line| line.contains(":="))
        .map(|line| line.trim().to_string())
        .collect()
}

#[spec_test(REQ_ETR_plc2plc_001)]
fn write_to_string_with_types_when_binary_expr_then_comment_after_operands_and_result() {
    assert_eq!(
        statements("count := count + v;"),
        vec!["count := ( count (* INT *) + v (* INT *) ) (* INT *) ;"]
    );
}

#[spec_test(REQ_ETR_plc2plc_002)]
fn write_to_string_with_types_when_implicit_conversion_then_comment_shows_from_and_to() {
    assert_eq!(
        statements("flag := big > total;"),
        vec!["flag := ( big (* LINT *) > total (* DINT -> LINT *) ) (* BOOL *) ;"]
    );
}

#[spec_test(REQ_ETR_plc2plc_003)]
fn write_to_string_with_types_when_literal_enumerated_value_or_null_then_only_they_are_constant() {
    assert_eq!(
        statements("count := 1; c := Green; r := NULL; v := count;"),
        vec![
            "count := 1 (* CONSTANT INT *) ;",
            "c := Green (* CONSTANT Color *) ;",
            "r := NULL (* CONSTANT NULL *) ;",
            "v := count (* INT *) ;",
        ]
    );
}

/// A typed literal keeps its type and is converted to its parameter's; an
/// untyped one takes the parameter's type.
#[test]
fn write_to_string_with_types_when_converted_literal_then_constant_prefix_and_from_and_to() {
    assert_eq!(
        statements("out := Pass(REAL#0.5); out := Pass(0.1);"),
        vec![
            "out := Pass ( REAL#0.5 (* CONSTANT REAL -> LREAL *) ) (* LREAL *) ;",
            "out := Pass ( 0.1 (* CONSTANT LREAL *) ) (* LREAL *) ;",
        ]
    );
}

#[spec_test(REQ_ETR_plc2plc_005)]
fn write_to_string_with_types_when_unary_or_deref_then_parenthesised() {
    assert_eq!(
        statements("flag := NOT flag; v := r^;"),
        vec![
            "flag := ( NOT flag (* BOOL *) ) (* BOOL *) ;",
            "v := ( r (* REF_TO INT *)^ ) (* INT *) ;",
        ]
    );
}

/// Gives every literal the generic category `ANY_INT`, as the analyzer
/// leaves a literal whose type it did not decide.
struct LiteralsAtAnyInt;

impl Fold<()> for LiteralsAtAnyInt {
    fn fold_expr(&mut self, node: Expr) -> Result<Expr, ()> {
        let mut node = Expr::recurse_fold(node, self)?;
        if matches!(node.kind, ExprKind::Const(_)) {
            node.expr_type = Some(ExprType::Literal(GenericTypeName::AnyInt));
        }
        Ok(node)
    }
}

#[spec_test(REQ_ETR_plc2plc_004)]
fn write_to_string_with_types_when_type_undecided_then_question_mark() {
    // A library that was only parsed has no types recorded. The literal is
    // then set to a generic category by hand rather than by analysis, so the
    // test does not depend on which literals the analyzer still leaves
    // undecided.
    let source = "PROGRAM main VAR v : INT; END_VAR v := v + 1; END_PROGRAM";
    let parsed: Library = parse_program(source, &FileId::default(), &edition3()).unwrap();
    let library = LiteralsAtAnyInt.fold_library(parsed).unwrap();
    let type_name = |id: TypeId| format!("T{}", id.raw());

    let rendered = write_to_string_with_types(&library, &type_name).unwrap();

    assert!(
        rendered.contains("v := ( v (* ? *) + 1 (* CONSTANT ? ANY_INT *) ) (* ? *) ;"),
        "expected undecided comments:\n{rendered}"
    );
}

#[spec_test(REQ_ETR_plc2plc_006)]
fn write_to_string_with_types_when_resource_then_matches_golden_and_reparses_as_plain() {
    let options = edition3();
    let source = read_resource("expression_types.st");

    let (library, rendered) = analyze_and_render_with_types(&source, &options);

    assert_annotated_reparses_as_plain(&library, &rendered, &options);
    assert_eq!(rendered, read_resource("expression_types_rendered.st"));
}
