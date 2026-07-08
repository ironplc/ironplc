//! Semantic rule that validates function call argument types match parameter
//! types, function return types match assignment destinations, and assignment
//! statement values match their target variable types.
//!
//! Both user-defined and standard-library function calls are checked. Standard
//! library parameters use the IEC 61131-3 generic categories (ANY_REAL, ANY_NUM,
//! etc.) or concrete types (for the `<SOURCE>_TO_<TARGET>` conversion functions);
//! `are_types_compatible` handles both.
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
    // Generic expected type (a standard-library parameter such as ANY_REAL,
    // ANY_NUM, or ANY_ELEMENTARY). The concrete or generic actual type must fall
    // within the generic category. See `is_compatible_with_generic_param`.
    if let Ok(expected_generic) = GenericTypeName::try_from(&expected.name) {
        return is_compatible_with_generic_param(&expected_generic, actual, options);
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
            // Temporal types come in a short and long form (TIME/LTIME,
            // DATE/LDATE, etc.). Duration and date literals always resolve to
            // the canonical short name regardless of the written form, so treat
            // the two widths of a temporal family as interchangeable here.
            if same_temporal_family(&actual_elem, &expected_elem) {
                return true;
            }
        }
    }
    false
}

/// Returns true if both types belong to the same temporal family (the short and
/// long widths of TIME, DATE, TIME_OF_DAY, or DATE_AND_TIME).
fn same_temporal_family(a: &ElementaryTypeName, b: &ElementaryTypeName) -> bool {
    use ElementaryTypeName::*;
    fn family(t: &ElementaryTypeName) -> Option<u8> {
        match t {
            TIME | LTIME => Some(0),
            DATE | LDATE => Some(1),
            TimeOfDay | LTimeOfDay => Some(2),
            DateAndTime | LDateAndTime => Some(3),
            _ => None,
        }
    }
    matches!((family(a), family(b)), (Some(x), Some(y)) if x == y)
}

/// Returns true if `actual` is acceptable where a generic parameter type
/// `expected` (e.g. `ANY_REAL`, `ANY_NUM`) is required.
///
/// Standard-library functions declare their parameters using the IEC 61131-3
/// generic type categories. A concrete argument type is checked with
/// [`GenericTypeName::is_compatible_with`]. A generic argument type (produced for
/// untyped literals — an untyped integer literal is `ANY_INT`, an untyped real
/// literal is `ANY_REAL`) is checked against the parameter category with
/// [`generic_actual_satisfies`].
fn is_compatible_with_generic_param(
    expected: &GenericTypeName,
    actual: &TypeName,
    options: &CompilerOptions,
) -> bool {
    if let Ok(actual_elem) = ElementaryTypeName::try_from(&actual.name) {
        return expected.is_compatible_with(&actual_elem);
    }
    if let Ok(actual_generic) = GenericTypeName::try_from(&actual.name) {
        return generic_actual_satisfies(&actual_generic, expected, options);
    }
    false
}

/// Returns true if a value whose type is the generic category `actual` can be
/// used where the generic category `expected` is required.
///
/// In practice `actual` originates from an untyped literal (`ANY_INT` for integer
/// literals, `ANY_REAL` for real literals) or an unresolved generic function
/// return. The relation models the IEC 61131-3 generic-type hierarchy plus the
/// integer-literal-to-real inference from ADR-0028 and the flag-gated
/// integer-literal-to-bit-string case from ADR-0031.
fn generic_actual_satisfies(
    actual: &GenericTypeName,
    expected: &GenericTypeName,
    options: &CompilerOptions,
) -> bool {
    use GenericTypeName::*;
    if actual == expected {
        return true;
    }
    match expected {
        Any | AnyElementary => true,
        AnyMagnitude => matches!(actual, AnyInt | AnyReal | AnyNum | AnyMagnitude),
        AnyNum => matches!(actual, AnyInt | AnyReal | AnyNum),
        // Integer literals infer as real (ADR-0028).
        AnyReal => matches!(actual, AnyReal | AnyInt),
        AnyInt => matches!(actual, AnyInt),
        // Integer literals to bit-string require the widening flag (ADR-0031).
        AnyBit => {
            matches!(actual, AnyBit) || (options.allow_cross_family_widening && *actual == AnyInt)
        }
        AnyString => matches!(actual, AnyString),
        AnyDate => matches!(actual, AnyDate),
        AnyDerived => false,
    }
}

/// Returns true if the type name is checkable in an assignment: an elementary
/// type or a generic category (untyped literal). User-defined types (enums,
/// structures, function blocks, arrays, sized strings, references) return false
/// so that the assignment check skips them.
fn is_checkable_assignment_type(type_name: &TypeName) -> bool {
    ElementaryTypeName::try_from(&type_name.name).is_ok()
        || GenericTypeName::try_from(&type_name.name).is_ok()
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
        let Some(declared) = self.var_types.get(&nv.name) else {
            return;
        };
        // Resolve aliases/subranges to the underlying elementary type so the
        // comparison matches the already-resolved right-hand side type.
        let target_type = self
            .context
            .types()
            .resolve_elementary_type_name(declared)
            .unwrap_or_else(|| declared.clone());
        if !is_checkable_assignment_type(&target_type) {
            return;
        }

        let Some(value_type) = &value.resolved_type else {
            return;
        };
        if !is_checkable_assignment_type(value_type) {
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
        self.check_assignment_type(&node.target, &node.value);
        node.recurse_visit(self)
    }

    fn visit_function(&mut self, node: &Function) -> Result<Self::Value, Diagnostic> {
        let func_sig = self.context.functions.get(&node.name);

        if let Some(signature) = func_sig {
            // Check each positional argument type against the parameter type.
            // Standard-library functions are checked too: their parameters use
            // generic ANY_* categories (or concrete types for the conversion
            // functions), all handled by `are_types_compatible`.
            let input_params: Vec<_> = signature.parameters.iter().filter(|p| p.is_input).collect();

            // Emit NotImplemented for output arguments on user-defined functions.
            // Standard-library functions do not take output arguments.
            if !signature.is_stdlib() {
                for p in &node.param_assignment {
                    if let ParamAssignmentKind::Output(_) = p {
                        self.diagnostics.push(Diagnostic::problem(
                            Problem::NotImplemented,
                            Label::span(node.name.span(), "Function call with output argument"),
                        ));
                    }
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
    fn apply_when_int_arg_to_real_param_lossless_then_ok() {
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
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_dint_arg_to_real_param_lossy_then_error() {
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
    x : DINT;
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
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("INT"),
            &TypeName::from("INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_to_int_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("INT"),
            &TypeName::from("ANY_INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_to_dint_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("DINT"),
            &TypeName::from("ANY_INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_real_to_real_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("REAL"),
            &TypeName::from("ANY_REAL"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_real_to_lreal_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("LREAL"),
            &TypeName::from("ANY_REAL"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_to_real_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("REAL"),
            &TypeName::from("ANY_INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_to_lreal_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("LREAL"),
            &TypeName::from("ANY_INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_dint_to_int_then_false() {
        let opts = CompilerOptions::default();
        assert!(!are_types_compatible(
            &TypeName::from("INT"),
            &TypeName::from("DINT"),
            &opts,
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

    // --- Implicit integer widening tests (ADR-0029) ---

    #[test]
    fn apply_when_sint_arg_to_int_param_then_ok() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_int_arg_to_dint_param_then_ok() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_sint_arg_to_lint_param_then_ok() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_usint_arg_to_uint_param_then_ok() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_usint_arg_to_int_param_then_ok() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_uint_arg_to_dint_param_then_ok() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_sint_return_to_dint_var_then_ok() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_dint_arg_to_int_param_then_error() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_int_arg_to_uint_param_then_error() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_byte_arg_to_int_param_then_error() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn are_types_compatible_when_int_to_dint_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("DINT"),
            &TypeName::from("INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_usint_to_int_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("INT"),
            &TypeName::from("USINT"),
            &opts,
        ));
    }

    // --- Standard widening tests (ADR-0031) ---

    #[test]
    fn are_types_compatible_when_int_to_real_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("REAL"),
            &TypeName::from("INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_dint_to_real_then_false() {
        let opts = CompilerOptions::default();
        assert!(!are_types_compatible(
            &TypeName::from("REAL"),
            &TypeName::from("DINT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_byte_to_word_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("WORD"),
            &TypeName::from("BYTE"),
            &opts,
        ));
    }

    // --- Integration tests for new widening cases ---

    #[test]
    fn apply_when_int_arg_to_real_param_then_ok() {
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
    y : INT;
END_VAR
    result := TAKES_REAL(y);
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_dint_arg_to_real_param_then_error() {
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
    y : DINT;
END_VAR
    result := TAKES_REAL(y);
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_byte_arg_to_word_param_then_ok() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_word_arg_to_byte_param_then_error() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_real_arg_to_int_param_then_error() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    // --- Cross-family widening tests (ADR-0031, requires flag) ---

    #[test]
    fn apply_when_byte_arg_to_int_param_with_flag_then_ok() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let opts = CompilerOptions {
            allow_cross_family_widening: true,
            ..CompilerOptions::default()
        };
        let result = apply(&library, &context, &opts);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_literal_zero_to_byte_param_with_flag_then_ok() {
        let program = "
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
        let (library, context) = parse_and_resolve_types_with_context(program);
        let opts = CompilerOptions {
            allow_cross_family_widening: true,
            ..CompilerOptions::default()
        };
        let result = apply(&library, &context, &opts);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_literal_zero_to_byte_param_without_flag_then_error() {
        let program = "
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
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_byte_return_to_int_var_with_flag_then_ok() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let opts = CompilerOptions {
            allow_cross_family_widening: true,
            ..CompilerOptions::default()
        };
        let result = apply(&library, &context, &opts);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_byte_return_to_int_var_without_flag_then_error() {
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_int_arg_to_byte_param_with_flag_then_error() {
        // Integer → bit-string is never allowed, even with flag
        let program = "
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
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let opts = CompilerOptions {
            allow_cross_family_widening: true,
            ..CompilerOptions::default()
        };
        let result = apply(&library, &context, &opts);
        assert!(result.is_err());
    }

    // --- Standard-library argument type checks ---

    #[test]
    fn apply_when_stdlib_sin_arg_is_bool_then_arg_type_error() {
        let program = "
PROGRAM main
VAR
    b : BOOL;
    r : REAL;
END_VAR
    r := SIN(b);
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        let diagnostics = result.unwrap_err();
        assert!(diagnostics
            .iter()
            .any(|d| d.code == Problem::FunctionCallArgTypeMismatch.code()));
    }

    #[test]
    fn apply_when_stdlib_sin_arg_is_real_then_ok() {
        let program = "
PROGRAM main
VAR
    x : REAL;
    r : REAL;
END_VAR
    r := SIN(x);
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_wrong_conversion_function_arg_then_arg_type_error() {
        // UINT_TO_REAL expects UINT, but the argument is UDINT.
        let program = "
PROGRAM main
VAR
    u : UDINT;
    r : REAL;
END_VAR
    r := UINT_TO_REAL(u);
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Problem::FunctionCallArgTypeMismatch.code()
        );
    }

    #[test]
    fn apply_when_correct_conversion_function_arg_then_ok() {
        let program = "
PROGRAM main
VAR
    u : UDINT;
    r : REAL;
END_VAR
    r := UDINT_TO_REAL(u);
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_stdlib_int_literal_arg_to_real_param_then_ok() {
        // ABS accepts ANY_NUM; a bare integer literal is accepted.
        let program = "
PROGRAM main
VAR
    r : REAL;
END_VAR
    r := SQRT(2.0);
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    // --- Assignment statement type checks (P4035) ---

    #[test]
    fn apply_when_bool_target_assigned_real_expr_then_error() {
        let program = "
PROGRAM main
VAR
    b : BOOL;
    x : REAL;
END_VAR
    b := x * 2.0;
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, Problem::AssignmentTypeMismatch.code());
    }

    #[test]
    fn apply_when_int_target_assigned_real_var_then_error() {
        let program = "
PROGRAM main
VAR
    i : INT;
    r : REAL;
END_VAR
    i := r;
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        let diagnostics = result.unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, Problem::AssignmentTypeMismatch.code());
    }

    #[test]
    fn apply_when_real_target_assigned_int_var_then_ok() {
        // INT widens losslessly to REAL, so this assignment is valid.
        let program = "
PROGRAM main
VAR
    i : INT;
    r : REAL;
END_VAR
    r := i;
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_matching_assignment_then_ok() {
        let program = "
PROGRAM main
VAR
    i : INT;
    j : INT;
END_VAR
    i := j + 1;
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_ltime_target_assigned_time_var_then_ok() {
        // Temporal short/long widths are treated as one family.
        let program = "
PROGRAM main
VAR
    lt : LTIME;
    t : TIME;
END_VAR
    lt := t;
END_PROGRAM";
        let (library, context) = parse_and_resolve_types_with_context(program);
        let result = apply(&library, &context, &CompilerOptions::default());
        assert!(result.is_ok());
    }

    // --- are_types_compatible: generic expected (stdlib parameters) ---

    #[test]
    fn are_types_compatible_when_any_real_expected_real_actual_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("ANY_REAL"),
            &TypeName::from("REAL"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_real_expected_bool_actual_then_false() {
        let opts = CompilerOptions::default();
        assert!(!are_types_compatible(
            &TypeName::from("ANY_REAL"),
            &TypeName::from("BOOL"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_num_expected_int_actual_then_true() {
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("ANY_NUM"),
            &TypeName::from("INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_real_expected_any_int_actual_then_true() {
        // Untyped integer literal (ANY_INT) inferred as real (ADR-0028).
        let opts = CompilerOptions::default();
        assert!(are_types_compatible(
            &TypeName::from("ANY_REAL"),
            &TypeName::from("ANY_INT"),
            &opts,
        ));
    }

    #[test]
    fn are_types_compatible_when_any_int_expected_any_real_actual_then_false() {
        let opts = CompilerOptions::default();
        assert!(!are_types_compatible(
            &TypeName::from("ANY_INT"),
            &TypeName::from("ANY_REAL"),
            &opts,
        ));
    }
}
