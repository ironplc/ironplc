//! Semantic rule that a function does not declare a `PERSISTENT` variable
//! (`--allow-persistent-var` extension).
//!
//! A `PERSISTENT` variable keeps its value through a download, a reset and
//! a power loss, so it needs storage that outlives a call. A function has no
//! instance to hold it: TwinCAT rejects `PERSISTENT` on a function's
//! `VAR_INPUT`, and Beckhoff documents a function's `PERSISTENT` variable as
//! having no effect. This rule reports each `PERSISTENT` or
//! `RETAIN PERSISTENT` variable a function declares.
//!
//! ## Passes
//!
//! ```ignore
//! FUNCTION_BLOCK Counter
//! VAR_INPUT PERSISTENT
//!     start : DINT;
//! END_VAR
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! FUNCTION Twice : DINT
//! VAR_INPUT PERSISTENT
//!     value : DINT;    (* P4098: a function has no instance *)
//! END_VAR
//!     Twice := value * 2;
//! END_FUNCTION
//! ```
use ironplc_dsl::{
    common::{DeclarationQualifier, FunctionDeclaration, Library},
    core::Located,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;
use std::convert::Infallible;

use crate::{
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
};

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RulePersistentVarOwner {
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RulePersistentVarOwner {
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RulePersistentVarOwner {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RulePersistentVarOwner {
    type Value = ();

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Infallible> {
        for var in &node.variables {
            if matches!(
                var.qualifier,
                DeclarationQualifier::Persistent | DeclarationQualifier::RetainPersistent
            ) {
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::PersistentVarInFunction,
                        Label::span(var.identifier.span(), "Persistent variable"),
                    )
                    .with_context("function", &node.name.to_string()),
                );
            }
        }
        node.recurse_visit(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn persistent_options() -> CompilerOptions {
        CompilerOptions {
            allow_persistent_var: true,
            ..CompilerOptions::default()
        }
    }

    rule_ok!(
        apply_when_function_block_has_persistent_io_then_ok,
        "
FUNCTION_BLOCK Counter
VAR_INPUT PERSISTENT
    start : DINT;
END_VAR
VAR_OUTPUT RETAIN PERSISTENT
    count : DINT;
END_VAR
END_FUNCTION_BLOCK",
        persistent_options()
    );

    rule_ok!(
        apply_when_function_has_retain_input_then_ok,
        "
FUNCTION Twice : DINT
VAR_INPUT RETAIN
    value : DINT;
END_VAR
    Twice := value * 2;
END_FUNCTION",
        persistent_options()
    );

    rule_err_at!(
        apply_when_function_has_persistent_input_then_p4098,
        "
FUNCTION Twice : DINT
VAR_INPUT PERSISTENT
    value : DINT;
END_VAR
    Twice := value * 2;
END_FUNCTION",
        Problem::PersistentVarInFunction,
        "value",
        persistent_options()
    );

    rule_err!(
        /// Each persistent variable is reported, in either kind of block.
        apply_when_function_has_persistent_output_and_retain_persistent_input_then_p4098_for_each,
        "
FUNCTION Twice : DINT
VAR_INPUT PERSISTENT RETAIN
    value : DINT;
END_VAR
VAR_OUTPUT PERSISTENT
    copy : DINT;
END_VAR
    Twice := value * 2;
    copy := value;
END_FUNCTION",
        [Problem::PersistentVarInFunction; 2],
        persistent_options()
    );
}
