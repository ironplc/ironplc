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
    core::SourcePosition,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

pub fn apply(lib: &Library) -> Result<(), Diagnostic> {
    let mut visitor = RuleVarDeclConstIsNotFunctionBlock {};
    visitor.walk(lib)
}

struct RuleVarDeclConstIsNotFunctionBlock {}

impl Visitor<Diagnostic> for RuleVarDeclConstIsNotFunctionBlock {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<(), Diagnostic> {
        if node.qualifier == DeclarationQualifier::Constant {
            if let InitialValueAssignmentKind::FunctionBlock(fb) = &node.initializer {
                return Err(Diagnostic::problem(
                    Problem::FunctionBlockNotConstant,
                    Label::source_loc(
                        node.identifier.position(),
                        "Declaration of function block instance",
                    ),
                )
                .with_context_id("function block", &fb.type_name));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
        let result = apply(&library);

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
        let result = apply(&library);

        assert!(result.is_ok())
    }
}
