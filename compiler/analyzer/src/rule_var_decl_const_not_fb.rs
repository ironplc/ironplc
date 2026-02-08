//! Semantic rule that function block instances cannot be
//! declared to be `CONSTANT`
//!
//! See section 2.4.3.
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION_BLOCK Callee
//!    VAR
//!    END_VAR
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK Caller
//!    VAR CONSTANT
//!       FB_INSTANCE : Callee;
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

use crate::{result::SemanticResult, semantic_context::SemanticContext};

pub fn apply(lib: &Library, _context: &SemanticContext) -> SemanticResult {
    let mut visitor = RuleVarDeclConstIsNotFunctionBlock {
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleVarDeclConstIsNotFunctionBlock {
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<Diagnostic> for RuleVarDeclConstIsNotFunctionBlock {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Diagnostic> {
        if node.qualifier == DeclarationQualifier::Constant {
            if let InitialValueAssignmentKind::FunctionBlock(fb) = &node.initializer {
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::FunctionBlockNotConstant,
                        Label::span(
                            node.identifier.span(),
                            "Declaration of function block instance",
                        ),
                    )
                    .with_context_type("function block", &fb.type_name),
                );
            }
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
    fn apply_when_var_init_function_block_is_const_then_error() {
        let program = "
FUNCTION_BLOCK Callee

END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR CONSTANT
FB_INSTANCE : Callee;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

        assert!(result.is_err())
    }
    #[test]
    fn apply_when_var_init_function_block_not_const_then_error() {
        let program = "
FUNCTION_BLOCK Callee

END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR

END_FUNCTION_BLOCK";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);

        assert!(result.is_ok())
    }
}
