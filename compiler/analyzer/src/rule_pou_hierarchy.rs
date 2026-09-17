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
//! Only functions can break the hierarchy in this syntax: a program is not a
//! type and cannot be invoked, so nothing declares or calls one. The rule
//! therefore looks inside functions only. Each offending declaration is
//! reported, and so is each invocation of it and each method call on it, so
//! both the declaration and every call site are marked.
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
use std::collections::HashMap;
use std::convert::Infallible;

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    textual::{FbCall, MethodCall, MethodReceiver},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RulePouHierarchy {
            function: None,
            diagnostics: Vec::new(),
        },
        lib,
    )
}

const HELP: &str = "A function has no state. Move the function block instance to a \
                    function block or program, or pass it in through VAR_IN_OUT.";

struct RulePouHierarchy {
    /// The function being walked, with the function block instances it
    /// declared outside `VAR_IN_OUT`. `None` outside any function.
    function: Option<InFunction>,
    diagnostics: Vec<Diagnostic>,
}

struct InFunction {
    name: Id,
    /// Instances already reported at their declaration, by variable name,
    /// so that every invocation of one is reported too.
    stateful_instances: HashMap<Id, TypeName>,
}

impl RulePouHierarchy {
    /// Reports `call` when it invokes an instance the function declared
    /// outside `VAR_IN_OUT`. An instance the function did not declare at
    /// all is `P4012`'s to report, not this rule's.
    fn check_invocation(&mut self, instance: &Id, call: &impl Located, label: &str) {
        let Some(function) = &self.function else {
            return;
        };
        let Some(fb_type) = function.stateful_instances.get(instance) else {
            return;
        };
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::FunctionBlockInFunction,
                Label::span(call.span(), label),
            )
            .with_context_id("function", &function.name)
            .with_context_id("instance", instance)
            .with_context_type("function block", fb_type)
            .with_help(HELP),
        );
    }
}

impl DiagnosticVisitor for RulePouHierarchy {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RulePouHierarchy {
    type Value = ();

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let mut stateful_instances = HashMap::new();
        for decl in &node.variables {
            // Type resolution has turned every instance declaration into a
            // function block initializer, so the initializer kind is the test.
            let InitialValueAssignmentKind::FunctionBlock(init) = &decl.initializer else {
                continue;
            };
            if decl.var_type == VariableType::InOut {
                continue;
            }
            let Some(name) = decl.identifier.symbolic_id() else {
                continue;
            };
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
            stateful_instances.insert(name.clone(), init.type_name.clone());
        }

        self.function = Some(InFunction {
            name: node.name.clone(),
            stateful_instances,
        });
        let result = node.recurse_visit(self);
        self.function = None;
        result
    }

    // A function block or program may declare and invoke function blocks,
    // and a function cannot be nested in either, so there is nothing to
    // find inside them.
    fn visit_function_block_declaration(
        &mut self,
        _node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Infallible> {
        Ok(())
    }

    fn visit_program_declaration(
        &mut self,
        _node: &ProgramDeclaration,
    ) -> Result<Self::Value, Infallible> {
        Ok(())
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

    rule_ok!(
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

    rule_ok!(
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
    rule_errn!(
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

    rule_errn!(
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

    rule_err1!(
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
    rule_err1!(
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
    rule_ok!(
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

    rule_errn_with!(
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
    rule_ok!(
        apply_when_function_invokes_undeclared_instance_then_not_this_rule,
        "
FUNCTION Delayed : BOOL
  timer(IN := TRUE, PT := T#1s);
  Delayed := FALSE;
END_FUNCTION"
    );
}
