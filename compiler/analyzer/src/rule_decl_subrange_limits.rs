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

use crate::{
    result::SemanticResult, symbol_environment::SymbolEnvironment,
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    _type_environment: &TypeEnvironment,
    _symbol_environment: &SymbolEnvironment,
) -> SemanticResult {
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
    use crate::test_helpers::parse_and_resolve_types;

    use super::*;

    #[test]
    fn apply_when_subrange_valid_then_ok() {
        let program = "
TYPE
    VALID_RANGE : INT(-10..10);
END_TYPE";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_subrange_invalid_then_error() {
        let program = "
TYPE
    INVALID_RANGE : INT(10..-10);
END_TYPE";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        assert!(result.is_err());
    }
}
