//! Semantic rule that validates the arguments a function call passes to
//! `VAR_IN_OUT` parameters.
//!
//! A `VAR_IN_OUT` parameter is passed by reference: the function reads and
//! writes the caller's variable. So the argument must be a variable
//! (P4057), and its type must be the parameter's type exactly (P4058). An
//! implicit conversion that is fine for a `VAR_INPUT` is not fine here: the
//! function writes values of the parameter's type into the caller's
//! variable, so a `DINT` parameter bound to an `INT` variable could store a
//! value the `INT` cannot hold.
//!
//! Argument types for `VAR_INPUT` parameters are checked by
//! `rule_function_call_type_check` (P4026), which skips `VAR_IN_OUT`
//! parameters so a mismatch is reported once.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION INC : DINT
//! VAR_IN_OUT
//!     data : DINT;
//! END_VAR
//!     data := data + 1;
//!     INC := data;
//! END_FUNCTION
//!
//! PROGRAM main
//! VAR
//!     x : DINT;
//!     result : DINT;
//! END_VAR
//!     result := INC(x);
//! END_PROGRAM
//! ```
//!
//! ## Fails (Argument Not a Variable)
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     result : DINT;
//! END_VAR
//!     result := INC(1);
//! END_PROGRAM
//! ```
//!
//! ## Fails (Argument Type Not the Parameter Type)
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     x : INT;
//!     result : DINT;
//! END_VAR
//!     result := INC(x);
//! END_PROGRAM
//! ```

use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    type_compat::is_checkable_type,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleFunctionCallInOutArgument {
            context,
            diagnostics: vec![],
        },
        lib,
    )
}

struct RuleFunctionCallInOutArgument<'a> {
    context: &'a SemanticContext,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleFunctionCallInOutArgument<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleFunctionCallInOutArgument<'_> {
    /// Resolves a type name through aliases and subranges to its
    /// elementary type, or `None` when it is not an elementary type.
    fn elementary(&self, type_name: &TypeName) -> Option<TypeName> {
        let resolved = self
            .context
            .types()
            .resolve_elementary_type_name(type_name)
            .unwrap_or_else(|| type_name.clone());
        is_checkable_type(&resolved).then_some(resolved)
    }
}

impl Visitor<Infallible> for RuleFunctionCallInOutArgument<'_> {
    type Value = ();

    fn visit_function(&mut self, node: &Function) -> Result<Self::Value, Infallible> {
        if let Some(signature) = self.context.functions.get(&node.name) {
            for (param, arg) in signature.bind_inputs(&node.param_assignment) {
                if !param.is_inout {
                    continue;
                }

                if !matches!(arg.kind, ExprKind::Variable(_) | ExprKind::LateBound(_)) {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::FunctionCallInOutArgNotVariable,
                            Label::span(arg.span(), "Argument"),
                        )
                        .with_context("function", &node.name.original().to_string())
                        .with_context("parameter", &param.name.original().to_string()),
                    );
                    continue;
                }

                // A REF_TO parameter's type is its target's; leave it to the
                // reference rules. Non-elementary types are not compared.
                if param.is_reference {
                    continue;
                }
                let Some(arg_type) = &arg.resolved_type else {
                    continue;
                };
                let (Some(expected), Some(actual)) = (
                    self.elementary(&param.param_type),
                    self.elementary(arg_type),
                ) else {
                    continue;
                };
                if expected != actual {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::FunctionCallInOutArgTypeMismatch,
                            Label::span(arg.span(), "Argument"),
                        )
                        .with_context("function", &node.name.original().to_string())
                        .with_context("parameter", &param.name.original().to_string())
                        .with_context("expected", &expected.to_string())
                        .with_context("actual", &actual.to_string()),
                    );
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
    use rstest::rstest;

    const INC: &str = "
TYPE MyDint : DINT := 0; END_TYPE

FUNCTION INC : DINT
VAR_INPUT
    step : DINT;
END_VAR
VAR_IN_OUT
    data : DINT;
END_VAR
    data := data + step;
    INC := data;
END_FUNCTION
";

    fn apply_to_call(vars: &str, call: &str) -> SemanticResult {
        let program = format!(
            "{INC}
PROGRAM main
VAR
    {vars}
    result : DINT;
END_VAR
    result := {call};
END_PROGRAM"
        );
        let (library, context) = parse_and_resolve_types_with_context(&program);
        apply(&library, &context, &CompilerOptions::default())
    }

    #[rstest]
    #[case::positional("x : DINT;", "INC(1, x)")]
    #[case::named("x : DINT;", "INC(data := x, step := 1)")]
    #[case::alias_of_parameter_type("x : MyDint;", "INC(1, x)")]
    #[case::input_widened_but_in_out_exact("x : DINT; s : INT;", "INC(s, x)")]
    fn apply_when_in_out_argument_is_variable_of_parameter_type_then_ok(
        #[case] vars: &str,
        #[case] call: &str,
    ) {
        let result = apply_to_call(vars, call);
        assert!(result.is_ok(), "{result:?}");
    }

    #[rstest]
    #[case::literal("", "INC(1, 2)")]
    #[case::expression("x : DINT;", "INC(1, x + 1)")]
    #[case::named_literal("", "INC(step := 1, data := 2)")]
    fn apply_when_in_out_argument_is_not_variable_then_p4057(
        #[case] vars: &str,
        #[case] call: &str,
    ) {
        let errors = apply_to_call(vars, call).unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(
            errors[0].code,
            Problem::FunctionCallInOutArgNotVariable.code()
        );
        assert!(errors[0].described.contains(&"parameter=data".to_owned()));
    }

    #[rstest]
    #[case::narrower_integer("x : INT;", "int")]
    #[case::other_category("x : REAL;", "real")]
    fn apply_when_in_out_argument_type_differs_then_p4058(
        #[case] vars: &str,
        #[case] actual: &str,
    ) {
        let errors = apply_to_call(vars, "INC(1, x)").unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(
            errors[0].code,
            Problem::FunctionCallInOutArgTypeMismatch.code()
        );
        assert!(
            errors[0].described.contains(&format!("actual={actual}")),
            "{:?}",
            errors[0].described
        );
    }

    rule_ctx_ok!(
        apply_when_input_only_function_with_expression_then_ok,
        "
FUNCTION SQ : DINT
VAR_INPUT
    x : DINT;
END_VAR
    SQ := x * x;
END_FUNCTION

PROGRAM main
VAR
    result : DINT;
END_VAR
    result := SQ(1 + 2);
END_PROGRAM"
    );
}
