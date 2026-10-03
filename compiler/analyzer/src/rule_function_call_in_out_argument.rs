//! Semantic rule that validates the arguments a function call passes to
//! `VAR_IN_OUT` parameters.
//!
//! A `VAR_IN_OUT` parameter is passed by reference: the function reads and
//! writes the caller's variable. So the argument must be a variable
//! (P4058), and its type must be the parameter's type exactly (P4059). An
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
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    intermediate_type::{FunctionBlockVarType, IntermediateType},
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::{ScopeTracker, SymbolInfo, SymbolKind},
    type_compat::is_checkable_type,
    variable_type,
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
            scope: ScopeTracker::default(),
        },
        lib,
    )
}

struct RuleFunctionCallInOutArgument<'a> {
    context: &'a SemanticContext,
    diagnostics: Vec<Diagnostic>,
    /// Where the traversal is, to look variables up in the symbol
    /// environment.
    scope: ScopeTracker,
}

/// Whether the variable `info` describes can be proved writable.
///
/// A variable is provably writable when it is declared without `CONSTANT`
/// in a section the declaring POU may assign. A `VAR_INPUT` is not: its
/// value came from the POU's caller, which may have passed a constant. A
/// `VAR_IN_OUT` is: every call that bound it proved its own argument
/// writable. A function's or method's result variable is assigned by its
/// body.
fn is_declared_writable(info: &SymbolInfo) -> bool {
    if info.kind == SymbolKind::ResultVariable {
        return true;
    }
    let section = match info.variable_type {
        Some(
            VariableType::Var
            | VariableType::VarTemp
            | VariableType::Output
            | VariableType::InOut
            | VariableType::External
            | VariableType::Global,
        ) => true,
        Some(VariableType::Input | VariableType::Access) | None => false,
    };
    section && !info.is_constant()
}

impl DiagnosticVisitor for RuleFunctionCallInOutArgument<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleFunctionCallInOutArgument<'_> {
    /// Returns whether `variable` can be proved writable: its root variable
    /// is, and no step from the root selects something the caller may not
    /// assign. Anything that cannot be proved is not writable.
    fn is_writable(&self, variable: &Variable) -> bool {
        match variable {
            Variable::Symbolic(kind) => self.is_symbolic_writable(kind),
            // A process input is written by the I/O image, not the program.
            Variable::Direct(address) => address.location != LocationPrefix::I,
        }
    }

    /// Whether the variable `name` names from the current scope can be
    /// proved writable.
    fn is_name_writable(&self, name: &Id) -> bool {
        self.context
            .symbols()
            .find(name, &self.scope.current())
            .is_some_and(is_declared_writable)
    }

    fn is_symbolic_writable(&self, kind: &SymbolicVariableKind) -> bool {
        match kind {
            SymbolicVariableKind::Named(named) => self.is_name_writable(&named.name),
            SymbolicVariableKind::Array(array) => {
                self.is_symbolic_writable(&array.subscripted_variable)
            }
            SymbolicVariableKind::Structured(structured) => {
                if !self.is_symbolic_writable(&structured.record) {
                    return false;
                }
                // Only a function block's inputs are assignable from
                // outside the instance; its outputs, locals and VAR_IN_OUT
                // are not.
                match variable_type::of(&structured.record, self.context, &self.scope.current()) {
                    Some(IntermediateType::FunctionBlock { fields, .. }) => fields
                        .iter()
                        .find(|field| field.name == structured.field)
                        .is_some_and(|field| {
                            matches!(field.var_type, Some(FunctionBlockVarType::Input))
                        }),
                    Some(_) => true,
                    None => false,
                }
            }
            // A reference may target a variable whose origin is constant,
            // and THIS^/SUPER^ are not variables of the caller.
            SymbolicVariableKind::Deref(_) | SymbolicVariableKind::SelfRef(_) => false,
            // Not a variable a reference can name; reported as P4058.
            SymbolicVariableKind::BitAccess(_) | SymbolicVariableKind::PartialAccess(_) => false,
        }
    }

    /// Names the type of `arg`'s value, resolved by its type id: the
    /// elementary type for an elementary type or an alias of one, and the
    /// type's own name otherwise. A reference is `REF_TO`, never the type it
    /// references, so it cannot pass for that type. `None` when the analyzer
    /// resolved no type.
    fn argument_type(&self, arg: &Expr) -> Option<TypeName> {
        // A variable declared REF_TO is a reference whatever type id its
        // reads carry.
        let name = match &arg.kind {
            ExprKind::Variable(Variable::Symbolic(SymbolicVariableKind::Named(named))) => {
                Some(&named.name)
            }
            ExprKind::LateBound(late_bound) => Some(&late_bound.value),
            _ => None,
        };
        let declared = name
            .and_then(|name| variable_type::declared(name, self.context, &self.scope.current()));
        if declared.is_some_and(IntermediateType::is_reference) {
            return Some(TypeName::from("REF_TO"));
        }
        let Some(ExprType::Concrete(id)) = &arg.expr_type else {
            return None;
        };
        let types = self.context.types();
        let representation = &types.get_by_id(*id)?.representation;
        if representation.is_reference() {
            return Some(TypeName::from("REF_TO"));
        }
        types
            .elementary_type_name_for(representation)
            .or_else(|| types.name_of(*id).cloned())
    }

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

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_function(&mut self, node: &Function) -> Result<Self::Value, Infallible> {
        if let Some(signature) = self.context.functions.get(&node.name) {
            for (param, arg) in signature.bind_inputs(&node.param_assignment) {
                if !param.is_inout {
                    continue;
                }

                // A bit or partial access selects part of a variable, which
                // no reference can name.
                let is_variable = match &arg.kind {
                    ExprKind::Variable(Variable::Symbolic(
                        SymbolicVariableKind::BitAccess(_) | SymbolicVariableKind::PartialAccess(_),
                    )) => false,
                    ExprKind::Variable(_) | ExprKind::LateBound(_) => true,
                    _ => false,
                };
                if !is_variable {
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

                let writable = match &arg.kind {
                    ExprKind::Variable(variable) => self.is_writable(variable),
                    ExprKind::LateBound(late_bound) => self.is_name_writable(&late_bound.value),
                    _ => false,
                };
                if !writable {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::InOutArgNotWritable,
                            Label::span(arg.span(), "Argument"),
                        )
                        .with_context("function", &node.name.original().to_string())
                        .with_context("parameter", &param.name.original().to_string()),
                    );
                }

                // A REF_TO parameter's type is its target's; leave it to the
                // reference rules. A parameter of another non-elementary type
                // is not compared.
                if param.is_reference {
                    continue;
                }
                let Some(expected) = self.elementary(&param.param_type) else {
                    continue;
                };
                let Some(actual) = self.argument_type(arg) else {
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
    use crate::test_helpers::rule_codes;
    use crate::test_helpers::{diagnostic_codes, edition3_options, rule_diagnostics};
    use rstest::rstest;

    const INC: &str = "
TYPE
    MyDint : DINT := 0;
    Pair : STRUCT a : DINT; b : DINT; END_STRUCT;
END_TYPE

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

    fn apply_to_call(vars: &str, call: &str) -> Vec<Diagnostic> {
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
        rule_diagnostics(apply, &program, &CompilerOptions::default())
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
        let diagnostics = apply_to_call(vars, call);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[rstest]
    #[case::literal("", "INC(1, 2)")]
    #[case::expression("x : DINT;", "INC(1, x + 1)")]
    #[case::named_literal("", "INC(step := 1, data := 2)")]
    fn apply_when_in_out_argument_is_not_variable_then_p4057(
        #[case] vars: &str,
        #[case] call: &str,
    ) {
        let errors = apply_to_call(vars, call);
        assert_eq!(
            diagnostic_codes(&errors),
            [Problem::FunctionCallInOutArgNotVariable.code()]
        );
        assert!(errors[0].described.contains(&"parameter=data".to_owned()));
    }

    #[rstest]
    #[case::narrower_integer("x : INT;", "int")]
    #[case::other_category("x : REAL;", "real")]
    #[case::structure("x : Pair;", "Pair")]
    fn apply_when_in_out_argument_type_differs_then_p4058(
        #[case] vars: &str,
        #[case] actual: &str,
    ) {
        let errors = apply_to_call(vars, "INC(1, x)");
        assert_eq!(
            diagnostic_codes(&errors),
            [Problem::FunctionCallInOutArgTypeMismatch.code()]
        );
        assert!(
            errors[0].described.contains(&format!("actual={actual}")),
            "{:?}",
            errors[0].described
        );
    }

    /// Runs the rule on `INC` plus `caller`, a POU that calls it.
    fn apply_to_caller(caller: &str) -> Vec<Diagnostic> {
        let program = format!("{INC}\n{caller}");
        rule_diagnostics(apply, &program, &CompilerOptions::default())
    }

    #[rstest]
    #[case::local("PROGRAM main VAR x : DINT; r : DINT; END_VAR r := INC(1, x); END_PROGRAM")]
    #[case::output(
        "FUNCTION_BLOCK FB VAR_OUTPUT q : DINT; END_VAR VAR r : DINT; END_VAR r := INC(1, q); END_FUNCTION_BLOCK"
    )]
    #[case::forwarded_in_out(
        "FUNCTION F : DINT VAR_IN_OUT v : DINT; END_VAR F := INC(1, v); END_FUNCTION"
    )]
    #[case::function_result("FUNCTION F : DINT F := 0; F := INC(1, F); END_FUNCTION")]
    #[case::array_element(
        "PROGRAM main VAR a : ARRAY[0..2] OF DINT; r : DINT; END_VAR r := INC(1, a[1]); END_PROGRAM"
    )]
    #[case::structure_field(
        "PROGRAM main VAR p : Pair; r : DINT; END_VAR r := INC(1, p.a); END_PROGRAM"
    )]
    #[case::function_block_input(
        "FUNCTION_BLOCK Inner VAR_INPUT i : DINT; END_VAR END_FUNCTION_BLOCK
PROGRAM main VAR fb : Inner; r : DINT; END_VAR r := INC(1, fb.i); END_PROGRAM"
    )]
    fn apply_when_in_out_argument_provably_writable_then_ok(#[case] caller: &str) {
        let diagnostics = apply_to_caller(caller);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[rstest]
    #[case::constant(
        "PROGRAM main VAR CONSTANT c : DINT := 1; END_VAR VAR r : DINT; END_VAR r := INC(1, c); END_PROGRAM"
    )]
    #[case::constant_array_element(
        "PROGRAM main VAR CONSTANT a : ARRAY[0..1] OF DINT := [1, 2]; END_VAR VAR r : DINT; END_VAR r := INC(1, a[0]); END_PROGRAM"
    )]
    // A constant passed into a function block's input, which the function
    // block then binds to a VAR_IN_OUT.
    #[case::function_block_input_passed_on(
        "FUNCTION_BLOCK FB VAR_INPUT i : DINT; END_VAR VAR r : DINT; END_VAR r := INC(1, i); END_FUNCTION_BLOCK"
    )]
    #[case::function_input(
        "FUNCTION F : DINT VAR_INPUT i : DINT; END_VAR F := INC(1, i); END_FUNCTION"
    )]
    #[case::function_block_output(
        "FUNCTION_BLOCK Inner VAR_OUTPUT q : DINT; END_VAR END_FUNCTION_BLOCK
PROGRAM main VAR fb : Inner; r : DINT; END_VAR r := INC(1, fb.q); END_PROGRAM"
    )]
    #[case::function_block_local(
        "FUNCTION_BLOCK Inner VAR l : DINT; END_VAR END_FUNCTION_BLOCK
PROGRAM main VAR fb : Inner; r : DINT; END_VAR r := INC(1, fb.l); END_PROGRAM"
    )]
    fn apply_when_in_out_argument_not_provably_writable_then_p4059(#[case] caller: &str) {
        let errors = apply_to_caller(caller);
        assert_eq!(
            diagnostic_codes(&errors),
            [Problem::InOutArgNotWritable.code()]
        );
        assert!(errors[0].described.contains(&"parameter=data".to_owned()));
    }

    #[test]
    fn apply_when_in_out_argument_is_constant_global_then_p4059() {
        let diagnostics = apply_to_caller(
            "PROGRAM main
VAR_EXTERNAL CONSTANT g : DINT; END_VAR
VAR r : DINT; END_VAR
    r := INC(1, g);
END_PROGRAM
CONFIGURATION config
    VAR_GLOBAL CONSTANT g : DINT := 1; END_VAR
    RESOURCE res ON PLC
        TASK t(INTERVAL := T#100ms, PRIORITY := 1);
        PROGRAM p WITH t : main;
    END_RESOURCE
END_CONFIGURATION",
        );
        let errors = diagnostics;
        let codes = diagnostic_codes(&errors);
        assert_eq!(codes, [Problem::InOutArgNotWritable.code()], "{errors:?}");
    }

    // A reference is not the type it references: binding it would let the
    // function write a DINT over the reference.
    #[test]
    fn apply_when_in_out_argument_is_reference_then_type_mismatch() {
        let program = format!(
            "{INC}
PROGRAM main
VAR x : DINT; p : REF_TO DINT; r : DINT; END_VAR
    p := REF(x);
    r := INC(1, p);
END_PROGRAM"
        );
        let codes = rule_codes(apply, &program, &edition3_options());
        assert_eq!(codes, [Problem::FunctionCallInOutArgTypeMismatch.code()]);
    }

    #[test]
    fn apply_when_in_out_argument_is_bit_access_then_p4057() {
        let errors = apply_to_caller(
            "FUNCTION TOGGLE : BOOL VAR_IN_OUT b : BOOL; END_VAR TOGGLE := b; END_FUNCTION
PROGRAM main VAR w : WORD; r : BOOL; END_VAR r := TOGGLE(w.3); END_PROGRAM",
        );
        assert_eq!(
            diagnostic_codes(&errors),
            [Problem::FunctionCallInOutArgNotVariable.code()]
        );
    }

    rule_ok!(
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

    fn apply_to_caller_ed3(caller: &str) -> Vec<Diagnostic> {
        let program = format!("{INC}\n{caller}");
        rule_diagnostics(apply, &program, &edition3_options())
    }

    #[rstest]
    #[case::method_result(
        "FUNCTION_BLOCK FB METHOD M : DINT M := 0; M := INC(1, M); END_METHOD END_FUNCTION_BLOCK"
    )]
    #[case::method_local(
        "FUNCTION_BLOCK FB METHOD M VAR x : DINT; r : DINT; END_VAR r := INC(1, x); END_METHOD END_FUNCTION_BLOCK"
    )]
    #[case::field_from_method(
        "FUNCTION_BLOCK FB VAR f : DINT; r : DINT; END_VAR METHOD M r := INC(1, f); END_METHOD END_FUNCTION_BLOCK"
    )]
    fn apply_when_in_out_argument_in_method_provably_writable_then_ok(#[case] caller: &str) {
        let diagnostics = apply_to_caller_ed3(caller);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn apply_when_in_out_argument_is_method_input_then_p4059() {
        let errors = apply_to_caller_ed3(
            "FUNCTION_BLOCK FB VAR r : DINT; END_VAR METHOD M VAR_INPUT i : DINT; END_VAR r := INC(1, i); END_METHOD END_FUNCTION_BLOCK",
        );
        assert_eq!(
            diagnostic_codes(&errors),
            [Problem::InOutArgNotWritable.code()]
        );
    }
}
