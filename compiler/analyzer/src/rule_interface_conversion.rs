//! Semantic rule that a value used where an interface is required
//! converts to that interface (OOP extension): a function block instance
//! whose type implements the interface, or a value of an interface that
//! extends it. See [`crate::supertypes`] for the relation.
//!
//! Checked where a variable of an interface type receives a value: an
//! assignment to it, and an input argument to a function block or method
//! parameter of an interface type.
//!
//! ## Passes
//!
//! ```ignore
//! INTERFACE I_Comm
//! END_INTERFACE
//!
//! FUNCTION_BLOCK FB_Serial IMPLEMENTS I_Comm
//! END_FUNCTION_BLOCK
//!
//! PROGRAM main
//!    VAR
//!       comm : I_Comm;
//!       serial : FB_Serial;
//!    END_VAR
//!    comm := serial;
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! INTERFACE I_Comm
//! END_INTERFACE
//!
//! FUNCTION_BLOCK FB_Serial
//! END_FUNCTION_BLOCK
//!
//! PROGRAM main
//!    VAR
//!       comm : I_Comm;
//!       serial : FB_Serial;
//!    END_VAR
//!    comm := serial;
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
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    callee_resolution::{instance_type, receiver_block, FunctionBlocks},
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::ScopeTracker,
    value_type,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    let function_blocks = FunctionBlocks::from_library(lib, context.types());
    run_rule(
        RuleInterfaceConversion {
            context,
            options,
            function_blocks: &function_blocks,
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleInterfaceConversion<'a> {
    context: &'a SemanticContext,
    options: &'a CompilerOptions,
    function_blocks: &'a FunctionBlocks<'a>,
    /// The scope the walk is in, which a variable is looked up from.
    scope: ScopeTracker,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleInterfaceConversion<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl RuleInterfaceConversion<'_> {
    /// Reports `value` when it does not convert to `interface`. `target`
    /// names what receives the value, for the diagnostic.
    fn check(&mut self, interface: &TypeName, value: &Expr, target: &str) {
        // `0` makes an interface variable refer to nothing (TwinCAT
        // 3.1.4024 accepts `itf := 0;`).
        if is_zero(value) {
            return;
        }
        if let Err(mismatch) =
            value_type::check(self.context.types(), interface, value, self.options)
        {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::InterfaceNotImplemented,
                    Label::span(value.span(), "Value"),
                )
                .with_context("target", &target.to_string())
                .with_context_type("interface", interface)
                .with_context("value_type", &mismatch.actual),
            );
        }
    }

    /// Checks each input argument that binds to a parameter of an
    /// interface type. `callee` names the function block or method.
    fn check_arguments(&mut self, callee: &dyn HasVariables, params: &[ParamAssignmentKind]) {
        for (argument, parameter) in bind_inputs(callee, params) {
            let Some(parameter) = parameter else {
                continue;
            };
            let InitialValueAssignmentKind::Interface(interface) = &parameter.initializer else {
                continue;
            };
            let value = match argument {
                ParamAssignmentKind::NamedInput(named) => &named.expr,
                ParamAssignmentKind::PositionalInput(positional) => &positional.expr,
                ParamAssignmentKind::Output(_) => continue,
            };
            self.check(
                &interface.type_name,
                value,
                &parameter.identifier.to_string(),
            );
        }
    }

    /// The interface type of the variable `name`, when it is a variable of
    /// an interface type.
    fn interface_of(&self, name: &Id) -> Option<TypeName> {
        let types = self.context.types();
        let id = self
            .context
            .symbols()
            .resolve_variable(name, &self.scope.current())
            .declared()?
            .type_id?;
        types
            .get_by_id(id)?
            .representation
            .is_interface()
            .then(|| types.name_of(id).cloned())
            .flatten()
    }
}

/// Whether `value` is the integer literal `0`.
fn is_zero(value: &Expr) -> bool {
    matches!(
        &value.kind,
        ExprKind::Const(ConstantKind::IntegerLiteral(literal)) if literal.value.value.value == 0
    )
}

impl Visitor<Infallible> for RuleInterfaceConversion<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<Self::Value, Infallible> {
        if let Variable::Symbolic(SymbolicVariableKind::Named(target)) = &node.target {
            if let Some(interface) = self.interface_of(&target.name) {
                self.check(&interface, &node.value, &target.name.to_string());
            }
        }
        node.recurse_visit(self)
    }

    fn visit_fb_call(&mut self, node: &FbCall) -> Result<Self::Value, Infallible> {
        let block = instance_type(
            &node.var_name,
            &self.scope.current(),
            self.context.symbols(),
            self.context.types(),
        )
        .and_then(|name| self.function_blocks.get(&name));
        if let Some(block) = block {
            self.check_arguments(block, &node.params);
        }
        node.recurse_visit(self)
    }

    fn visit_method_call(&mut self, node: &MethodCall) -> Result<Self::Value, Infallible> {
        let method = receiver_block(
            &node.receiver,
            &self.scope.current(),
            self.context.symbols(),
            self.context.types(),
        )
        .and_then(|name| self.function_blocks.resolve_method(&name, &node.method));
        if let Some((_, method)) = method {
            self.check_arguments(method, &node.params);
        }
        node.recurse_visit(self)
    }
}

/// Pairs every input argument of a call with the declared parameter it
/// binds to: a named input (`x := e`) by name against `VAR_INPUT` and
/// `VAR_IN_OUT`, a positional input by its position among the `VAR_INPUT`
/// declarations alone, in declaration order. The parameter is `None` when
/// the callee declares nothing for the argument, which the call rules
/// report. Output assignments (`=>`) are not inputs and are not returned.
fn bind_inputs<'a>(
    owner: &'a dyn HasVariables,
    params: &'a [ParamAssignmentKind],
) -> Vec<(&'a ParamAssignmentKind, Option<&'a VarDecl>)> {
    let mut positional = owner
        .variables()
        .iter()
        .filter(|item| item.var_type == VariableType::Input);
    params
        .iter()
        .filter_map(|param| match param {
            ParamAssignmentKind::NamedInput(named) => {
                let declared = owner.variables().iter().find(|item| {
                    matches!(item.var_type, VariableType::Input | VariableType::InOut)
                        && item.identifier.symbolic_id() == Some(&named.name)
                });
                Some((param, declared))
            }
            ParamAssignmentKind::PositionalInput(_) => Some((param, positional.next())),
            ParamAssignmentKind::Output(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests;
