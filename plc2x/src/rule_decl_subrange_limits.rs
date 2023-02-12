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
use crate::error::SemanticDiagnostic;
use ironplc_dsl::{common::*, visitor::Visitor};

pub fn apply(lib: &Library) -> Result<(), SemanticDiagnostic> {
    let mut visitor = RuleDeclSubrangeLimits {};
    visitor.walk(lib)
}

struct RuleDeclSubrangeLimits {}

impl Visitor<SemanticDiagnostic> for RuleDeclSubrangeLimits {
    type Value = ();

    fn visit_subrange(&mut self, node: &Subrange) -> Result<(), SemanticDiagnostic> {
        // TODO return error if try fails
        let minimum: i128 = node.start.clone().try_into().unwrap();
        let maximum: i128 = node.end.clone().try_into().unwrap();

        if minimum >= maximum {
            return Err(SemanticDiagnostic::error(
                "S0004",
                format!(
                    "Subrange declaration minimum value {} is not less than the maximum {}",
                    node.start, node.end
                ),
            )
            .with_label(&node.start.value.position, "Expected smaller value")
            .with_label(&node.end.value.position, "Expected greater value"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::stages::parse;

    #[test]
    fn apply_when_subrange_valid_then_ok() {
        let program = "
TYPE
    VALID_RANGE : INT(-10..10);
END_TYPE";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_subrange_invalid_then_error() {
        let program = "
TYPE
    INVALID_RANGE : INT(10..-10);
END_TYPE";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_err());
    }
}
