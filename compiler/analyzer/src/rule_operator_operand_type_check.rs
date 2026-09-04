//! Semantic rule that checks the operands of an operator against the type
//! category the operator is defined for.
//!
//! IEC 61131-3 defines `MOD` over `ANY_INT` (Table 24). The function form
//! `MOD(a, b)` is held to that by the function-call rule, whose signature is
//! derived from the `MOD` row of the operator-form table. The operator
//! spelling `a MOD b` has no signature, so this rule reads the same row and
//! asks the same question of each operand, through
//! [`are_types_compatible`]. The two spellings therefore agree by
//! construction: whatever `MOD(a, b)` accepts, `a MOD b` accepts.
//!
//! Only `MOD` is checked (see [`checked_form`]). An operand whose resolved
//! type the predicate cannot judge (a subrange, an enumeration, a
//! structure) is skipped rather than reported, as the assignment check
//! skips such targets: `p MOD 2` on a subrange of `INT` compiles today and
//! this rule leaves that alone.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     d : DINT;
//! END_VAR
//!     d := d MOD 2;
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     r : REAL;
//! END_VAR
//!     r := r MOD 2.0;    (* P4049: MOD is not defined for REAL *)
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

use crate::intermediates::operator_function_form::{
    form_of_operator, FormOf, OperatorFunctionForm,
};
use crate::result::SemanticResult;
use crate::rule_support::{run_rule, DiagnosticVisitor};
use crate::semantic_context::SemanticContext;
use crate::type_compat::{are_types_compatible, is_checkable_type};

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleOperatorOperandTypeCheck {
            options,
            diagnostics: vec![],
        },
        lib,
    )
}

/// Returns the operator-form row whose operand type the operands of `op`
/// must have, or `None` for an operator this rule does not check.
///
/// `MOD` is the only arithmetic operator checked. It is the one whose
/// category is narrower than the others' (`ANY_INT` rather than `ANY_NUM`)
/// and the one codegen has no floating-point opcode for, so a real `MOD`
/// that gets past analysis fails in codegen as an internal error. The other
/// arithmetic operators are declared `ANY_NUM` in the table, but their
/// operator spellings also compile for `TIME` and bit-string operands
/// (`t1 + t2`, `b1 + 1`), and IEC 61131-3 Table 30 defines `ADD` and `SUB`
/// on `TIME`, so holding them to the table is a separate decision from this
/// rule. See issue #1621.
fn checked_form(op: &Operator) -> Option<&'static OperatorFunctionForm> {
    match op {
        Operator::Mod => form_of_operator(&FormOf::Arithmetic(Operator::Mod)),
        Operator::Add | Operator::Sub | Operator::Mul | Operator::Div | Operator::Pow => None,
    }
}

struct RuleOperatorOperandTypeCheck<'a> {
    options: &'a CompilerOptions,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleOperatorOperandTypeCheck<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleOperatorOperandTypeCheck<'_> {
    /// Reports P4049 when `operand`'s resolved type is one the predicate can
    /// judge and it is not acceptable where `expected` is required.
    fn check_operand(&mut self, op: &Operator, expected: &TypeName, operand: &Expr) {
        let Some(actual) = operand.resolved_type.as_ref() else {
            return;
        };
        if !is_checkable_type(actual) {
            return;
        }
        if !are_types_compatible(expected, actual, self.options) {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::OperatorOperandTypeMismatch,
                    Label::span(operand.span(), "Operand"),
                )
                .with_context("operator", &op.to_string())
                .with_context("expected", &expected.to_string())
                .with_context("actual", &actual.to_string()),
            );
        }
    }
}

impl Visitor<Infallible> for RuleOperatorOperandTypeCheck<'_> {
    type Value = ();

    fn visit_binary_expr(&mut self, node: &BinaryExpr) -> Result<Self::Value, Infallible> {
        if let Some(form) = checked_form(&node.op) {
            let expected = form.operand_type();
            self.check_operand(&node.op, &expected, &node.left);
            self.check_operand(&node.op, &expected, &node.right);
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    rule_ctx_ok!(
        apply_when_mod_of_integer_variables_then_ok,
        "
PROGRAM main
VAR
    a : DINT;
    b : DINT;
    c : DINT;
END_VAR
    c := a MOD b;
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_mod_of_integer_variable_and_literal_then_ok,
        "
PROGRAM main
VAR
    a : UINT;
    c : UINT;
END_VAR
    c := a MOD 2;
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_mod_of_typed_integer_literals_then_ok,
        "
PROGRAM main
VAR
    c : LINT;
END_VAR
    c := LINT#100 MOD LINT#7;
END_PROGRAM"
    );

    rule_ctx_ok!(
        /// The operand type is a subrange, which the predicate cannot judge, so
        /// the rule leaves it alone rather than reporting it.
        apply_when_mod_of_subrange_variable_then_ok,
        "
TYPE
    Pct : INT (0..100);
END_TYPE

PROGRAM main
VAR
    p : Pct := 5;
    c : INT;
END_VAR
    c := p MOD 2;
END_PROGRAM"
    );

    rule_ctx_ok!(
        /// Only MOD is checked: `+` on TIME compiles today and stays accepted.
        apply_when_add_of_time_variables_then_ok,
        "
PROGRAM main
VAR
    t1 : TIME := T#1s;
    t2 : TIME := T#2s;
    t3 : TIME;
END_VAR
    t3 := t1 + t2;
END_PROGRAM"
    );

    rule_ctx_errn!(
        apply_when_mod_of_real_variables_then_p4049_per_operand,
        "
PROGRAM main
VAR
    r1 : REAL := 7.5;
    r2 : REAL := 2.0;
    r3 : REAL;
END_VAR
    r3 := r1 MOD r2;
END_PROGRAM",
        2,
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_mod_of_integer_variable_by_real_literal_then_p4049,
        "
PROGRAM main
VAR
    d : DINT;
END_VAR
    d := d MOD 2.0;
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_mod_of_lreal_variable_by_integer_literal_then_p4049,
        "
PROGRAM main
VAR
    l : LREAL;
END_VAR
    l := l MOD 2;
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_errn!(
        /// Real literals are not folded for MOD, so the rule sees them.
        apply_when_mod_of_real_literals_then_p4049_per_operand,
        "
PROGRAM main
VAR
    r : REAL;
END_VAR
    r := 7.5 MOD 2.0;
END_PROGRAM",
        2,
        Problem::OperatorOperandTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_mod_nested_in_larger_expression_then_p4049,
        "
PROGRAM main
VAR
    r : REAL;
    d : DINT;
END_VAR
    d := 1 + (d MOD (r * 2.0));
END_PROGRAM",
        Problem::OperatorOperandTypeMismatch
    );

    #[test]
    fn apply_when_mod_of_real_then_diagnostic_names_operator_and_types() {
        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_context(
            "
PROGRAM main
VAR
    d : DINT;
    r : REAL;
END_VAR
    d := d MOD r;
END_PROGRAM",
        );
        let errors = apply(&library, &context, &CompilerOptions::default()).unwrap_err();
        assert_eq!(errors.len(), 1);
        let described = &errors[0].described;
        assert!(
            described.contains(&"operator=MOD".to_owned()),
            "{described:?}"
        );
        assert!(
            described.contains(&"expected=ANY_INT".to_owned()),
            "{described:?}"
        );
        assert!(
            described.contains(&"actual=real".to_owned()),
            "{described:?}"
        );
    }

    #[test]
    fn analyze_when_mod_of_real_then_pipeline_reports_p4049() {
        // The rule is wired into the full `analyze` pipeline, which collects
        // semantic diagnostics into the context rather than returning Err.
        use crate::stages::analyze;
        let library = crate::test_helpers::parse_only(
            "
PROGRAM main
VAR
    r1 : REAL := 7.5;
    r2 : REAL := 2.0;
    r3 : REAL;
END_VAR
    r3 := r1 MOD r2;
END_PROGRAM
",
        );
        let (_lib, context) = analyze(&[&library], &CompilerOptions::default()).unwrap();
        assert!(context
            .diagnostics()
            .iter()
            .any(|d| d.code == Problem::OperatorOperandTypeMismatch.code()));
    }
}
