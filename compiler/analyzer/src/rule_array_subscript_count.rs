//! Semantic rule that a subscript list gives one subscript for each
//! dimension of the array it subscripts.
//!
//! A list subscripts one array. An element of a multi-dimensional array is
//! selected by one list (`m[1, 2]`), and an element of an array of arrays by
//! one list per array (`rows[1][2]`). So `m[1]` and `m[1][2]` on a
//! two-dimensional array, and `rows[1, 2]` on an array of arrays, select no
//! element: the first list has too few or too many subscripts for its array.
//!
//! An array type that records no dimensions has none to count against, and a
//! list on something that is not an array is not this rule's to judge.
//!
//! See section 2.4.1.2.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//!    Row : ARRAY[1..3] OF INT;
//! END_TYPE
//!
//! PROGRAM main
//!    VAR
//!       m : ARRAY[1..2, 1..3] OF INT;
//!       rows : ARRAY[1..2] OF Row;
//!    END_VAR
//!    m[1, 2] := rows[1][2];
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! TYPE
//!    Row : ARRAY[1..3] OF INT;
//! END_TYPE
//!
//! PROGRAM main
//!    VAR
//!       m : ARRAY[1..2, 1..3] OF INT;
//!       rows : ARRAY[1..2] OF Row;
//!    END_VAR
//!    m[1][2] := rows[1, 2];  (* m takes two subscripts, rows one *)
//! END_PROGRAM
//! ```
use std::convert::Infallible;

use ironplc_dsl::{
    common::*,
    core::{Located, SourceSpan},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::ArrayVariable,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::ScopeTracker,
    variable_type,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleArraySubscriptCount {
            context,
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

/// Checks each subscript list against the dimensions of the array it
/// subscripts, which the type environment answers for.
struct RuleArraySubscriptCount<'a> {
    context: &'a SemanticContext,
    /// Where the traversal is, to type the subscripted variable from.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl RuleArraySubscriptCount<'_> {
    /// Reports `node` when the array it subscripts has a number of
    /// dimensions other than the number of subscripts it gives.
    fn check(&mut self, node: &ArrayVariable) {
        let types = self.context.types();
        let Some(subscripted) = variable_type::type_id(
            &node.subscripted_variable,
            self.context.symbols(),
            types,
            &self.scope.current(),
        ) else {
            return;
        };
        let Some(dimensions) = variable_type::array_dimensions(subscripted, types) else {
            return;
        };
        if dimensions.len() == node.subscripts.len() {
            return;
        }
        let span = match (node.subscripts.first(), node.subscripts.last()) {
            (Some(first), Some(last)) => SourceSpan::join2(first, last),
            _ => node.span(),
        };
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::ArraySubscriptCountMismatch,
                Label::span(span, "Subscripts"),
            )
            .with_context("variable", &node.subscripted_variable.to_string())
            .with_context("dimensions", &dimensions.len().to_string())
            .with_context("subscripts", &node.subscripts.len().to_string()),
        );
    }
}

impl DiagnosticVisitor for RuleArraySubscriptCount<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleArraySubscriptCount<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_array_variable(&mut self, node: &ArrayVariable) -> Result<(), Infallible> {
        // The subscripted variable and the subscripts first, so that the
        // lists of one reference are reported in the order they are written.
        node.recurse_visit(self)?;
        self.check(node);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{
        codes, diagnostic_codes, edition3_options, rule_codes, rule_diagnostics,
    };
    use rstest::rstest;

    /// No problems: every list gives one subscript for each dimension.
    const OK: &[Problem] = &[];
    /// The one problem a list with the wrong number of subscripts reports.
    const MISMATCH: &[Problem] = &[Problem::ArraySubscriptCountMismatch];

    /// A program that declares an array of arrays `rows`, a two-dimensional
    /// array `m`, a one-dimensional array `a`, a structure `s` holding an
    /// array and a reference `p` to `m`, and whose body reads `access` into
    /// `x`.
    fn reading(access: &str) -> String {
        format!(
            "TYPE
    Row : ARRAY[1..3] OF DINT;
    Grid : ARRAY[1..2, 1..3] OF DINT;
    Holder : STRUCT g : Grid; END_STRUCT;
END_TYPE

PROGRAM main
VAR
    rows : ARRAY[1..2] OF Row;
    m : ARRAY[1..2, 1..3] OF DINT;
    a : ARRAY[1..3] OF DINT;
    s : Holder;
    p : REF_TO Grid;
    i : DINT;
    x : DINT;
END_VAR
    x := {access};
END_PROGRAM"
        )
    }

    #[rstest]
    #[case::one_dimension("a[1]", OK)]
    #[case::two_dimensions("m[1, 2]", OK)]
    #[case::array_of_arrays_one_bracket_per_array("rows[1][2]", OK)]
    #[case::array_of_arrays_whole_inner_array("rows[1][i]", OK)]
    #[case::field("s.g[1, 2]", OK)]
    #[case::dereference("p^[1, 2]", OK)]
    #[case::one_dimension_two_subscripts("a[1, 2]", MISMATCH)]
    #[case::two_dimensions_one_subscript("m[1]", MISMATCH)]
    #[case::two_dimensions_one_bracket_per_dimension("m[1][2]", MISMATCH)]
    #[case::two_dimensions_three_subscripts("m[1, 2, 3]", MISMATCH)]
    #[case::array_of_arrays_one_list("rows[1, 2]", MISMATCH)]
    #[case::field_one_subscript("s.g[1]", MISMATCH)]
    #[case::dereference_one_subscript("p^[1]", MISMATCH)]
    #[case::inside_subscript("a[m[1]]", MISMATCH)]
    fn apply_when_subscripts_then_counted_against_dimensions(
        #[case] access: &str,
        #[case] expected: &[Problem],
    ) {
        assert_eq!(
            rule_codes(apply, &reading(access), &edition3_options()),
            codes(expected)
        );
    }

    rule_ok!(
        apply_when_subscripted_variable_not_array_then_ok,
        "PROGRAM main
VAR
    x : DINT;
    y : DINT;
END_VAR
    y := x[1, 2];
END_PROGRAM"
    );

    rule_err_at!(
        apply_when_written_with_too_many_subscripts_then_labels_subscripts,
        "PROGRAM main
VAR
    a : ARRAY[1..3] OF DINT;
    i : DINT;
END_VAR
    a[i, i + 1] := 1;
END_PROGRAM",
        Problem::ArraySubscriptCountMismatch,
        "i, i + 1"
    );

    #[test]
    fn apply_when_mismatch_then_names_variable_dimensions_and_subscripts() {
        let diagnostics = rule_diagnostics(apply, &reading("rows[2, 3]"), &edition3_options());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::ArraySubscriptCountMismatch.code()]
        );
        assert!(diagnostics[0]
            .described
            .contains(&"variable=rows".to_owned()));
        assert!(diagnostics[0]
            .described
            .contains(&"dimensions=1".to_owned()));
        assert!(diagnostics[0]
            .described
            .contains(&"subscripts=2".to_owned()));
    }

    // The examples of `docs/reference/compiler/problems/P4085.rst`.
    const DOCUMENTED_PROGRAM: &str = "
TYPE
  Row : ARRAY[1..3] OF INT;
END_TYPE

PROGRAM main
  VAR
    grid : ARRAY[1..2, 1..3] OF INT;
    rows : ARRAY[1..2] OF Row;
  END_VAR
  grid[2][3] := 7;
  rows[2, 3] := 7;
END_PROGRAM";

    rule_err!(
        apply_when_documented_failing_example_then_error,
        DOCUMENTED_PROGRAM,
        [Problem::ArraySubscriptCountMismatch; 2]
    );

    rule_ok!(
        apply_when_documented_fixed_example_then_ok,
        &DOCUMENTED_PROGRAM
            .replace("grid[2][3]", "grid[2, 3]")
            .replace("rows[2, 3]", "rows[2][3]")
    );
}
