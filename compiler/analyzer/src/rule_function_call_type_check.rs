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
    intermediates::operator_function_form::operator_function_form,
    result::SemanticResult,
    rule_string_encoding_compat,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::ScopeTracker,
    type_compat::is_checkable_type,
    value_type::{self, ValueType},
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
            scope: ScopeTracker::default(),
        },
        lib,
    )
}

struct RuleFunctionCallTypeCheck<'a> {
    context: &'a SemanticContext,
    options: &'a CompilerOptions,
    diagnostics: Vec<Diagnostic>,
    /// Where the traversal is, to look variables up in the symbol
    /// environment. A method's scope nests inside its function block's,
    /// so a local shadows a field of the same name only within its own
    /// body.
    scope: ScopeTracker,
}

impl DiagnosticVisitor for RuleFunctionCallTypeCheck<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleFunctionCallTypeCheck<'_> {
    /// The type name a variable in scope was declared with, or `None` for
    /// one declared with an inline type or not declared at all.
    ///
    /// A function's or method's own name is its result variable, so
    /// assigning it is an assignment with a target type like any other.
    fn declared_type_name(&self, id: &Id) -> Option<TypeName> {
        let type_id = self
            .context
            .symbols()
            .find(id, &self.scope.current())?
            .type_id?;
        self.context.types().name_of(type_id).cloned()
    }

    /// Checks whether a function call expression assigned to a variable has a
    /// matching return type. Emits P4027 if there is a mismatch.
    ///
    /// Standard-library calls are checked like any other. Their declared
    /// return type may be a generic category, but `xform_resolve_expr_types`
    /// has already narrowed it to the concrete type of the argument the
    /// category binds to; where it could not, `expr_type` is `None` and
    /// the call is skipped below. A call naming a function the environment
    /// does not hold resolves to `None` the same way, so the signature
    /// itself is never needed here.
    fn check_return_type(&mut self, target: &Variable, value: &Expr) {
        let ExprKind::Function(ref func_call) = value.kind else {
            return;
        };
        let Variable::Symbolic(SymbolicVariableKind::Named(ref nv)) = target else {
            return;
        };
        let Some(target_type) = self.declared_type_name(&nv.name) else {
            return;
        };
        if self.generic_return_bound_to_mismatched_argument(func_call) {
            return;
        }

        if let Err(mismatch) =
            value_type::check(self.context.types(), &target_type, value, self.options)
        {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::FunctionCallReturnTypeMismatch,
                    Label::span(func_call.name.span(), "Function call return type"),
                )
                .with_context("function", &func_call.name.original().to_string())
                .with_context("return_type", &mismatch.actual)
                .with_context("target_type", &target_type.to_string()),
            );
        }
    }

    /// Whether `call`'s return type is a generic category bound by an argument
    /// that is itself outside that category.
    ///
    /// `xform_resolve_expr_types` narrows a generic return type to the type
    /// of the argument that binds it, even when that argument fails the
    /// category: `SIN(b)` on a `BOOL` is typed `BOOL`. The argument is already
    /// reported (P4026, by `visit_function`), so checking the return type
    /// would report the same mistake a second time.
    fn generic_return_bound_to_mismatched_argument(&self, call: &Function) -> bool {
        let Some(signature) = self.context.functions.get(&call.name) else {
            return false;
        };
        let Some(return_type) = signature.return_type.as_ref().map(|t| t.to_type_name()) else {
            return false;
        };
        if GenericTypeName::try_from(&return_type.name).is_err() {
            return false;
        }
        signature
            .bind_inputs(&call.param_assignment)
            .any(|(param, argument)| {
                param.param_type == return_type
                    && value_type::check(
                        self.context.types(),
                        &param.param_type,
                        argument,
                        self.options,
                    )
                    .is_err()
            })
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
        // A string variable of the other encoding is P4034's business;
        // reporting it here too would give two diagnostics for one mistake.
        if rule_string_encoding_compat::mixes_encodings(
            target,
            value,
            self.context,
            &self.scope.current(),
        ) {
            return;
        }
        let Some(declared) = self.declared_type_name(&nv.name) else {
            return;
        };
        // Resolve aliases/subranges to the underlying elementary type so the
        // comparison matches the already-resolved right-hand side type.
        let target_type = self
            .context
            .types()
            .resolve_elementary_type_name(&declared)
            .unwrap_or(declared);
        if !is_checkable_type(&target_type) {
            return;
        }

        // A scalar value the compatibility relation cannot judge (a
        // reference, a sized string) is skipped; a composite one is never
        // assignable to an elementary target.
        let types = self.context.types();
        let checkable = match value_type::of(types, value) {
            None => false,
            Some(ValueType::Scalar(value_type)) => is_checkable_type(&value_type),
            Some(ValueType::Composite(_)) => true,
        };
        if !checkable {
            return;
        }

        if let Err(mismatch) = value_type::check(types, &target_type, value, self.options) {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::AssignmentTypeMismatch,
                    Label::span(value.span(), "Assignment value"),
                )
                .with_context("target", &nv.name.original().to_string())
                .with_context("target_type", &target_type.to_string())
                .with_context("value_type", &mismatch.actual),
            );
        }
    }
}

impl Visitor<Infallible> for RuleFunctionCallTypeCheck<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
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
            // function, so every input of `AND(a, b, c)` is checked. A
            // VAR_IN_OUT argument must match exactly, not just be compatible;
            // `rule_function_call_in_out_argument` checks it.
            //
            // `ADD`, `SUB`, `MUL` and `DIV` are the exception. Their inputs
            // are checked against every overload (the numeric one and the
            // typed ones on the time and date types) by the operator rule,
            // which reports a mismatch as P4049; their `ANY_NUM` signature
            // states only the numeric overload.
            let overloaded = operator_function_form(&node.name.to_string())
                .is_some_and(|form| !form.typed_overloads().is_empty());
            let inputs = signature
                .bind_inputs(&node.param_assignment)
                .filter(|_| !overloaded);
            for (param, arg_expr) in inputs {
                if param.is_inout {
                    continue;
                }
                if let Err(mismatch) = value_type::check(
                    self.context.types(),
                    &param.param_type,
                    arg_expr,
                    self.options,
                ) {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::FunctionCallArgTypeMismatch,
                            Label::span(node.name.span(), "Function call"),
                        )
                        .with_context("function", &node.name.original().to_string())
                        .with_context("parameter", &param.name.original().to_string())
                        .with_context("expected", &param.param_type.to_string())
                        .with_context("actual", &mismatch.actual),
                    );
                }
            }
        }

        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod composite_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{codes, fb_inheritance_options, rule_codes};
    use crate::test_helpers::{diagnostic_codes, rule_diagnostics};
    use rstest::rstest;

    rule_ok!(
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

    rule_ok!(
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

    rule_err!(
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
        [Problem::FunctionCallArgTypeMismatch]
    );

    rule_ok!(
        apply_when_stdlib_arg_matches_param_then_ok,
        "
PROGRAM main
VAR
    result : REAL;
    x : INT;
END_VAR
    result := INT_TO_REAL(x);
END_PROGRAM"
    );

    // A standard-library return type is checked against the assignment
    // target like a user-defined one: INT_TO_REAL yields REAL, which does
    // not fit an INT.
    rule_err!(
        apply_when_stdlib_return_type_mismatches_target_then_error,
        "
PROGRAM main
VAR
    result : INT;
    x : INT;
END_VAR
    result := INT_TO_REAL(x);
END_PROGRAM",
        [Problem::FunctionCallReturnTypeMismatch]
    );

    // Integer widening still applies to a standard-library return
    // (ADR-0029, ADR-0031): INT_TO_DINT yields DINT, which fits a LINT.
    rule_ok!(
        apply_when_stdlib_return_widens_to_target_then_ok,
        "
PROGRAM main
VAR
    result : LINT;
    x : INT;
END_VAR
    result := INT_TO_DINT(x);
END_PROGRAM"
    );

    // A generic return type is narrowed by `xform_resolve_expr_types` before
    // this rule runs, so ADD over INT arguments is an INT return and not a
    // false P4027 against the ANY_NUM the signature declares.
    rule_ok!(
        apply_when_stdlib_generic_return_resolves_to_target_then_ok,
        "
PROGRAM main
VAR
    result : INT;
    a : INT;
    b : INT;
END_VAR
    result := ADD(a, b);
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
        let diagnostics = rule_diagnostics(apply, &program, &CompilerOptions::default());
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    rule_err!(
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
        [Problem::FunctionCallArgTypeMismatch; 2]
    );

    // An extensible call is checked past its declared parameters, so the
    // third input of ADD is checked like the first two (#1618).
    rule_ok!(
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

    rule_err!(
        apply_when_extensible_call_third_arg_mismatch_then_error,
        "
PROGRAM main
VAR
    a : WORD;
    b : WORD;
    c : STRING;
    result : WORD;
END_VAR
    result := AND(a, b, c);
END_PROGRAM",
        [Problem::FunctionCallArgTypeMismatch]
    );

    rule_err!(
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
        [Problem::FunctionCallArgTypeMismatch]
    );

    // A VAR_IN_OUT declared before a VAR_INPUT takes the first positional
    // argument, so a mismatch on the second names the VAR_INPUT (#1658).
    #[test]
    fn apply_when_in_out_before_input_mismatch_then_names_input_parameter() {
        let program = "
FUNCTION Scale : INT
VAR_IN_OUT acc : INT; END_VAR
VAR_INPUT factor : INT; END_VAR
    acc := acc * factor;
    Scale := acc;
END_FUNCTION

PROGRAM main
VAR
    total : INT;
    s : STRING;
    result : INT;
END_VAR
    result := Scale(total, s);
END_PROGRAM";
        let errors = rule_diagnostics(apply, program, &CompilerOptions::default());
        assert_eq!(
            diagnostic_codes(&errors),
            [Problem::FunctionCallArgTypeMismatch.code()]
        );
        assert!(
            errors[0].described.contains(&"parameter=factor".to_owned()),
            "{:?}",
            errors[0].described
        );
    }

    // NOT(x) parses as the unary operator; the named-argument spelling is the
    // one that reaches the function signature.
    rule_ok!(
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

    rule_err!(
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
        [Problem::FunctionCallArgTypeMismatch]
    );

    rule_err!(
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
        [Problem::FunctionCallReturnTypeMismatch]
    );

    rule_ok!(
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

    rule_ok!(
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

    rule_ok!(
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

    rule_ok!(
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

    rule_ok!(
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

    rule_ok!(
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
    rule_ok!(
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
    rule_err!(
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
END_PROGRAM",
        [Problem::FunctionCallArgTypeMismatch]
    );

    rule_err!(
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
        [Problem::FunctionCallArgTypeMismatch]
    );

    rule_err!(
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
        [Problem::FunctionCallArgTypeMismatch]
    );

    rule_ok!(
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

    rule_ok!(
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

    rule_ok!(
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

    rule_ok!(
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

    rule_ok!(
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

    rule_ok!(
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

    rule_ok!(
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

    rule_ok!(
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

    rule_ok!(
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

    rule_err!(
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
END_PROGRAM",
        [Problem::FunctionCallArgTypeMismatch]
    );

    rule_err!(
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
END_PROGRAM",
        [Problem::FunctionCallArgTypeMismatch]
    );

    rule_err!(
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
END_PROGRAM",
        [Problem::FunctionCallArgTypeMismatch]
    );

    // --- Integration tests for new widening cases ---

    rule_ok!(
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

    rule_err!(
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
END_PROGRAM",
        [Problem::FunctionCallArgTypeMismatch]
    );

    rule_ok!(
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

    rule_err!(
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
END_PROGRAM",
        [Problem::FunctionCallArgTypeMismatch]
    );

    rule_err!(
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
END_PROGRAM",
        [Problem::FunctionCallArgTypeMismatch]
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
        ACCEPTED
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
        ACCEPTED
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
        ACCEPTED
    )]
    // Integer → bit-string is allowed only for the UDINT ↔ DWORD pair, which
    // ElementaryTypeName::can_widen_cross_family_to carves out at equal width.
    #[case::udint_arg_to_dword_param_ok(
        "
FUNCTION TAKES_DWORD : DWORD
VAR_INPUT
    x : DWORD;
END_VAR
    TAKES_DWORD := x;
END_FUNCTION

PROGRAM main
VAR
    result : DWORD;
    y : UDINT;
END_VAR
    result := TAKES_DWORD(y);
END_PROGRAM",
        ACCEPTED
    )]
    // INT → BYTE is not that pair, so it stays an error even with the flag.
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
        ARG_MISMATCH
    )]
    fn apply_when_cross_family_flags_on_call_then_matches_expectation(
        #[case] program: &str,
        #[case] expected: &[Problem],
    ) {
        let opts = CompilerOptions {
            allow_cross_family_widening: true,
            allow_cross_family_conversion: true,
            allow_int_literal_to_bit_string: true,
            ..CompilerOptions::default()
        };
        assert_eq!(rule_codes(apply, program, &opts), codes(expected));
    }

    /// One program per cross-family rule. Each is accepted under exactly one
    /// of the three flags, which is what makes them three flags rather than
    /// one: enabling any other flag leaves the program rejected.
    const WIDENING_PROGRAM: &str = "
FUNCTION TAKES_INT : INT
VAR_INPUT
    x : INT;
END_VAR
    TAKES_INT := x;
END_FUNCTION

PROGRAM main
VAR
    result : INT;
    b : BYTE;
END_VAR
    result := TAKES_INT(b);
END_PROGRAM";

    const CONVERSION_PROGRAM: &str = "
FUNCTION TAKES_DWORD : DWORD
VAR_INPUT
    x : DWORD;
END_VAR
    TAKES_DWORD := x;
END_FUNCTION

PROGRAM main
VAR
    result : DWORD;
    u : UDINT;
END_VAR
    result := TAKES_DWORD(u);
END_PROGRAM";

    const LITERAL_PROGRAM: &str = "
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
END_PROGRAM";

    const ACCEPTED: &[Problem] = &[];
    const ARG_MISMATCH: &[Problem] = &[Problem::FunctionCallArgTypeMismatch];
    const ASSIGNMENT_MISMATCH: &[Problem] = &[Problem::AssignmentTypeMismatch];

    fn only(flag: &str) -> CompilerOptions {
        let mut opts = CompilerOptions::default();
        match flag {
            "widening" => opts.allow_cross_family_widening = true,
            "conversion" => opts.allow_cross_family_conversion = true,
            "literal" => opts.allow_int_literal_to_bit_string = true,
            "none" => {}
            other => panic!("unknown cross-family flag {other}"),
        }
        opts
    }

    #[rstest]
    #[case::widening_under_widening(WIDENING_PROGRAM, "widening", ACCEPTED)]
    #[case::widening_under_conversion(WIDENING_PROGRAM, "conversion", ARG_MISMATCH)]
    #[case::widening_under_literal(WIDENING_PROGRAM, "literal", ARG_MISMATCH)]
    #[case::conversion_under_widening(CONVERSION_PROGRAM, "widening", ARG_MISMATCH)]
    #[case::conversion_under_conversion(CONVERSION_PROGRAM, "conversion", ACCEPTED)]
    #[case::conversion_under_literal(CONVERSION_PROGRAM, "literal", ARG_MISMATCH)]
    #[case::literal_under_widening(LITERAL_PROGRAM, "widening", ARG_MISMATCH)]
    #[case::literal_under_conversion(LITERAL_PROGRAM, "conversion", ARG_MISMATCH)]
    #[case::literal_under_literal(LITERAL_PROGRAM, "literal", ACCEPTED)]
    #[case::widening_under_none(WIDENING_PROGRAM, "none", ARG_MISMATCH)]
    #[case::conversion_under_none(CONVERSION_PROGRAM, "none", ARG_MISMATCH)]
    #[case::literal_under_none(LITERAL_PROGRAM, "none", ARG_MISMATCH)]
    fn apply_when_one_cross_family_flag_on_then_only_its_rule_is_accepted(
        #[case] program: &str,
        #[case] flag: &str,
        #[case] expected: &[Problem],
    ) {
        assert_eq!(rule_codes(apply, program, &only(flag)), codes(expected));
    }

    rule_err!(
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
END_PROGRAM",
        [Problem::FunctionCallArgTypeMismatch]
    );

    rule_err!(
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
END_PROGRAM",
        [Problem::FunctionCallReturnTypeMismatch]
    );

    // --- Standard-library argument type checks ---

    // Reported once, as the argument: SIN's ANY_REAL return is bound to that
    // same BOOL argument, so its return type is not checked as well.
    rule_err!(
        apply_when_stdlib_sin_arg_is_bool_then_arg_type_error,
        "
PROGRAM main
VAR
    b : BOOL;
    r : REAL;
END_VAR
    r := SIN(b);
END_PROGRAM",
        [Problem::FunctionCallArgTypeMismatch]
    );

    // The argument fits ANY_REAL, so the narrowed REAL return is checked
    // against the BOOL target.
    rule_err!(
        apply_when_stdlib_sin_arg_fits_and_target_differs_then_return_type_error,
        "
PROGRAM main
VAR
    x : REAL;
    b : BOOL;
END_VAR
    b := SIN(x);
END_PROGRAM",
        [Problem::FunctionCallReturnTypeMismatch]
    );

    // A concrete return type does not come from the argument, so a bad
    // argument and a mismatched return are two mistakes.
    rule_err!(
        apply_when_user_function_arg_and_return_both_mismatch_then_both_reported,
        "
FUNCTION TAKES_INT : INT
VAR_INPUT
    x : INT;
END_VAR
    TAKES_INT := x;
END_FUNCTION

PROGRAM main
VAR
    r : REAL;
    b : BOOL;
END_VAR
    b := TAKES_INT(r);
END_PROGRAM",
        [
            Problem::FunctionCallReturnTypeMismatch,
            Problem::FunctionCallArgTypeMismatch
        ]
    );

    rule_ok!(
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
    rule_err!(
        apply_when_wrong_conversion_function_arg_then_arg_type_error,
        "
PROGRAM main
VAR
    u : UDINT;
    r : REAL;
END_VAR
    r := UINT_TO_REAL(u);
END_PROGRAM",
        [Problem::FunctionCallArgTypeMismatch]
    );

    rule_ok!(
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
    rule_ok!(
        apply_when_stdlib_int_literal_arg_to_real_param_then_ok,
        "
PROGRAM main
VAR
    r : REAL;
END_VAR
    r := SQRT(2.0);
END_PROGRAM"
    );

    // Call arguments go through the same resolved type, so a wide literal
    // reaches a WSTRING parameter and a narrow one does not.
    rule_ok!(
        apply_when_wstring_parameter_given_wide_literal_then_ok,
        "
FUNCTION wide_len : INT
VAR_INPUT
    w : WSTRING[10];
END_VAR
    wide_len := LEN(w);
END_FUNCTION

PROGRAM main
VAR
    n : INT;
END_VAR
    n := wide_len(\"abc\");
END_PROGRAM"
    );

    rule_err!(
        apply_when_wstring_parameter_given_narrow_literal_then_error,
        "
FUNCTION wide_len : INT
VAR_INPUT
    w : WSTRING[10];
END_VAR
    wide_len := LEN(w);
END_FUNCTION

PROGRAM main
VAR
    n : INT;
END_VAR
    n := wide_len('abc');
END_PROGRAM",
        [Problem::FunctionCallArgTypeMismatch]
    );

    // --- Assignment statement type checks (P4035) ---

    // A character-string literal is typed by its delimiter (IEC 61131-3
    // Table 5), so each spelling belongs to exactly one of the two targets.
    rule_ok!(
        apply_when_wstring_target_assigned_wide_literal_then_ok,
        "
PROGRAM main
VAR
    w : WSTRING[10];
END_VAR
    w := \"abc\";
END_PROGRAM"
    );

    rule_err!(
        apply_when_wstring_target_assigned_narrow_literal_then_error,
        "
PROGRAM main
VAR
    w : WSTRING[10];
END_VAR
    w := 'abc';
END_PROGRAM",
        [Problem::AssignmentTypeMismatch]
    );

    // A WSTRING variable assigned to a STRING one is reported once, as
    // P4034, by rule_string_encoding_compat.
    rule_ok!(
        apply_when_string_target_assigned_wstring_variable_then_left_to_encoding_rule,
        "
PROGRAM main
VAR
    s : STRING[10];
    w : WSTRING[10];
END_VAR
    s := w;
END_PROGRAM"
    );

    rule_ok!(
        apply_when_string_target_assigned_narrow_literal_then_ok,
        "
PROGRAM main
VAR
    s : STRING[10];
END_VAR
    s := 'abc';
END_PROGRAM"
    );

    rule_err!(
        apply_when_string_target_assigned_wide_literal_then_error,
        "
PROGRAM main
VAR
    s : STRING[10];
END_VAR
    s := \"abc\";
END_PROGRAM",
        [Problem::AssignmentTypeMismatch]
    );

    rule_err_at!(
        apply_when_bool_target_assigned_real_expr_then_error,
        "
PROGRAM main
VAR
    b : BOOL;
    x : REAL;
END_VAR
    b := x * 2.0;
END_PROGRAM",
        Problem::AssignmentTypeMismatch,
        "x * 2.0"
    );

    rule_err!(
        apply_when_int_target_assigned_real_var_then_error,
        "
PROGRAM main
VAR
    i : INT;
    r : REAL;
END_VAR
    i := r;
END_PROGRAM",
        [Problem::AssignmentTypeMismatch]
    );

    // INT widens losslessly to REAL, so this assignment is valid.
    rule_ok!(
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

    rule_ok!(
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
        ACCEPTED
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
        ACCEPTED
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
        ASSIGNMENT_MISMATCH
    )]
    fn apply_when_cross_family_flags_on_assignment_then_matches_expectation(
        #[case] program: &str,
        #[case] expected: &[Problem],
    ) {
        let opts = CompilerOptions {
            allow_cross_family_widening: true,
            allow_cross_family_conversion: true,
            allow_int_literal_to_bit_string: true,
            ..CompilerOptions::default()
        };
        assert_eq!(rule_codes(apply, program, &opts), codes(expected));
    }

    rule_err!(
        apply_when_dword_target_assigned_udint_var_without_flag_then_error,
        "
PROGRAM main
VAR
    dwFromUdint : DWORD;
    udValue : UDINT;
END_VAR
    dwFromUdint := udValue;
END_PROGRAM",
        [Problem::AssignmentTypeMismatch]
    );

    // Temporal short/long widths are treated as one family.
    rule_ok!(
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

    fn apply_with_methods(program: &str) -> Vec<String> {
        rule_codes(super::apply, program, &fb_inheritance_options())
    }

    rule_err!(
        /// A method's local belongs to the method. It used to be recorded
        /// against the enclosing function block, so it overwrote a field of
        /// the same name for every method compiled after it -- and the
        /// mismatch below was accepted because `v` was still recorded as the
        /// `REAL` from `A`.
        apply_when_method_local_shadows_field_then_sibling_method_uses_field_type,
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
        [Problem::AssignmentTypeMismatch],
        fb_inheritance_options()
    );

    rule_err!(
        /// A method's locals are still checked against their own declared
        /// types once they live in the method's own scope.
        apply_when_method_local_assigned_wrong_type_then_error,
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
        [Problem::AssignmentTypeMismatch],
        fb_inheritance_options()
    );

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
        .is_empty());
    }

    // ---------------------------------------------------------------------
    // Result variables. A declaration's own name is an assignment target.
    // ---------------------------------------------------------------------

    rule_err!(
        apply_when_function_result_assigned_wrong_type_then_error,
        "
FUNCTION GetFlag : BOOL
VAR
    n : INT;
END_VAR
    GetFlag := n;
END_FUNCTION",
        [Problem::AssignmentTypeMismatch]
    );

    rule_ok!(
        apply_when_function_result_assigned_correct_type_then_ok,
        "
FUNCTION GetN : INT
VAR
    n : INT;
END_VAR
    GetN := n;
END_FUNCTION"
    );

    rule_err!(
        apply_when_method_result_assigned_wrong_type_then_error,
        "
FUNCTION_BLOCK FB_Motor
METHOD GetFlag : BOOL
VAR
    n : INT;
END_VAR
    GetFlag := n;
END_METHOD
END_FUNCTION_BLOCK",
        [Problem::AssignmentTypeMismatch],
        fb_inheritance_options()
    );

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
        .is_empty());
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
        .is_empty());
    }
}
