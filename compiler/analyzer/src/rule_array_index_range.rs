//! Semantic rule that a constant array subscript selects an element that
//! exists.
//!
//! An array's declaration fixes the index range of each dimension:
//! `ARRAY[1..5] OF INT` has elements 1 through 5. A subscript that is an
//! integer literal outside that range names an element the array does not
//! have, which is known before the program runs.
//!
//! Only a literal subscript is checked, as it stands after constant folding:
//! `a[3 + 3]` is `a[6]` by then. A subscript computed at run time, or one that
//! names a constant, is not.
//!
//! Each subscript list is checked against the dimensions of the array it
//! subscripts: in `a[1][2]`, `2` against the element type of `a`. A list whose
//! length differs from the number of dimensions is not this rule's to judge.
//!
//! See section 2.4.1.2.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       a : ARRAY[1..5] OF INT;
//!       m : ARRAY[-1..1, 0..2] OF INT;
//!       i : INT;
//!    END_VAR
//!    a[1] := a[5];
//!    m[-1, 2] := a[i];
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//!    VAR
//!       a : ARRAY[1..5] OF INT;
//!       m : ARRAY[-1..1, 0..2] OF INT;
//!    END_VAR
//!    a[6] := 1;          (* a has elements 1..5 *)
//!    m[0, 3] := a[0];    (* both subscripts outside their dimension *)
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::*,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    semantic_type::{ArrayDimension, SemanticType},
    symbol_environment::ScopeTracker,
    variable_type,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleArrayIndexRange {
            context,
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleArrayIndexRange<'a> {
    context: &'a SemanticContext,
    /// Where the traversal is, to look variables up in the symbol
    /// environment.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleArrayIndexRange<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleArrayIndexRange<'_> {
    /// Checks each literal subscript of `node` against its dimension of the
    /// array `node` subscripts.
    fn check(&mut self, node: &ArrayVariable) {
        let Some(SemanticType::Array { dimensions, .. }) =
            self.subscripted_type(&node.subscripted_variable)
        else {
            return;
        };
        // An array of unknown size (`ARRAY[*]`) has no dimensions to check.
        if dimensions.is_empty() || dimensions.len() != node.subscripts.len() {
            return;
        }
        for (subscript, dimension) in node.subscripts.iter().zip(&dimensions) {
            if let ExprKind::Const(ConstantKind::IntegerLiteral(literal)) = &subscript.kind {
                self.check_literal(literal, dimension);
            }
        }
    }

    /// The type of the variable a subscript list applies to.
    ///
    /// `variable_type::of` answers a dereference with the reference's own
    /// type, so `p^[6]` is checked against the type `p` refers to.
    fn subscripted_type(&self, kind: &SymbolicVariableKind) -> Option<SemanticType> {
        let subscripted = variable_type::of(kind, self.context, &self.scope.current())?;
        match (kind, subscripted) {
            (SymbolicVariableKind::Deref(_), SemanticType::Reference { target_type }) => {
                Some(*target_type)
            }
            (_, subscripted) => Some(subscripted),
        }
    }

    /// Reports `literal` when its value is outside `dimension`.
    fn check_literal(&mut self, literal: &IntegerLiteral, dimension: &ArrayDimension) {
        let lower = i128::from(dimension.lower);
        let upper = i128::from(dimension.upper);
        // A literal too large for `i128` is outside every dimension.
        let in_range = i128::try_from(literal.value.clone())
            .is_ok_and(|index| index >= lower && index <= upper);
        if in_range {
            return;
        }
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::ArrayIndexOutOfBounds,
                Label::span(
                    literal.value.value.span(),
                    format!("Index must be in the range {lower} to {upper}"),
                ),
            )
            .with_context("index", &literal.value.to_string())
            .with_context("minimum", &lower.to_string())
            .with_context("maximum", &upper.to_string()),
        );
    }
}

impl Visitor<Infallible> for RuleArrayIndexRange<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_array_variable(&mut self, node: &ArrayVariable) -> Result<(), Infallible> {
        self.check(node);
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::{codes, rule_codes};
    use ironplc_parser::options::CompilerOptions;
    use rstest::rstest;

    use super::*;

    /// No problems: every literal subscript is in range.
    const OK: &[Problem] = &[];
    /// The one problem an out-of-range subscript reports.
    const OUT_OF_BOUNDS: &[Problem] = &[Problem::ArrayIndexOutOfBounds];

    /// A program that declares `declaration` and reads `access` into `x`.
    fn reading(declaration: &str, access: &str) -> String {
        format!(
            "PROGRAM main
VAR
    {declaration}
    x : INT;
END_VAR
    x := {access};
END_PROGRAM"
        )
    }

    #[rstest]
    #[case::lower_bound("a[1]", OK)]
    #[case::upper_bound("a[5]", OK)]
    #[case::below_lower_bound("a[0]", OUT_OF_BOUNDS)]
    #[case::above_upper_bound("a[6]", OUT_OF_BOUNDS)]
    #[case::folded_expression("a[3 + 3]", OUT_OF_BOUNDS)]
    #[case::beyond_i64("a[5000000000]", OUT_OF_BOUNDS)]
    #[case::beyond_i128("a[340282366920938463463374607431768211455]", OUT_OF_BOUNDS)]
    fn apply_when_literal_subscript_then_checked_against_dimension(
        #[case] access: &str,
        #[case] expected: &[Problem],
    ) {
        assert_eq!(
            rule_codes(
                apply,
                &reading("a : ARRAY[1..5] OF INT;", access),
                &CompilerOptions::default()
            ),
            codes(expected)
        );
    }

    #[rstest]
    #[case::lower_bound("a[-2]", OK)]
    #[case::upper_bound("a[2]", OK)]
    #[case::below_lower_bound("a[-3]", OUT_OF_BOUNDS)]
    #[case::above_upper_bound("a[3]", OUT_OF_BOUNDS)]
    fn apply_when_dimension_has_negative_bound_then_checked(
        #[case] access: &str,
        #[case] expected: &[Problem],
    ) {
        assert_eq!(
            rule_codes(
                apply,
                &reading("a : ARRAY[-2..2] OF INT;", access),
                &CompilerOptions::default()
            ),
            codes(expected)
        );
    }

    #[rstest]
    #[case::both_in_range("m[1, 2]", OK)]
    #[case::first_out_of_range("m[2, 0]", OUT_OF_BOUNDS)]
    #[case::second_out_of_range("m[0, 3]", OUT_OF_BOUNDS)]
    #[case::both_out_of_range(
        "m[-2, -1]",
        &[Problem::ArrayIndexOutOfBounds, Problem::ArrayIndexOutOfBounds]
    )]
    fn apply_when_array_has_two_dimensions_then_each_subscript_checked(
        #[case] access: &str,
        #[case] expected: &[Problem],
    ) {
        assert_eq!(
            rule_codes(
                apply,
                &reading("m : ARRAY[-1..1, 0..2] OF INT;", access),
                &CompilerOptions::default()
            ),
            codes(expected)
        );
    }

    #[rstest]
    #[case::outer_out_of_range("a[3][1]")]
    #[case::inner_out_of_range("a[1][4]")]
    fn apply_when_array_of_arrays_then_each_level_checked(#[case] access: &str) {
        let program = format!(
            "TYPE
    Row : ARRAY[1..3] OF INT;
END_TYPE
{}",
            reading("a : ARRAY[1..2] OF Row;", access)
        );
        assert_eq!(
            rule_codes(apply, &program, &CompilerOptions::default()),
            codes(OUT_OF_BOUNDS)
        );
    }

    rule_err_at!(
        apply_when_subscript_out_of_range_then_labels_subscript,
        "PROGRAM main
VAR
    a : ARRAY[1..5] OF INT;
END_VAR
    a[6] := 1;
END_PROGRAM",
        Problem::ArrayIndexOutOfBounds,
        "6"
    );

    rule_ok!(
        apply_when_subscript_is_variable_then_ok,
        "PROGRAM main
VAR
    a : ARRAY[1..5] OF INT;
    i : INT := 6;
END_VAR
    a[i] := 1;
END_PROGRAM"
    );

    rule_ok!(
        apply_when_subscript_names_constant_then_not_checked,
        "PROGRAM main
VAR CONSTANT
    K : INT := 6;
END_VAR
VAR
    a : ARRAY[1..5] OF INT;
END_VAR
    a[K] := 1;
END_PROGRAM"
    );

    rule_err!(
        apply_when_named_array_type_then_err,
        "TYPE
    Readings : ARRAY[1..5] OF INT;
END_TYPE
PROGRAM main
VAR
    a : Readings;
END_VAR
    a[6] := 1;
END_PROGRAM",
        [Problem::ArrayIndexOutOfBounds]
    );

    rule_err!(
        apply_when_structure_field_array_then_err,
        "TYPE
    S : STRUCT
        f : ARRAY[1..5] OF INT;
    END_STRUCT;
END_TYPE
PROGRAM main
VAR
    s : S;
END_VAR
    s.f[6] := 1;
END_PROGRAM",
        [Problem::ArrayIndexOutOfBounds]
    );

    rule_err!(
        apply_when_array_through_reference_then_err,
        "PROGRAM main
VAR
    a : ARRAY[1..5] OF INT;
    p : REF_TO ARRAY[1..5] OF INT;
END_VAR
    p := REF(a);
    p^[6] := 1;
END_PROGRAM",
        [Problem::ArrayIndexOutOfBounds],
        CompilerOptions {
            allow_ref_to: true,
            ..CompilerOptions::default()
        }
    );

    rule_err!(
        apply_when_function_local_array_then_err,
        "FUNCTION F : INT
VAR_INPUT
    i : INT;
END_VAR
VAR
    a : ARRAY[1..5] OF INT;
END_VAR
    F := a[0];
END_FUNCTION",
        [Problem::ArrayIndexOutOfBounds]
    );

    fn through_this(body: &str) -> Vec<String> {
        let program = format!(
            "
FUNCTION_BLOCK FB_A
VAR
    arr : ARRAY[1..10] OF INT;
END_VAR
METHOD M
{body}
END_METHOD
END_FUNCTION_BLOCK"
        );
        rule_codes(
            apply,
            &program,
            &crate::test_helpers::fb_inheritance_options(),
        )
    }

    #[rstest]
    #[case::in_range("    THIS^.arr[1] := 1;", OK)]
    #[case::above_upper_bound("    THIS^.arr[11] := 1;", OUT_OF_BOUNDS)]
    fn apply_when_subscript_through_this_then_checked_against_dimension(
        #[case] body: &str,
        #[case] expected: &[Problem],
    ) {
        assert_eq!(codes(expected), through_this(body));
    }
}
