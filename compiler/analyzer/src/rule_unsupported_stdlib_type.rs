//! Semantic rule that checks for references to standard library types that are
//! recognized but not yet implemented in the compiler.
//!
//! This gives a nicer error message than "unknown type" when the problem is
//! that the compiler doesn't support a particular stdlib type variant yet.
//!
//! Note: Many standard library function blocks ARE supported (TON, TOF, TP, etc.).
//! This rule only flags the types that are known but NOT yet implemented
//! (e.g., counter variants with different integer types like CTU_DINT).
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK FUNC
//!    VAR_INPUT
//!       NAME : CTU_DINT;  // Unsupported variant of CTU
//!    END_VAR
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult, semantic_context::SemanticContext,
    stdlib::is_unsupported_standard_type,
};

pub fn apply(lib: &Library, _context: &SemanticContext) -> SemanticResult {
    let mut visitor = RuleUnsupportedStdLibType {
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleUnsupportedStdLibType {
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<Diagnostic> for RuleUnsupportedStdLibType {
    type Value = ();

    fn visit_function_block_initial_value_assignment(
        &mut self,
        node: &FunctionBlockInitialValueAssignment,
    ) -> Result<(), Diagnostic> {
        if is_unsupported_standard_type(&node.type_name) {
            self.diagnostics.push(Diagnostic::problem(
                Problem::UnsupportedStdLibType,
                Label::span(node.type_name.span(), "Unsupported variable type name"),
            ));
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::semantic_context::SemanticContextBuilder;
    use crate::test_helpers::parse_and_resolve_types;

    #[test]
    fn apply_when_has_ctu_dint_supported_type_then_ok() {
        // CTU_DINT is now a supported stdlib type variant
        let program = "
FUNCTION_BLOCK DUMMY
VAR_INPUT
counter : CTU_DINT;
END_VAR

END_FUNCTION_BLOCK";

        let input = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&input, &context);

        // CTU_DINT is now supported, so this should pass (no unsupported stdlib type error)
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_has_ton_supported_type_then_ok() {
        // TON is a supported stdlib type - should not trigger this rule
        let program = "
FUNCTION_BLOCK DUMMY
VAR_INPUT
timer : TON;
END_VAR

END_FUNCTION_BLOCK";

        let input = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&input, &context);

        // TON is now supported, so this should pass (no unsupported stdlib type error)
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_has_user_defined_function_block_then_ok() {
        // User-defined function blocks should not trigger this rule
        let program = "
FUNCTION_BLOCK MY_CUSTOM_FB
VAR_INPUT
value : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK DUMMY
VAR_INPUT
my_var : MY_CUSTOM_FB;
END_VAR
END_FUNCTION_BLOCK";

        let input = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&input, &context);

        // User-defined function blocks are not stdlib types, so this should pass
        assert!(result.is_ok());
    }
}
