//! Semantic rule that global variables declared with the CONSTANT
//! qualifier class must be declared constant in contained element.
//!
//! See section 2.4.3.
//!
//! ## Passes
//!
//! ```ignore
//! CONFIGURATION config
//!   VAR_GLOBAL CONSTANT
//!     ResetCounterValue : INT := 17;
//!   END_VAR
//! END_CONFIGURATION
//!
//! FUNCTION_BLOCK func
//!   VAR_EXTERNAL CONSTANT
//!     ResetCounterValue : INT
//!   END_VAR
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! CONFIGURATION config
//!   VAR_GLOBAL CONSTANT
//!     ResetCounterValue : INT := 17;
//!   END_VAR
//! END_CONFIGURATION
//!
//! FUNCTION_BLOCK func
//!   VAR_EXTERNAL
//!     ResetCounterValue : INT
//!   END_VAR
//! END_FUNCTION_BLOCK
//! ```
use std::collections::HashSet;

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
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
    let mut global_consts = HashSet::new();

    let mut diagnostics = Vec::new();

    // Collect the global constants
    if let Err(errs) = run_rule(
        FindGlobalConstVars {
            global_consts: &mut global_consts,
            diagnostics: Vec::new(),
        },
        lib,
    ) {
        diagnostics.extend(errs);
    }

    // Check that externals with the same name are constants. This runs even
    // when collection reported a problem: the constants it did collect are
    // still worth checking, and stopping here would hide every violation
    // behind one unhandled declaration.
    if let Err(errs) = run_rule(
        RuleExternalGlobalConst {
            global_consts: &mut global_consts,
            diagnostics: Vec::new(),
        },
        lib,
    ) {
        diagnostics.extend(errs);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

struct FindGlobalConstVars<'a> {
    global_consts: &'a mut HashSet<Id>,
    diagnostics: Vec<Diagnostic>,
}
impl DiagnosticVisitor for FindGlobalConstVars<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Diagnostic> for FindGlobalConstVars<'_> {
    type Value = ();
    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        if node.qualifier == DeclarationQualifier::Constant {
            match &node.identifier {
                VariableIdentifier::Symbol(name) => {
                    self.global_consts.insert(name.clone());
                }
                // A located CONSTANT declaration (`AT %QW0 : INT`) is not
                // handled yet. Record that and keep collecting, so the rule
                // still reports on every other declaration.
                VariableIdentifier::Direct(_) => self.diagnostics.push(Diagnostic::todo()),
            }
        }
        Ok(())
    }
}

struct RuleExternalGlobalConst<'a> {
    global_consts: &'a mut HashSet<Id>,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleExternalGlobalConst<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Diagnostic> for RuleExternalGlobalConst<'_> {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Diagnostic> {
        if node.var_type == VariableType::External
            && node.qualifier != DeclarationQualifier::Constant
        {
            if let Some(name) = node.identifier.symbolic_id() {
                // Cloned so that the borrow of `global_consts` ends before the
                // push, which borrows `self` mutably.
                let global = self.global_consts.get(name).cloned();
                if let Some(global) = global {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::VariableMustBeConst,
                            Label::span(node.identifier.span(), "Reference to global variable"),
                        )
                        .with_context("variable", &node.identifier.to_string())
                        .with_secondary(Label::span(global.span(), "Constant global variable")),
                    );
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    rule_err!(
        apply_when_global_const_external_not_const_then_error,
        "
CONFIGURATION config
    VAR_GLOBAL CONSTANT
        ResetCounterValue : INT := 17;
    END_VAR
    RESOURCE resource1 ON PLC
        TASK plc_task(INTERVAL := T#100ms,PRIORITY := 1);
        PROGRAM plc_task_instance WITH plc_task : plc_prg;
    END_RESOURCE
END_CONFIGURATION

FUNCTION_BLOCK func
    VAR_EXTERNAL
        ResetCounterValue : INT;
    END_VAR
END_FUNCTION_BLOCK"
    );

    rule_ok!(
        apply_when_global_const_external_const_then_ok,
        "
CONFIGURATION config
    VAR_GLOBAL CONSTANT
        ResetCounterValue : INT := 17;
    END_VAR
    RESOURCE resource1 ON PLC
        TASK plc_task(INTERVAL := T#100ms,PRIORITY := 1);
        PROGRAM plc_task_instance WITH plc_task : plc_prg;
    END_RESOURCE

END_CONFIGURATION

FUNCTION_BLOCK func
    VAR_EXTERNAL CONSTANT
        ResetCounterValue : INT;
    END_VAR

END_FUNCTION_BLOCK"
    );

    rule_errn!(
        apply_when_two_non_const_externals_then_reports_both,
        "
CONFIGURATION config
    VAR_GLOBAL CONSTANT
        FirstValue : INT := 17;
        SecondValue : INT := 18;
    END_VAR
    RESOURCE resource1 ON PLC
        TASK plc_task(INTERVAL := T#100ms,PRIORITY := 1);
        PROGRAM plc_task_instance WITH plc_task : plc_prg;
    END_RESOURCE
END_CONFIGURATION

FUNCTION_BLOCK func
    VAR_EXTERNAL
        FirstValue : INT;
        SecondValue : INT;
    END_VAR
END_FUNCTION_BLOCK",
        2,
        ironplc_problems::Problem::VariableMustBeConst
    );
}
