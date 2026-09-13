//! Semantic rule that checks the order of the two bounds of a range.
//!
//! What order is valid depends on what the range is for:
//!
//! * A **subrange type** narrows an integer type to the values between its
//!   bounds, so the minimum must be strictly less than the maximum: a
//!   subrange of one value or none is not a useful type. Reported as P2002.
//!   See 2.3.3.2.
//! * An **array dimension** counts elements, and one element is an ordinary
//!   array, so the minimum may equal the maximum; only an inverted range is
//!   an error. Reported as P2024, the same code the type-environment builder
//!   uses for a `TYPE`-declared array, so an inverted array range gets one
//!   code wherever it is declared. See 2.4.2.1.
//! * A **`CASE` label range** selects a branch for every value between its
//!   bounds, and one value is an ordinary label, so the minimum may equal
//!   the maximum; an inverted range selects nothing, so the branch can never
//!   run. Reported as P4051. See 3.3.2.3.
//!
//! The rule is keyed on the node that owns the range rather than on the bare
//! `Subrange`, because the DSL reuses one `Subrange` struct for all three.
//! Applying the subrange-type check to every `Subrange` is what once
//! rejected `ARRAY[0..0]` and `CASE` label `5..5:`.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//!    VALID_RANGE : INT(-10..10);
//!    ONE_ELEMENT : ARRAY[0..0] OF INT;
//! END_TYPE
//! PROGRAM main
//!    VAR x : INT; y : INT; END_VAR
//!    CASE x OF
//!       5..5: y := 1;
//!    END_CASE;
//! END_PROGRAM
//! ```
//!
//! ## Fails
//! ```ignore
//! TYPE
//!    INVALID_RANGE : INT(10..-10);
//!    SINGLE_VALUE : INT(5..5);
//!    INVERTED : ARRAY[1..0] OF INT;
//! END_TYPE
//! PROGRAM main
//!    VAR x : INT; y : INT; END_VAR
//!    CASE x OF
//!       10..1: y := 1;
//!    END_CASE;
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::CaseSelectionKind,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleRangeLimits {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleRangeLimits {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleRangeLimits {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// The two ends of a range whose bounds are both literals, with their
/// signed values.
///
/// A range keeps its ends as [`SignedIntegerRef`] because either may still
/// name a constant; the rule only judges ranges whose ends have already
/// been resolved to literals, and skips the others (an unresolved constant
/// is reported by the resolver, not here).
struct LiteralBounds<'a> {
    start: &'a SignedInteger,
    end: &'a SignedInteger,
    min: i128,
    max: i128,
}

impl<'a> LiteralBounds<'a> {
    fn of(range: &'a Subrange) -> Option<Self> {
        let start = range.start.as_signed_integer()?;
        let end = range.end.as_signed_integer()?;
        let min = start.clone().try_into().ok()?;
        let max = end.clone().try_into().ok()?;
        Some(Self {
            start,
            end,
            min,
            max,
        })
    }
}

/// What a range is for, which decides the bound orderings it may have.
#[derive(Clone, Copy)]
enum RangeContext {
    /// `INT(-10..10)`: the set of values of a type.
    SubrangeType,
    /// `ARRAY[0..9]`: the index positions of one array dimension.
    ArrayDimension,
    /// `CASE x OF 1..9:`: the selector values that pick a branch.
    CaseLabel,
}

impl RangeContext {
    /// Whether a range from `min` to `max` is valid in this context.
    fn accepts(self, min: i128, max: i128) -> bool {
        match self {
            RangeContext::SubrangeType => min < max,
            RangeContext::ArrayDimension | RangeContext::CaseLabel => min <= max,
        }
    }

    fn problem(self) -> Problem {
        match self {
            RangeContext::SubrangeType => Problem::SubrangeMinStrictlyLessMax,
            RangeContext::ArrayDimension => Problem::ArrayDimensionInvalid,
            RangeContext::CaseLabel => Problem::CaseLabelRangeInvalid,
        }
    }
}

impl RuleRangeLimits {
    fn check(&mut self, range: &Subrange, context: RangeContext) {
        let Some(bounds) = LiteralBounds::of(range) else {
            return;
        };

        if !context.accepts(bounds.min, bounds.max) {
            self.diagnostics.push(
                Diagnostic::problem(
                    context.problem(),
                    Label::span(bounds.start.value.span(), "Expected smaller value"),
                )
                .with_context("minimum", &bounds.start.to_string())
                .with_context("maximum", &bounds.end.to_string())
                .with_secondary(Label::span(
                    bounds.end.value.span(),
                    "Expected greater value",
                )),
            );
        }
    }
}

impl Visitor<Infallible> for RuleRangeLimits {
    type Value = ();

    fn visit_subrange_specification(
        &mut self,
        node: &SubrangeSpecification,
    ) -> Result<(), Infallible> {
        self.check(&node.subrange, RangeContext::SubrangeType);
        node.recurse_visit(self)
    }

    fn visit_array_subranges(&mut self, node: &ArraySubranges) -> Result<(), Infallible> {
        for range in &node.ranges {
            self.check(range, RangeContext::ArrayDimension);
        }
        node.recurse_visit(self)
    }

    fn visit_case_selection_kind(&mut self, node: &CaseSelectionKind) -> Result<(), Infallible> {
        if let CaseSelectionKind::Subrange(range) = node {
            self.check(range, RangeContext::CaseLabel);
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use ironplc_problems::Problem;

    rule_ok!(
        apply_when_subrange_valid_then_ok,
        "
TYPE
    VALID_RANGE : INT(-10..10);
END_TYPE"
    );

    rule_err1_at!(
        apply_when_subrange_invalid_then_error,
        "
TYPE
    INVALID_RANGE : INT(10..-10);
END_TYPE",
        Problem::SubrangeMinStrictlyLessMax,
        "10"
    );

    rule_err1!(
        apply_when_subrange_single_value_then_error,
        "
TYPE
    SINGLE_VALUE : INT(5..5);
END_TYPE",
        Problem::SubrangeMinStrictlyLessMax
    );

    rule_ok!(
        apply_when_array_single_element_in_var_then_ok,
        "
PROGRAM main
    VAR
        arr : ARRAY[0..0] OF LWORD;
    END_VAR
    arr[0] := 0;
END_PROGRAM"
    );

    rule_ok!(
        apply_when_array_single_element_in_type_then_ok,
        "
TYPE
    ONE_ELEMENT : ARRAY[1..1] OF INT;
END_TYPE"
    );

    rule_ok!(
        apply_when_array_single_element_every_dimension_then_ok,
        "
PROGRAM main
    VAR
        arr : ARRAY[0..0, 1..1] OF INT;
    END_VAR
END_PROGRAM"
    );

    rule_err1_at!(
        apply_when_array_inverted_in_var_then_error,
        "
PROGRAM main
    VAR
        arr : ARRAY[1..0] OF INT;
    END_VAR
END_PROGRAM",
        Problem::ArrayDimensionInvalid,
        "1"
    );

    rule_errn!(
        apply_when_array_inverted_every_dimension_then_error_per_dimension,
        "
PROGRAM main
    VAR
        arr : ARRAY[1..0, 2..1] OF INT;
    END_VAR
END_PROGRAM",
        2,
        Problem::ArrayDimensionInvalid
    );

    rule_ok!(
        apply_when_case_label_single_value_range_then_ok,
        "
PROGRAM main
    VAR
        x : INT;
        y : INT;
    END_VAR
    CASE x OF
        5..5: y := 1;
    END_CASE;
END_PROGRAM"
    );

    rule_ok!(
        apply_when_case_label_range_spans_zero_then_ok,
        "
PROGRAM main
    VAR
        x : INT;
        y : INT;
    END_VAR
    CASE x OF
        -1..1: y := 1;
    END_CASE;
END_PROGRAM"
    );

    rule_err1_at!(
        apply_when_case_label_range_inverted_then_error,
        "
PROGRAM main
    VAR
        x : INT;
        y : INT;
    END_VAR
    CASE x OF
        10..1: y := 1;
    END_CASE;
END_PROGRAM",
        Problem::CaseLabelRangeInvalid,
        "10"
    );

    rule_errn!(
        apply_when_case_label_range_inverted_twice_then_error_per_label,
        "
PROGRAM main
    VAR
        x : INT;
        y : INT;
    END_VAR
    CASE x OF
        10..1, 2: y := 1;
        3..2: y := 2;
    END_CASE;
END_PROGRAM",
        2,
        Problem::CaseLabelRangeInvalid
    );
}
