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
    callee_resolution::{FunctionBlocks, InstanceTypes},
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    semantic_type::SemanticType,
    symbol_environment::ScopeTracker,
    variable_type,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let function_blocks = FunctionBlocks::from_library(lib);

    run_rule(RuleMethodCallDeclared::new(&function_blocks, context), lib)
}

struct RuleMethodCallDeclared<'a> {
    function_blocks: &'a FunctionBlocks<'a>,

    /// The instances declared in the unit being walked.
    instances: InstanceTypes,

    context: &'a SemanticContext,

    /// Where the walk is, to find the function block `THIS^`/`SUPER^` name.
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
            instances: InstanceTypes::default(),
            context,
            scope: ScopeTracker::default(),
            in_expression: false,
            diagnostics: Vec::new(),
        }
    }

    fn check_assignments(
        owner_label: &str,
        method: &MethodDeclaration,
        call: &MethodCall,
    ) -> Vec<Diagnostic> {
        crate::call_assignment_check::check_assignments(
            method,
            method.span(),
            call.span(),
            &call.params,
            &crate::call_assignment_check::AssignmentCheckLabels {
                call_label: "Method invocation",
                context_key: "method",
                owner_name: owner_label,
                decl_label: "Method declaration",
            },
        )
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

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let res = node.recurse_visit(self);
        self.instances.clear();
        res
    }

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let res = node.recurse_visit(self);
        self.instances.clear();
        res
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let res = node.recurse_visit(self);
        self.instances.clear();
        res
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Infallible> {
        self.instances.declare(node);
        Ok(())
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
        // `THIS^.M()` resolves against the enclosing function block and
        // `SUPER^.M()` against its base, so a `SUPER^` call starts the
        // `EXTENDS` walk one block up. Where there is nothing to name,
        // `rule_self_reference_context` reports the receiver.
        let instance = match &call.receiver {
            MethodReceiver::Instance(id) => id,
            MethodReceiver::SelfRef(self_ref) => {
                let scope = self.scope.current();
                if let Some(fb_type) = self.context.symbols().self_type(&scope, self_ref.kind) {
                    if self.is_member_instance(call, &fb_type) {
                        return;
                    }
                    self.check_on_type(call, in_expression, &fb_type);
                }
                return;
            }
        };

        // Cloned so that the borrow of `instances` ends here: the arms below
        // push onto `self.diagnostics`, which borrows `self` mutably.
        let fb_type = self.instances.type_of(instance).cloned();
        let Some(fb_type) = fb_type else {
            self.diagnostics.push(Self::not_in_scope(call, instance));
            return;
        };

        if !self.function_blocks.contains(&fb_type) {
            self.diagnostics.push(Self::not_in_scope(call, instance));
            return;
        }

        if self.is_member_instance(call, &fb_type) {
            return;
        }
        self.check_on_type(call, in_expression, &fb_type);
    }

    /// Whether `receiver.name(...)` invokes a function block instance that
    /// `fb_type` declares rather than a method: `THIS^.inner(i := 1)` and
    /// `inst.inner(i := 1)`. TwinCAT accepts both. The arguments are not
    /// checked against the instance's inputs here.
    fn is_member_instance(&self, call: &MethodCall, fb_type: &TypeName) -> bool {
        if self
            .function_blocks
            .resolve_method(fb_type, &call.method)
            .is_some()
        {
            return false;
        }
        variable_type::member_of_block(fb_type, &call.method, self.context.symbols())
            .and_then(|member| member.type_id)
            .and_then(|id| self.context.types().get_by_id(id))
            .is_some_and(|ty| matches!(ty.representation, SemanticType::FunctionBlock { .. }))
    }

    /// Checks `call` against the methods of `fb_type` and its `EXTENDS`
    /// chain: the method exists, has a result where one is used, and gets
    /// the arguments it declares.
    fn check_on_type(&mut self, call: &MethodCall, in_expression: bool, fb_type: &TypeName) {
        match self.function_blocks.resolve_method(fb_type, &call.method) {
            None => self.diagnostics.push(
                Diagnostic::problem(
                    Problem::MethodNotFound,
                    Label::span(call.span(), "Method invocation"),
                )
                .with_context_type("function block", fb_type)
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

    rule_ok!(
        apply_when_this_and_super_methods_called_then_ok,
        "
FUNCTION_BLOCK FB_Base
METHOD Stop : BOOL
    Stop := TRUE;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Motor EXTENDS FB_Base
VAR
    ok : BOOL;
END_VAR
METHOD Start
VAR_INPUT
    speed : INT;
END_VAR
END_METHOD
METHOD Run
    THIS^.Start(speed := 3);
    ok := THIS^.Stop();
    ok := SUPER^.Stop();
END_METHOD
END_FUNCTION_BLOCK",
        fb_inheritance_options()
    );

    rule_err!(
        apply_when_this_method_not_declared_then_error,
        "
FUNCTION_BLOCK FB_Motor
METHOD Run
    THIS^.Nope();
END_METHOD
END_FUNCTION_BLOCK",
        [Problem::MethodNotFound],
        fb_inheritance_options()
    );

    rule_err!(
        apply_when_super_method_only_on_derived_then_error,
        "
FUNCTION_BLOCK FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Motor EXTENDS FB_Base
METHOD Start
END_METHOD
METHOD Run
    SUPER^.Start();
END_METHOD
END_FUNCTION_BLOCK",
        [Problem::MethodNotFound],
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
}
