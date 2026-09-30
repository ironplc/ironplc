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
use std::convert::Infallible;

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    intermediates::global_vars::collect_global_var_decls,
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
    // Collect the global constants. Only a global constant obliges its
    // externals to be constant. A `VAR CONSTANT` local to one unit says
    // nothing about a global that happens to share its name.
    //
    // A located constant (`Limit AT %MW8 : INT := 3`) is collected by its
    // name like any other. One without a name (`AT %MW8 : INT := 3`) cannot
    // be named by a `VAR_EXTERNAL`, so there is nothing to check for it.
    let mut global_consts: HashSet<Id> = collect_global_var_decls(lib)
        .iter()
        .filter(|decl| decl.qualifier == DeclarationQualifier::Constant)
        .filter_map(|decl| decl.identifier.symbolic_id().cloned())
        .collect();

    // Check that externals with the same name are constants.
    run_rule(
        RuleExternalGlobalConst {
            global_consts: &mut global_consts,
            diagnostics: Vec::new(),
        },
        lib,
    )
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

impl Visitor<Infallible> for RuleExternalGlobalConst<'_> {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Infallible> {
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
        apply_when_local_const_shares_name_with_plain_global_then_ok,
        "
CONFIGURATION config
    VAR_GLOBAL
        Limit : INT := 17;
    END_VAR
    RESOURCE resource1 ON PLC
        TASK plc_task(INTERVAL := T#100ms,PRIORITY := 1);
        PROGRAM plc_task_instance WITH plc_task : plc_prg;
    END_RESOURCE
END_CONFIGURATION

FUNCTION_BLOCK reader
    VAR_EXTERNAL
        Limit : INT;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK other
    VAR CONSTANT
        Limit : INT := 1;
    END_VAR
END_FUNCTION_BLOCK

PROGRAM plc_prg
END_PROGRAM"
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

    rule_err_code!(
        apply_when_located_global_const_external_not_const_then_error,
        "
CONFIGURATION config
    VAR_GLOBAL CONSTANT
        Limit AT %MW8 : INT := 3;
    END_VAR
    RESOURCE resource1 ON PLC
        PROGRAM plc_task_instance : plc_prg;
    END_RESOURCE
END_CONFIGURATION

PROGRAM plc_prg
    VAR_EXTERNAL
        Limit : INT;
    END_VAR
END_PROGRAM",
        ironplc_problems::Problem::VariableMustBeConst
    );

    rule_ok!(
        apply_when_located_global_const_external_const_then_ok,
        "
CONFIGURATION config
    VAR_GLOBAL CONSTANT
        Limit AT %MW8 : INT := 3;
    END_VAR
    RESOURCE resource1 ON PLC
        PROGRAM plc_task_instance : plc_prg;
    END_RESOURCE
END_CONFIGURATION

PROGRAM plc_prg
    VAR_EXTERNAL CONSTANT
        Limit : INT;
    END_VAR
END_PROGRAM"
    );

    rule_ok!(
        apply_when_unnamed_located_global_const_then_ok,
        "
CONFIGURATION config
    VAR_GLOBAL CONSTANT
        AT %MW8 : INT := 3;
    END_VAR
    RESOURCE resource1 ON PLC
        PROGRAM plc_task_instance : plc_prg;
    END_RESOURCE
END_CONFIGURATION

PROGRAM plc_prg
END_PROGRAM"
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
