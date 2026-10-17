//! Semantic rule that reference to a function block must be to a function
//! block that is declared.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK Callee
//! END_FUNCTION_BLOCK
//!
//! FUNCTION_BLOCK Caller
//!    VAR
//!       FB_INSTANCE : Callee;
//!    END_VAR
//!    FB_INSTANCE();
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails (Incorrect Parameters)
//!
//! ```ignore
//! FUNCTION_BLOCK Callee
//!    VAR_INPUT
//!       IN1: BOOL;
//!    END_VAR
//! END_FUNCTION_BLOCK
//!     
//! FUNCTION_BLOCK Caller
//!    VAR
//!       FB_INSTANCE : Callee;
//!    END_VAR
//!    FB_INSTANCE(IN1 := TRUE, BAR := TRUE);
//! END_FUNCTION_BLOCK
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    scope::ScopeNode,
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
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

    // Walk the library to find all references to function blocks
    run_rule(RuleFunctionBlockUse::new(&function_blocks, context), lib)
}

struct RuleFunctionBlockUse<'a> {
    function_blocks: &'a FunctionBlocks<'a>,
    context: &'a SemanticContext,

    /// The scope the walk is in, which an instance is looked up from.
    scope: ScopeTracker,

    diagnostics: Vec<Diagnostic>,
}
impl<'a> RuleFunctionBlockUse<'a> {
    fn new(function_blocks: &'a FunctionBlocks<'a>, context: &'a SemanticContext) -> Self {
        Self {
            function_blocks,
            context,
            scope: ScopeTracker::default(),
            diagnostics: Vec::new(),
        }
    }

    fn check_assignments(
        &self,
        block: &TypeName,
        parameters: &CallParameters,
        fb_call: &FbCall,
    ) -> Vec<Diagnostic> {
        crate::call_assignment_check::check_assignments(
            parameters,
            self.function_blocks.get(block).map(|fb| fb.span()),
            fb_call.span(),
            &fb_call.params,
            &crate::call_assignment_check::AssignmentCheckLabels {
                call_label: "Function block invocation",
                context_key: "invocation",
                owner_name: &block.to_string(),
                decl_label: "Function block declaration",
            },
        )
    }

    fn not_in_scope(fb_call: &FbCall) -> Diagnostic {
        Diagnostic::problem(
            Problem::FunctionBlockNotInScope,
            Label::span(fb_call.span(), "Function block invocation"),
        )
        .with_context_id("invocation", &fb_call.var_name)
    }
}

impl DiagnosticVisitor for RuleFunctionBlockUse<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleFunctionBlockUse<'_> {
    type Value = ();

    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Infallible> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn visit_fb_call(&mut self, fb_call: &FbCall) -> Result<Self::Value, Infallible> {
        // Check if function block is defined because you cannot
        // call a function block that doesn't exist
        let function_block_name = instance_type(
            &fb_call.var_name,
            &self.scope.current(),
            self.context.symbols(),
            self.context.types(),
        );
        let Some(function_block_name) = function_block_name else {
            self.diagnostics.push(Self::not_in_scope(fb_call));
            return Ok(());
        };

        // A block the library declares, or a standard one.
        match CallParameters::of_block(&function_block_name, self.context.types()) {
            None => self.diagnostics.push(Self::not_in_scope(fb_call)),
            Some(parameters) => {
                let diagnostics =
                    self.check_assignments(&function_block_name, &parameters, fb_call);
                self.diagnostics.extend(diagnostics);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ironplc_problems::Problem;
    use spec_test_macro::spec_test;

    rule_ok!(
        apply_when_instance_declared_with_member_initializer_then_ok,
        "
FUNCTION_BLOCK Callee
VAR_INPUT
    IN1 : BOOL;
END_VAR
VAR
    count : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM main
VAR
    inst : Callee := (count := 1);
END_VAR
    inst(IN1 := TRUE);
END_PROGRAM"
    );

    rule_ok!(
        apply_when_no_names_uses_default_then_return_ok,
        "
FUNCTION_BLOCK Callee

END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE();
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_some_formal_input_names_assigned_then_ok,
        "
FUNCTION_BLOCK Callee
VAR_INPUT
IN1: BOOL;
IN2: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(IN1 := TRUE);
END_FUNCTION_BLOCK"
    );

    rule_err!(
        apply_when_mixed_formal_nonformal_then_error,
        "
FUNCTION_BLOCK Callee
VAR_INPUT
IN1: BOOL;
IN2: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(IN1 := TRUE, FALSE);
END_FUNCTION_BLOCK",
        [Problem::FunctionCallMixedArgTypes]
    );

    rule_err!(
        apply_when_function_block_definition_not_defined_then_error,
        "
FUNCTION_BLOCK Caller
VAR
IN1: BOOL;
END_VAR
FB_INSTANCE(IN1 := TRUE);
END_FUNCTION_BLOCK",
        [Problem::FunctionBlockNotInScope]
    );

    rule_ok!(
        apply_when_nonformal_input_names_assigned_then_ok,
        "
FUNCTION_BLOCK Callee
VAR_INPUT
IN1: BOOL;
IN2: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(TRUE, FALSE);
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_some_output_names_assigned_then_ok,
        "
FUNCTION_BLOCK Callee
VAR_OUTPUT
OUT1: BOOL;
OUT2: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
LOCAL: BOOL;
END_VAR
FB_INSTANCE(OUT1 => LOCAL);
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_all_formal_input_names_assigned_then_ok,
        "
FUNCTION_BLOCK Callee
VAR_INPUT
IN1: BOOL;
IN2: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(IN1 := TRUE, IN2 := FALSE);
END_FUNCTION_BLOCK"
    );

    rule_err_at!(
        apply_when_formal_names_incorrect_then_error,
        "
FUNCTION_BLOCK Callee
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(BAR := TRUE);
END_FUNCTION_BLOCK",
        Problem::FunctionInvocationMissingInput,
        "FB_INSTANCE(BAR := TRUE)"
    );

    rule_err!(
        apply_when_nonformal_names_too_few_then_error,
        "
FUNCTION_BLOCK Callee
VAR_INPUT
IN1: BOOL;
IN2: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(TRUE);
END_FUNCTION_BLOCK",
        [Problem::FunctionInvocationRequiresFormal]
    );

    rule_err!(
        apply_when_nonformal_names_too_many_then_error,
        "
FUNCTION_BLOCK Callee
VAR_INPUT
IN2: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(TRUE, FALSE);
END_FUNCTION_BLOCK",
        [Problem::FunctionInvocationRequiresFormal]
    );

    rule_err!(
        apply_when_one_input_name_incorrect_then_error,
        "
FUNCTION_BLOCK Callee
VAR_INPUT
IN1: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(IN1 := TRUE, BAR := TRUE);
END_FUNCTION_BLOCK",
        [Problem::FunctionInvocationMissingInput]
    );

    rule_err!(
        apply_when_one_output_name_incorrect_then_error,
        "
FUNCTION_BLOCK Callee
VAR_OUTPUT
OUT1: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
LOCAL: BOOL;
END_VAR
FB_INSTANCE(OUT2 => LOCAL);
END_FUNCTION_BLOCK",
        [Problem::FunctionInvocationUndefinedOutput]
    );

    rule_ok!(
        apply_when_program_invokes_function_block_then_ok,
        "
FUNCTION_BLOCK Callee
VAR_INPUT
IN1: BOOL;
END_VAR
END_FUNCTION_BLOCK
        
PROGRAM prgm
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(IN1 := TRUE);
END_PROGRAM"
    );

    rule_err!(
        apply_when_two_undeclared_function_block_calls_then_reports_both,
        "
PROGRAM main
VAR
    x : INT;
END_VAR
FIRST();
SECOND();
END_PROGRAM",
        [ironplc_problems::Problem::FunctionBlockNotInScope; 2]
    );

    rule_err!(
        apply_when_call_names_two_undeclared_inputs_then_reports_both,
        "
FUNCTION_BLOCK Callee
VAR_INPUT
IN1 : BOOL;
END_VAR
END_FUNCTION_BLOCK

PROGRAM main
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(NOPE1 := TRUE, NOPE2 := TRUE);
END_PROGRAM",
        [ironplc_problems::Problem::FunctionInvocationMissingInput; 2]
    );

    /// A block with two inputs and two outputs, and a program that calls an
    /// instance of it with `call`.
    fn call(call: &str) -> String {
        format!(
            "
FUNCTION_BLOCK Callee
VAR_INPUT a : INT; b : INT; END_VAR
VAR_OUTPUT q : INT; done : BOOL; END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR inst : Callee; x : INT; d : BOOL; END_VAR
{call}
END_PROGRAM"
        )
    }

    /// A program that calls a timer with `call`.
    fn timer_call(call: &str) -> String {
        format!(
            "
PROGRAM main
VAR t : TON; preset : TIME; q : BOOL; e : TIME; END_VAR
{call}
END_PROGRAM"
        )
    }

    rule_err!(
        #[spec_test(REQ_CB_analyzer_024)]
        apply_when_input_named_twice_then_duplicate,
        &call("inst(a := 1, a := 3, b := 2);"),
        [Problem::FunctionCallDuplicateNamedArg]
    );

    rule_err_at!(
        #[spec_test(REQ_CB_analyzer_024)]
        apply_when_output_named_twice_then_duplicate_at_second,
        "
FUNCTION_BLOCK Callee
VAR_OUTPUT q : INT; END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR inst : Callee; x : INT; y : INT; END_VAR
inst(q => x, Q => y);
END_PROGRAM",
        Problem::FunctionCallDuplicateNamedArg,
        "Q"
    );

    /// A block that extends another, and a program that calls an instance of
    /// it with `call`.
    fn derived_call(call: &str) -> String {
        format!(
            "
FUNCTION_BLOCK Base
VAR_INPUT count : INT; END_VAR
VAR_OUTPUT seen : INT; END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK Derived EXTENDS Base
VAR_INPUT speed : INT; END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR d : Derived; r : INT; END_VAR
{call}
END_PROGRAM"
        )
    }

    rule_ok!(
        #[spec_test(REQ_CB_analyzer_053)]
        apply_when_derived_block_names_inherited_input_and_output_then_ok,
        &derived_call("d(count := 3, speed := 4, seen => r);"),
        crate::test_helpers::edition3_options()
    );

    rule_err!(
        apply_when_derived_block_names_input_no_block_declares_then_missing_input,
        &derived_call("d(count := 3, nope := 4);"),
        [Problem::FunctionInvocationMissingInput],
        crate::test_helpers::edition3_options()
    );

    rule_ok!(
        #[spec_test(REQ_CB_analyzer_025)]
        apply_when_standard_block_positional_then_ok,
        &timer_call("t(TRUE, preset);")
    );

    rule_err!(
        #[spec_test(REQ_CB_analyzer_025)]
        apply_when_standard_block_input_unknown_then_missing_input,
        &timer_call("t(IN := TRUE, FOO := preset);"),
        [Problem::FunctionInvocationMissingInput]
    );

    rule_err!(
        #[spec_test(REQ_CB_analyzer_025)]
        apply_when_standard_block_positional_count_differs_then_requires_formal,
        &timer_call("t(TRUE, preset, 5);"),
        [Problem::FunctionInvocationRequiresFormal]
    );

    rule_err!(
        #[spec_test(REQ_CB_analyzer_025)]
        apply_when_standard_block_mixed_then_mixed,
        &timer_call("t(TRUE, PT := preset);"),
        [Problem::FunctionCallMixedArgTypes]
    );

    rule_err!(
        #[spec_test(REQ_CB_analyzer_025)]
        apply_when_standard_block_output_unknown_then_undefined_output,
        &timer_call("t(IN := TRUE, NOPE => q);"),
        [Problem::FunctionInvocationUndefinedOutput]
    );

    rule_err!(
        #[spec_test(REQ_CB_analyzer_025)]
        apply_when_standard_block_input_named_twice_then_duplicate,
        &timer_call("t(IN := TRUE, IN := FALSE);"),
        [Problem::FunctionCallDuplicateNamedArg]
    );
}
