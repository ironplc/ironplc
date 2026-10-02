//! Unit tests for `xform_insert_implicit_conversions`.

use std::convert::Infallible;

use ironplc_dsl::core::FileId;
use ironplc_dsl::textual::*;
use ironplc_dsl::visitor::Visitor;
use ironplc_parser::options::CompilerOptions;

use spec_test_macro::spec_test;

use crate::stages::analyze;
use crate::type_environment::TypeEnvironment;
use crate::value_type::operand_type_name;

/// The operands of every comparison in a library, in source order, each as
/// the type it is compared at: `DINT->LINT` for a `DINT` converted to
/// `LINT`, `LINT` for an operand compared as it is.
struct ComparisonOperands<'a> {
    types: &'a TypeEnvironment,
    operands: Vec<[String; 2]>,
}

impl ComparisonOperands<'_> {
    fn describe(&self, expr: &Expr) -> String {
        let name = |expr: &Expr| {
            expr.expr_type
                .as_ref()
                .and_then(|t| operand_type_name(self.types, t))
                .map_or("?".to_string(), |name| name.to_string().to_uppercase())
        };
        match &expr.kind {
            ExprKind::ImplicitConversion(inner) => format!("{}->{}", name(inner), name(expr)),
            _ => name(expr),
        }
    }
}

impl Visitor<Infallible> for ComparisonOperands<'_> {
    type Value = ();

    fn visit_compare_expr(&mut self, node: &CompareExpr) -> Result<(), Infallible> {
        if node.op.is_comparison() {
            let pair = [self.describe(&node.left), self.describe(&node.right)];
            self.operands.push(pair);
        }
        node.recurse_visit(self)
    }

    fn visit_function(&mut self, node: &Function) -> Result<(), Infallible> {
        if let [ParamAssignmentKind::PositionalInput(left), ParamAssignmentKind::PositionalInput(right)] =
            node.param_assignment.as_slice()
        {
            let pair = [self.describe(&left.expr), self.describe(&right.expr)];
            self.operands.push(pair);
        }
        node.recurse_visit(self)
    }
}

/// Analyzes `source`, which must be free of diagnostics, and returns the
/// operands of its comparisons.
fn comparison_operands(source: &str) -> Vec<[String; 2]> {
    let options = CompilerOptions::default();
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
    let (library, context) = analyze(&[&library], &options).unwrap();
    assert!(
        !context.has_diagnostics(),
        "unexpected diagnostics: {:?}",
        context.diagnostics()
    );
    let mut visitor = ComparisonOperands {
        types: context.types(),
        operands: vec![],
    };
    let Ok(()) = visitor.walk(&library);
    visitor.operands
}

/// A program declaring `VAR <vars> END_VAR` whose body is `b := <compare>;`.
fn program(vars: &str, compare: &str) -> String {
    format!("PROGRAM main VAR b : BOOL; {vars} END_VAR b := {compare}; END_PROGRAM")
}

fn pair(left: &str, right: &str) -> Vec<[String; 2]> {
    vec![[left.to_string(), right.to_string()]]
}

#[spec_test(REQ_IC_analyzer_001)]
#[test]
fn apply_when_left_operand_wider_then_right_variable_converted() {
    let source = program("d : DINT; l : LINT;", "l > d");
    assert_eq!(comparison_operands(&source), pair("LINT", "DINT->LINT"));
}

#[test]
fn apply_when_integer_variable_compared_with_real_then_converted_to_real() {
    let source = program("i : INT; r : REAL;", "r >= i");
    assert_eq!(comparison_operands(&source), pair("REAL", "INT->REAL"));
}

#[spec_test(REQ_IC_analyzer_010)]
#[test]
fn apply_when_right_operand_wider_then_left_converted() {
    let source = program("d : DINT; l : LINT;", "d < l");
    assert_eq!(comparison_operands(&source), pair("DINT->LINT", "LINT"));
}

#[spec_test(REQ_IC_analyzer_011)]
#[test]
fn apply_when_neither_widens_then_right_converted_to_left() {
    let source = program("d : DINT; u : UDINT;", "d < u");
    assert_eq!(comparison_operands(&source), pair("DINT", "UDINT->DINT"));
}

/// The widening relation does not see through a named subrange to its base
/// type, so the comparison falls back to the left operand's type and narrows
/// the `LINT`, as codegen did before the analyzer chose.
#[test]
fn apply_when_named_subrange_and_wider_integer_then_wider_converted_to_subrange() {
    let source = "TYPE Small : INT (0..10); END_TYPE
        PROGRAM main VAR b : BOOL; s : Small; l : LINT; END_VAR b := s < l; END_PROGRAM";
    assert_eq!(comparison_operands(source), pair("SMALL", "LINT->SMALL"));
}

#[spec_test(REQ_IC_analyzer_003)]
#[test]
fn apply_when_literal_on_right_then_literal_takes_operand_type() {
    let source = program("l : LINT;", "l > 1");
    assert_eq!(comparison_operands(&source), pair("LINT", "LINT"));
}

#[test]
fn apply_when_literal_on_left_then_literal_takes_operand_type() {
    let source = program("l : LINT;", "1 < l");
    assert_eq!(comparison_operands(&source), pair("LINT", "LINT"));
}

#[test]
fn apply_when_operands_same_type_then_unchanged() {
    let source = program("d1 : DINT; d2 : DINT;", "d1 = d2");
    assert_eq!(comparison_operands(&source), pair("DINT", "DINT"));
}

#[spec_test(REQ_IC_analyzer_004)]
#[test]
fn apply_when_alias_of_operand_type_then_unchanged() {
    let source = "TYPE MyInt : INT; END_TYPE
        PROGRAM main VAR b : BOOL; a : MyInt; i : INT; END_VAR b := a = i; END_PROGRAM";
    assert_eq!(comparison_operands(source), pair("INT", "INT"));
}

#[spec_test(REQ_IC_analyzer_012)]
#[test]
fn apply_when_call_of_narrower_type_then_converted() {
    let source = program("d : DINT; l : LINT;", "ABS(d) < l");
    assert_eq!(comparison_operands(&source), pair("DINT->LINT", "LINT"));
}

#[test]
fn apply_when_short_and_long_time_then_short_converted() {
    let source = program("t : TIME; lt : LTIME;", "t < lt");
    assert_eq!(comparison_operands(&source), pair("TIME->LTIME", "LTIME"));
}

#[spec_test(REQ_IC_analyzer_013)]
#[test]
fn apply_when_function_form_then_inputs_converted() {
    let source = program("d : DINT; l : LINT;", "GT(d, l)");
    assert_eq!(comparison_operands(&source), pair("DINT->LINT", "LINT"));
}

#[spec_test(REQ_IC_analyzer_007)]
#[test]
fn apply_when_strings_then_unchanged() {
    let source = program("s1 : STRING; s2 : STRING[10];", "s1 = s2");
    let operands = comparison_operands(&source);
    assert!(
        operands
            .iter()
            .flatten()
            .all(|operand| !operand.contains("->")),
        "{operands:?}"
    );
}

#[test]
fn apply_when_bitwise_operator_then_operands_unchanged() {
    let options = CompilerOptions::default();
    let source =
        "PROGRAM main VAR w : WORD; d : DWORD; r : DWORD; END_VAR r := w AND d; END_PROGRAM";
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
    let (library, _context) = analyze(&[&library], &options).unwrap();
    assert!(!format!("{library:?}").contains("ImplicitConversion"));
}

#[spec_test(REQ_IC_analyzer_008)]
#[test]
fn apply_when_nested_in_condition_then_conversion_spans_operand() {
    let options = CompilerOptions::default();
    let source =
        "PROGRAM main VAR d : DINT; l : LINT; END_VAR IF l > d THEN d := 1; END_IF; END_PROGRAM";
    let library = ironplc_parser::parse_program(source, &FileId::default(), &options).unwrap();
    let (library, _context) = analyze(&[&library], &options).unwrap();

    struct Conversions(Vec<String>);
    impl Visitor<Infallible> for Conversions {
        type Value = ();
        fn visit_expr(&mut self, node: &Expr) -> Result<(), Infallible> {
            if let ExprKind::ImplicitConversion(inner) = &node.kind {
                assert_eq!(node.span, inner.span);
                self.0.push(node.to_string());
            }
            node.recurse_visit(self)
        }
    }
    let mut conversions = Conversions(vec![]);
    let Ok(()) = conversions.walk(&library);
    // Written as the operand it converts: it is not source syntax.
    assert_eq!(conversions.0, vec!["d"]);
}

#[spec_test(REQ_IC_analyzer_009)]
#[test]
fn apply_when_typed_literal_out_of_range_of_narrower_operand_then_rules_still_report() {
    let options = CompilerOptions::default();
    let source = program("s : SINT;", "DINT#300 < s");
    let library = ironplc_parser::parse_program(&source, &FileId::default(), &options).unwrap();
    let (_library, context) = analyze(&[&library], &options).unwrap();
    let codes: Vec<&str> = context
        .diagnostics()
        .iter()
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(codes, vec!["P2026"]);
}
