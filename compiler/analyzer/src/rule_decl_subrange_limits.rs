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

use crate::{result::SemanticResult, semantic_context::SemanticContext};

pub fn apply(lib: &Library, _context: &SemanticContext) -> SemanticResult {
    let mut visitor = RuleDeclSubrangeLimits {
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleDeclSubrangeLimits {
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<Diagnostic> for RuleDeclSubrangeLimits {
    type Value = ();

    fn visit_subrange(&mut self, node: &Subrange) -> Result<(), Diagnostic> {
        let minimum: i128 = node.start.clone().try_into().expect("Value in range i128");
        let maximum: i128 = node.end.clone().try_into().expect("Value in range i128");

        if minimum >= maximum {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::SubrangeMinStrictlyLessMax,
                    Label::span(node.start.value.span(), "Expected smaller value"),
                )
                .with_context("minimum", &node.start.to_string())
                .with_context("maximum", &node.end.to_string())
                .with_secondary(Label::span(node.end.value.span(), "Expected greater value")),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic_context::SemanticContextBuilder;
    use crate::test_helpers::parse_and_resolve_types;

    use super::*;

    #[test]
    fn apply_when_subrange_valid_then_ok() {
        let program = "
TYPE
    VALID_RANGE : INT(-10..10);
END_TYPE";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_subrange_invalid_then_error() {
        let program = "
TYPE
    INVALID_RANGE : INT(10..-10);
END_TYPE";

        // With the new implementation, invalid subranges are caught during type resolution
        // The parse_and_resolve_types function will now fail, so we need to handle this differently
        use crate::stages::resolve_types;
        use ironplc_dsl::core::FileId;
        use ironplc_parser::{options::ParseOptions, parse_program};

        let library = parse_program(program, &FileId::default(), &ParseOptions::default()).unwrap();
        let result = resolve_types(&[&library]);

        // Should fail during type resolution due to invalid subrange
        assert!(result.is_err());
    }
}
