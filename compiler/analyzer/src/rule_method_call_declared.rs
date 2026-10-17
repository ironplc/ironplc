//! Semantic rule that a method call (`instance.MethodName(args)`, the
//! CODESYS/TwinCAT OOP extension) refers to a method that is actually
//! declared -- either directly on the instance's function block type, or
//! on a function block reached by following that type's `EXTENDS` chain.
//!
//! This is the static-dispatch resolution algorithm from ADR-0041 Phase 1:
//! walk the static type's own methods first, then its base, then the
//! base's base, and so on.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Base
//!    METHOD Start
//!    END_METHOD
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
//! END_FUNCTION_BLOCK
//!
//! PROGRAM main
//!    VAR
//!       inst : FB_Derived;
//!    END_VAR
//!    inst.Start();
//! END_PROGRAM
//! ```
//!
//! ## Fails (Method Not Declared Anywhere In The Chain)
//!
//! ```ignore
//! FUNCTION_BLOCK FB_Base
//! END_FUNCTION_BLOCK
//!
//! PROGRAM main
//!    VAR
//!       inst : FB_Base;
//!    END_VAR
//!    inst.Start();
//! END_PROGRAM
//! ```
use std::convert::Infallible;

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    call_binding::bind,
    callee_resolution::{instance_type, CallParameters, FunctionBlocks},
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
    let function_blocks = FunctionBlocks::from_library(lib, context.types());

    run_rule(RuleMethodCallDeclared::new(&function_blocks, context), lib)
}

struct RuleMethodCallDeclared<'a> {
    function_blocks: &'a FunctionBlocks<'a>,
    context: &'a SemanticContext,

    /// The scope the walk is in, which an instance is looked up from.
    scope: ScopeTracker,

    /// Whether the method call being visited is in expression position,
    /// where its value is used and the method must have a return type.
    in_expression: bool,

    diagnostics: Vec<Diagnostic>,
}

impl<'a> RuleMethodCallDeclared<'a> {
    fn new(function_blocks: &'a FunctionBlocks<'a>, context: &'a SemanticContext) -> Self {
        Self {
            function_blocks,
            context,
            scope: ScopeTracker::default(),
            in_expression: false,
            diagnostics: Vec::new(),
        }
    }

    /// Reports why `call`'s arguments do not bind to `method`'s parameters,
    /// or else each input and `VAR_IN_OUT` the call does not supply (P4018):
    /// a method's inputs are fresh on every call, so a call supplies them
    /// all.
    fn check_assignments(
        owner_label: &str,
        method: &MethodDeclaration,
        call: &MethodCall,
    ) -> Vec<Diagnostic> {
        let parameters = CallParameters::of_declaration(method);
        let diagnostics = crate::call_assignment_check::check_assignments(
            &parameters,
            Some(method.span()),
            call.span(),
            &call.params,
            &crate::call_assignment_check::AssignmentCheckLabels {
                call_label: "Method invocation",
                context_key: "method",
                owner_name: owner_label,
                decl_label: "Method declaration",
            },
        );
        if !diagnostics.is_empty() {
            return diagnostics;
        }
        let Ok(binding) = bind(&parameters, &call.params) else {
            return diagnostics;
        };
        let owner = owner_label.to_string();
        binding
            .omitted(&parameters)
            .into_iter()
            .map(|parameter| {
                Diagnostic::problem(
                    Problem::FunctionCallWrongArgCount,
                    Label::span(call.span(), "Method invocation"),
                )
                .with_context("method", &owner)
                .with_context_id("missing input", &parameter.name)
                .with_secondary(Label::span(method.span(), "Method declaration"))
            })
            .collect()
    }

    /// The diagnostic for a method call whose receiver is not a variable of a
    /// known function block type. Both the unknown-variable and the
    /// unknown-type cases report it, against the same instance name.
    fn not_in_scope(call: &MethodCall, instance: &Id) -> Diagnostic {
        Diagnostic::problem(
            Problem::FunctionBlockNotInScope,
            Label::span(call.span(), "Method invocation"),
        )
        .with_context_id("invocation", instance)
    }
}

impl DiagnosticVisitor for RuleMethodCallDeclared<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleMethodCallDeclared<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_expr_kind(&mut self, node: &ExprKind) -> Result<Self::Value, Infallible> {
        if let ExprKind::MethodCall(call) = node {
            self.in_expression = true;
            return self.visit_method_call(call);
        }
        node.recurse_visit(self)
    }

    fn visit_method_call(&mut self, call: &MethodCall) -> Result<Self::Value, Infallible> {
        let in_expression = std::mem::replace(&mut self.in_expression, false);
        self.check_call(call, in_expression);
        // The arguments may hold method calls of their own.
        call.recurse_visit(self)
    }
}

impl RuleMethodCallDeclared<'_> {
    fn check_call(&mut self, call: &MethodCall, in_expression: bool) {
        // `THIS^.M()` / `SUPER^.M()` resolve against the enclosing function
        // block (and, for SUPER^, its base) rather than a variable's declared
        // type. That resolution is not implemented yet, so say so rather than
        // skipping the call: a silent skip would keep quietly passing once
        // the receiver becomes resolvable. Tracked in issue #1406.
        let instance = match &call.receiver {
            MethodReceiver::Instance(id) => id,
            MethodReceiver::SelfRef(self_ref) => {
                self.diagnostics
                    .push(Diagnostic::not_implemented(Label::span(
                        self_ref.span(),
                        format!(
                            "{} method invocation is recognized but not yet resolved by IronPLC",
                            self_ref.kind.spelling()
                        ),
                    )));
                return;
            }
        };

        let fb_type = instance_type(
            instance,
            &self.scope.current(),
            self.context.symbols(),
            self.context.types(),
        );
        let Some(fb_type) = fb_type else {
            self.diagnostics.push(Self::not_in_scope(call, instance));
            return;
        };

        if !self.function_blocks.contains(&fb_type) {
            self.diagnostics.push(Self::not_in_scope(call, instance));
            return;
        }

        match self.function_blocks.resolve_method(&fb_type, &call.method) {
            None => self.diagnostics.push(
                Diagnostic::problem(
                    Problem::MethodNotFound,
                    Label::span(call.span(), "Method invocation"),
                )
                .with_context_type("function block", &fb_type)
                .with_context_id("method", &call.method),
            ),
            Some((owning_fb, method)) => {
                let owner_label = format!("{}.{}", owning_fb.name, method.name);
                if in_expression && method.return_type.is_none() {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::MethodCallWithoutReturnValue,
                            Label::span(call.span(), "Method invocation in an expression"),
                        )
                        .with_context_id("method", &method.name),
                    );
                }
                let diagnostics = Self::check_assignments(&owner_label, method, call);
                self.diagnostics.extend(diagnostics);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::fb_inheritance_options;
    use spec_test_macro::spec_test;

    rule_ok!(
        apply_when_method_declared_on_own_type_then_ok,
        "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
METHOD Start
    bRunning := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
m.Start();
END_PROGRAM",
        fb_inheritance_options()
    );

    rule_ok!(
        apply_when_method_declared_on_base_via_extends_then_ok,
        "
FUNCTION_BLOCK FB_Base
METHOD Start
    ;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Derived;
END_VAR
m.Start();
END_PROGRAM",
        fb_inheritance_options()
    );

    rule_ok!(
        apply_when_method_declared_two_levels_up_extends_chain_then_ok,
        "
FUNCTION_BLOCK FB_Base
METHOD Start
    ;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Mid EXTENDS FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Mid
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Derived;
END_VAR
m.Start();
END_PROGRAM",
        fb_inheritance_options()
    );

    rule_err!(
        apply_when_method_not_declared_anywhere_then_error,
        "
FUNCTION_BLOCK FB_Motor
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
m.Start();
END_PROGRAM",
        [Problem::MethodNotFound],
        fb_inheritance_options()
    );

    rule_err_at!(
        apply_when_method_call_has_wrong_arg_count_then_error,
        "
FUNCTION_BLOCK FB_Motor
METHOD SetSpeed
VAR_INPUT
    rSpeed : REAL;
END_VAR
    ;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
m.SetSpeed(1.0, 2.0);
END_PROGRAM",
        Problem::FunctionInvocationRequiresFormal,
        "m.SetSpeed(1.0, 2.0)",
        fb_inheritance_options()
    );

    rule_err!(
        apply_when_two_undeclared_methods_called_then_reports_both,
        "
FUNCTION_BLOCK FB_Motor
METHOD Start
    ;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
m.NopeOne();
m.NopeTwo();
END_PROGRAM",
        [Problem::MethodNotFound; 2],
        fb_inheritance_options()
    );

    // ---------------------------------------------------------------------
    // Method calls in expression position.
    // ---------------------------------------------------------------------

    rule_ok!(
        apply_when_method_with_return_type_called_in_expression_then_ok,
        "
FUNCTION_BLOCK FB_Motor
METHOD IsRunning : BOOL
    IsRunning := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
    b : BOOL;
END_VAR
b := m.IsRunning();
END_PROGRAM",
        fb_inheritance_options()
    );

    rule_ok!(
        /// The value is discarded, as in CODESYS and TwinCAT.
        apply_when_method_with_return_type_called_as_statement_then_ok,
        "
FUNCTION_BLOCK FB_Motor
METHOD IsRunning : BOOL
    IsRunning := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
m.IsRunning();
END_PROGRAM",
        fb_inheritance_options()
    );

    rule_err!(
        apply_when_method_without_return_type_called_in_expression_then_error,
        "
FUNCTION_BLOCK FB_Motor
METHOD Start
    ;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
    b : BOOL;
END_VAR
b := m.Start();
END_PROGRAM",
        [Problem::MethodCallWithoutReturnValue],
        fb_inheritance_options()
    );

    rule_err!(
        /// A method called in an argument of a statement call is in expression
        /// position, even though the outer call is not.
        apply_when_void_method_is_argument_of_statement_call_then_error,
        "
FUNCTION_BLOCK FB_Motor
METHOD Start
    ;
END_METHOD
METHOD SetRunning
VAR_INPUT
    b : BOOL;
END_VAR
    ;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
END_VAR
m.SetRunning(m.Start());
END_PROGRAM",
        [Problem::MethodCallWithoutReturnValue],
        fb_inheritance_options()
    );

    rule_err!(
        /// Arguments are checked like any other call: before method calls could
        /// appear in expressions, this rule never looked inside them.
        apply_when_undeclared_method_is_argument_then_error,
        "
FUNCTION_BLOCK FB_Motor
METHOD Scaled : REAL
VAR_INPUT
    factor : REAL;
END_VAR
    Scaled := factor;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM main
VAR
    m : FB_Motor;
    v : REAL;
END_VAR
v := m.Scaled(m.Nope());
END_PROGRAM",
        [Problem::MethodNotFound],
        fb_inheritance_options()
    );

    /// A program calling `call` on an instance of a block whose method `M`
    /// declares an in-out between two inputs, the second with an initial
    /// value.
    fn method_call(call: &str) -> String {
        format!(
            "
FUNCTION_BLOCK FB
METHOD M
VAR_INPUT a : DINT; END_VAR
VAR_IN_OUT io : DINT; END_VAR
VAR_INPUT b : DINT := 7; END_VAR
END_METHOD
END_FUNCTION_BLOCK
PROGRAM main
VAR inst : FB; t : DINT; END_VAR
{call}
END_PROGRAM"
        )
    }

    rule_ok!(
        apply_when_method_named_out_of_declaration_order_then_ok,
        &method_call("inst.M(b := 2, io := t, a := 1);"),
        fb_inheritance_options()
    );

    rule_err!(
        /// An initial value is not a default: a method's inputs are fresh on
        /// every call.
        #[spec_test(REQ_CB_analyzer_051)]
        apply_when_method_input_omitted_then_wrong_arg_count,
        &method_call("inst.M(a := 1, io := t);"),
        [Problem::FunctionCallWrongArgCount],
        fb_inheritance_options()
    );

    rule_err!(
        #[spec_test(REQ_CB_analyzer_051)]
        apply_when_method_inputs_and_in_out_omitted_then_wrong_arg_count_for_each,
        &method_call("inst.M();"),
        [Problem::FunctionCallWrongArgCount; 3],
        fb_inheritance_options()
    );

    rule_err!(
        /// A positional argument binds to a `VAR_INPUT`, never to a
        /// `VAR_IN_OUT`, so the in-out is omitted.
        #[spec_test(REQ_CB_analyzer_029)]
        apply_when_method_positional_then_in_out_omitted,
        &method_call("inst.M(1, 2);"),
        [Problem::FunctionCallWrongArgCount],
        fb_inheritance_options()
    );

    rule_err!(
        #[spec_test(REQ_CB_analyzer_024)]
        apply_when_method_input_named_twice_then_duplicate,
        &method_call("inst.M(a := 1, a := 3, io := t, b := 2);"),
        [Problem::FunctionCallDuplicateNamedArg],
        fb_inheritance_options()
    );

    rule_err!(
        /// The call's shape is in doubt, so nothing is said about what it
        /// omits.
        #[spec_test(REQ_CB_analyzer_023)]
        apply_when_method_mixed_then_only_mixed,
        &method_call("inst.M(1, io := t);"),
        [Problem::FunctionCallMixedArgTypes],
        fb_inheritance_options()
    );

    rule_err!(
        #[spec_test(REQ_CB_analyzer_051)]
        apply_when_inherited_method_input_omitted_then_wrong_arg_count,
        "
FUNCTION_BLOCK FB_Base
METHOD Scale : DINT
VAR_INPUT v : DINT; k : DINT; END_VAR
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
END_FUNCTION_BLOCK
PROGRAM main
VAR d : FB_Derived; r : DINT; END_VAR
r := d.Scale(v := 1);
END_PROGRAM",
        [Problem::FunctionCallWrongArgCount],
        fb_inheritance_options()
    );
}
