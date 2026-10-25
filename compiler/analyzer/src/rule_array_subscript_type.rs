//! Semantic rule that an array subscript is not a `BOOL`.
//!
//! IEC 61131-3 requires each subscript of an array to be an integer (see
//! 2.4.1.2). A `BOOL` is not one: it has no numeric value until a conversion
//! such as `BOOL_TO_INT` gives it one. No dialect is known to accept a `BOOL`
//! subscript, so no flag relaxes the rule; a program with one compiled before
//! this rule existed, but should not have.
//!
//! A subscript is judged by the type the analyzer resolved for it, so a
//! `BOOL` variable, an alias of `BOOL`, a comparison, a bit access (`w.3`), a
//! `BOOL` literal and a function result of type `BOOL` are each reported. A
//! subscript the analyzer left without a type is skipped: the rule that failed
//! to type it has already said why.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     levels : ARRAY[0..1] OF DINT;
//!     high : BOOL;
//!     level : DINT;
//! END_VAR
//!     level := levels[BOOL_TO_INT(high)];
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     levels : ARRAY[0..1] OF DINT;
//!     high : BOOL;
//!     level : DINT;
//! END_VAR
//!     level := levels[high];      (* P4086: high is a BOOL *)
//!     levels[level > 0] := 1;     (* P4086: a comparison is a BOOL *)
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::Library,
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::{ArrayVariable, Expr, ExprType},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    semantic_type::SemanticType,
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleArraySubscriptType {
            types: context.types(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleArraySubscriptType<'a> {
    types: &'a TypeEnvironment,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleArraySubscriptType<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleArraySubscriptType<'_> {
    /// Whether the analyzer resolved `subscript` to `BOOL` or an alias of it.
    fn is_bool(&self, subscript: &Expr) -> bool {
        let Some(ExprType::Concrete(id) | ExprType::Inferred(id)) = &subscript.expr_type else {
            return false;
        };
        self.types
            .get_by_id(*id)
            .is_some_and(|attributes| attributes.representation == SemanticType::Bool)
    }
}

impl Visitor<Infallible> for RuleArraySubscriptType<'_> {
    type Value = ();

    fn visit_array_variable(&mut self, node: &ArrayVariable) -> Result<Self::Value, Infallible> {
        for subscript in &node.subscripts {
            if self.is_bool(subscript) {
                self.diagnostics.push(Diagnostic::problem(
                    Problem::ArraySubscriptBool,
                    Label::span(subscript.span(), "Subscript"),
                ));
            }
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::rule_codes;
    use ironplc_parser::options::Dialect;
    use rstest::rstest;

    /// A program with an array `a` of `DINT`s, a two-dimensional array `m`,
    /// and variables `b : BOOL`, `w : WORD`, `i : DINT` and `x : DINT`,
    /// whose body is `statement`.
    fn program_with(statement: &str) -> String {
        format!(
            "
PROGRAM main
VAR
    a : ARRAY[0..3] OF DINT;
    m : ARRAY[0..1, 0..1] OF DINT;
    b : BOOL;
    w : WORD;
    i : DINT;
    x : DINT;
END_VAR
    {statement}
END_PROGRAM"
        )
    }

    fn codes_for(program: &str, options: &CompilerOptions) -> Vec<String> {
        rule_codes(apply, program, options)
    }

    fn p4086() -> String {
        Problem::ArraySubscriptBool.code().to_string()
    }

    #[rstest]
    #[case::read("x := a[b];")]
    #[case::write("a[b] := 1;")]
    #[case::comparison("x := a[i > 0];")]
    #[case::bit_access("x := a[w.3];")]
    #[case::literal("x := a[TRUE];")]
    #[case::boolean_operator("x := a[b AND b];")]
    #[case::conversion_to_bool("x := a[INT_TO_BOOL(i)];")]
    #[case::second_dimension("x := m[0, b];")]
    #[case::inside_another_subscript("x := a[a[b]];")]
    fn apply_when_subscript_is_bool_then_p4086(#[case] statement: &str) {
        assert_eq!(
            codes_for(&program_with(statement), &CompilerOptions::default()),
            vec![p4086()]
        );
    }

    #[test]
    fn apply_when_subscript_is_bool_in_every_dialect_then_p4086() {
        for dialect in Dialect::ALL {
            let options = CompilerOptions::from_dialect(*dialect);
            assert_eq!(
                codes_for(&program_with("a[b] := 1;"), &options),
                vec![p4086()],
                "{dialect:?}"
            );
        }
    }

    #[rstest]
    #[case::dint("x := a[i];")]
    #[case::literal("x := a[1];")]
    #[case::arithmetic("x := a[i + 1];")]
    #[case::converted_bool("x := a[BOOL_TO_INT(b)];")]
    #[case::two_dimensions("x := m[i, 1];")]
    fn apply_when_subscript_is_integer_then_ok(#[case] statement: &str) {
        assert!(codes_for(&program_with(statement), &CompilerOptions::default()).is_empty());
    }

    rule_err!(
        apply_when_subscript_is_alias_of_bool_then_p4086,
        "
TYPE
    Flag : BOOL := FALSE;
END_TYPE

PROGRAM main
VAR
    a : ARRAY[0..1] OF DINT;
    f : Flag;
    x : DINT;
END_VAR
    x := a[f];
END_PROGRAM",
        [Problem::ArraySubscriptBool]
    );

    rule_err!(
        apply_when_subscript_is_bool_function_result_then_p4086,
        "
FUNCTION is_high : BOOL
VAR_INPUT
    level : INT;
END_VAR
    is_high := level > 90;
END_FUNCTION

PROGRAM main
VAR
    a : ARRAY[0..1] OF DINT;
    level : INT;
    x : DINT;
END_VAR
    x := a[is_high(level)];
END_PROGRAM",
        [Problem::ArraySubscriptBool]
    );

    rule_ok!(
        /// A subscript the analyzer did not type is left to the rule that
        /// failed to type it.
        apply_when_subscript_is_untyped_then_ok,
        "
PROGRAM main
VAR
    a : ARRAY[0..1] OF DINT;
    x : DINT;
END_VAR
    x := a[undeclared];
END_PROGRAM"
    );

    rule_err_at!(
        apply_when_subscript_is_comparison_then_p4086_at_subscript,
        &program_with("x := a[i > 0];"),
        Problem::ArraySubscriptBool,
        "i > 0"
    );
}
