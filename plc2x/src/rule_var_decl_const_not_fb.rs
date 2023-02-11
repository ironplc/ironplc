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
use ironplc_dsl::{common::*, core::SourcePosition, visitor::Visitor};

use crate::error::SemanticDiagnostic;

pub fn apply(lib: &Library) -> Result<(), SemanticDiagnostic> {
    let mut visitor = RuleVarDeclConstIsNotFunctionBlock {};
    visitor.walk(lib)
}

struct RuleVarDeclConstIsNotFunctionBlock {}

impl Visitor<SemanticDiagnostic> for RuleVarDeclConstIsNotFunctionBlock {
    type Value = ();

    fn visit_variable_declaration(&mut self, node: &VarDecl) -> Result<(), SemanticDiagnostic> {
        if node.qualifier == DeclarationQualifier::Constant {
            if let InitialValueAssignmentKind::FunctionBlock(fb) = &node.initializer {
                return Err(SemanticDiagnostic::error(
                    "S0001",
                    format!(
                        "CONSTANT qualifier is not permitted for function block instance type {}",
                        fb.type_name
                    ),
                )
                .with_label(
                    node.name.position(),
                    "Declaration of function block instance",
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::stages::parse;

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

        let library = parse(program).unwrap();
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

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_ok())
    }
}
