//! Semantic rule that a `CASE` on an integer does not label a statement
//! group with an enumerated value.
//!
//! The parser reads a label written as a bare name (`red:`) as an
//! enumerated value, whatever the selector is. When the selector is an
//! enumeration, `rule_use_declared_enumerated_value` checks that the name
//! is one of its values (`P2006`). When the selector is an integer (an
//! `ANY_INT` type, a subrange of one, or an untyped integer literal), its
//! labels are integers. An enumerated value is a value of its enumeration,
//! not of the selector's type, and a name that nothing declares is a value
//! of no type. This rule reports both (`P4101`).
//!
//! A name that is a variable in scope, such as a `VAR CONSTANT`, is
//! skipped. Edition 3 allows a constant expression as a label, and what a
//! label that names a constant or another variable reports is not decided
//! yet (issue #2616).
//!
//! A label written with its type (`E_Color#red`) names an enumeration,
//! which `rule_use_declared_enumerated_value` reports on an integer
//! selector (`P2045`). A selector a `CASE` cannot select on, such as a
//! `REAL`, is reported by `rule_case_selector_type` (`P4053`), and its
//! labels are not checked here. A selector whose type the analyzer did not
//! resolve is skipped.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//!     Mode : (Idle, Fill);
//! END_TYPE
//! PROGRAM main
//! VAR
//!     step : INT;
//!     mode : Mode;
//!     alarm : BOOL;
//! END_VAR
//!     CASE step OF
//!         1, 2..5: alarm := TRUE;
//!     END_CASE;
//!     CASE mode OF
//!         Fill: alarm := FALSE;
//!     END_CASE;
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! TYPE
//!     Mode : (Idle, Fill);
//! END_TYPE
//! PROGRAM main
//! VAR
//!     step : INT;
//!     alarm : BOOL;
//! END_VAR
//!     CASE step OF
//!         Fill: alarm := TRUE;     (* P4101: Fill is a Mode, not an INT *)
//!         Drain: alarm := FALSE;   (* P4101: nothing declares Drain *)
//!     END_CASE;
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::{GenericTypeName, Library},
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::{Case, CaseSelectionKind, Expr, ExprType},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_case_selector_type::is_selectable,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::ScopeTracker,
    value_type,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleCaseLabelType {
            context,
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleCaseLabelType<'a> {
    context: &'a SemanticContext,
    /// Where the traversal is, to look the name of a label up.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleCaseLabelType<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleCaseLabelType<'_> {
    /// The type of `selector`, as a diagnostic shows it, when it is an
    /// integer that a `CASE` selects on; `None` when it is an enumeration,
    /// a type a `CASE` cannot select on, or a type that is not known.
    fn integer_type(&self, selector: &Expr) -> Option<String> {
        let types = self.context.types();
        match selector.expr_type.as_ref()? {
            ExprType::Concrete(id) | ExprType::Inferred(id) => {
                let representation = &types.get_by_id(*id)?.representation;
                (is_selectable(representation) && !representation.is_enumeration())
                    .then(|| value_type::describe(types, *id))
            }
            // An untyped integer literal is `ANY_INT`.
            ExprType::Literal(GenericTypeName::AnyInt) => {
                Some(GenericTypeName::AnyInt.as_str().to_string())
            }
            ExprType::Literal(_) | ExprType::Null | ExprType::Error => None,
        }
    }

    /// Whether `name` is a variable where the traversal is, including one
    /// the scope declares `VAR CONSTANT` and a global it does not reach.
    fn is_variable(&self, name: &Id) -> bool {
        self.context
            .symbols()
            .resolve_variable(name, &self.scope.current())
            .declared()
            .is_some()
    }
}

impl Visitor<Infallible> for RuleCaseLabelType<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_case(&mut self, node: &Case) -> Result<Self::Value, Infallible> {
        if let Some(expected) = self.integer_type(&node.selector) {
            for selection in node
                .statement_groups
                .iter()
                .flat_map(|group| &group.selectors)
            {
                let CaseSelectionKind::EnumeratedValue(value) = selection else {
                    continue;
                };
                if value.type_name.is_some() || self.is_variable(&value.value) {
                    continue;
                }
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::CaseLabelTypeMismatch,
                        Label::span(value.span(), "CASE label"),
                    )
                    .with_secondary(Label::span(node.selector.span(), "CASE selector"))
                    .with_context_id("label", &value.value)
                    .with_context("expected", &expected),
                );
            }
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{diagnostic_codes, rule_codes, rule_diagnostics};
    use rstest::rstest;

    /// A program that selects on `sel`, declared as `selector_type`, with
    /// one `CASE` label, `label`, and the types the tests use. The types
    /// follow the program, so that the first place a value's name appears
    /// is the label.
    fn program_with_label(selector_type: &str, label: &str) -> String {
        format!(
            "
PROGRAM main
VAR
    sel : {selector_type};
    y : INT;
END_VAR
    CASE sel OF
        {label}: y := 1;
    END_CASE;
END_PROGRAM

TYPE
    E_Color : (red, green);
    E_Paint : E_Color;
    Pct : INT (0..100);
    Counter : DINT;
END_TYPE"
        )
    }

    fn codes_of(program: &str) -> Vec<String> {
        rule_codes(apply, program, &CompilerOptions::default())
    }

    rule_err_at!(
        apply_when_label_is_enumerated_value_on_integer_selector_then_p4101,
        &program_with_label("INT", "red"),
        Problem::CaseLabelTypeMismatch,
        "red"
    );

    rule_err_at!(
        apply_when_label_is_undeclared_name_on_integer_selector_then_p4101,
        &program_with_label("INT", "blue"),
        Problem::CaseLabelTypeMismatch,
        "blue"
    );

    #[rstest]
    #[case::sint("SINT")]
    #[case::dint("DINT")]
    #[case::ulint("ULINT")]
    #[case::named_subrange("Pct")]
    #[case::inline_subrange("INT (0..100)")]
    #[case::alias_of_integer("Counter")]
    fn apply_when_label_is_enumerated_value_on_integer_type_then_p4101(
        #[case] selector_type: &str,
    ) {
        assert_eq!(
            codes_of(&program_with_label(selector_type, "red")),
            [Problem::CaseLabelTypeMismatch.code()]
        );
    }

    rule_err!(
        /// An untyped integer literal is `ANY_INT`, whose values are
        /// integers.
        apply_when_label_is_enumerated_value_on_integer_literal_selector_then_p4101,
        "
TYPE
    E_Color : (red, green);
END_TYPE

PROGRAM main
VAR
    y : INT;
END_VAR
    CASE 1 OF
        red: y := 1;
    END_CASE;
END_PROGRAM",
        [Problem::CaseLabelTypeMismatch]
    );

    #[test]
    fn apply_when_label_is_enumerated_value_on_integer_selector_then_names_label_and_selector_type()
    {
        let diagnostics = rule_diagnostics(
            apply,
            &program_with_label("Pct", "red"),
            &CompilerOptions::default(),
        );

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::CaseLabelTypeMismatch.code()]
        );
        assert_eq!(
            diagnostics[0]
                .described
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["label=red", "expected=Pct"]
        );
    }

    #[rstest]
    #[case::integer("1")]
    #[case::negative_integer("-1")]
    #[case::subrange("1..5")]
    #[case::several_integers("1, 2, 3")]
    fn apply_when_labels_are_integers_then_ok(#[case] label: &str) {
        assert_eq!(
            codes_of(&program_with_label("INT", label)),
            Vec::<String>::new()
        );
    }

    #[rstest]
    #[case::member("E_Color", "red")]
    // A name that is not a member is `rule_use_declared_enumerated_value`'s.
    #[case::not_a_member("E_Color", "blue")]
    #[case::member_through_alias("E_Paint", "green")]
    #[case::inline_enumeration("(on, off)", "on")]
    fn apply_when_selector_is_enumeration_then_ok(
        #[case] selector_type: &str,
        #[case] label: &str,
    ) {
        assert_eq!(
            codes_of(&program_with_label(selector_type, label)),
            Vec::<String>::new()
        );
    }

    rule_ok!(
        /// A label written with its type names an enumeration, which
        /// `rule_use_declared_enumerated_value` reports on an integer
        /// selector.
        apply_when_label_written_with_type_on_integer_selector_then_ok,
        &program_with_label("INT", "E_Color#red")
    );

    rule_ok!(
        /// `rule_case_selector_type` reports the selector.
        apply_when_selector_is_not_selectable_then_ok,
        &program_with_label("REAL", "red")
    );

    rule_ok!(
        /// What a label that names a constant reports is not decided yet
        /// (issue #2616).
        apply_when_label_names_constant_then_ok,
        "
PROGRAM main
VAR CONSTANT
    K : INT := 3;
END_VAR
VAR
    n : INT;
    y : INT;
END_VAR
    CASE n OF
        K: y := 1;
    END_CASE;
END_PROGRAM"
    );

    rule_ok!(
        /// What a label that names a variable reports is not decided yet
        /// (issue #2616).
        apply_when_label_names_variable_then_ok,
        "
PROGRAM main
VAR
    n : INT;
    m : INT;
    y : INT;
END_VAR
    CASE n OF
        m: y := 1;
    END_CASE;
END_PROGRAM"
    );

    rule_ok!(
        apply_when_label_names_input_of_function_then_ok,
        "
FUNCTION pick : INT
VAR_INPUT
    n : INT;
    k : INT;
END_VAR
    CASE n OF
        k: pick := 1;
    END_CASE;
END_FUNCTION"
    );

    rule_err_at!(
        /// A name is looked up where the `CASE` is: another block's
        /// variable is not in scope there.
        apply_when_label_names_variable_of_other_block_then_p4101,
        "
PROGRAM main
VAR
    n : INT;
    y : INT;
END_VAR
    CASE n OF
        k: y := 1;
    END_CASE;
END_PROGRAM

FUNCTION_BLOCK other
VAR
    k : INT;
END_VAR
END_FUNCTION_BLOCK",
        Problem::CaseLabelTypeMismatch,
        "k"
    );

    rule_err!(
        apply_when_two_labels_are_not_values_then_reports_each,
        &program_with_label("INT", "1, red, blue"),
        [Problem::CaseLabelTypeMismatch; 2]
    );

    rule_err_at!(
        apply_when_nested_case_label_is_enumerated_value_then_p4101,
        "
PROGRAM main
VAR
    n : INT;
    m : INT;
    y : INT;
END_VAR
    CASE n OF
        1:
            CASE m OF
                green: y := 1;
            END_CASE;
    END_CASE;
END_PROGRAM

TYPE
    E_Color : (red, green);
END_TYPE",
        Problem::CaseLabelTypeMismatch,
        "green"
    );

    // The examples of `docs/reference/compiler/problems/P4101.rst`.
    rule_err!(
        apply_when_documented_failing_example_then_p4101,
        "
TYPE
    Mode : (Idle, Fill);
END_TYPE

PROGRAM main
    VAR
        step : INT;
        alarm : BOOL;
    END_VAR
    CASE step OF
        Fill: alarm := TRUE;
        Drain: alarm := FALSE;
    END_CASE;
END_PROGRAM",
        [Problem::CaseLabelTypeMismatch; 2]
    );

    rule_ok!(
        apply_when_documented_fixed_example_then_ok,
        "
TYPE
    Mode : (Idle, Fill);
END_TYPE

PROGRAM main
    VAR
        step : INT;
        mode : Mode;
        alarm : BOOL;
    END_VAR
    CASE step OF
        1: alarm := TRUE;
        2: alarm := FALSE;
    END_CASE;
    CASE mode OF
        Fill: alarm := TRUE;
        Idle: alarm := FALSE;
    END_CASE;
END_PROGRAM"
    );
}
