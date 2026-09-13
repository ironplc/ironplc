//! Semantic rule that checks that the first value in a subrange
//! is less than the second value in a subrange.
//!
//! See 2.3.3.2.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//!    VALID_RANGE : INT(-10..10);
//! END_TYPE
//! ```
//!
//! ## Fails
//! ```ignore
//! TYPE
//!    INVALID_RANGE : INT(10..-10);
//! END_TYPE
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
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
        RuleDeclSubrangeLimits {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleDeclSubrangeLimits {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleDeclSubrangeLimits {
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

impl Visitor<Infallible> for RuleDeclSubrangeLimits {
    type Value = ();

    fn visit_subrange(&mut self, node: &Subrange) -> Result<(), Infallible> {
        let Some(bounds) = LiteralBounds::of(node) else {
            return Ok(());
        };

        if bounds.min >= bounds.max {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::SubrangeMinStrictlyLessMax,
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
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    rule_ok!(
        apply_when_subrange_valid_then_ok,
        "
TYPE
    VALID_RANGE : INT(-10..10);
END_TYPE"
    );

    #[test]
    fn apply_when_subrange_invalid_then_error() {
        let program = "
TYPE
    INVALID_RANGE : INT(10..-10);
END_TYPE";

        use crate::stages::analyze;
        use ironplc_dsl::core::FileId;
        use ironplc_parser::{options::CompilerOptions, parse_program};

        let library =
            parse_program(program, &FileId::default(), &CompilerOptions::default()).unwrap();
        let result = analyze(&[&library], &CompilerOptions::default());

        let (_library, context) = result.unwrap();
        assert!(context.has_diagnostics());
    }
}
