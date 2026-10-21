//! Semantic rule that the step of a `FOR` loop is not zero.
//!
//! A `FOR` loop adds its step to the control variable after each iteration
//! and leaves when the control variable passes the final value (see
//! 3.3.2.4). A step of zero never moves the control variable, so the loop
//! never ends by counting: a program error, known before the program runs.
//!
//! The step is an expression (`BY expression`), so a step read from a
//! variable is valid IEC 61131-3. IronPLC compiles only a literal step and
//! reports any other as not implemented (P9999) when the loop is compiled,
//! which is a gap in the compiler rather than an error in the program, so
//! this rule does not report it.
//!
//! The rule judges a step whose value is known before the program runs: an
//! integer literal, as it stands after constant folding (`BY -0` and
//! `BY 1 - 1` are `BY 0` by then), and a variable that holds its initial
//! value for the whole run, declared `CONSTANT` or inferred so because
//! nothing writes it, whose value the symbol environment records (see
//! `specs/design/constant-variable-inference.md`). A global is reached
//! through its `VAR_EXTERNAL`. A step of any other shape, a field or an
//! array element among them, is not judged.
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
//! VAR CONSTANT
//!     inc : INT := 0;
//! END_VAR
//! VAR
//!     i : INT;
//!     n : INT;
//! END_VAR
//!     FOR i := 1 TO 5 BY 0 DO       (* P4093: the step is zero *)
//!         n := n + 1;
//!     END_FOR;
//!     FOR i := 1 TO 5 BY inc DO     (* P4093: inc is zero for the whole run *)
//!         n := n + 1;
//!     END_FOR;
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::{ConstantKind, Library},
    core::Located,
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::{Expr, ExprKind, For, SymbolicVariableKind, Variable},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::ScopeTracker,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleForStepNonzero {
            context,
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

/// Whether `constant` is an integer literal whose value is zero. A literal
/// of another kind is not this rule's to judge.
fn is_integer_zero(constant: &ConstantKind) -> bool {
    match constant {
        ConstantKind::IntegerLiteral(literal) => literal.value.value.value == 0,
        _ => false,
    }
}

struct RuleForStepNonzero<'a> {
    context: &'a SemanticContext,
    /// Where the traversal is, to look a step that names a constant up.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleForStepNonzero<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleForStepNonzero<'_> {
    /// Whether the value of `step` is known before the program runs and is
    /// zero: a literal zero, or a constant the step names whose value is
    /// zero, either in parentheses or not. A step of any other shape is not
    /// judged.
    fn is_known_zero(&self, step: &Expr) -> bool {
        let value = match &step.kind {
            ExprKind::Expression(inner) => return self.is_known_zero(inner),
            ExprKind::Const(constant) => Some(constant),
            ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(named))) => self
                .context
                .symbols()
                .constant_value(&named.name, &self.scope.current()),
            _ => None,
        };
        value.is_some_and(is_integer_zero)
    }
}

impl Visitor<Infallible> for RuleForStepNonzero<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_for(&mut self, node: &For) -> Result<Self::Value, Infallible> {
        if let Some(step) = node.step.as_ref().filter(|step| self.is_known_zero(step)) {
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
    use crate::test_helpers::{codes, diagnostic_codes, rule_codes, rule_diagnostics};
    use rstest::rstest;

    /// A program whose only `FOR` loop counts `i` from `from` to `to` with
    /// the `BY` clause `by`, which may be empty.
    fn program_stepping_by(from: &str, to: &str, by: &str) -> String {
        format!(
            "
PROGRAM main
VAR
    i : INT;
    n : INT;
END_VAR
    FOR i := {from} TO {to} {by} DO
        n := n + 1;
    END_FOR;
END_PROGRAM"
        )
    }

    /// A program whose only `FOR` loop steps by `inc`, declared in a block
    /// opened by `section` with the initializer `initial`.
    fn program_stepping_by_declared(section: &str, initial: &str) -> String {
        format!(
            "
PROGRAM main
{section}
    inc : INT := {initial};
END_VAR
VAR
    i : INT;
    n : INT;
END_VAR
    FOR i := 1 TO 5 BY inc DO
        n := n + 1;
    END_FOR;
END_PROGRAM"
        )
    }

    fn codes_for(program: &str) -> Vec<String> {
        rule_codes(apply, program, &CompilerOptions::default())
    }

    fn expression_initializers() -> CompilerOptions {
        CompilerOptions {
            allow_constant_initializer_expressions: true,
            ..CompilerOptions::default()
        }
    }

    #[rstest]
    #[case::no_step("1", "5", "")]
    #[case::one("1", "5", "BY 1")]
    #[case::two("1", "5", "BY 2")]
    #[case::negative("5", "1", "BY -1")]
    #[case::typed("1", "5", "BY INT#1")]
    #[case::folded("1", "5", "BY 3 - 1")]
    fn apply_when_step_is_nonzero_then_ok(#[case] from: &str, #[case] to: &str, #[case] by: &str) {
        assert!(codes_for(&program_stepping_by(from, to, by)).is_empty());
    }

    #[rstest]
    #[case::zero("BY 0")]
    #[case::negated_zero("BY -0")]
    #[case::typed_zero("BY INT#0")]
    #[case::parenthesized_zero("BY (0)")]
    #[case::folded_difference("BY 1 - 1")]
    #[case::folded_product("BY 0 * 5")]
    fn apply_when_step_is_literal_zero_then_p4093(#[case] by: &str) {
        assert_eq!(
            codes_for(&program_stepping_by("1", "5", by)),
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

    /// A declared constant and a variable nothing writes both hold their
    /// initial value for the whole run, so a zero is reported for either.
    #[rstest]
    #[case::declared_constant("VAR CONSTANT")]
    #[case::inferred_constant("VAR")]
    fn apply_when_step_names_constant_zero_then_p4093(#[case] section: &str) {
        assert_eq!(
            codes_for(&program_stepping_by_declared(section, "0")),
            codes(&[Problem::ForStepZero])
        );
    }

    #[rstest]
    #[case::declared_constant("VAR CONSTANT")]
    #[case::inferred_constant("VAR")]
    fn apply_when_step_names_nonzero_constant_then_ok(#[case] section: &str) {
        assert!(codes_for(&program_stepping_by_declared(section, "2")).is_empty());
    }

    #[test]
    fn apply_when_step_names_constant_zero_then_p4093_at_step() {
        let program = &program_stepping_by_declared("VAR CONSTANT", "0");
        let diagnostics = rule_diagnostics(apply, program, &CompilerOptions::default());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::ForStepZero.code()]
        );
        let start = program.find("BY inc").unwrap() + "BY ".len();
        let location = &diagnostics[0].primary.location;
        assert_eq!((location.start, location.end), (start, start + "inc".len()));
    }

    rule_err!(
        /// A constant initialized by an expression holds what the
        /// expression folds to.
        apply_when_step_names_constant_folded_to_zero_then_p4093,
        "
PROGRAM main
VAR CONSTANT
    inc : INT := 1 - 1;
END_VAR
VAR
    i : INT;
    n : INT;
END_VAR
    FOR i := 1 TO 5 BY inc DO
        n := n + 1;
    END_FOR;
END_PROGRAM",
        [Problem::ForStepZero],
        expression_initializers()
    );

    rule_err!(
        /// A global constant is reached through the `VAR_EXTERNAL` that
        /// names it, and holds the value its `VAR_GLOBAL` declares.
        apply_when_step_names_global_constant_zero_then_p4093,
        "
CONFIGURATION config
    VAR_GLOBAL CONSTANT
        inc : INT := 0;
    END_VAR
    RESOURCE resource1 ON PLC
        TASK plc_task(INTERVAL := T#100ms,PRIORITY := 1);
        PROGRAM plc_task_instance WITH plc_task : main;
    END_RESOURCE
END_CONFIGURATION

PROGRAM main
VAR_EXTERNAL CONSTANT
    inc : INT;
END_VAR
VAR
    i : INT;
    n : INT;
END_VAR
    FOR i := 1 TO 5 BY inc DO
        n := n + 1;
    END_FOR;
END_PROGRAM",
        [Problem::ForStepZero]
    );

    rule_err!(
        /// The constant is looked up from the scope of the body that steps
        /// by it: a function's own constant here.
        apply_when_function_steps_by_its_constant_zero_then_p4093,
        "
FUNCTION Count : INT
VAR CONSTANT
    inc : INT := 0;
END_VAR
VAR
    i : INT;
END_VAR
    FOR i := 1 TO 5 BY inc DO
        Count := Count + 1;
    END_FOR;
END_FUNCTION",
        [Problem::ForStepZero]
    );

    rule_ok!(
        /// A step read from a variable the program writes is valid IEC
        /// 61131-3, and its value is not known before the program runs,
        /// whatever it starts at. The backends report it as not
        /// implemented.
        apply_when_step_is_written_variable_then_ok,
        "
PROGRAM main
VAR
    i : INT;
    n : INT;
    delta : INT := 0;
END_VAR
    delta := 2;
    FOR i := 1 TO 5 BY delta DO
        n := n + 1;
    END_FOR;
END_PROGRAM"
    );

    rule_ok!(
        /// A field is not a step whose value the environment records, even
        /// of a structure nothing writes.
        apply_when_step_is_field_then_ok,
        "
TYPE
    Steps : STRUCT size : INT; END_STRUCT;
END_TYPE

PROGRAM main
VAR
    i : INT;
    n : INT;
    steps : Steps;
END_VAR
    FOR i := 1 TO 5 BY steps.size DO
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
