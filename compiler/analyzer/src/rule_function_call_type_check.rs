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
//!     x : DINT;
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
fn are_types_compatible(expected: &TypeName, actual: &TypeName, options: &CompilerOptions) -> bool {
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
            // Bare integer literals (ANY_INT) to ANY_BIT types (BYTE, WORD, etc.)
            // requires --allow-cross-family-widening. See ADR-0031.
            if options.allow_cross_family_widening
                && generic == GenericTypeName::AnyInt
                && matches!(
                    elementary,
                    ElementaryTypeName::BYTE
                        | ElementaryTypeName::WORD
                        | ElementaryTypeName::DWORD
                        | ElementaryTypeName::LWORD
                )
            {
                return true;
            }
        }
    }
    // Implicit widening: integer-to-integer, integer-to-real (lossless),
    // bit-string-to-bit-string. See ADR-0029 and ADR-0031.
    if let Ok(actual_elem) = ElementaryTypeName::try_from(&actual.name) {
        if let Ok(expected_elem) = ElementaryTypeName::try_from(&expected.name) {
            if actual_elem.can_widen_to(&expected_elem) {
                return true;
            }
            // Cross-family widening (bit-string → integer) requires flag.
            if options.allow_cross_family_widening
                && actual_elem.can_widen_cross_family_to(&expected_elem)
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
    options: &CompilerOptions,
) -> SemanticResult {
    let mut visitor = RuleFunctionCallTypeCheck {
        context,
        options,
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
    options: &'a CompilerOptions,
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
                            if !are_types_compatible(target_type, return_type, self.options) {
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
                    if !are_types_compatible(&param.param_type, arg_type, self.options) {
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
    use crate::test_helpers::{
        assert_rule_err, assert_rule_ok, parse_and_resolve_types_with_context,
    };
    use rstest::rstest;

    /// Source with a single-param, single-arg identity function. `ret_ty` and
    /// `param_ty` are always the function signature; `result_ty` and `arg_ty`
    /// are the caller-side types; `arg_expr` is the expression passed to the
    /// call (bare identifier `y`, literal, or typed literal like `DINT#5`).
    fn identity_fn_source(
        ret_ty: &str,
        param_ty: &str,
        result_ty: &str,
        arg_ty: Option<&str>,
        arg_expr: &str,
    ) -> String {
        let arg_decl = match arg_ty {
            Some(ty) => format!("y : {ty};"),
            None => String::new(),
        };
        format!(
            "FUNCTION MY_F : {ret_ty} VAR_INPUT x : {param_ty}; END_VAR MY_F := x; END_FUNCTION PROGRAM main VAR result : {result_ty}; {arg_decl} END_VAR result := MY_F({arg_expr}); END_PROGRAM"
        )
    }

    // --- Non-identity shapes kept as individual tests because they vary the
    // function body, arity, or call pattern. ---

    #[test]
    fn apply_when_matching_types_then_ok() {
        assert_rule_ok(
            apply,
            "FUNCTION ADD_INTS : INT VAR_INPUT A : INT; B : INT; END_VAR ADD_INTS := A + B; END_FUNCTION PROGRAM main VAR result : INT; a : INT; b : INT; END_VAR result := ADD_INTS(a, b); END_PROGRAM",
        );
    }

    // Standard library functions (INT_TO_REAL) use ANY_* generic signatures
    // and are skipped by this rule.
    #[test]
    fn apply_when_stdlib_function_then_skipped() {
        assert_rule_ok(
            apply,
            "PROGRAM main VAR result : REAL; x : INT; END_VAR result := INT_TO_REAL(x); END_PROGRAM",
        );
    }

    // Two-parameter function where exactly one argument's type is wrong.
    #[test]
    fn apply_when_multiple_args_one_mismatch_then_one_error() {
        assert_rule_err(
            apply,
            "FUNCTION MY_FUNC : INT VAR_INPUT A : INT; B : SINT; END_VAR MY_FUNC := A; END_FUNCTION PROGRAM main VAR result : INT; x : INT; END_VAR result := MY_FUNC(x, x); END_PROGRAM",
            Problem::FunctionCallArgTypeMismatch.code(),
        );
    }

    // Return type `REAL` vs. declaration target `INT`.
    #[test]
    fn apply_when_return_type_mismatch_then_error() {
        assert_rule_err(
            apply,
            "FUNCTION GET_VALUE : REAL VAR_INPUT A : REAL; END_VAR GET_VALUE := A; END_FUNCTION PROGRAM main VAR result : INT; x : REAL; END_VAR result := GET_VALUE(x); END_PROGRAM",
            Problem::FunctionCallReturnTypeMismatch.code(),
        );
    }

    // Nested function call: DOUBLE(DOUBLE(x)) with INT throughout.
    #[test]
    fn apply_when_nested_function_call_types_match_then_ok() {
        assert_rule_ok(
            apply,
            "FUNCTION DOUBLE : INT VAR_INPUT A : INT; END_VAR DOUBLE := A + A; END_FUNCTION PROGRAM main VAR result : INT; x : INT; END_VAR result := DOUBLE(DOUBLE(x)); END_PROGRAM",
        );
    }

    // 3-parameter function, all arguments matching.
    #[test]
    fn apply_when_all_args_match_then_ok() {
        assert_rule_ok(
            apply,
            "FUNCTION ADD3 : DINT VAR_INPUT A : DINT; B : DINT; C : DINT; END_VAR ADD3 := A + B + C; END_FUNCTION PROGRAM main VAR result : DINT; a : DINT; b : DINT; c : DINT; END_VAR result := ADD3(a, b, c); END_PROGRAM",
        );
    }

    // Simple identity REAL → REAL.
    #[test]
    fn apply_when_return_type_matches_then_ok() {
        assert_rule_ok(
            apply,
            &identity_fn_source("REAL", "REAL", "REAL", Some("REAL"), "y"),
        );
    }

    // --- Identity-shape matrix: single-param function, caller passes one
    // argument expression.  Covers argument-type and return-type checks in
    // both the ok and err directions.  All cases build on `identity_fn_source`. ---

    #[rstest]
    // Bare literal → numeric param.
    #[case::bare_int_to_int("INT", "INT", "INT", None, "5")]
    #[case::bare_int_to_sint("SINT", "SINT", "SINT", None, "5")]
    #[case::bare_real_to_lreal("LREAL", "LREAL", "LREAL", None, "3.14")]
    #[case::bare_int_to_real("REAL", "REAL", "REAL", None, "0")]
    #[case::bare_int_to_lreal("LREAL", "LREAL", "LREAL", None, "42")]
    // Lossless int-literal → REAL via ANY_INT coercion.
    #[case::int_literal_arg_to_real_param_lossless("REAL", "REAL", "REAL", Some("INT"), "x")]
    // Implicit integer widening (ADR-0029).
    #[case::sint_to_int("INT", "INT", "INT", Some("SINT"), "y")]
    #[case::int_to_dint("DINT", "DINT", "DINT", Some("INT"), "y")]
    #[case::sint_to_lint("LINT", "LINT", "LINT", Some("SINT"), "y")]
    #[case::usint_to_uint("UINT", "UINT", "UINT", Some("USINT"), "y")]
    #[case::usint_to_int("INT", "INT", "INT", Some("USINT"), "y")]
    #[case::uint_to_dint("DINT", "DINT", "DINT", Some("UINT"), "y")]
    // Return-type widening: SINT → DINT target.
    #[case::sint_return_to_dint_var("SINT", "SINT", "DINT", Some("SINT"), "y")]
    // Standard widening (ADR-0031).
    #[case::int_to_real("REAL", "REAL", "REAL", Some("INT"), "y")]
    #[case::byte_to_word("WORD", "WORD", "WORD", Some("BYTE"), "y")]
    fn apply_identity_when_compatible_then_ok(
        #[case] ret_ty: &str,
        #[case] param_ty: &str,
        #[case] result_ty: &str,
        #[case] arg_ty: Option<&str>,
        #[case] arg_expr: &str,
    ) {
        assert_rule_ok(
            apply,
            &identity_fn_source(ret_ty, param_ty, result_ty, arg_ty, arg_expr),
        );
    }

    // Same envelope, expected to fail with an arg-type mismatch diagnostic.
    #[rstest]
    // Typed DINT literal passed where INT is required.
    #[case::typed_dint_lit_to_int_param("INT", "INT", "INT", None, "DINT#5")]
    // Widening where the source is strictly larger or a different family.
    #[case::dint_var_to_int_param("INT", "INT", "INT", Some("DINT"), "y")]
    #[case::dint_to_real_lossy("REAL", "REAL", "REAL", Some("DINT"), "y")]
    #[case::word_to_byte("BYTE", "BYTE", "BYTE", Some("WORD"), "y")]
    #[case::real_to_int("INT", "INT", "INT", Some("REAL"), "y")]
    fn apply_identity_when_arg_type_mismatch_then_err(
        #[case] ret_ty: &str,
        #[case] param_ty: &str,
        #[case] result_ty: &str,
        #[case] arg_ty: Option<&str>,
        #[case] arg_expr: &str,
    ) {
        assert_rule_err(
            apply,
            &identity_fn_source(ret_ty, param_ty, result_ty, arg_ty, arg_expr),
            Problem::FunctionCallArgTypeMismatch.code(),
        );
    }

    // Same envelope, expected to fail with a bare is_err() (no code check)
    // for the cases that don't warrant a specific problem-code assertion.
    #[rstest]
    #[case::dint_to_int("INT", "INT", "INT", Some("DINT"), "y")]
    #[case::int_to_uint("UINT", "UINT", "UINT", Some("INT"), "y")]
    #[case::byte_to_int("INT", "INT", "INT", Some("BYTE"), "y")]
    fn apply_identity_when_incompatible_types_then_err(
        #[case] ret_ty: &str,
        #[case] param_ty: &str,
        #[case] result_ty: &str,
        #[case] arg_ty: Option<&str>,
        #[case] arg_expr: &str,
    ) {
        let (library, context) =
            parse_and_resolve_types_with_context(&identity_fn_source(
                ret_ty, param_ty, result_ty, arg_ty, arg_expr,
            ));
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    // --- are_types_compatible unit tests. Pure helper, no AST involvement. ---

    #[rstest]
    // Same-family compatibility cases.
    #[case::exact_match("INT", "INT", true)]
    #[case::any_int_to_int("INT", "ANY_INT", true)]
    #[case::any_int_to_dint("DINT", "ANY_INT", true)]
    #[case::any_real_to_real("REAL", "ANY_REAL", true)]
    #[case::any_real_to_lreal("LREAL", "ANY_REAL", true)]
    #[case::any_int_to_real("REAL", "ANY_INT", true)]
    #[case::any_int_to_lreal("LREAL", "ANY_INT", true)]
    #[case::dint_to_int_false("INT", "DINT", false)]
    // Implicit widening (ADR-0029).
    #[case::int_to_dint_widen("DINT", "INT", true)]
    #[case::usint_to_int_widen("INT", "USINT", true)]
    // Standard widening (ADR-0031).
    #[case::int_to_real_widen("REAL", "INT", true)]
    #[case::dint_to_real_lossy_false("REAL", "DINT", false)]
    #[case::byte_to_word_widen("WORD", "BYTE", true)]
    fn are_types_compatible_cases(
        #[case] expected: &str,
        #[case] actual: &str,
        #[case] want: bool,
    ) {
        let opts = CompilerOptions::default();
        assert_eq!(
            are_types_compatible(&TypeName::from(expected), &TypeName::from(actual), &opts),
            want,
        );
    }

    // --- Cross-family widening gated by `allow_cross_family_widening`. ---

    fn cross_family_opts() -> CompilerOptions {
        CompilerOptions {
            allow_cross_family_widening: true,
            ..CompilerOptions::default()
        }
    }

    // Runs `rule` on an identity-fn source with `cross_family_opts` and returns
    // the result so the caller can assert ok / err.
    fn apply_identity_with_cross_family(
        ret_ty: &str,
        param_ty: &str,
        result_ty: &str,
        arg_ty: Option<&str>,
        arg_expr: &str,
    ) -> SemanticResult {
        let source = identity_fn_source(ret_ty, param_ty, result_ty, arg_ty, arg_expr);
        let (library, context) = parse_and_resolve_types_with_context(&source);
        apply(&library, &context, &cross_family_opts())
    }

    // BYTE → INT arg: ok with flag, err without.
    #[test]
    fn apply_when_byte_arg_to_int_param_with_flag_then_ok() {
        assert!(apply_identity_with_cross_family("INT", "INT", "INT", Some("BYTE"), "y").is_ok());
    }

    // Literal 0 → BYTE arg: ok with flag, err without.
    #[test]
    fn apply_when_literal_zero_to_byte_param_with_flag_then_ok() {
        assert!(apply_identity_with_cross_family("BYTE", "BYTE", "BYTE", None, "0").is_ok());
    }

    #[test]
    fn apply_when_literal_zero_to_byte_param_without_flag_then_error() {
        let (library, context) = parse_and_resolve_types_with_context(&identity_fn_source(
            "BYTE", "BYTE", "BYTE", None, "0",
        ));
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    // BYTE return → INT target var: ok with flag, err without.
    #[test]
    fn apply_when_byte_return_to_int_var_with_flag_then_ok() {
        assert!(apply_identity_with_cross_family("BYTE", "BYTE", "INT", Some("BYTE"), "y").is_ok());
    }

    #[test]
    fn apply_when_byte_return_to_int_var_without_flag_then_error() {
        let (library, context) = parse_and_resolve_types_with_context(&identity_fn_source(
            "BYTE", "BYTE", "INT", Some("BYTE"), "y",
        ));
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    // Integer → bit-string is never allowed, even with the flag.
    #[test]
    fn apply_when_int_arg_to_byte_param_with_flag_then_error() {
        assert!(apply_identity_with_cross_family("BYTE", "BYTE", "BYTE", Some("INT"), "y").is_err());
    }
}
