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
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    call_assignment_check::bind_inputs,
    callee_resolution::{FunctionBlocks, InstanceTypes},
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    value_type,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    let function_blocks = FunctionBlocks::from_library(lib);
    run_rule(
        RuleInterfaceConversion {
            context,
            options,
            function_blocks: &function_blocks,
            instances: InstanceTypes::default(),
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleInterfaceConversion<'a> {
    context: &'a SemanticContext,
    options: &'a CompilerOptions,
    function_blocks: &'a FunctionBlocks<'a>,
    /// The instances and interface variables declared in the unit being
    /// walked.
    instances: InstanceTypes,
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

    fn leave_unit(&mut self, result: Result<(), Infallible>) -> Result<(), Infallible> {
        self.instances.clear();
        result
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

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let result = node.recurse_visit(self);
        self.leave_unit(result)
    }

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let result = node.recurse_visit(self);
        self.leave_unit(result)
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let result = node.recurse_visit(self);
        self.leave_unit(result)
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Infallible> {
        self.instances.declare(node);
        Ok(())
    }

    fn visit_assignment(&mut self, node: &Assignment) -> Result<Self::Value, Infallible> {
        if let Variable::Symbolic(SymbolicVariableKind::Named(target)) = &node.target {
            if let Some(interface) = self.instances.interface_of(&target.name).cloned() {
                self.check(&interface, &node.value, &target.name.to_string());
            }
        }
        node.recurse_visit(self)
    }

    fn visit_fb_call(&mut self, node: &FbCall) -> Result<Self::Value, Infallible> {
        let block = self
            .instances
            .type_of(&node.var_name)
            .and_then(|name| self.function_blocks.get(name));
        if let Some(block) = block {
            self.check_arguments(block, &node.params);
        }
        node.recurse_visit(self)
    }

    fn visit_method_call(&mut self, node: &MethodCall) -> Result<Self::Value, Infallible> {
        let method = match &node.receiver {
            MethodReceiver::Instance(instance) => self
                .instances
                .type_of(instance)
                .and_then(|name| self.function_blocks.resolve_method(name, &node.method)),
            // `THIS^`/`SUPER^` receivers are not resolved yet (#1888).
            MethodReceiver::SelfRef(_) => None,
        };
        if let Some((_, method)) = method {
            self.check_arguments(method, &node.params);
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests;
