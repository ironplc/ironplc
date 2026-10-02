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
use std::convert::Infallible;

use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    symbol_environment::{ScopeKind, SymbolEnvironment, SymbolInfo},
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let symbols = context.symbols();
    let mut diagnostics = Vec::new();

    // A located CONSTANT declaration (`AT %QW0 : INT`) is not handled yet.
    // Record that and keep going, so the rule still reports on every other
    // declaration.
    for (_, info) in symbols.get_variables_in_scope(&ScopeKind::Global) {
        if is_global_constant(info) && info.address.is_some() {
            diagnostics.push(Diagnostic::not_implemented(Label::span(
                info.span.clone(),
                "Located CONSTANT declaration",
            )));
        }
    }

    // Check that externals naming a global constant are constants. This runs
    // even when a located constant was reported: stopping there would hide
    // every violation behind one unhandled declaration.
    if let Err(errs) = run_rule(
        RuleExternalGlobalConst {
            symbols,
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

/// Whether `info` is a `VAR_GLOBAL` declared `CONSTANT`.
///
/// Only a global constant obliges its externals to be constant. A `VAR
/// CONSTANT` local to one unit says nothing about a global that happens to
/// share its name. The qualifier is the one the source wrote, before the
/// compiler infers constants, so an inferred constant global (whose
/// externals inference marks too) does not oblige anything here.
fn is_global_constant(info: &SymbolInfo) -> bool {
    info.variable_type == Some(VariableType::Global) && info.is_constant()
}

struct RuleExternalGlobalConst<'a> {
    symbols: &'a SymbolEnvironment,
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
                // A located global is reported as not implemented above.
                let global = self
                    .symbols
                    .find(name, &ScopeKind::Global)
                    .filter(|info| is_global_constant(info) && info.address.is_none());
                if let Some(global) = global {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::VariableMustBeConst,
                            Label::span(node.identifier.span(), "Reference to global variable"),
                        )
                        .with_context("variable", &node.identifier.to_string())
                        .with_secondary(Label::span(
                            global.span.clone(),
                            "Constant global variable",
                        )),
                    );
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    rule_ctx_err!(
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

    rule_ctx_ok!(
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

    rule_ctx_ok!(
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

    rule_ctx_errn!(
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
