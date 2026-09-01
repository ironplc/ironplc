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

impl Visitor<Infallible> for RuleDeclSubrangeLimits {
    type Value = ();

    fn visit_subrange(&mut self, node: &Subrange) -> Result<(), Infallible> {
        let start = match node.start.as_signed_integer() {
            Some(si) => si,
            None => return Ok(()),
        };
        let end = match node.end.as_signed_integer() {
            Some(si) => si,
            None => return Ok(()),
        };
        let minimum: i128 = start.clone().try_into().expect("Value in range i128");
        let maximum: i128 = end.clone().try_into().expect("Value in range i128");

        if minimum >= maximum {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::SubrangeMinStrictlyLessMax,
                    Label::span(start.value.span(), "Expected smaller value"),
                )
                .with_context("minimum", &start.to_string())
                .with_context("maximum", &end.to_string())
                .with_secondary(Label::span(end.value.span(), "Expected greater value")),
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
