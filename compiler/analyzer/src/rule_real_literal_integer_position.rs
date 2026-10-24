//! Semantic rule that an untyped real literal is not used where a `FOR`
//! loop or an array subscript requires an integer.
//!
//! An untyped literal has no type of its own: `1.5` is of the category
//! `ANY_REAL` until the place it is used gives it a type (ADR-0028). A `FOR`
//! loop counts with an integer control variable (P4091), and IEC 61131-3
//! requires its initial value, final value and increment to be "expressions
//! of the same integer type" (see 3.3.2.4); each subscript of an array is an
//! integer (see 2.4.1.2). Neither place has a real type to give the literal,
//! and a real literal is a value of a real type whatever it spells: `2.0` is
//! a `REAL`, not the integer 2. So the literal is reported rather than
//! rounded. A loop from `1.5` has no first iteration a program can mean, and
//! `a[1.5]` names no element.
//!
//! A literal is judged by its type, so `-1.5`, `(1.5)`, `1.0 + 0.5` and
//! `ABS(1.5)`, each an expression of only untyped real literals and so of
//! `ANY_REAL`, are reported as `1.5` is. A value of a real type the program
//! states, `REAL#1.5` or a `REAL` variable, is not an untyped literal and is
//! not this rule's question: the analyzer records its conversion to the
//! integer type (ADR-0056).
//!
//! The bounds and step of a loop whose control variable is not an integer are
//! skipped: `rule_for_control_type` reports that loop (P4091), and a real
//! bound on a `REAL` control variable is not the mistake. A loop whose
//! control variable the analyzer did not resolve is skipped too.
//!
//! The other places that take a literal check it themselves: an assignment
//! (P4035), an operand (P4049), an argument (P4026) and a `CASE` selector
//! (P4053).
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     i : DINT;
//!     levels : ARRAY[1..3] OF DINT;
//!     total : DINT;
//! END_VAR
//!     FOR i := 1 TO 3 DO
//!         total := total + levels[i];
//!     END_FOR;
//!     total := total + levels[REAL_TO_DINT(1.5)];
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     i : DINT;
//!     levels : ARRAY[1..3] OF DINT;
//!     total : DINT;
//! END_VAR
//!     FOR i := 1.5 TO 3 DO             (* P4094: i is a DINT *)
//!         total := total + 1;
//!     END_FOR;
//!     total := levels[1.5];            (* P4094: a subscript is an integer *)
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::{GenericTypeName, Library},
    core::Located,
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::{ArrayVariable, Expr, ExprType, For},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_for_control_type::is_countable,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::ScopeTracker,
    value_type,
    variable_type::declared_type_id,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleRealLiteralIntegerPosition {
            context,
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

/// Returns true if `expr` is an untyped real literal, or an expression of
/// only untyped real literals (`-1.5`, `ABS(1.5)`): a value of the category
/// `ANY_REAL`, which has no type until the place it is used gives it one.
fn is_untyped_real(expr: &Expr) -> bool {
    matches!(
        expr.expr_type,
        Some(ExprType::Literal(GenericTypeName::AnyReal))
    )
}

struct RuleRealLiteralIntegerPosition<'a> {
    context: &'a SemanticContext,
    /// Where the traversal is, to look a loop's control variable up.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleRealLiteralIntegerPosition<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleRealLiteralIntegerPosition<'_> {
    /// The type of the control variable of `node`, as a diagnostic shows it,
    /// when it is an integer the loop's bounds and step take; `None` when it
    /// is not an integer, which `rule_for_control_type` reports, or when it
    /// is not known.
    fn integer_control_type(&self, node: &For) -> Option<String> {
        let id = declared_type_id(&node.control, self.context.symbols(), &self.scope.current())?;
        let types = self.context.types();
        if !is_countable(&types.get_by_id(id)?.representation) {
            return None;
        }
        Some(value_type::describe(types, id))
    }

    /// Reports `expr`, the value at `position`, when it is an untyped real
    /// literal and the position requires `expected`.
    fn check(&mut self, expr: &Expr, position: &str, expected: &str) {
        if !is_untyped_real(expr) {
            return;
        }
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::RealLiteralInIntegerPosition,
                Label::span(expr.span(), position),
            )
            .with_context("expected", &expected.to_string())
            .with_context("actual", &GenericTypeName::AnyReal.as_str().to_string()),
        );
    }
}

impl Visitor<Infallible> for RuleRealLiteralIntegerPosition<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_for(&mut self, node: &For) -> Result<Self::Value, Infallible> {
        if let Some(expected) = self.integer_control_type(node) {
            self.check(&node.from, "FOR start value", &expected);
            self.check(&node.to, "FOR end value", &expected);
            if let Some(step) = &node.step {
                self.check(step, "FOR step", &expected);
            }
        }
        node.recurse_visit(self)
    }

    fn visit_array_variable(&mut self, node: &ArrayVariable) -> Result<Self::Value, Infallible> {
        for subscript in &node.subscripts {
            self.check(subscript, "Subscript", GenericTypeName::AnyInt.as_str());
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{diagnostic_codes, edition3_options, rule_codes, rule_diagnostics};
    use rstest::rstest;

    /// A program with an integer counter `i`, an array `a` of `DINT`s, a
    /// two-dimensional array `m`, a `REAL` `x` and a `DINT` `n`, whose body
    /// is `statement`.
    fn program_with(statement: &str) -> String {
        format!(
            "
PROGRAM main
VAR
    i : DINT;
    a : ARRAY[0..3] OF DINT;
    m : ARRAY[0..1, 0..1] OF DINT;
    x : REAL;
    n : DINT;
END_VAR
    {statement}
END_PROGRAM"
        )
    }

    fn codes_for(program: &str) -> Vec<String> {
        rule_codes(apply, program, &CompilerOptions::default())
    }

    fn p4093() -> Vec<String> {
        vec![Problem::RealLiteralInIntegerPosition.code().to_string()]
    }

    #[rstest]
    #[case::start("FOR i := 1.5 TO 3 DO n := n + 1; END_FOR;")]
    #[case::end("FOR i := 1 TO 3.5 DO n := n + 1; END_FOR;")]
    #[case::step("FOR i := 1 TO 3 BY 0.5 DO n := n + 1; END_FOR;")]
    #[case::whole_valued("FOR i := 2.0 TO 3 DO n := n + 1; END_FOR;")]
    #[case::negative("FOR i := -1.5 TO 3 DO n := n + 1; END_FOR;")]
    #[case::parenthesized("FOR i := (1.5) TO 3 DO n := n + 1; END_FOR;")]
    #[case::arithmetic_of_literals("FOR i := 1.0 + 0.5 TO 3 DO n := n + 1; END_FOR;")]
    #[case::function_of_literal("FOR i := ABS(1.5) TO 3 DO n := n + 1; END_FOR;")]
    fn apply_when_for_bound_is_real_literal_then_p4093(#[case] statement: &str) {
        assert_eq!(codes_for(&program_with(statement)), p4093());
    }

    #[rstest]
    #[case::read("n := a[1.5];")]
    #[case::write("a[1.5] := 1;")]
    #[case::whole_valued("n := a[2.0];")]
    #[case::negative("n := a[-1.5];")]
    #[case::arithmetic_of_literals("n := a[1.5 * 2.0];")]
    #[case::function_of_literal("n := a[ABS(1.5)];")]
    #[case::second_dimension("n := m[0, 1.5];")]
    #[case::inside_another_subscript("n := a[a[1.5]];")]
    #[case::in_for_bound("FOR i := a[1.5] TO 3 DO n := n + 1; END_FOR;")]
    fn apply_when_subscript_is_real_literal_then_p4093(#[case] statement: &str) {
        assert_eq!(codes_for(&program_with(statement)), p4093());
    }

    #[rstest]
    #[case::integer_literal("FOR i := 1 TO 3 DO n := n + 1; END_FOR;")]
    #[case::integer_variable("FOR i := n TO 3 DO n := n + 1; END_FOR;")]
    #[case::typed_integer_literal("FOR i := DINT#1 TO 3 DO n := n + 1; END_FOR;")]
    #[case::converted_real("FOR i := REAL_TO_DINT(1.5) TO 3 DO n := n + 1; END_FOR;")]
    #[case::truncated_real("FOR i := TRUNC(1.5) TO 3 DO n := n + 1; END_FOR;")]
    #[case::subscript_integer("n := a[1];")]
    #[case::subscript_variable("n := a[i];")]
    #[case::subscript_arithmetic("n := a[i + 1];")]
    #[case::subscript_converted_real("n := a[REAL_TO_DINT(1.5)];")]
    #[case::real_elsewhere("x := 1.5; n := a[1];")]
    fn apply_when_position_is_integer_then_ok(#[case] statement: &str) {
        assert!(codes_for(&program_with(statement)).is_empty());
    }

    /// A value of a real type the program states is not an untyped literal:
    /// the analyzer records its conversion to the integer type, and whether
    /// the position accepts it is not this rule's question.
    #[rstest]
    #[case::typed_literal_bound("FOR i := REAL#1.5 TO 3 DO n := n + 1; END_FOR;")]
    #[case::real_variable_bound("FOR i := x TO 3 DO n := n + 1; END_FOR;")]
    #[case::typed_literal_subscript("n := a[LREAL#1.5];")]
    #[case::real_variable_subscript("n := a[x];")]
    fn apply_when_value_is_of_a_stated_real_type_then_ok(#[case] statement: &str) {
        assert!(codes_for(&program_with(statement)).is_empty());
    }

    rule_ok!(
        /// The loop counts with a `REAL`, which `rule_for_control_type`
        /// reports (P4091); a real bound is then not the mistake.
        apply_when_control_is_real_then_ok,
        "
PROGRAM main
VAR
    r : REAL;
    n : DINT;
END_VAR
    FOR r := 1.5 TO 3.0 DO
        n := n + 1;
    END_FOR;
END_PROGRAM"
    );

    rule_ok!(
        /// The loop counts with a bit string, which `rule_for_control_type`
        /// reports (P4091), so its bounds are left to that rule.
        apply_when_control_is_word_then_ok,
        "
PROGRAM main
VAR
    w : WORD;
    n : DINT;
END_VAR
    FOR w := 1.5 TO 3 DO
        n := n + 1;
    END_FOR;
END_PROGRAM"
    );

    rule_ok!(
        /// A control variable the analyzer did not resolve is left to the
        /// rule that reports it as undeclared.
        apply_when_control_is_undeclared_then_ok,
        "
PROGRAM main
VAR
    n : DINT;
END_VAR
    FOR undeclared := 1.5 TO 3 DO
        n := n + 1;
    END_FOR;
END_PROGRAM"
    );

    rule_err!(
        /// A subrange narrows an integer type, so its loop takes an integer.
        apply_when_control_is_named_subrange_then_p4093,
        "
TYPE
    Slot : INT (1..8);
END_TYPE

PROGRAM main
VAR
    slot : Slot;
    n : INT;
END_VAR
    FOR slot := 1.5 TO 8 DO
        n := n + 1;
    END_FOR;
END_PROGRAM",
        [Problem::RealLiteralInIntegerPosition]
    );

    rule_err!(
        /// An alias of an integer type resolves to the integer type.
        apply_when_control_is_alias_of_integer_then_p4093,
        "
TYPE
    Counter : DINT := 0;
END_TYPE

PROGRAM main
VAR
    c : Counter;
    n : INT;
END_VAR
    FOR c := 1 TO 3.5 DO
        n := n + 1;
    END_FOR;
END_PROGRAM",
        [Problem::RealLiteralInIntegerPosition]
    );

    rule_err!(
        /// The control variable is looked up from the scope of the body
        /// that counts with it: a function's own variable here.
        apply_when_function_counts_from_real_literal_then_p4093,
        "
FUNCTION Sum : INT
VAR
    i : INT;
END_VAR
    FOR i := 1.5 TO 3 DO
        Sum := Sum + 1;
    END_FOR;
END_FUNCTION",
        [Problem::RealLiteralInIntegerPosition]
    );

    rule_err!(
        /// A method counts with a field of its block, an integer.
        apply_when_method_counts_from_real_literal_then_p4093,
        "
FUNCTION_BLOCK Block
VAR
    i : INT;
    n : INT;
END_VAR
METHOD Count
    FOR i := 1.5 TO 3 DO
        n := n + 1;
    END_FOR;
END_METHOD
END_FUNCTION_BLOCK",
        [Problem::RealLiteralInIntegerPosition],
        edition3_options()
    );

    rule_err!(
        /// Each bound is judged on its own.
        apply_when_both_bounds_are_real_literals_then_p4093_twice,
        &program_with("FOR i := 1.5 TO 3.5 DO n := n + 1; END_FOR;"),
        [Problem::RealLiteralInIntegerPosition; 2]
    );

    rule_err!(
        /// A bound and a subscript in the loop's body are each reported.
        apply_when_bound_and_subscript_are_real_literals_then_p4093_twice,
        &program_with("FOR i := 1.5 TO 3 DO n := a[2.5]; END_FOR;"),
        [Problem::RealLiteralInIntegerPosition; 2]
    );

    rule_err_at!(
        apply_when_for_step_is_real_literal_then_p4093_at_step,
        &program_with("FOR i := 1 TO 3 BY 0.5 DO n := n + 1; END_FOR;"),
        Problem::RealLiteralInIntegerPosition,
        "0.5"
    );

    rule_err_at!(
        apply_when_subscript_is_real_literal_then_p4093_at_subscript,
        &program_with("n := a[1.5];"),
        Problem::RealLiteralInIntegerPosition,
        "1.5"
    );

    #[test]
    fn apply_when_for_start_is_real_literal_then_names_position_and_control_type() {
        let program = program_with("FOR i := 1.5 TO 3 DO n := n + 1; END_FOR;");
        let diagnostics = rule_diagnostics(apply, &program, &CompilerOptions::default());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::RealLiteralInIntegerPosition.code()]
        );
        assert_eq!(diagnostics[0].primary.message, "FOR start value");
        assert_eq!(
            diagnostics[0].described,
            ["expected=dint", "actual=ANY_REAL"]
        );
    }

    #[test]
    fn apply_when_subscript_is_real_literal_then_names_position_and_integer() {
        let program = program_with("n := a[1.5];");
        let diagnostics = rule_diagnostics(apply, &program, &CompilerOptions::default());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::RealLiteralInIntegerPosition.code()]
        );
        assert_eq!(diagnostics[0].primary.message, "Subscript");
        assert_eq!(
            diagnostics[0].described,
            ["expected=ANY_INT", "actual=ANY_REAL"]
        );
    }

    // The examples of `docs/reference/compiler/problems/P4094.rst`.

    rule_err!(
        apply_when_doc_example_for_then_p4093,
        "
PROGRAM main
VAR
    i : DINT;
    total : DINT;
END_VAR
    FOR i := 1.5 TO 3 DO
        total := total + i;
    END_FOR;
END_PROGRAM",
        [Problem::RealLiteralInIntegerPosition]
    );

    rule_ok!(
        apply_when_doc_example_for_fixed_then_ok,
        "
PROGRAM main
VAR
    i : DINT;
    total : DINT;
END_VAR
    FOR i := 2 TO 3 DO
        total := total + i;
    END_FOR;
END_PROGRAM"
    );

    rule_err!(
        apply_when_doc_example_subscript_then_p4093,
        "
PROGRAM main
VAR
    levels : ARRAY[1..3] OF DINT;
    level : DINT;
END_VAR
    level := levels[1.5];
END_PROGRAM",
        [Problem::RealLiteralInIntegerPosition]
    );

    rule_ok!(
        apply_when_doc_example_subscript_fixed_then_ok,
        "
PROGRAM main
VAR
    levels : ARRAY[1..3] OF DINT;
    position : REAL;
    level : DINT;
END_VAR
    level := levels[REAL_TO_DINT(position)];
END_PROGRAM"
    );
}
