//! Semantic rule that validates function calls reference declared functions
//! and have the correct number of arguments.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK CALLER
//! VAR
//!     result : REAL;
//!     value : INT;
//! END_VAR
//!     result := INT_TO_REAL(value);
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails (Function Not Declared)
//!
//! ```ignore
//! FUNCTION_BLOCK CALLER
//! VAR
//!     result : REAL;
//!     value : INT;
//! END_VAR
//!     result := NONEXISTENT_FUNC(value);
//! END_FUNCTION_BLOCK
//! ```

use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{result::SemanticResult, semantic_context::SemanticContext};

pub fn apply(lib: &Library, context: &SemanticContext) -> SemanticResult {
    let mut visitor = RuleFunctionCallDeclared {
        context,
        diagnostics: vec![],
    };

    visitor.walk(lib).map_err(|e| vec![e])?;

    if visitor.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(visitor.diagnostics)
    }
}

struct RuleFunctionCallDeclared<'a> {
    context: &'a SemanticContext,
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<Diagnostic> for RuleFunctionCallDeclared<'_> {
    type Value = ();

    fn visit_function(&mut self, node: &Function) -> Result<Self::Value, Diagnostic> {
        // Look up the function in the function environment
        let func_sig = self.context.functions.get(&node.name);

        match func_sig {
            None => {
                // Function is not declared
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::FunctionCallUndeclared,
                        Label::span(node.name.span(), "Function call"),
                    )
                    .with_context("function", &node.name.original().to_string()),
                );
            }
            Some(signature) => {
                // Function exists, check argument count
                // Count positional input arguments in the call
                let call_input_count = node
                    .param_assignment
                    .iter()
                    .filter(|p| matches!(p, ParamAssignmentKind::PositionalInput(_)))
                    .count();

                // Count named input arguments
                let call_named_count = node
                    .param_assignment
                    .iter()
                    .filter(|p| matches!(p, ParamAssignmentKind::NamedInput(_)))
                    .count();

                // Total input arguments provided
                let total_inputs = call_input_count + call_named_count;

                // Expected input parameter count
                let expected_inputs = signature.input_parameter_count();

                if total_inputs != expected_inputs {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::FunctionCallWrongArgCount,
                            Label::span(node.name.span(), "Function call"),
                        )
                        .with_context("function", &node.name.original().to_string())
                        .with_context("expected", &expected_inputs.to_string())
                        .with_context("actual", &total_inputs.to_string()),
                    );
                }
            }
        }

        // Continue visiting children (arguments may contain nested function calls)
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::parse_and_resolve_types_with_context;

    #[test]
    fn apply_when_stdlib_function_called_then_ok() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : REAL;
    value : INT;
END_VAR
    result := INT_TO_REAL(value);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_user_function_called_then_ok() {
        let program = "
FUNCTION ADD_INTS : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    ADD_INTS := A + B;
END_FUNCTION

FUNCTION_BLOCK CALLER
VAR
    result : INT;
END_VAR
    result := ADD_INTS(1, 2);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_function_not_declared_then_error() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : INT;
END_VAR
    result := NONEXISTENT_FUNC(1);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, Problem::FunctionCallUndeclared.code());
    }

    #[test]
    fn apply_when_wrong_arg_count_then_error() {
        let program = "
FUNCTION_BLOCK CALLER
VAR
    result : REAL;
    value : INT;
END_VAR
    result := INT_TO_REAL(value, value);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallWrongArgCount.code()
        );
    }

    #[test]
    fn apply_when_too_few_args_then_error() {
        let program = "
FUNCTION ADD_INTS : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    ADD_INTS := A + B;
END_FUNCTION

FUNCTION_BLOCK CALLER
VAR
    result : INT;
END_VAR
    result := ADD_INTS(1);
END_FUNCTION_BLOCK";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context);

        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallWrongArgCount.code()
        );
    }
}
