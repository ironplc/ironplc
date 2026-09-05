//! Semantic rule that validates function call argument types match parameter
//! types, function return types match assignment destinations, and assignment
//! statement values match their target variable types.
//!
//! Both user-defined and standard-library function calls are checked. Standard
//! library parameters use the IEC 61131-3 generic categories (ANY_REAL, ANY_NUM,
//! etc.) or concrete types (for the `<SOURCE>_TO_<TARGET>` conversion functions);
//! [`are_types_compatible`] handles both.
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

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    scoped_table::ScopedTable,
    semantic_context::SemanticContext,
    type_compat::{are_types_compatible, is_checkable_type},
};
use ironplc_parser::options::CompilerOptions;
pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleFunctionCallTypeCheck {
            context,
            options,
            diagnostics: vec![],
            var_types: ScopedTable::new(),
        },
        lib,
    )
}

struct RuleFunctionCallTypeCheck<'a> {
    context: &'a SemanticContext,
    options: &'a CompilerOptions,
    diagnostics: Vec<Diagnostic>,
    /// Maps variable name to declared type, scoped.
    ///
    /// Each declaration the traversal enters pushes a frame, so a
    /// method's locals do not outlive the method and a local shadows a
    /// field of the same name only within its own body.
    var_types: ScopedTable<'static, Id, TypeName>,
}

impl DiagnosticVisitor for RuleFunctionCallTypeCheck<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
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
                    if let Some(target_type) = self.var_types.find(&nv.name) {
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

    /// Checks whether the value assigned in an assignment statement is
    /// type-compatible with the target variable. Emits P4035 on a mismatch.
    ///
    /// This complements [`Self::check_return_type`], which handles the case where
    /// the right-hand side is a user-function call. Here we handle every other
    /// right-hand side (arithmetic, variables, literals, stdlib calls) by
    /// comparing the target's declared type against the resolved expression type.
    /// Only simple named targets that resolve to an elementary type are checked;
    /// user-defined targets (enums, structures, arrays, function blocks) are
    /// skipped to avoid false positives.
    fn check_assignment_type(&mut self, target: &Variable, value: &Expr) {
        // Function-call right-hand sides are validated by `check_return_type`.
        if matches!(value.kind, ExprKind::Function(_)) {
            return;
        }

        let Variable::Symbolic(SymbolicVariableKind::Named(nv)) = target else {
            return;
        };
        let Some(declared) = self.var_types.find(&nv.name) else {
            return;
        };
        // Resolve aliases/subranges to the underlying elementary type so the
        // comparison matches the already-resolved right-hand side type.
        let target_type = self
            .context
            .types()
            .resolve_elementary_type_name(declared)
            .unwrap_or_else(|| declared.clone());
        if !is_checkable_type(&target_type) {
            return;
        }

        let Some(value_type) = &value.resolved_type else {
            return;
        };
        if !is_checkable_type(value_type) {
            return;
        }

        if !are_types_compatible(&target_type, value_type, self.options) {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::AssignmentTypeMismatch,
                    Label::span(value.span(), "Assignment value"),
                )
                .with_context("target", &nv.name.original().to_string())
                .with_context("target_type", &target_type.to_string())
                .with_context("value_type", &value_type.to_string()),
            );
        }
    }
}

impl Visitor<Infallible> for RuleFunctionCallTypeCheck<'_> {
    type Value = ();

    /// Opens a declaration's scope.
    ///
    /// Replaces the per-POU `clear()` this rule used to do, which could
    /// not express a method: clearing at a method boundary would discard
    /// the enclosing function block's fields, and not clearing left a
    /// method's locals shadowing those fields for every later method.
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.var_types.enter();

        // A declaration's own name is its result variable, so assigning
        // it is an assignment with a target type like any other. Without
        // this the target lookup missed and the check returned early,
        // leaving `Foo := <wrong type>` unreported -- for a FUNCTION as
        // much as for a METHOD.
        match node {
            ScopeNode::Function(node) => {
                self.var_types
                    .add(&node.name, node.return_type.to_type_name());
            }
            // Only a method that declares a return type has a result to
            // assign; `rule_use_declared_symbolic_var` rejects the
            // assignment outright for one that does not.
            ScopeNode::Method(node) => {
                if let Some(return_type) = &node.return_type {
                    self.var_types.add(&node.name, return_type.to_type_name());
                }
            }
            // Neither has a result variable.
            ScopeNode::FunctionBlock(_) | ScopeNode::Program(_) => {}
        }

        Ok(())
    }

    fn exit_scope(&mut self) {
        self.var_types.exit();
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Infallible> {
        if let VariableIdentifier::Symbol(ref id) = node.identifier {
            if let TypeReference::Named(ref type_name) = node.type_name() {
                self.var_types.add(id, type_name.clone());
            }
        }
        node.recurse_visit(self)
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<Self::Value, Infallible> {
        self.check_return_type(&node.target, &node.value);
        self.check_assignment_type(&node.target, &node.value);
        node.recurse_visit(self)
    }

    fn visit_function(&mut self, node: &Function) -> Result<Self::Value, Infallible> {
        let func_sig = self.context.functions.get(&node.name);

        if let Some(signature) = func_sig {
            // Emit NotImplemented for output arguments on user-defined functions.
            // Standard-library functions do not take output arguments.
            if !signature.is_stdlib() {
                for p in &node.param_assignment {
                    if let ParamAssignmentKind::Output(_) = p {
                        self.diagnostics
                            .push(Diagnostic::not_implemented(Label::span(
                                node.name.span(),
                                "Function call with output argument",
                            )));
                    }
                }
            }

            // Check each positional argument type against the parameter type.
            // Standard-library functions are checked too: their parameters use
            // generic ANY_* categories (or concrete types for the conversion
            // functions), all handled by `are_types_compatible`. The parameter
            // list continues past the declared ones for an extensible
            // function, so every input of `ADD(a, b, c)` is checked.
            for (param, arg_expr) in signature.bind_inputs(&node.param_assignment) {
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
    use crate::test_helpers::parse_and_resolve_types_with_context;
    use rstest::rstest;

    rule_ctx_ok!(
        apply_when_matching_types_then_ok,
        "
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
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_int_arg_to_real_param_lossless_then_ok,
        "
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
END_PROGRAM"
    );

    rule_ctx_err1!(
        apply_when_dint_arg_to_real_param_lossy_then_error,
        "
FUNCTION DOUBLE_REAL : REAL
VAR_INPUT
    A : REAL;
END_VAR
    DOUBLE_REAL := A;
END_FUNCTION

PROGRAM main
VAR
    result : REAL;
    x : DINT;
END_VAR
    result := DOUBLE_REAL(x);
END_PROGRAM",
        Problem::FunctionCallArgTypeMismatch
    );

    rule_ctx_ok!(
        apply_when_stdlib_function_then_skipped,
        "
PROGRAM main
VAR
    result : REAL;
    x : INT;
END_VAR
    result := INT_TO_REAL(x);
END_PROGRAM"
    );

    /// The function forms of the bitwise boolean operators accept every
    /// ANY_BIT type, as the operators do (#1567).
    #[rstest]
    #[case::and_bool("AND", "BOOL")]
    #[case::and_byte("AND", "BYTE")]
    #[case::and_word("AND", "WORD")]
    #[case::and_dword("AND", "DWORD")]
    #[case::and_lword("AND", "LWORD")]
    #[case::or_word("OR", "WORD")]
    #[case::xor_word("XOR", "WORD")]
    fn apply_when_bitwise_function_form_on_bit_string_then_ok(
        #[case] function: &str,
        #[case] type_name: &str,
    ) {
        let program = format!(
            "
PROGRAM main
VAR
    a : {type_name};
    b : {type_name};
    result : {type_name};
END_VAR
    result := {function}(a, b);
END_PROGRAM"
        );
        let (library, context) = parse_and_resolve_types_with_context(&program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok(), "{result:?}");
    }

    rule_ctx_errn!(
        apply_when_bitwise_function_form_on_int_then_error_per_argument,
        "
PROGRAM main
VAR
    a : INT;
    b : INT;
    result : INT;
END_VAR
    result := AND(a, b);
END_PROGRAM",
        2,
        Problem::FunctionCallArgTypeMismatch
    );

    // An extensible call is checked past its declared parameters, so the
    // third input of ADD is checked like the first two (#1618).
    rule_ctx_ok!(
        apply_when_extensible_call_third_arg_matches_then_ok,
        "
PROGRAM main
VAR
    a : DINT;
    b : DINT;
    c : DINT;
    result : DINT;
END_VAR
    result := ADD(a, b, c);
END_PROGRAM"
    );

    rule_ctx_err1!(
        apply_when_extensible_call_third_arg_mismatch_then_error,
        "
PROGRAM main
VAR
    a : DINT;
    b : DINT;
    c : STRING;
    result : DINT;
END_VAR
    result := ADD(a, b, c);
END_PROGRAM",
        Problem::FunctionCallArgTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_mux_fourth_input_mismatch_then_error,
        "
PROGRAM main
VAR
    a : DINT;
    s : STRING;
    result : DINT;
END_VAR
    result := MUX(0, a, a, a, s);
END_PROGRAM",
        Problem::FunctionCallArgTypeMismatch
    );

    // NOT(x) parses as the unary operator; the named-argument spelling is the
    // one that reaches the function signature.
    rule_ctx_ok!(
        apply_when_not_function_form_on_word_then_ok,
        "
PROGRAM main
VAR
    a : WORD;
    result : WORD;
END_VAR
    result := NOT(IN := a);
END_PROGRAM"
    );

    rule_ctx_err1!(
        apply_when_multiple_args_one_mismatch_then_one_error,
        "
FUNCTION MY_FUNC : INT
VAR_INPUT
    A : INT;
    B : SINT;
END_VAR
    MY_FUNC := A;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    x : INT;
END_VAR
    result := MY_FUNC(x, x);
END_PROGRAM",
        Problem::FunctionCallArgTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_return_type_mismatch_then_error,
        "
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
END_PROGRAM",
        Problem::FunctionCallReturnTypeMismatch
    );

    rule_ctx_ok!(
        apply_when_nested_function_call_types_match_then_ok,
        "
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
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_all_args_match_then_ok,
        "
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
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_return_type_matches_then_ok,
        "
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
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_bare_literal_arg_to_int_param_then_ok,
        "
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
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_bare_literal_arg_to_sint_param_then_ok,
        "
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
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_bare_real_literal_arg_to_lreal_param_then_ok,
        "
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
END_PROGRAM"
    );

    // REAL -> LREAL is lossless, standard widening (unlike the bare
    // literal case above, this argument is a typed REAL variable, not
    // an untyped ANY_REAL literal -- a separate code path through
    // ElementaryTypeName::can_widen_to()).
    rule_ctx_ok!(
        apply_when_typed_real_var_arg_to_lreal_param_then_ok,
        "
FUNCTION DBL : LREAL
VAR_INPUT
    x : LREAL;
END_VAR
    DBL := x;
END_FUNCTION

PROGRAM main
VAR
    input : REAL;
    result : LREAL;
END_VAR
    result := DBL(input);
END_PROGRAM"
    );

    // The reverse direction (LREAL -> REAL) is narrowing and must
    // remain an error -- guards against accidentally allowing both
    // directions.
    rule_ctx_err!(
        apply_when_typed_lreal_var_arg_to_real_param_then_error,
        "
FUNCTION SNGL : REAL
VAR_INPUT
    x : REAL;
END_VAR
    SNGL := x;
END_FUNCTION

PROGRAM main
VAR
    input : LREAL;
    result : REAL;
END_VAR
    result := SNGL(input);
END_PROGRAM"
    );

    rule_ctx_err1!(
        apply_when_typed_dint_literal_arg_to_int_param_then_error,
        "
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
END_PROGRAM",
        Problem::FunctionCallArgTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_dint_var_arg_to_int_param_then_error,
        "
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
END_PROGRAM",
        Problem::FunctionCallArgTypeMismatch
    );

    rule_ctx_ok!(
        apply_when_bare_int_literal_arg_to_real_param_then_ok,
        "
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
"
    );

    rule_ctx_ok!(
        apply_when_bare_int_literal_arg_to_lreal_param_then_ok,
        "
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
"
    );

    // --- Implicit integer widening tests (ADR-0029) ---

    rule_ctx_ok!(
        apply_when_sint_arg_to_int_param_then_ok,
        "
FUNCTION TAKES_INT : INT
VAR_INPUT
    x : INT;
END_VAR
    TAKES_INT := x;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    y : SINT;
END_VAR
    result := TAKES_INT(y);
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_int_arg_to_dint_param_then_ok,
        "
FUNCTION TAKES_DINT : DINT
VAR_INPUT
    x : DINT;
END_VAR
    TAKES_DINT := x;
END_FUNCTION

PROGRAM main
VAR
    result : DINT;
    y : INT;
END_VAR
    result := TAKES_DINT(y);
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_sint_arg_to_lint_param_then_ok,
        "
FUNCTION TAKES_LINT : LINT
VAR_INPUT
    x : LINT;
END_VAR
    TAKES_LINT := x;
END_FUNCTION

PROGRAM main
VAR
    result : LINT;
    y : SINT;
END_VAR
    result := TAKES_LINT(y);
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_usint_arg_to_uint_param_then_ok,
        "
FUNCTION TAKES_UINT : UINT
VAR_INPUT
    x : UINT;
END_VAR
    TAKES_UINT := x;
END_FUNCTION

PROGRAM main
VAR
    result : UINT;
    y : USINT;
END_VAR
    result := TAKES_UINT(y);
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_usint_arg_to_int_param_then_ok,
        "
FUNCTION TAKES_INT : INT
VAR_INPUT
    x : INT;
END_VAR
    TAKES_INT := x;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    y : USINT;
END_VAR
    result := TAKES_INT(y);
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_uint_arg_to_dint_param_then_ok,
        "
FUNCTION TAKES_DINT : DINT
VAR_INPUT
    x : DINT;
END_VAR
    TAKES_DINT := x;
END_FUNCTION

PROGRAM main
VAR
    result : DINT;
    y : UINT;
END_VAR
    result := TAKES_DINT(y);
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_sint_return_to_dint_var_then_ok,
        "
FUNCTION GET_SINT : SINT
VAR_INPUT
    x : SINT;
END_VAR
    GET_SINT := x;
END_FUNCTION

PROGRAM main
VAR
    result : DINT;
    y : SINT;
END_VAR
    result := GET_SINT(y);
END_PROGRAM"
    );

    rule_ctx_err!(
        apply_when_dint_arg_to_int_param_then_error,
        "
FUNCTION TAKES_INT : INT
VAR_INPUT
    x : INT;
END_VAR
    TAKES_INT := x;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    y : DINT;
END_VAR
    result := TAKES_INT(y);
END_PROGRAM"
    );

    rule_ctx_err!(
        apply_when_int_arg_to_uint_param_then_error,
        "
FUNCTION TAKES_UINT : UINT
VAR_INPUT
    x : UINT;
END_VAR
    TAKES_UINT := x;
END_FUNCTION

PROGRAM main
VAR
    result : UINT;
    y : INT;
END_VAR
    result := TAKES_UINT(y);
END_PROGRAM"
    );

    rule_ctx_err!(
        apply_when_byte_arg_to_int_param_then_error,
        "
FUNCTION TAKES_INT : INT
VAR_INPUT
    x : INT;
END_VAR
    TAKES_INT := x;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    y : BYTE;
END_VAR
    result := TAKES_INT(y);
END_PROGRAM"
    );

    // --- Integration tests for new widening cases ---

    rule_ctx_ok!(
        apply_when_int_arg_to_real_param_then_ok,
        "
FUNCTION TAKES_REAL : REAL
VAR_INPUT
    x : REAL;
END_VAR
    TAKES_REAL := x;
END_FUNCTION

PROGRAM main
VAR
    result : REAL;
    y : INT;
END_VAR
    result := TAKES_REAL(y);
END_PROGRAM"
    );

    rule_ctx_err!(
        apply_when_dint_arg_to_real_param_then_error,
        "
FUNCTION TAKES_REAL : REAL
VAR_INPUT
    x : REAL;
END_VAR
    TAKES_REAL := x;
END_FUNCTION

PROGRAM main
VAR
    result : REAL;
    y : DINT;
END_VAR
    result := TAKES_REAL(y);
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_byte_arg_to_word_param_then_ok,
        "
FUNCTION TAKES_WORD : WORD
VAR_INPUT
    x : WORD;
END_VAR
    TAKES_WORD := x;
END_FUNCTION

PROGRAM main
VAR
    result : WORD;
    y : BYTE;
END_VAR
    result := TAKES_WORD(y);
END_PROGRAM"
    );

    rule_ctx_err!(
        apply_when_word_arg_to_byte_param_then_error,
        "
FUNCTION TAKES_BYTE : BYTE
VAR_INPUT
    x : BYTE;
END_VAR
    TAKES_BYTE := x;
END_FUNCTION

PROGRAM main
VAR
    result : BYTE;
    y : WORD;
END_VAR
    result := TAKES_BYTE(y);
END_PROGRAM"
    );

    rule_ctx_err!(
        apply_when_real_arg_to_int_param_then_error,
        "
FUNCTION TAKES_INT : INT
VAR_INPUT
    x : INT;
END_VAR
    TAKES_INT := x;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    y : REAL;
END_VAR
    result := TAKES_INT(y);
END_PROGRAM"
    );

    // --- Cross-family widening tests (ADR-0031, requires flag) ---

    /// Cross-family widening on function-call arguments/returns with
    /// `--allow-cross-family-widening` enabled (ADR-0031), against a resolved
    /// context. Each case resolves the program, applies the rule with the flag
    /// on, and asserts the expected outcome; each row still runs as an
    /// individually-named test.
    #[rstest]
    #[case::byte_arg_to_int_param_ok(
        "
FUNCTION TAKES_INT : INT
VAR_INPUT
    x : INT;
END_VAR
    TAKES_INT := x;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    y : BYTE;
END_VAR
    result := TAKES_INT(y);
END_PROGRAM",
        true
    )]
    #[case::literal_zero_to_byte_param_ok(
        "
FUNCTION TAKES_BYTE : BYTE
VAR_INPUT
    x : BYTE;
END_VAR
    TAKES_BYTE := x;
END_FUNCTION

PROGRAM main
VAR
    result : BYTE;
END_VAR
    result := TAKES_BYTE(0);
END_PROGRAM",
        true
    )]
    #[case::byte_return_to_int_var_ok(
        "
FUNCTION GET_BYTE : BYTE
VAR_INPUT
    x : BYTE;
END_VAR
    GET_BYTE := x;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    y : BYTE;
END_VAR
    result := GET_BYTE(y);
END_PROGRAM",
        true
    )]
    // Integer → bit-string is never allowed, even with flag.
    #[case::int_arg_to_byte_param_error(
        "
FUNCTION TAKES_BYTE : BYTE
VAR_INPUT
    x : BYTE;
END_VAR
    TAKES_BYTE := x;
END_FUNCTION

PROGRAM main
VAR
    result : BYTE;
    y : INT;
END_VAR
    result := TAKES_BYTE(y);
END_PROGRAM",
        false
    )]
    fn apply_when_cross_family_widening_flag_on_call_then_matches_expectation(
        #[case] program: &str,
        #[case] expect_ok: bool,
    ) {
        let (library, context) = parse_and_resolve_types_with_context(program);
        let opts = CompilerOptions {
            allow_cross_family_widening: true,
            ..CompilerOptions::default()
        };
        let result = apply(&library, &context, &opts);
        assert_eq!(result.is_ok(), expect_ok);
    }

    rule_ctx_err!(
        apply_when_literal_zero_to_byte_param_without_flag_then_error,
        "
FUNCTION TAKES_BYTE : BYTE
VAR_INPUT
    x : BYTE;
END_VAR
    TAKES_BYTE := x;
END_FUNCTION

PROGRAM main
VAR
    result : BYTE;
END_VAR
    result := TAKES_BYTE(0);
END_PROGRAM"
    );

    rule_ctx_err!(
        apply_when_byte_return_to_int_var_without_flag_then_error,
        "
FUNCTION GET_BYTE : BYTE
VAR_INPUT
    x : BYTE;
END_VAR
    GET_BYTE := x;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    y : BYTE;
END_VAR
    result := GET_BYTE(y);
END_PROGRAM"
    );

    // --- Standard-library argument type checks ---

    rule_ctx_err_code!(
        apply_when_stdlib_sin_arg_is_bool_then_arg_type_error,
        "
PROGRAM main
VAR
    b : BOOL;
    r : REAL;
END_VAR
    r := SIN(b);
END_PROGRAM",
        Problem::FunctionCallArgTypeMismatch
    );

    rule_ctx_ok!(
        apply_when_stdlib_sin_arg_is_real_then_ok,
        "
PROGRAM main
VAR
    x : REAL;
    r : REAL;
END_VAR
    r := SIN(x);
END_PROGRAM"
    );

    // UINT_TO_REAL expects UINT, but the argument is UDINT.
    rule_ctx_err1!(
        apply_when_wrong_conversion_function_arg_then_arg_type_error,
        "
PROGRAM main
VAR
    u : UDINT;
    r : REAL;
END_VAR
    r := UINT_TO_REAL(u);
END_PROGRAM",
        Problem::FunctionCallArgTypeMismatch
    );

    rule_ctx_ok!(
        apply_when_correct_conversion_function_arg_then_ok,
        "
PROGRAM main
VAR
    u : UDINT;
    r : REAL;
END_VAR
    r := UDINT_TO_REAL(u);
END_PROGRAM"
    );

    // ABS accepts ANY_NUM; a bare integer literal is accepted.
    rule_ctx_ok!(
        apply_when_stdlib_int_literal_arg_to_real_param_then_ok,
        "
PROGRAM main
VAR
    r : REAL;
END_VAR
    r := SQRT(2.0);
END_PROGRAM"
    );

    // --- Assignment statement type checks (P4035) ---

    rule_ctx_err1!(
        apply_when_bool_target_assigned_real_expr_then_error,
        "
PROGRAM main
VAR
    b : BOOL;
    x : REAL;
END_VAR
    b := x * 2.0;
END_PROGRAM",
        Problem::AssignmentTypeMismatch
    );

    rule_ctx_err1!(
        apply_when_int_target_assigned_real_var_then_error,
        "
PROGRAM main
VAR
    i : INT;
    r : REAL;
END_VAR
    i := r;
END_PROGRAM",
        Problem::AssignmentTypeMismatch
    );

    // INT widens losslessly to REAL, so this assignment is valid.
    rule_ctx_ok!(
        apply_when_real_target_assigned_int_var_then_ok,
        "
PROGRAM main
VAR
    i : INT;
    r : REAL;
END_VAR
    r := i;
END_PROGRAM"
    );

    rule_ctx_ok!(
        apply_when_matching_assignment_then_ok,
        "
PROGRAM main
VAR
    i : INT;
    j : INT;
END_VAR
    i := j + 1;
END_PROGRAM"
    );

    /// Cross-family widening on assignment statements with
    /// `--allow-cross-family-widening` enabled (ADR-0031), against a resolved
    /// context. Each case resolves the program, applies the rule with the flag
    /// on, and asserts the expected outcome; each row still runs as an
    /// individually-named test.
    #[rstest]
    // UDINT -> DWORD is allowed even though the two are the same width, so it
    // is a reinterpretation rather than a widening. Real TcXaeShell accepts it,
    // which is why the rule is permissive here; ADR-0031 sets the cross-family
    // policy but does not speak to the equal-width case.
    #[case::dword_target_assigned_udint_var_ok(
        "
PROGRAM main
VAR
    dwFromUdint : DWORD;
    udValue : UDINT;
END_VAR
    dwFromUdint := udValue;
END_PROGRAM",
        true
    )]
    #[case::udint_target_assigned_dword_var_ok(
        "
PROGRAM main
VAR
    udFromDword : UDINT;
    dwValue : DWORD;
END_VAR
    udFromDword := dwValue;
END_PROGRAM",
        true
    )]
    // Signed integer, equal width -- not part of the verified exception, must
    // stay rejected even with the flag on.
    #[case::dword_target_assigned_dint_var_error(
        "
PROGRAM main
VAR
    dwFromDint : DWORD;
    diValue : DINT;
END_VAR
    dwFromDint := diValue;
END_PROGRAM",
        false
    )]
    fn apply_when_cross_family_widening_flag_on_assignment_then_matches_expectation(
        #[case] program: &str,
        #[case] expect_ok: bool,
    ) {
        let (library, context) = parse_and_resolve_types_with_context(program);
        let opts = CompilerOptions {
            allow_cross_family_widening: true,
            ..CompilerOptions::default()
        };
        let result = apply(&library, &context, &opts);
        assert_eq!(result.is_ok(), expect_ok);
    }

    rule_ctx_err!(
        apply_when_dword_target_assigned_udint_var_without_flag_then_error,
        "
PROGRAM main
VAR
    dwFromUdint : DWORD;
    udValue : UDINT;
END_VAR
    dwFromUdint := udValue;
END_PROGRAM"
    );

    // Temporal short/long widths are treated as one family.
    rule_ctx_ok!(
        apply_when_ltime_target_assigned_time_var_then_ok,
        "
PROGRAM main
VAR
    lt : LTIME;
    t : TIME;
END_VAR
    lt := t;
END_PROGRAM"
    );

    // ---------------------------------------------------------------------
    // METHOD scoping.
    // ---------------------------------------------------------------------

    fn apply_with_methods(program: &str) -> crate::result::SemanticResult {
        let options = CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        };
        let (library, context) =
            crate::test_helpers::parse_and_resolve_types_with_options(program, &options);
        super::apply(&library, &context, &options)
    }

    /// A method's local belongs to the method. It used to be recorded
    /// against the enclosing function block, so it overwrote a field of
    /// the same name for every method compiled after it -- and the
    /// mismatch below was accepted because `v` was still recorded as the
    /// `REAL` from `A`.
    #[test]
    fn apply_when_method_local_shadows_field_then_sibling_method_uses_field_type() {
        let errors = apply_with_methods(
            "
FUNCTION_BLOCK FB_Motor
VAR
    v : INT;
END_VAR
METHOD A
VAR
    v : REAL;
END_VAR
    v := 1.5;
END_METHOD
METHOD B
    v := 2.5;
END_METHOD
END_FUNCTION_BLOCK",
        )
        .unwrap_err();

        assert!(
            errors
                .iter()
                .any(|d| d.code == Problem::AssignmentTypeMismatch.code()),
            "expected an assignment type mismatch on the INT field, got {errors:?}"
        );
    }

    /// A method's locals are still checked against their own declared
    /// types once they live in the method's own scope.
    #[test]
    fn apply_when_method_local_assigned_wrong_type_then_error() {
        let errors = apply_with_methods(
            "
FUNCTION_BLOCK FB_Motor
METHOD A
VAR
    b : BOOL;
    i : INT;
END_VAR
    b := i;
END_METHOD
END_FUNCTION_BLOCK",
        )
        .unwrap_err();

        assert!(errors
            .iter()
            .any(|d| d.code == Problem::AssignmentTypeMismatch.code()));
    }

    /// A method reading the instance's field is not a mismatch: the
    /// method scope nests inside the function block's.
    #[test]
    fn apply_when_method_assigns_field_from_matching_local_then_ok() {
        assert!(apply_with_methods(
            "
FUNCTION_BLOCK FB_Motor
VAR
    speed : INT;
END_VAR
METHOD SetSpeed
VAR_INPUT
    newSpeed : INT;
END_VAR
    speed := newSpeed;
END_METHOD
END_FUNCTION_BLOCK",
        )
        .is_ok());
    }

    // ---------------------------------------------------------------------
    // Result variables. A declaration's own name is an assignment target.
    // ---------------------------------------------------------------------

    rule_ctx_err_code!(
        apply_when_function_result_assigned_wrong_type_then_error,
        "
FUNCTION GetFlag : BOOL
VAR
    n : INT;
END_VAR
    GetFlag := n;
END_FUNCTION",
        Problem::AssignmentTypeMismatch,
    );

    rule_ctx_ok!(
        apply_when_function_result_assigned_correct_type_then_ok,
        "
FUNCTION GetN : INT
VAR
    n : INT;
END_VAR
    GetN := n;
END_FUNCTION"
    );

    #[test]
    fn apply_when_method_result_assigned_wrong_type_then_error() {
        let errors = apply_with_methods(
            "
FUNCTION_BLOCK FB_Motor
METHOD GetFlag : BOOL
VAR
    n : INT;
END_VAR
    GetFlag := n;
END_METHOD
END_FUNCTION_BLOCK",
        )
        .unwrap_err();

        assert!(errors
            .iter()
            .any(|d| d.code == Problem::AssignmentTypeMismatch.code()));
    }

    #[test]
    fn apply_when_method_result_assigned_correct_type_then_ok() {
        assert!(apply_with_methods(
            "
FUNCTION_BLOCK FB_Motor
METHOD GetN : INT
VAR
    n : INT;
END_VAR
    GetN := n;
END_METHOD
END_FUNCTION_BLOCK",
        )
        .is_ok());
    }

    /// A method with no return type has no result variable, so its name
    /// is not an assignment target here either. The assignment is
    /// rejected earlier, by `rule_use_declared_symbolic_var`; this pins
    /// that this rule adds no target type for it.
    #[test]
    fn apply_when_method_has_no_return_type_then_name_is_not_a_target() {
        assert!(apply_with_methods(
            "
FUNCTION_BLOCK FB_Motor
METHOD DoThing
VAR
    n : INT;
END_VAR
    DoThing := n;
END_METHOD
END_FUNCTION_BLOCK",
        )
        .is_ok());
    }
}
