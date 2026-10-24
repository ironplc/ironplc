//! Semantic rule that an input of a function block call or a method call is
//! of a type its variable accepts.
//!
//! A function's inputs are checked against its parameters by
//! `rule_function_call_type_check` (P4026). An input of a function block
//! call is stored in a variable of the instance, and an input of a method
//! call in a variable of the method, in the same way, so each is checked
//! against its variable's type by the same relation an assignment is, and a
//! mismatch is P4026 too: `f(i := s)` with `i : DINT` and `s : STRING`, or
//! `f.Go(d := b)` with `d : DINT` and `b : BOOL`. A value that widens to the
//! variable's type passes (ADR-0029, ADR-0031), and a narrowing one does
//! not: `f(i := u)` with `u : UDINT` is P4026, as `F(u)` is for a function's
//! `DINT` parameter.
//!
//! As for an assignment, only a variable whose type the relation can judge
//! (an elementary type, or an alias or a subrange of one) is checked; an
//! input of another type, and an input whose variable or value type is not
//! known, is left alone.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK Counter
//! VAR_INPUT
//!     step : DINT;
//! END_VAR
//! END_FUNCTION_BLOCK
//!
//! PROGRAM main
//! VAR
//!     c : Counter;
//!     n : INT;
//! END_VAR
//!     c(step := n);
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//! VAR
//!     c : Counter;
//!     s : STRING;
//!     u : UDINT;
//! END_VAR
//!     c(step := s);       (* P4026: a STRING is not a DINT *)
//!     c(step := u);       (* P4026: a UDINT does not fit a DINT *)
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::Library,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::{Expr, FbCall, MethodCall, MethodReceiver, ParamAssignmentKind},
    type_id::TypeId,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::ScopeTracker,
    type_compat::is_checkable_type,
    value_type::{self, ValueType},
    variable_type::declared_type_id,
};
use ironplc_dsl::common::VariableType;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleCallInputType {
            context,
            options,
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleCallInputType<'a> {
    context: &'a SemanticContext,
    options: &'a CompilerOptions,
    /// Where the traversal is, to look the called instance up.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleCallInputType<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

/// What a call names, for the diagnostic: the instance of a function block
/// call, or the method of a method call.
struct Callee<'n> {
    kind: &'static str,
    name: &'n Id,
}

impl RuleCallInputType<'_> {
    /// The id of the function block type of the instance `instance` names,
    /// when it names one.
    fn block_of(&self, instance: &Id) -> Option<TypeId> {
        let block = declared_type_id(instance, self.context.symbols(), &self.scope.current())?;
        self.context
            .types()
            .get_by_id(block)?
            .representation
            .is_function_block()
            .then_some(block)
    }

    /// Reports P4026 when `value`, stored in the input `input` of `callee`
    /// whose variable has the type `variable_type`, is of a type the variable
    /// does not accept.
    fn check_input(&mut self, callee: &Callee, input: &Id, variable_type: TypeId, value: &Expr) {
        let types = self.context.types();
        let Some(declared) = types.name_of(variable_type) else {
            return;
        };
        // An alias or a subrange is checked as the elementary type it names,
        // as the value's type is.
        let expected = types
            .resolve_elementary_type_name(declared)
            .unwrap_or_else(|| declared.clone());
        if !is_checkable_type(&expected) {
            return;
        }
        let checkable = match value_type::of(types, value) {
            None => false,
            Some(ValueType::Scalar(value_type)) => is_checkable_type(&value_type),
            Some(ValueType::Composite(_)) => true,
        };
        if !checkable {
            return;
        }
        if let Err(mismatch) = value_type::check(types, &expected, value, self.options) {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::FunctionCallArgTypeMismatch,
                    Label::span(value.span(), "Input value"),
                )
                .with_context(callee.kind, &callee.name.original().to_string())
                .with_context("parameter", &input.original().to_string())
                .with_context("expected", &expected.to_string())
                .with_context("actual", &mismatch.actual),
            );
        }
    }
}

impl Visitor<Infallible> for RuleCallInputType<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    /// The binding pass has written every call that binds with named
    /// inputs; a positional input is left only in a call that does not
    /// bind, which other rules report.
    fn visit_fb_call(&mut self, node: &FbCall) -> Result<Self::Value, Infallible> {
        if let Some(block) = self.block_of(&node.var_name) {
            let callee = Callee {
                kind: "function_block",
                name: &node.var_name,
            };
            for param in &node.params {
                if let ParamAssignmentKind::NamedInput(input) = param {
                    if let Some(field_type) = self.context.types().field_type(block, &input.name) {
                        self.check_input(&callee, &input.name, field_type, &input.expr);
                    }
                }
            }
        }
        node.recurse_visit(self)
    }

    fn visit_method_call(&mut self, node: &MethodCall) -> Result<Self::Value, Infallible> {
        let block = match &node.receiver {
            MethodReceiver::Instance(instance) => self.block_of(instance),
            // `THIS^.M()` and `SUPER^.M()` are not checked here: the
            // receiver names no variable to look the block up by.
            MethodReceiver::SelfRef(_) => None,
        };
        let signature = block.and_then(|block| self.context.types().method(block, &node.method));
        if let Some((_, signature)) = signature {
            let callee = Callee {
                kind: "method",
                name: &node.method,
            };
            let inputs: Vec<(Id, TypeId)> = signature
                .parameters
                .iter()
                .filter(|parameter| parameter.section == VariableType::Input)
                .filter_map(|parameter| Some((parameter.name.clone(), parameter.type_id?)))
                .collect();
            for param in &node.params {
                if let ParamAssignmentKind::NamedInput(input) = param {
                    if let Some((_, input_type)) =
                        inputs.iter().find(|(name, _)| *name == input.name)
                    {
                        self.check_input(&callee, &input.name, *input_type, &input.expr);
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
    use crate::test_helpers::{diagnostic_codes, edition3_options, rule_diagnostics};
    use rstest::rstest;
    use spec_test_macro::spec_test;

    /// A program calling an instance `f` of a block with a `DINT` input `i`,
    /// and a method `Go` of it with a `DINT` input `d`, whose body is `body`.
    fn program(body: &str) -> String {
        format!(
            "
FUNCTION_BLOCK Counter
VAR_INPUT
    i : DINT;
END_VAR
METHOD Go
VAR_INPUT
    d : DINT;
END_VAR
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    f : Counter;
    n : INT;
    k : DINT;
    u : UDINT;
    l : LINT;
    r : REAL;
    b : BOOL;
    s : STRING;
    t : TON;
END_VAR
    {body}
END_PROGRAM"
        )
    }

    fn codes_for(body: &str) -> Vec<String> {
        diagnostic_codes(&rule_diagnostics(
            apply,
            &program(body),
            &edition3_options(),
        ))
        .into_iter()
        .map(str::to_string)
        .collect()
    }

    #[spec_test(REQ_IC_analyzer_123)]
    #[rstest]
    #[case::same_type("f(i := k);")]
    #[case::widened("f(i := n);")]
    #[case::literal("f(i := 3);")]
    #[case::expression("f(i := k + 1);")]
    #[case::method_input("f.Go(d := n);")]
    #[case::standard_block("t(IN := b, PT := T#1s);")]
    fn apply_when_input_accepted_then_ok(#[case] body: &str) {
        let codes = codes_for(body);
        assert!(codes.is_empty(), "{codes:?}");
    }

    #[spec_test(REQ_IC_analyzer_123)]
    #[rstest]
    #[case::string("f(i := s);")]
    #[case::bool("f(i := b);")]
    #[case::real("f(i := r);")]
    #[case::narrowed_unsigned("f(i := u);")]
    #[case::narrowed_wider("f(i := l);")]
    #[case::method_string("f.Go(d := s);")]
    #[case::method_bool("f.Go(d := b);")]
    #[case::method_narrowed("f.Go(d := l);")]
    #[case::standard_block_time("t(IN := b, PT := k);")]
    fn apply_when_input_not_accepted_then_p4026(#[case] body: &str) {
        assert_eq!(
            codes_for(body),
            [Problem::FunctionCallArgTypeMismatch.code()]
        );
    }

    #[test]
    fn apply_when_input_not_accepted_then_p4026_at_value() {
        let program = program("f(i := s);");
        let diagnostics = rule_diagnostics(apply, &program, &edition3_options());

        assert_eq!(
            diagnostic_codes(&diagnostics),
            [Problem::FunctionCallArgTypeMismatch.code()]
        );
        let start = program.find("i := s").unwrap() + "i := ".len();
        let location = &diagnostics[0].primary.location;
        assert_eq!((location.start, location.end), (start, start + 1));
    }

    rule_ok!(
        /// A method of the enclosing instance names no variable to look the
        /// block up by, so its inputs are left alone.
        apply_when_method_called_on_this_then_ok,
        "
FUNCTION_BLOCK Counter
METHOD Go
VAR_INPUT
    d : DINT;
END_VAR
END_METHOD
METHOD Run
VAR
    s : STRING;
END_VAR
    THIS^.Go(d := s);
END_METHOD
END_FUNCTION_BLOCK",
        edition3_options()
    );
}
