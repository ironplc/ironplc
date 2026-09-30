//! Semantic rule that checks a comparison has an operand type.
//!
//! A comparison (`=`, `<>`, `<`, `<=`, `>`, `>=`, and the function forms
//! `EQ`, `NE`, `LT`, `LE`, `GT`, `GE`) compares its operands at the type one
//! of them widens to (`comparison_operand_type`). IEC 61131-3 declares the
//! comparison functions over `ANY_ELEMENTARY` with every input of the same
//! type, and the project's implicit conversions relax that to "one widens to
//! the other", as they do for the arithmetic operators. A pair where neither
//! widens to the other, such as `DINT` and `UDINT` or `DINT` and `REAL`, has
//! no type that holds both, so it is reported as P4049, the code `d + r`
//! reports. See `specs/design/comparison-operand-type.md`.
//!
//! An operand whose resolved type the relation cannot judge (a subrange, an
//! enumeration, a structure, `NULL`) is skipped, as the other operand rules
//! skip it. Two string operands are left to
//! `rule_string_encoding_compat` (P4034).
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     d : DINT;
//!     l : LINT;
//!     b : BOOL;
//! END_VAR
//!     b := d < l;
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     d : DINT;
//!     u : UDINT;
//!     r : REAL;
//!     b : BOOL;
//! END_VAR
//!     b := d < u;        (* P4049: neither DINT nor UDINT widens to the other *)
//!     b := GT(d, r);     (* P4049: DINT does not widen to REAL *)
//! END_PROGRAM
//! ```

use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::intermediates::comparison_operand::comparison_operand_type;
use crate::intermediates::operator_function_form::{operator_function_form, FormOf};
use crate::result::SemanticResult;
use crate::rule_support::{run_rule, DiagnosticVisitor};
use crate::semantic_context::SemanticContext;
use crate::type_compat::is_checkable_type;
use crate::type_environment::TypeEnvironment;
use crate::value_type::operand_type_name;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleComparisonOperandType {
            types: context.types(),
            options,
            diagnostics: vec![],
        },
        lib,
    )
}

/// Returns `true` for the comparisons `=`, `<>`, `<`, `<=`, `>` and `>=`,
/// and `false` for the logical and bitwise operators that share
/// `CompareOp`.
fn is_comparison(op: &CompareOp) -> bool {
    match op {
        CompareOp::Eq
        | CompareOp::Ne
        | CompareOp::Lt
        | CompareOp::Gt
        | CompareOp::LtEq
        | CompareOp::GtEq => true,
        CompareOp::And
        | CompareOp::Or
        | CompareOp::Xor
        | CompareOp::AndThen
        | CompareOp::OrElse => false,
    }
}

/// Returns `true` for a string type, whose operand pairs the string
/// encoding check judges.
fn is_string_type(type_name: &TypeName) -> bool {
    matches!(
        ElementaryTypeName::try_from(&type_name.name),
        Ok(ElementaryTypeName::STRING | ElementaryTypeName::WSTRING)
    ) || matches!(
        GenericTypeName::try_from(&type_name.name),
        Ok(GenericTypeName::AnyString)
    )
}

struct RuleComparisonOperandType<'a> {
    types: &'a TypeEnvironment,
    options: &'a CompilerOptions,
    diagnostics: Vec<Diagnostic>,
}

impl RuleComparisonOperandType<'_> {
    /// The name the compatibility relation knows `expr`'s value by (see
    /// `value_type::operand_type_name`), when the relation can judge it.
    fn checkable_name(&self, expr: &Expr) -> Option<TypeName> {
        operand_type_name(self.types, expr.expr_type.as_ref()?).filter(is_checkable_type)
    }

    /// Reports P4049 at `label` when `left` and `right` have types the
    /// relation can judge and neither widens to the other.
    fn check(&mut self, label: Label, operator: &str, left: &Expr, right: &Expr) {
        let (Some(left), Some(right)) = (self.checkable_name(left), self.checkable_name(right))
        else {
            return;
        };
        if is_string_type(&left) && is_string_type(&right) {
            return;
        }
        if comparison_operand_type(Some(&left), Some(&right), self.options).is_some() {
            return;
        }
        self.diagnostics.push(
            Diagnostic::problem(Problem::OperatorOperandTypeMismatch, label)
                .with_context("operator", &operator.to_string())
                .with_context("left", &left.to_string().to_uppercase())
                .with_context("right", &right.to_string().to_uppercase()),
        );
    }

    /// Checks a call to `EQ`, `NE`, `LT`, `LE`, `GT` or `GE` on two
    /// positional inputs, labelled at the function name.
    fn check_call(&mut self, function: &Function) {
        let Some(form) = operator_function_form(&function.name.to_string()) else {
            return;
        };
        let FormOf::Compare(op) = &form.operator else {
            return;
        };
        if !is_comparison(op) {
            return;
        }
        let [ParamAssignmentKind::PositionalInput(left), ParamAssignmentKind::PositionalInput(right)] =
            function.param_assignment.as_slice()
        else {
            return;
        };
        self.check(
            Label::span(function.name.span(), "Function call"),
            &function.name.original().to_string(),
            &left.expr,
            &right.expr,
        );
    }
}

impl DiagnosticVisitor for RuleComparisonOperandType<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleComparisonOperandType<'_> {
    type Value = ();

    /// Checks a comparison at the `Expr` that holds it, so the diagnostic
    /// spans the whole expression.
    fn visit_expr(&mut self, node: &Expr) -> Result<Self::Value, Infallible> {
        match &node.kind {
            ExprKind::Compare(compare) if is_comparison(&compare.op) => self.check(
                Label::span(node.span(), "Expression"),
                compare.op.as_str(),
                &compare.left,
                &compare.right,
            ),
            ExprKind::Function(function) => self.check_call(function),
            _ => {}
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::parse_and_resolve_types_with_options;
    use ironplc_parser::options::Dialect;
    use ironplc_problems::Problem;
    use rstest::rstest;
    use spec_test_macro::spec_test;

    /// Runs the rule over `program` with `options`, returning the codes and
    /// described context of each diagnostic.
    fn check_with(program: &str, options: &CompilerOptions) -> Vec<Diagnostic> {
        let (library, context) = parse_and_resolve_types_with_options(program, options);
        match apply(&library, &context, options) {
            Ok(()) => vec![],
            Err(diagnostics) => diagnostics,
        }
    }

    fn check(program: &str) -> Vec<Diagnostic> {
        check_with(program, &CompilerOptions::default())
    }

    /// A program comparing `left` with `right` as `expr`, declaring the
    /// variables `a` of type `left` and `b` of type `right`.
    fn program(left: &str, right: &str, expr: &str) -> String {
        format!(
            "
PROGRAM main
VAR
    a : {left};
    b : {right};
    x : BOOL;
END_VAR
    x := {expr};
END_PROGRAM"
        )
    }

    fn assert_ok(diagnostics: &[Diagnostic]) {
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    fn assert_p4049(diagnostics: &[Diagnostic]) {
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(
            diagnostics[0].code,
            Problem::OperatorOperandTypeMismatch.code()
        );
    }

    #[spec_test(REQ_CMP_analyzer_007)]
    #[rstest]
    #[case::signed_unsigned("DINT", "UDINT", "a < b")]
    #[case::unsigned_signed("UDINT", "DINT", "a >= b")]
    #[case::integer_real_lossy("DINT", "REAL", "a = b")]
    #[case::real_integer_lossy("REAL", "DINT", "a <> b")]
    #[case::different_temporal_families("TIME", "DATE", "a > b")]
    #[case::bit_string_integer("WORD", "INT", "a <= b")]
    fn apply_when_neither_operand_widens_then_p4049(
        #[case] left: &str,
        #[case] right: &str,
        #[case] expr: &str,
    ) {
        assert_p4049(&check(&program(left, right, expr)));
    }

    #[spec_test(REQ_CMP_analyzer_008)]
    #[rstest]
    #[case::gt("GT(a, b)")]
    #[case::eq("EQ(a, b)")]
    #[case::le("LE(a, b)")]
    fn apply_when_function_form_on_types_neither_widens_then_p4049(#[case] expr: &str) {
        assert_p4049(&check(&program("DINT", "REAL", expr)));
    }

    #[test]
    fn apply_when_neither_operand_widens_then_names_operator_and_types() {
        let diagnostics = check(&program("DINT", "UDINT", "a < b"));
        assert_p4049(&diagnostics);
        let described = diagnostics[0].described.join(" ");
        assert!(described.contains("operator=<"), "{described}");
        assert!(described.contains("left=DINT"), "{described}");
        assert!(described.contains("right=UDINT"), "{described}");
    }

    #[spec_test(REQ_CMP_analyzer_009)]
    #[rstest]
    #[case::same_type("DINT", "DINT", "a < b")]
    #[case::right_wider("DINT", "LINT", "a < b")]
    #[case::unsigned_into_signed("UDINT", "LINT", "a = b")]
    #[case::integer_into_real("INT", "REAL", "a > b")]
    #[case::bit_strings("BYTE", "LWORD", "a <> b")]
    #[case::short_long_temporal("DATE_AND_TIME", "LDATE_AND_TIME", "a < b")]
    #[case::function_form("DINT", "LINT", "GT(a, b)")]
    fn apply_when_one_operand_widens_then_ok(
        #[case] left: &str,
        #[case] right: &str,
        #[case] expr: &str,
    ) {
        assert_ok(&check(&program(left, right, expr)));
    }

    #[spec_test(REQ_CMP_analyzer_010)]
    #[rstest]
    #[case::integer_literal_unsigned("UDINT", "a > 3")]
    #[case::integer_literal_left("SINT", "3 < a")]
    #[case::integer_literal_real("REAL", "a < 1")]
    #[case::real_literal_real("LREAL", "a >= 1.5")]
    #[case::literals("DINT", "1 < 2")]
    fn apply_when_literal_of_operand_category_then_ok(#[case] left: &str, #[case] expr: &str) {
        assert_ok(&check(&program(left, "BOOL", expr)));
    }

    #[spec_test(REQ_CMP_analyzer_011)]
    #[rstest]
    #[case::real_literal_integer("DINT", "a < 1.5")]
    #[case::integer_literal_time("TIME", "a > 0")]
    #[case::integer_literal_bit_string("WORD", "a <> 0")]
    fn apply_when_literal_outside_operand_category_then_p4049(
        #[case] left: &str,
        #[case] expr: &str,
    ) {
        assert_p4049(&check(&program(left, "BOOL", expr)));
    }

    #[spec_test(REQ_CMP_analyzer_012)]
    #[rstest]
    #[case::conversion("UDINT", "DWORD", "a = b")]
    #[case::widening("BYTE", "INT", "a < b")]
    #[case::integer_literal_bit_string("WORD", "BOOL", "a <> 0")]
    fn apply_when_dialect_allows_conversion_then_ok(
        #[case] left: &str,
        #[case] right: &str,
        #[case] expr: &str,
    ) {
        let codesys = CompilerOptions::from_dialect(Dialect::Codesys);
        assert_ok(&check_with(&program(left, right, expr), &codesys));
        assert_p4049(&check(&program(left, right, expr)));
    }

    #[rstest]
    #[case::strings("STRING", "STRING")]
    #[case::string_encodings("STRING", "WSTRING")]
    fn apply_when_operand_left_to_other_rules_then_ok(#[case] left: &str, #[case] right: &str) {
        assert_ok(&check(&program(left, right, "a = b")));
    }

    #[test]
    fn apply_when_enumeration_compared_then_ok() {
        let diagnostics = check(
            "
TYPE
    Mode : (Idle, Run);
END_TYPE

PROGRAM main
VAR
    m : Mode;
    x : BOOL;
END_VAR
    x := m = Run;
END_PROGRAM",
        );
        assert_ok(&diagnostics);
    }

    #[test]
    fn apply_when_subrange_compared_then_ok() {
        let diagnostics = check(
            "
TYPE
    Pct : INT (0..100);
END_TYPE

PROGRAM main
VAR
    p : Pct;
    u : UDINT;
    x : BOOL;
END_VAR
    x := p < u;
END_PROGRAM",
        );
        assert_ok(&diagnostics);
    }

    #[test]
    fn apply_when_logical_operator_then_not_this_rule() {
        assert_ok(&check(&program("BOOL", "BOOL", "a AND b")));
    }

    #[test]
    fn analyze_when_comparison_has_no_operand_type_then_pipeline_reports_p4049() {
        use crate::stages::analyze;
        let library = crate::test_helpers::parse_only(&program("DINT", "UDINT", "a < b"));
        let (_lib, context) = analyze(&[&library], &CompilerOptions::default()).unwrap();
        assert!(context
            .diagnostics()
            .iter()
            .any(|d| d.code == Problem::OperatorOperandTypeMismatch.code()));
    }
}
