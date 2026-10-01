//! Semantic rule that program organization units respect the IEC 61131-3
//! call hierarchy.
//!
//! A program may invoke functions and function blocks; a function block may
//! invoke functions and function blocks; a function may invoke only
//! functions (Ed.2 §2.5.1, Ed.3 §6.6.1). The distinction is state: a function
//! has none, so it declares no function block instance and invokes none.
//! The one exception is an instance the caller passes in through
//! `VAR_IN_OUT`, which Ed.3 permits because the state stays the caller's.
//!
//! Only a function can break the hierarchy in a way this rule has to find.
//! The other direction, a function or function block reaching for a program,
//! cannot be written: a program is not a type, so naming one as a variable
//! type is an undeclared type (`P2008`), and invoking one names no variable
//! in scope (`P4012`). A program is instantiated only by a `PROGRAM ... WITH`
//! in a resource. `stages.rs` pins that, since it holds for the whole
//! pipeline rather than for this rule.
//!
//! This rule therefore reports only what a function declares and calls: each
//! offending declaration, and each invocation of it and method call on it, so
//! both the declaration and every call site are marked. Function block and
//! program bodies are walked like any other, and yield nothing because the
//! rule only checks declarations and invocations while inside a function.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION Twice : INT
//!    VAR_INPUT
//!       x : INT;
//!    END_VAR
//!    Twice := Double(x);
//! END_FUNCTION
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION Delayed : BOOL
//!    VAR
//!       timer : TON;
//!    END_VAR
//!    timer(IN := TRUE, PT := T#1s);
//!    Delayed := timer.Q;
//! END_FUNCTION
//! ```
use std::convert::Infallible;

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::{FbCall, MethodCall, MethodReceiver},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::ScopeTracker,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RulePouHierarchy {
            context,
            scope: ScopeTracker::default(),
            function: None,
            diagnostics: Vec::new(),
        },
        lib,
    )
}

const HELP: &str = "A function has no state. Move the function block instance to a \
                    function block or program, or pass it in through VAR_IN_OUT.";

struct RulePouHierarchy<'a> {
    context: &'a SemanticContext,
    /// Where the traversal is, to look variables up in the symbol
    /// environment.
    scope: ScopeTracker,
    /// The name of the function being walked. `None` outside any function.
    function: Option<Id>,
    diagnostics: Vec<Diagnostic>,
}

impl RulePouHierarchy<'_> {
    /// Reports `call` when it invokes a function block instance the
    /// function declared outside `VAR_IN_OUT`. An instance the function
    /// did not declare at all is `P4012`'s to report, not this rule's.
    fn check_invocation(&mut self, instance: &Id, call: &impl Located, label: &str) {
        let Some(function) = &self.function else {
            return;
        };
        let scope = self.scope.current();
        let Some(info) = self.context.symbols().find(instance, &scope) else {
            return;
        };
        // Declared by the function itself, not reached through it.
        if info.scope != scope || info.variable_type == Some(VariableType::InOut) {
            return;
        }
        let types = self.context.types();
        let Some(type_id) = info.type_id.filter(|id| {
            types
                .get_by_id(*id)
                .is_some_and(|attrs| attrs.representation.is_function_block())
        }) else {
            return;
        };
        let mut diagnostic = Diagnostic::problem(
            Problem::FunctionBlockInFunction,
            Label::span(call.span(), label),
        )
        .with_context_id("function", function)
        .with_context_id("instance", instance);
        if let Some(fb_type) = types.name_of(type_id) {
            diagnostic = diagnostic.with_context_type("function block", fb_type);
        }
        self.diagnostics.push(diagnostic.with_help(HELP));
    }
}

impl DiagnosticVisitor for RulePouHierarchy<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RulePouHierarchy<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<Self::Value, Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Infallible> {
        for decl in &node.variables {
            // Type resolution has turned every instance declaration into a
            // function block initializer, so the initializer kind is the test.
            let InitialValueAssignmentKind::FunctionBlock(init) = &decl.initializer else {
                continue;
            };
            if decl.var_type == VariableType::InOut {
                continue;
            }
            if decl.identifier.symbolic_id().is_none() {
                continue;
            }
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::FunctionBlockInFunction,
                    Label::span(
                        decl.identifier.span(),
                        "Function block instance declared in a function",
                    ),
                )
                .with_context_id("function", &node.name)
                .with_context_type("function block", &init.type_name)
                .with_help(HELP),
            );
        }

        self.function = Some(node.name.clone());
        let result = node.recurse_visit(self);
        self.function = None;
        result
    }

    fn visit_fb_call(&mut self, node: &FbCall) -> Result<Self::Value, Infallible> {
        self.check_invocation(&node.var_name, node, "Function block invoked in a function");
        Ok(())
    }

    fn visit_method_call(&mut self, node: &MethodCall) -> Result<Self::Value, Infallible> {
        if let MethodReceiver::Instance(instance) = &node.receiver {
            self.check_invocation(instance, node, "Function block method called in a function");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ironplc_parser::options::CompilerOptions;
    use ironplc_problems::Problem;

    fn oop_options() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        }
    }

    rule_ctx_ok!(
        apply_when_function_calls_function_then_ok,
        "
FUNCTION Double : INT
  VAR_INPUT
    x : INT;
  END_VAR
  Double := x * 2;
END_FUNCTION

FUNCTION Twice : INT
  VAR_INPUT
    x : INT;
  END_VAR
  Twice := Double(x);
END_FUNCTION"
    );

    rule_ctx_ok!(
        apply_when_program_and_function_block_declare_and_invoke_function_blocks_then_ok,
        "
FUNCTION_BLOCK Callee
  VAR_INPUT
    IN1 : BOOL;
  END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Caller
  VAR
    inner : Callee;
    timer : TON;
  END_VAR
  inner(IN1 := TRUE);
  timer(IN := TRUE, PT := T#1s);
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    outer : Caller;
  END_VAR
  outer();
END_PROGRAM"
    );

    rule_err1_at!(
        apply_when_function_declares_function_block_instance_then_error_at_declaration,
        "
FUNCTION_BLOCK Callee
  VAR_INPUT
    IN1 : BOOL;
  END_VAR
END_FUNCTION_BLOCK

FUNCTION Caller : BOOL
  VAR
    inst : Callee;
  END_VAR
  Caller := FALSE;
END_FUNCTION",
        Problem::FunctionBlockInFunction,
        "inst"
    );

    // The declaration and the invocation are each reported, so both the
    // cause and the call site are marked.
    rule_ctx_errn!(
        apply_when_function_declares_and_invokes_function_block_then_reports_both,
        "
FUNCTION Delayed : BOOL
  VAR
    timer : TON;
  END_VAR
  timer(IN := TRUE, PT := T#1s);
  Delayed := timer.Q;
END_FUNCTION",
        2,
        Problem::FunctionBlockInFunction
    );

    rule_ctx_errn!(
        apply_when_function_invokes_instance_twice_then_reports_each_invocation,
        "
FUNCTION Delayed : BOOL
  VAR
    timer : TON;
  END_VAR
  timer(IN := TRUE, PT := T#1s);
  timer(IN := FALSE, PT := T#1s);
  Delayed := timer.Q;
END_FUNCTION",
        3,
        Problem::FunctionBlockInFunction
    );

    rule_ctx_err1!(
        apply_when_function_declares_function_block_as_temp_then_error,
        "
FUNCTION Delayed : BOOL
  VAR_TEMP
    timer : TON;
  END_VAR
  Delayed := FALSE;
END_FUNCTION",
        Problem::FunctionBlockInFunction
    );

    // Passing an instance by value would copy its state into the function,
    // so an input is as stateful as a local.
    rule_ctx_err1!(
        apply_when_function_declares_function_block_as_input_then_error,
        "
FUNCTION Delayed : BOOL
  VAR_INPUT
    timer : TON;
  END_VAR
  Delayed := timer.Q;
END_FUNCTION",
        Problem::FunctionBlockInFunction
    );

    // Ed.3 permits a function block instance as VAR_IN_OUT of a function:
    // the state stays the caller's.
    rule_ctx_ok!(
        apply_when_function_declares_function_block_as_in_out_then_ok,
        "
FUNCTION Delayed : BOOL
  VAR_IN_OUT
    timer : TON;
  END_VAR
  timer(IN := TRUE, PT := T#1s);
  Delayed := timer.Q;
END_FUNCTION"
    );

    rule_ctx_errn_with!(
        apply_when_function_calls_method_on_own_instance_then_reports_declaration_and_call,
        oop_options(),
        "
FUNCTION_BLOCK FB_Motor
  VAR
    running : BOOL;
  END_VAR
  METHOD Start
    running := TRUE;
  END_METHOD
END_FUNCTION_BLOCK

FUNCTION Spin : BOOL
  VAR
    motor : FB_Motor;
  END_VAR
  motor.Start();
  Spin := TRUE;
END_FUNCTION",
        2,
        Problem::FunctionBlockInFunction
    );

    // An instance the function never declared is P4012's to report.
    rule_ctx_ok!(
        apply_when_function_invokes_undeclared_instance_then_not_this_rule,
        "
FUNCTION Delayed : BOOL
  timer(IN := TRUE, PT := T#1s);
  Delayed := FALSE;
END_FUNCTION"
    );
}
