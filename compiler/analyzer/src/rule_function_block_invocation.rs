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
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    textual::*,
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::collections::HashMap;
use std::convert::Infallible;

use crate::{
    intermediates::stdlib_function_block::is_stdlib_function_block,
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
    // Collect the names from the library into a map so that
    // we can quickly look up invocations
    let mut function_blocks = HashMap::new();
    for x in lib.elements.iter() {
        if let LibraryElementKind::FunctionBlockDeclaration(fb) = x {
            function_blocks.insert(fb.name.clone(), fb);
        }
    }

    // Walk the library to find all references to function blocks
    run_rule(RuleFunctionBlockUse::new(&function_blocks), lib)
}

struct RuleFunctionBlockUse<'a> {
    // Map of the name of a function block declaration to the
    // declaration itself.
    function_blocks: &'a HashMap<TypeName, &'a FunctionBlockDeclaration>,

    // Map of variable name to the function block name that is the implementation
    var_to_fb: HashMap<Id, TypeName>,

    diagnostics: Vec<Diagnostic>,
}
impl<'a> RuleFunctionBlockUse<'a> {
    fn new(decls: &'a HashMap<TypeName, &'a FunctionBlockDeclaration>) -> Self {
        Self {
            function_blocks: decls,
            var_to_fb: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    fn check_assignments(
        function_block: &FunctionBlockDeclaration,
        fb_call: &FbCall,
    ) -> Vec<Diagnostic> {
        crate::call_assignment_check::check_assignments(
            function_block,
            function_block.span(),
            fb_call.span(),
            &fb_call.params,
            &crate::call_assignment_check::AssignmentCheckLabels {
                call_label: "Function block invocation",
                context_key: "invocation",
                owner_name: &function_block.name.to_string(),
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

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let res = node.recurse_visit(self);

        // Remove all items from var init decl since we have left this context
        self.var_to_fb.clear();
        res
    }

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let res = node.recurse_visit(self);

        // Remove all items from var init decl since we have left this context
        self.var_to_fb.clear();
        res
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Infallible> {
        let res = node.recurse_visit(self);

        // Remove all items from var init decl since we have left this context
        self.var_to_fb.clear();
        res
    }

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Infallible> {
        if let InitialValueAssignmentKind::FunctionBlock(fbi) = &node.initializer {
            if let Some(id) = node.identifier.symbolic_id() {
                self.var_to_fb.insert(id.clone(), fbi.type_name.clone());
            }
        }
        Ok(())
    }

    fn visit_fb_call(&mut self, fb_call: &FbCall) -> Result<Self::Value, Infallible> {
        // Check if function block is defined because you cannot
        // call a function block that doesn't exist
        // Cloned so that the borrow of `var_to_fb` ends here: the arms below
        // push onto `self.diagnostics`, which borrows `self` mutably.
        let function_block_name = self.var_to_fb.get(&fb_call.var_name).cloned();
        let Some(function_block_name) = function_block_name else {
            self.diagnostics.push(Self::not_in_scope(fb_call));
            return Ok(());
        };

        // Standard library function blocks (TON, TOF, TP, CTU, etc.)
        // are validated during type resolution, not here.
        if is_stdlib_function_block(&function_block_name.name) {
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
}
