//! Semantic rule that the step of a `FOR` loop is not zero.
//!
//! A `FOR` loop adds its step to the control variable after each iteration
//! and leaves when the control variable passes the final value (see
//! 3.3.2.4). A step of zero never moves the control variable, so the loop
//! never ends by counting: a program error, known before the program runs.
//!
//! The step is an expression (`BY expression`), so a step read from a
//! variable is valid IEC 61131-3. IronPLC compiles only a constant step and
//! reports any other as not implemented (P9999) when the loop is compiled,
//! which is a gap in the compiler rather than an error in the program, so
//! this rule does not report it.
//!
//! The rule judges a step that is an integer literal, as it stands after
//! constant folding: `BY -0` and `BY 1 - 1` are `BY 0` by then. A step that
//! names a constant, declared `CONSTANT` or inferred, is not read: no stage
//! substitutes a named constant into a statement, and neither backend
//! compiles such a step (P9999), so a named constant whose value is zero is
//! not reported here.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     i : INT;
//!     n : INT;
//! END_VAR
//!     FOR i := 10 TO 1 BY -1 DO
//!         n := n + 1;
//!     END_FOR;
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     i : INT;
//!     n : INT;
//! END_VAR
//!     FOR i := 1 TO 5 BY 0 DO     (* P4093: the step is zero *)
//!         n := n + 1;
//!     END_FOR;
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::{ConstantKind, Library},
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::{Expr, ExprKind, For},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
};

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(RuleForStepNonzero::default(), lib)
}

/// Whether `step` is an integer literal whose value is zero. Any other
/// expression, a variable or a literal of another kind, is not this rule's
/// to judge.
fn is_literal_zero(step: &Expr) -> bool {
    match &step.kind {
        ExprKind::Const(ConstantKind::IntegerLiteral(literal)) => literal.value.value.value == 0,
        _ => false,
    }
}

#[derive(Default)]
struct RuleForStepNonzero {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleForStepNonzero {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleForStepNonzero {
    type Value = ();

    fn visit_for(&mut self, node: &For) -> Result<Self::Value, Infallible> {
        if let Some(step) = node.step.as_ref().filter(|step| is_literal_zero(step)) {
            self.diagnostics.push(Diagnostic::problem(
                Problem::ForStepZero,
                Label::span(step.span(), "FOR step"),
            ));
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{codes, rule_codes};
    use rstest::rstest;

    /// A program whose only `FOR` loop counts `i` from `from` to `to` by
    /// `step`, written as `BY step`, or with no `BY` clause when `step` is
    /// empty.
    fn program_stepping_by(from: &str, to: &str, step: &str) -> String {
        let by = if step.is_empty() {
            String::new()
        } else {
            format!(" BY {step}")
        };
        format!(
            "
PROGRAM main
VAR
    i : INT;
    n : INT;
END_VAR
    FOR i := {from} TO {to}{by} DO
        n := n + 1;
    END_FOR;
END_PROGRAM"
        )
    }

    fn codes_for(program: &str) -> Vec<String> {
        rule_codes(apply, program, &CompilerOptions::default())
    }

    #[rstest]
    #[case::no_step("1", "5", "")]
    #[case::one("1", "5", "1")]
    #[case::two("1", "5", "2")]
    #[case::negative("5", "1", "-1")]
    #[case::typed("1", "5", "INT#1")]
    #[case::folded("1", "5", "3 - 1")]
    fn apply_when_step_is_nonzero_then_ok(
        #[case] from: &str,
        #[case] to: &str,
        #[case] step: &str,
    ) {
        assert!(codes_for(&program_stepping_by(from, to, step)).is_empty());
    }

    #[rstest]
    #[case::zero("0")]
    #[case::negated_zero("-0")]
    #[case::typed_zero("INT#0")]
    #[case::folded_difference("1 - 1")]
    #[case::folded_product("0 * 5")]
    fn apply_when_step_is_literal_zero_then_p4093(#[case] step: &str) {
        assert_eq!(
            codes_for(&program_stepping_by("1", "5", step)),
            codes(&[Problem::ForStepZero])
        );
    }

    rule_err_at!(
        /// The label is at the step, not at the control variable.
        apply_when_step_is_zero_then_p4093_at_step,
        "
PROGRAM main
VAR
    i : INT;
    n : INT;
END_VAR
    FOR i := 1 TO 5 BY 0 DO
        n := n + 1;
    END_FOR;
END_PROGRAM",
        Problem::ForStepZero,
        "0"
    );

    rule_err_at!(
        /// A folded step keeps the span of the expression it was folded
        /// from, so the label covers what was written.
        apply_when_step_folds_to_zero_then_p4093_at_expression,
        "
PROGRAM main
VAR
    i : INT;
    n : INT;
END_VAR
    FOR i := 1 TO 5 BY 1 - 1 DO
        n := n + 1;
    END_FOR;
END_PROGRAM",
        Problem::ForStepZero,
        "1 - 1"
    );

    rule_ok!(
        /// A step read from a variable is valid IEC 61131-3; it is not a
        /// constant, so this rule has nothing to judge. The backends report
        /// it as not implemented.
        apply_when_step_is_variable_then_ok,
        "
PROGRAM main
VAR
    i : INT;
    n : INT;
    delta : INT;
END_VAR
    delta := 2;
    FOR i := 1 TO 5 BY delta DO
        n := n + 1;
    END_FOR;
END_PROGRAM"
    );

    rule_ok!(
        /// A step that names a constant is not read, so a constant whose
        /// value is zero is not reported here: the backends report the step
        /// as not implemented, as they do any step that is not a literal.
        apply_when_step_names_constant_zero_then_ok,
        "
PROGRAM main
VAR
    i : INT;
    n : INT;
END_VAR
VAR CONSTANT
    inc : INT := 0;
END_VAR
    FOR i := 1 TO 5 BY inc DO
        n := n + 1;
    END_FOR;
END_PROGRAM"
    );

    rule_ok!(
        /// A real step is not an integer literal. The control variable is
        /// not an integer either, which `rule_for_control_type` reports.
        apply_when_step_is_real_zero_then_ok,
        "
PROGRAM main
VAR
    r : REAL;
    n : INT;
END_VAR
    FOR r := 1.0 TO 3.0 BY 0.0 DO
        n := n + 1;
    END_FOR;
END_PROGRAM"
    );

    rule_err!(
        apply_when_function_steps_by_zero_then_p4093,
        "
FUNCTION Count : INT
VAR
    i : INT;
END_VAR
    FOR i := 1 TO 5 BY 0 DO
        Count := Count + 1;
    END_FOR;
END_FUNCTION",
        [Problem::ForStepZero]
    );

    rule_err!(
        /// Each loop is judged on its own: the inner one steps by zero, the
        /// outer by one.
        apply_when_nested_loop_steps_by_zero_then_p4093_once,
        "
PROGRAM main
VAR
    i : INT;
    j : INT;
    n : INT;
END_VAR
    FOR i := 1 TO 3 BY 1 DO
        FOR j := 1 TO 3 BY 0 DO
            n := n + 1;
        END_FOR;
    END_FOR;
END_PROGRAM",
        [Problem::ForStepZero]
    );

    rule_err!(
        apply_when_two_loops_step_by_zero_then_p4093_twice,
        "
PROGRAM main
VAR
    i : INT;
    n : INT;
END_VAR
    FOR i := 1 TO 3 BY 0 DO
        n := n + 1;
    END_FOR;
    FOR i := 3 TO 1 BY 0 DO
        n := n - 1;
    END_FOR;
END_PROGRAM",
        [Problem::ForStepZero; 2]
    );
}
