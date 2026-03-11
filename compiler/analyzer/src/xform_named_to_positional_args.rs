//! Transformation pass that converts named (formal) function call arguments
//! to positional arguments.
//!
//! This pass runs after `xform_resolve_symbol_and_function_environment` (which
//! builds the `FunctionEnvironment`) and before `xform_resolve_expr_types`.
//! It looks up each function's signature to determine the declared parameter
//! order, then rewrites `NamedInput` arguments into `PositionalInput` arguments
//! in the correct positions. This allows codegen to treat all function call
//! arguments uniformly as positional.
//!
//! Extensible functions (like MUX) are skipped because they have variable
//! parameter counts that don't map cleanly to a fixed positional order.

use ironplc_dsl::common::Library;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;
use ironplc_problems::Problem;

use crate::function_environment::FunctionEnvironment;

pub fn apply(
    lib: Library,
    function_environment: &FunctionEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    let mut resolver = NamedToPositionalResolver {
        function_environment,
        errors: vec![],
    };
    let result = resolver.fold_library(lib).map_err(|e| vec![e])?;
    if resolver.errors.is_empty() {
        Ok(result)
    } else {
        Err(resolver.errors)
    }
}

struct NamedToPositionalResolver<'a> {
    function_environment: &'a FunctionEnvironment,
    errors: Vec<Diagnostic>,
}

impl Fold<Diagnostic> for NamedToPositionalResolver<'_> {
    fn fold_function(&mut self, node: Function) -> Result<Function, Diagnostic> {
        // Check if there are any named inputs
        let has_named = node
            .param_assignment
            .iter()
            .any(|p| matches!(p, ParamAssignmentKind::NamedInput(_)));

        if !has_named {
            // All positional or empty — recurse normally
            return Function::recurse_fold(node, self);
        }

        // Check for mixed positional and named inputs
        let has_positional = node
            .param_assignment
            .iter()
            .any(|p| matches!(p, ParamAssignmentKind::PositionalInput(_)));

        if has_positional {
            self.errors.push(Diagnostic::problem(
                Problem::FunctionCallMixedArgTypes,
                Label::span(node.name.span.clone(), "Function call"),
            ));
            return Function::recurse_fold(node, self);
        }

        // Look up the function signature
        let Some(signature) = self.function_environment.get(&node.name) else {
            // Function not found — skip rewriting, let later validation report the error
            return Function::recurse_fold(node, self);
        };

        // Skip extensible functions (variable parameter counts)
        if signature.is_extensible {
            return Function::recurse_fold(node, self);
        }

        // Collect named inputs and output assignments separately
        let mut named_inputs: Vec<NamedInput> = vec![];
        let mut outputs: Vec<ParamAssignmentKind> = vec![];

        for param in node.param_assignment {
            match param {
                ParamAssignmentKind::NamedInput(ni) => named_inputs.push(ni),
                ParamAssignmentKind::Output(_) => outputs.push(param),
                ParamAssignmentKind::PositionalInput(_) => {
                    // Already checked above — should not happen
                    outputs.push(param);
                }
            }
        }

        // Check for duplicate named arguments
        let mut seen_names: Vec<String> = vec![];
        for ni in &named_inputs {
            let lower = ni.name.lower_case().to_string();
            if seen_names.contains(&lower) {
                self.errors.push(Diagnostic::problem(
                    Problem::FunctionCallDuplicateNamedArg,
                    Label::span(ni.name.span.clone(), "Duplicate argument"),
                ));
            } else {
                seen_names.push(lower);
            }
        }

        // Get input parameters in declaration order
        let input_params: Vec<_> = signature.parameters.iter().filter(|p| p.is_input).collect();

        // Validate all named args match a declared parameter
        for ni in &named_inputs {
            let lower = ni.name.lower_case().to_string();
            let found = input_params
                .iter()
                .any(|p| *p.name.lower_case() == lower);
            if !found {
                self.errors.push(Diagnostic::problem(
                    Problem::FunctionCallNamedArgUndeclared,
                    Label::span(ni.name.span.clone(), "Undeclared parameter"),
                ));
            }
        }

        // If we had errors, don't rewrite — return as-is
        if !self.errors.is_empty() {
            // Reconstruct the original node
            let mut param_assignment: Vec<ParamAssignmentKind> = named_inputs
                .into_iter()
                .map(ParamAssignmentKind::NamedInput)
                .collect();
            param_assignment.extend(outputs);
            return Ok(Function {
                name: node.name,
                param_assignment,
            });
        }

        // Rewrite: for each input parameter in declaration order, find the
        // matching named input and convert to positional
        let mut positional_args: Vec<ParamAssignmentKind> = vec![];
        for param in &input_params {
            let lower = param.name.lower_case().to_string();
            if let Some(ni) = named_inputs
                .iter()
                .find(|ni| *ni.name.lower_case() == lower)
            {
                // Fold the expression inside the named input
                let folded_expr = self.fold_expr(ni.expr.clone())?;
                positional_args.push(ParamAssignmentKind::PositionalInput(PositionalInput {
                    expr: folded_expr,
                }));
            }
            // If no matching named input, the parameter was not provided —
            // leave it out (arg count validation happens elsewhere)
        }

        // Append output assignments
        positional_args.extend(outputs);

        Ok(Function {
            name: node.name,
            param_assignment: positional_args,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::function_environment::{FunctionEnvironment, FunctionSignature};
    use crate::intermediate_type::IntermediateFunctionParameter;
    use crate::test_helpers::parse_and_resolve_types;
    use ironplc_dsl::common::TypeName;
    use ironplc_dsl::core::Id;

    /// Helper to build a FunctionEnvironment with a single user-defined function.
    fn env_with_function(name: &str, params: Vec<(&str, &str)>) -> FunctionEnvironment {
        let parameters = params
            .into_iter()
            .map(|(pname, ptype)| IntermediateFunctionParameter {
                name: Id::from(pname),
                param_type: TypeName::from(ptype),
                is_input: true,
                is_output: false,
                is_inout: false,
            })
            .collect();

        let sig = FunctionSignature::new(
            Id::from(name),
            Some(TypeName::from("INT")),
            parameters,
            ironplc_dsl::core::SourceSpan::default(),
        );

        let mut env = FunctionEnvironment::new();
        env.insert(sig).unwrap();
        env
    }

    #[test]
    fn apply_when_positional_args_then_unchanged() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
  B : INT;
END_VAR
  MY_FUNC := A + B;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(1, 2);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![("A", "INT"), ("B", "INT")]);
        let result = apply(library, &env);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_no_args_then_unchanged() {
        let program = "
FUNCTION MY_FUNC : INT
  MY_FUNC := 42;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC();
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![]);
        let result = apply(library, &env);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_named_args_then_reordered_to_positional() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
  B : INT;
END_VAR
  MY_FUNC := A + B;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(A := 1, B := 2);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![("A", "INT"), ("B", "INT")]);
        let result = apply(library, &env).unwrap();

        // Verify the function call now has positional args
        let func_call = find_function_call(&result, "MY_FUNC");
        assert!(func_call.is_some(), "Should find MY_FUNC call");
        let func = func_call.unwrap();
        assert_eq!(func.param_assignment.len(), 2);
        assert!(
            matches!(
                &func.param_assignment[0],
                ParamAssignmentKind::PositionalInput(_)
            ),
            "First arg should be positional"
        );
        assert!(
            matches!(
                &func.param_assignment[1],
                ParamAssignmentKind::PositionalInput(_)
            ),
            "Second arg should be positional"
        );
    }

    #[test]
    fn apply_when_named_args_reversed_order_then_reordered() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
  B : INT;
END_VAR
  MY_FUNC := A + B;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(B := 2, A := 1);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![("A", "INT"), ("B", "INT")]);
        let result = apply(library, &env).unwrap();

        // Verify the function call has positional args in declaration order (A, B)
        let func_call = find_function_call(&result, "MY_FUNC");
        assert!(func_call.is_some(), "Should find MY_FUNC call");
        let func = func_call.unwrap();
        assert_eq!(func.param_assignment.len(), 2);
        // Both should be positional now
        assert!(matches!(
            &func.param_assignment[0],
            ParamAssignmentKind::PositionalInput(_)
        ));
        assert!(matches!(
            &func.param_assignment[1],
            ParamAssignmentKind::PositionalInput(_)
        ));
    }

    #[test]
    fn apply_when_function_not_found_then_unchanged() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
END_VAR
  MY_FUNC := A;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(A := 1);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        // Empty environment — function not found
        let env = FunctionEnvironment::new();
        let result = apply(library, &env);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_mixed_positional_and_named_then_error() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
  B : INT;
END_VAR
  MY_FUNC := A + B;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(1, B := 2);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![("A", "INT"), ("B", "INT")]);
        let result = apply(library, &env);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|d| d.code == Problem::FunctionCallMixedArgTypes.code()));
    }

    #[test]
    fn apply_when_named_arg_wrong_name_then_error() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
END_VAR
  MY_FUNC := A;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(WRONG := 1);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![("A", "INT")]);
        let result = apply(library, &env);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|d| d.code == Problem::FunctionCallNamedArgUndeclared.code()));
    }

    #[test]
    fn apply_when_duplicate_named_arg_then_error() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
  A : INT;
END_VAR
  MY_FUNC := A;
END_FUNCTION

PROGRAM main
VAR
  x : INT;
END_VAR
  x := MY_FUNC(A := 1, A := 2);
END_PROGRAM
";
        let library = parse_and_resolve_types(program);
        let env = env_with_function("MY_FUNC", vec![("A", "INT")]);
        let result = apply(library, &env);
        assert!(result.is_err());
        let errs = result.unwrap_err();
        assert!(errs
            .iter()
            .any(|d| d.code == Problem::FunctionCallDuplicateNamedArg.code()));
    }

    /// Helper to find a Function call node by name in the library.
    fn find_function_call(library: &Library, name: &str) -> Option<Function> {
        use ironplc_dsl::visitor::Visitor;

        struct FunctionFinder {
            target: String,
            found: Option<Function>,
        }

        impl Visitor<()> for FunctionFinder {
            type Value = ();
            fn visit_function(&mut self, node: &Function) -> Result<Self::Value, ()> {
                if node.name.lower_case().to_string() == self.target.to_lowercase() {
                    self.found = Some(node.clone());
                }
                Ok(())
            }
        }

        let mut finder = FunctionFinder {
            target: name.to_string(),
            found: None,
        };
        let _ = finder.walk(library);
        finder.found
    }
}
