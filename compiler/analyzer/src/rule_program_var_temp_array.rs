//! Semantic rule that a PROGRAM `VAR_TEMP` array declares initial values.
//!
//! A program's `VAR_TEMP` variables start from their initial values on every
//! scan, so the scan re-runs their initialization. For an array that sets
//! only the elements an initial value names: the others rely on the data
//! region starting zeroed, which holds for the first scan only. Without
//! initial values the array would silently keep the previous scan's values,
//! so it is refused (P9997) until code generation can reset it.
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!   VAR_TEMP a : ARRAY[1..3] OF INT := [1, 2, 3]; END_VAR
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//!   VAR_TEMP a : ARRAY[1..3] OF INT; END_VAR
//! END_PROGRAM
//! ```
use ironplc_dsl::{
    common::*,
    core::Located,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use std::convert::Infallible;

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
    run_rule(RuleProgramVarTempArray::default(), lib)
}

#[derive(Default)]
struct RuleProgramVarTempArray {
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<Infallible> for RuleProgramVarTempArray {
    type Value = ();

    fn visit_program_declaration(&mut self, node: &ProgramDeclaration) -> Result<(), Infallible> {
        for decl in &node.variables {
            if decl.var_type != VariableType::VarTemp {
                continue;
            }
            if let InitialValueAssignmentKind::Array(array) = &decl.initializer {
                if array.initial_values.is_empty() {
                    self.diagnostics.push(Diagnostic::not_supported(Label::span(
                        decl.identifier.span(),
                        "VAR_TEMP array in a PROGRAM without initial values",
                    )));
                }
            }
        }
        Ok(())
    }
}

impl DiagnosticVisitor for RuleProgramVarTempArray {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::apply;
    use crate::test_helpers::resolve_fresh_with;
    use ironplc_parser::options::CompilerOptions;

    #[test]
    fn apply_when_program_var_temp_array_without_initial_values_then_not_supported() {
        let options = CompilerOptions::default();
        let (library, context) = resolve_fresh_with(
            "
        PROGRAM main
        VAR_TEMP a : ARRAY[1..3] OF INT; END_VAR
            a[1] := 1;
        END_PROGRAM",
            &options,
        );
        let codes: Vec<String> = apply(&library, &context, &options)
            .unwrap_err()
            .iter()
            .map(|d| d.code.clone())
            .collect();
        assert_eq!(vec!["P9997".to_string()], codes);
    }

    rule_ok!(
        apply_when_program_var_temp_array_with_initial_values_then_ok,
        "
        PROGRAM main
        VAR_TEMP a : ARRAY[1..3] OF INT := [1, 2, 3]; END_VAR
            a[1] := 1;
        END_PROGRAM"
    );

    rule_ok!(
        apply_when_program_var_array_without_initial_values_then_ok,
        "
        PROGRAM main
        VAR a : ARRAY[1..3] OF INT; END_VAR
            a[1] := 1;
        END_PROGRAM"
    );

    rule_ok!(
        apply_when_function_block_var_temp_array_then_ok,
        "
        FUNCTION_BLOCK fb
        VAR_TEMP a : ARRAY[1..3] OF INT; END_VAR
            a[1] := 1;
        END_FUNCTION_BLOCK"
    );
}
