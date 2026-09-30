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
//!
//! ## Non-formal calls
//!
//! A non-formal call, `FB_INSTANCE(TRUE, FALSE)`, binds its arguments to
//! the block's `VAR_INPUT` variables in declaration order, and must give one
//! argument for each of them (P4003). The same holds for a standard-library
//! block such as `TON`, whose inputs come from its type in the type
//! environment. A non-formal call to a block that declares `VAR_IN_OUT` is
//! refused as not implemented: the standard places `VAR_IN_OUT` in the
//! non-formal order, and function block `VAR_IN_OUT` is not implemented.
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    call_assignment_check::{check_not_mixed, check_positional_count, AssignmentCheckLabels},
    callee_resolution::{FunctionBlocks, InstanceTypes},
    intermediate_type::{FunctionBlockVarType, IntermediateType},
    intermediates::stdlib_function_block::is_stdlib_function_block,
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    type_environment::TypeEnvironment,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let function_blocks = FunctionBlocks::from_library(lib);

    // Walk the library to find all references to function blocks
    run_rule(
        RuleFunctionBlockUse::new(&function_blocks, context.types()),
        lib,
    )
}

struct RuleFunctionBlockUse<'a> {
    function_blocks: &'a FunctionBlocks<'a>,

    /// Where a standard-library block's inputs are found: it has no
    /// declaration in the library.
    types: &'a TypeEnvironment,

    /// The instances declared in the unit being walked.
    instances: InstanceTypes,

    diagnostics: Vec<Diagnostic>,
}
impl<'a> RuleFunctionBlockUse<'a> {
    fn new(function_blocks: &'a FunctionBlocks<'a>, types: &'a TypeEnvironment) -> Self {
        Self {
            function_blocks,
            types,
            instances: InstanceTypes::default(),
            diagnostics: Vec::new(),
        }
    }

    fn labels(owner_name: &str) -> AssignmentCheckLabels<'_> {
        AssignmentCheckLabels {
            call_label: "Function block invocation",
            context_key: "invocation",
            owner_name,
            decl_label: "Function block declaration",
        }
    }

    fn check_assignments(
        function_block: &FunctionBlockDeclaration,
        fb_call: &FbCall,
    ) -> Vec<Diagnostic> {
        let owner_name = function_block.name.to_string();
        let labels = Self::labels(&owner_name);
        if let Some(refusal) = Self::nonformal_with_in_out(function_block, fb_call, &labels) {
            return vec![refusal];
        }
        crate::call_assignment_check::check_assignments(
            function_block,
            function_block.span(),
            fb_call.span(),
            &fb_call.params,
            &labels,
        )
    }

    /// Refuses a non-formal call to a block that declares `VAR_IN_OUT`.
    ///
    /// IEC 61131-3 lists `VAR_IN_OUT` with the inputs in the non-formal
    /// order, but the binding here counts `VAR_INPUT` alone (see
    /// `call_assignment_check::bind_inputs`), so accepting the call would
    /// bind an argument to a different parameter from the one the standard
    /// names. Function block `VAR_IN_OUT` is not implemented, so the call is
    /// refused as such rather than bound either way. A call that also mixes
    /// named inputs is left to the P4001 check, which describes it better.
    fn nonformal_with_in_out(
        function_block: &FunctionBlockDeclaration,
        fb_call: &FbCall,
        labels: &AssignmentCheckLabels,
    ) -> Option<Diagnostic> {
        let nonformal = fb_call
            .params
            .iter()
            .any(|p| matches!(p, ParamAssignmentKind::PositionalInput(_)));
        let has_in_out = function_block
            .variables
            .iter()
            .any(|decl| decl.var_type == VariableType::InOut);
        let mixed = check_not_mixed(&fb_call.span(), &fb_call.params, labels).is_some();
        (nonformal && has_in_out && !mixed).then(|| {
            Diagnostic::not_implemented(Label::span(
                fb_call.span(),
                format!(
                    "Non-formal call of function block '{}', which declares VAR_IN_OUT",
                    function_block.name
                ),
            ))
        })
    }

    /// Checks the shape of a call to a standard-library block: named and
    /// positional inputs not mixed (P4001), and one positional argument for
    /// each input of the block's type (P4003).
    fn check_stdlib_call(&self, fb_name: &TypeName, fb_call: &FbCall) -> Vec<Diagnostic> {
        let owner_name = fb_name.to_string();
        let labels = Self::labels(&owner_name);
        if let Some(mixed) = check_not_mixed(&fb_call.span(), &fb_call.params, &labels) {
            return vec![mixed];
        }
        let inputs = match self.types.get(fb_name).map(|attrs| &attrs.representation) {
            Some(IntermediateType::FunctionBlock { fields, .. }) => fields
                .iter()
                .filter(|field| field.var_type == Some(FunctionBlockVarType::Input))
                .count(),
            // Every standard-library block is in the type environment; if
            // one were not, code generation refuses a positional argument
            // it cannot place.
            _ => return vec![],
        };
        check_positional_count(inputs, &fb_call.span(), &fb_call.params, &labels)
            .into_iter()
            .collect()
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

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let res = node.recurse_visit(self);

        // Remove all items from var init decl since we have left this context
        self.instances.clear();
        res
    }

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let res = node.recurse_visit(self);

        // Remove all items from var init decl since we have left this context
        self.instances.clear();
        res
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let res = node.recurse_visit(self);

        // Remove all items from var init decl since we have left this context
        self.instances.clear();
        res
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Infallible> {
        self.instances.declare(node);
        Ok(())
    }

    fn visit_fb_call(&mut self, fb_call: &FbCall) -> Result<Self::Value, Infallible> {
        // Check if function block is defined because you cannot
        // call a function block that doesn't exist
        // Cloned so that the borrow of `instances` ends here: the arms below
        // push onto `self.diagnostics`, which borrows `self` mutably.
        let function_block_name = self.instances.type_of(&fb_call.var_name).cloned();
        let Some(function_block_name) = function_block_name else {
            self.diagnostics.push(Self::not_in_scope(fb_call));
            return Ok(());
        };

        // Standard library function blocks (TON, TOF, TP, CTU, etc.) have
        // no declaration in the library to check the call against.
        if is_stdlib_function_block(&function_block_name.name) {
            let diagnostics = self.check_stdlib_call(&function_block_name, fb_call);
            self.diagnostics.extend(diagnostics);
            return Ok(());
        }

        match self.function_blocks.get(&function_block_name) {
            None => self.diagnostics.push(Self::not_in_scope(fb_call)),
            Some(fb) => {
                // Validate the parameter assignments
                let diagnostics = RuleFunctionBlockUse::check_assignments(fb, fb_call);
                self.diagnostics.extend(diagnostics);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
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
END_FUNCTION_BLOCK"
    );

    rule_err!(
        apply_when_function_block_definition_not_defined_then_error,
        "
FUNCTION_BLOCK Caller
VAR
IN1: BOOL;
END_VAR
FB_INSTANCE(IN1 := TRUE);
END_FUNCTION_BLOCK"
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

    rule_err!(
        apply_when_formal_names_incorrect_then_error,
        "
FUNCTION_BLOCK Callee
END_FUNCTION_BLOCK
        
FUNCTION_BLOCK Caller
VAR
FB_INSTANCE : Callee;
END_VAR
FB_INSTANCE(BAR := TRUE);
END_FUNCTION_BLOCK"
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
END_FUNCTION_BLOCK"
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
END_FUNCTION_BLOCK"
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
END_FUNCTION_BLOCK"
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
END_FUNCTION_BLOCK"
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

    rule_errn!(
        apply_when_two_undeclared_function_block_calls_then_reports_both,
        "
PROGRAM main
VAR
    x : INT;
END_VAR
FIRST();
SECOND();
END_PROGRAM",
        2,
        ironplc_problems::Problem::FunctionBlockNotInScope
    );

    rule_errn!(
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
        2,
        ironplc_problems::Problem::FunctionInvocationMissingInput
    );

    // A standard-library block's inputs, in declaration order, are the
    // positions a non-formal call binds: `TON` takes `IN, PT`, `CTU` takes
    // `CU, R, PV`.
    rule_ctx_ok!(
        apply_when_stdlib_nonformal_binds_every_input_then_ok,
        "
PROGRAM main
VAR
    timer : TON;
    counter : CTU;
    done : BOOL;
END_VAR
    timer(TRUE, T#1s, Q => done);
    counter(TRUE, FALSE, 5);
END_PROGRAM"
    );

    rule_ctx_err1!(
        apply_when_stdlib_nonformal_too_few_then_p4003,
        "
PROGRAM main
VAR
    timer : TON;
END_VAR
    timer(TRUE);
END_PROGRAM",
        ironplc_problems::Problem::FunctionInvocationRequiresFormal
    );

    rule_ctx_err1!(
        apply_when_stdlib_nonformal_too_many_then_p4003,
        "
PROGRAM main
VAR
    timer : TON;
END_VAR
    timer(TRUE, T#1s, 3);
END_PROGRAM",
        ironplc_problems::Problem::FunctionInvocationRequiresFormal
    );

    rule_ctx_err1!(
        apply_when_stdlib_mixed_formal_nonformal_then_p4001,
        "
PROGRAM main
VAR
    timer : TON;
END_VAR
    timer(IN := TRUE, T#1s);
END_PROGRAM",
        ironplc_problems::Problem::FunctionCallMixedArgTypes
    );

    const IN_OUT_CALLEE: &str = "
FUNCTION_BLOCK Callee
VAR_IN_OUT
    total : INT;
END_VAR
VAR_INPUT
    step : INT;
END_VAR
    total := total + step;
END_FUNCTION_BLOCK
";

    // IEC 61131-3 puts VAR_IN_OUT in the non-formal order, which function
    // block VAR_IN_OUT does not implement: `inst(5)` must not bind 5 to
    // `step` when the standard binds it to `total`.
    #[test]
    fn apply_when_nonformal_call_to_block_with_in_out_then_not_implemented() {
        let program = format!(
            "{IN_OUT_CALLEE}
PROGRAM main
VAR
    inst : Callee;
    x : INT;
END_VAR
    inst(5);
END_PROGRAM"
        );
        let opts = ironplc_parser::options::CompilerOptions::default();
        let (library, context) = crate::test_helpers::resolve_fresh_with(&program, &opts);
        let errors = super::apply(&library, &context, &opts).unwrap_err();
        assert_eq!(1, errors.len(), "{errors:?}");
        // P9999 == Problem::NotImplemented; the enum variant is #[deprecated]
        assert_eq!("P9999", errors[0].code);
    }

    #[test]
    fn apply_when_formal_call_to_block_with_in_out_then_ok() {
        let program = format!(
            "{IN_OUT_CALLEE}
PROGRAM main
VAR
    inst : Callee;
    x : INT;
END_VAR
    inst(total := x, step := 5);
END_PROGRAM"
        );
        let opts = ironplc_parser::options::CompilerOptions::default();
        let (library, context) = crate::test_helpers::resolve_fresh_with(&program, &opts);
        assert!(super::apply(&library, &context, &opts).is_ok());
    }
}
