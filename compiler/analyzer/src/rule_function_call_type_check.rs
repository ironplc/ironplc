//! Semantic rule that validates function call argument types match parameter types
//! and return types match assignment destinations.
//!
//! This rule only checks user-defined functions. Standard library functions are
//! skipped because they use ANY_* generic types which require different handling.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION ADD_INTS : INT
//! VAR_INPUT
//!     A : INT;
//!     B : INT;
//! END_VAR
//!     ADD_INTS := A + B;
//! END_FUNCTION
//!
//! PROGRAM main
//! VAR
//!     result : INT;
//! END_VAR
//!     result := ADD_INTS(1, 2);
//! END_PROGRAM
//! ```
//!
//! ## Fails (Argument Type Mismatch)
//!
//! ```ignore
//! FUNCTION ADD_REALS : REAL
//! VAR_INPUT
//!     A : REAL;
//! END_VAR
//!     ADD_REALS := A;
//! END_FUNCTION
//!
//! PROGRAM main
//! VAR
//!     result : REAL;
//!     x : INT;
//! END_VAR
//!     result := ADD_REALS(x);
//! END_PROGRAM
//! ```

use std::collections::HashMap;

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{result::SemanticResult, semantic_context::SemanticContext};
use ironplc_parser::options::CompilerOptions;

/// Returns true if `actual` is type-compatible with `expected`.
///
/// Exact matches always pass. If `actual` is a generic type (ANY_INT,
/// ANY_REAL, etc.) and `expected` is a concrete elementary type, delegates
/// to `GenericTypeName::is_compatible_with`.
///
/// Bare integer literals (ANY_INT) are also accepted where REAL or LREAL
/// is expected. This is type inference for untyped literals, not implicit
/// widening of typed expressions (see ADR-0028).
fn are_types_compatible(expected: &TypeName, actual: &TypeName) -> bool {
    if *expected == *actual {
        return true;
    }
    if let Ok(generic) = GenericTypeName::try_from(&actual.name) {
        if let Ok(elementary) = ElementaryTypeName::try_from(&expected.name) {
            if generic.is_compatible_with(&elementary) {
                return true;
            }
            // Bare integer literals (ANY_INT) can be inferred as REAL/LREAL.
            // See ADR-0028 for rationale.
            if generic == GenericTypeName::AnyInt
                && matches!(
                    elementary,
                    ElementaryTypeName::REAL | ElementaryTypeName::LREAL
                )
            {
                return true;
            }
        }
    }
    false
}

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let mut visitor = RuleFunctionCallTypeCheck {
        context,
        diagnostics: vec![],
        var_types: HashMap::new(),
    };

    visitor.walk(lib).map_err(|e| vec![e])?;

    if visitor.diagnostics.is_empty() {
        Ok(())
    } else {
        Err(visitor.diagnostics)
    }
}

struct RuleFunctionCallTypeCheck<'a> {
    context: &'a SemanticContext,
    diagnostics: Vec<Diagnostic>,
    /// Maps variable name to declared type for the current scope.
    var_types: HashMap<Id, TypeName>,
}

impl RuleFunctionCallTypeCheck<'_> {
    /// Checks whether a function call expression assigned to a variable has a
    /// matching return type. Emits P4027 if there is a mismatch.
    fn check_return_type(&mut self, target: &Variable, value: &Expr) {
        if let ExprKind::Function(ref func_call) = value.kind {
            if let Some(signature) = self.context.functions.get(&func_call.name) {
                if signature.is_stdlib() {
                    return;
                }
                if let Variable::Symbolic(SymbolicVariableKind::Named(ref nv)) = target {
                    if let Some(target_type) = self.var_types.get(&nv.name) {
                        if let Some(ref return_type) = value.resolved_type {
                            if !are_types_compatible(target_type, return_type) {
                                self.diagnostics.push(
                                    Diagnostic::problem(
                                        Problem::FunctionCallReturnTypeMismatch,
                                        Label::span(
                                            func_call.name.span(),
                                            "Function call return type",
                                        ),
                                    )
                                    .with_context(
                                        "function",
                                        &func_call.name.original().to_string(),
                                    )
                                    .with_context("return_type", &return_type.to_string())
                                    .with_context("target_type", &target_type.to_string()),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Visitor<Diagnostic> for RuleFunctionCallTypeCheck<'_> {
    type Value = ();

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.var_types.clear();
        node.recurse_visit(self)
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.var_types.clear();
        node.recurse_visit(self)
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.var_types.clear();
        node.recurse_visit(self)
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        if let VariableIdentifier::Symbol(ref id) = node.identifier {
            if let TypeReference::Named(ref type_name) = node.type_name() {
                self.var_types.insert(id.clone(), type_name.clone());
            }
        }
        node.recurse_visit(self)
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<Self::Value, Diagnostic> {
        self.check_return_type(&node.target, &node.value);
        node.recurse_visit(self)
    }

    fn visit_function(&mut self, node: &Function) -> Result<Self::Value, Diagnostic> {
        let func_sig = self.context.functions.get(&node.name);

        if let Some(signature) = func_sig {
            // Skip stdlib functions — they use ANY_* types
            if signature.is_stdlib() {
                return node.recurse_visit(self);
            }

            // Check each positional argument type against the parameter type
            let input_params: Vec<_> = signature.parameters.iter().filter(|p| p.is_input).collect();

            // Emit NotImplemented for output arguments on user-defined functions.
            for p in &node.param_assignment {
                if let ParamAssignmentKind::Output(_) = p {
                    self.diagnostics.push(Diagnostic::problem(
                        Problem::NotImplemented,
                        Label::span(node.name.span(), "Function call with output argument"),
                    ));
                }
            }

            let positional_args: Vec<_> = node
                .param_assignment
                .iter()
                .filter_map(|p| match p {
                    ParamAssignmentKind::PositionalInput(pos) => Some(&pos.expr),
                    // NamedInput is already converted to PositionalInput by
                    // xform_named_to_positional_args; Output is handled above.
                    _ => None,
                })
                .collect();

            for (i, arg_expr) in positional_args.iter().enumerate() {
                if i >= input_params.len() {
                    break;
                }
                let param = &input_params[i];

                if let Some(ref arg_type) = arg_expr.resolved_type {
                    if !are_types_compatible(&param.param_type, arg_type) {
                        self.diagnostics.push(
                            Diagnostic::problem(
                                Problem::FunctionCallArgTypeMismatch,
                                Label::span(node.name.span(), "Function call"),
                            )
                            .with_context("function", &node.name.original().to_string())
                            .with_context("parameter", &param.name.original().to_string())
                            .with_context("expected", &param.param_type.to_string())
                            .with_context("actual", &arg_type.to_string()),
                        );
                    }
                }
            }
        }

        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::parse_and_resolve_types_with_context;

    #[test]
    fn apply_when_matching_types_then_ok() {
        let program = "
FUNCTION ADD_INTS : INT
VAR_INPUT
    A : INT;
    B : INT;
END_VAR
    ADD_INTS := A + B;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := ADD_INTS(a, b);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_arg_type_mismatch_then_error() {
        let program = "
FUNCTION DOUBLE_REAL : REAL
VAR_INPUT
    A : REAL;
END_VAR
    DOUBLE_REAL := A;
END_FUNCTION

PROGRAM main
VAR
    result : REAL;
    x : INT;
END_VAR
    result := DOUBLE_REAL(x);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallArgTypeMismatch.code()
        );
    }

    #[test]
    fn apply_when_stdlib_function_then_skipped() {
        let program = "
PROGRAM main
VAR
    result : REAL;
    x : INT;
END_VAR
    result := INT_TO_REAL(x);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_multiple_args_one_mismatch_then_one_error() {
        let program = "
FUNCTION MY_FUNC : INT
VAR_INPUT
    A : INT;
    B : DINT;
END_VAR
    MY_FUNC := A;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    x : INT;
END_VAR
    result := MY_FUNC(x, x);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallArgTypeMismatch.code()
        );
    }

    #[test]
    fn apply_when_return_type_mismatch_then_error() {
        let program = "
FUNCTION GET_VALUE : REAL
VAR_INPUT
    A : REAL;
END_VAR
    GET_VALUE := A;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    x : REAL;
END_VAR
    result := GET_VALUE(x);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallReturnTypeMismatch.code()
        );
    }

    #[test]
    fn apply_when_nested_function_call_types_match_then_ok() {
        let program = "
FUNCTION DOUBLE : INT
VAR_INPUT
    A : INT;
END_VAR
    DOUBLE := A + A;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    x : INT;
END_VAR
    result := DOUBLE(DOUBLE(x));
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_all_args_match_then_ok() {
        let program = "
FUNCTION ADD3 : DINT
VAR_INPUT
    A : DINT;
    B : DINT;
    C : DINT;
END_VAR
    ADD3 := A + B + C;
END_FUNCTION

PROGRAM main
VAR
    result : DINT;
    a : DINT;
    b : DINT;
    c : DINT;
END_VAR
    result := ADD3(a, b, c);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_return_type_matches_then_ok() {
        let program = "
FUNCTION GET_REAL : REAL
VAR_INPUT
    A : REAL;
END_VAR
    GET_REAL := A;
END_FUNCTION

PROGRAM main
VAR
    result : REAL;
    x : REAL;
END_VAR
    result := GET_REAL(x);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_bare_literal_arg_to_int_param_then_ok() {
        let program = "
FUNCTION ADD_ONE : INT
VAR_INPUT
    x : INT;
END_VAR
    ADD_ONE := x + 1;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
END_VAR
    result := ADD_ONE(5);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_bare_literal_arg_to_sint_param_then_ok() {
        let program = "
FUNCTION INC : SINT
VAR_INPUT
    x : SINT;
END_VAR
    INC := x;
END_FUNCTION

PROGRAM main
VAR
    result : SINT;
END_VAR
    result := INC(5);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_bare_real_literal_arg_to_lreal_param_then_ok() {
        let program = "
FUNCTION DBL : LREAL
VAR_INPUT
    x : LREAL;
END_VAR
    DBL := x;
END_FUNCTION

PROGRAM main
VAR
    result : LREAL;
END_VAR
    result := DBL(3.14);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_typed_dint_literal_arg_to_int_param_then_error() {
        let program = "
FUNCTION ADD_ONE : INT
VAR_INPUT
    x : INT;
END_VAR
    ADD_ONE := x;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
END_VAR
    result := ADD_ONE(DINT#5);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallArgTypeMismatch.code()
        );
    }

    #[test]
    fn apply_when_dint_var_arg_to_int_param_then_error() {
        let program = "
FUNCTION ADD_ONE : INT
VAR_INPUT
    x : INT;
END_VAR
    ADD_ONE := x;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    y : DINT;
END_VAR
    result := ADD_ONE(y);
END_PROGRAM";

        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallArgTypeMismatch.code()
        );
    }

    #[test]
    fn are_types_compatible_when_exact_match_then_true() {
        assert!(are_types_compatible(
            &TypeName::from("INT"),
            &TypeName::from("INT")
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_to_int_then_true() {
        assert!(are_types_compatible(
            &TypeName::from("INT"),
            &TypeName::from("ANY_INT")
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_to_dint_then_true() {
        assert!(are_types_compatible(
            &TypeName::from("DINT"),
            &TypeName::from("ANY_INT")
        ));
    }

    #[test]
    fn are_types_compatible_when_any_real_to_real_then_true() {
        assert!(are_types_compatible(
            &TypeName::from("REAL"),
            &TypeName::from("ANY_REAL")
        ));
    }

    #[test]
    fn are_types_compatible_when_any_real_to_lreal_then_true() {
        assert!(are_types_compatible(
            &TypeName::from("LREAL"),
            &TypeName::from("ANY_REAL")
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_to_real_then_true() {
        assert!(are_types_compatible(
            &TypeName::from("REAL"),
            &TypeName::from("ANY_INT")
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_to_lreal_then_true() {
        assert!(are_types_compatible(
            &TypeName::from("LREAL"),
            &TypeName::from("ANY_INT")
        ));
    }

    #[test]
    fn are_types_compatible_when_dint_to_int_then_false() {
        assert!(!are_types_compatible(
            &TypeName::from("INT"),
            &TypeName::from("DINT")
        ));
    }

    #[test]
    fn apply_when_bare_int_literal_arg_to_real_param_then_ok() {
        let program = "
FUNCTION TAKES_REAL : REAL
VAR_INPUT
    x : REAL;
END_VAR
    TAKES_REAL := x;
END_FUNCTION

PROGRAM main
VAR
    result : REAL;
END_VAR
    result := TAKES_REAL(0);
END_PROGRAM
";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_bare_int_literal_arg_to_lreal_param_then_ok() {
        let program = "
FUNCTION TAKES_LREAL : LREAL
VAR_INPUT
    x : LREAL;
END_VAR
    TAKES_LREAL := x;
END_FUNCTION

PROGRAM main
VAR
    result : LREAL;
END_VAR
    result := TAKES_LREAL(42);
END_PROGRAM
";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }
}
