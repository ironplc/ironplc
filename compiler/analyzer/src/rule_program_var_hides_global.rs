//! Semantic rule that a program does not declare a variable with the
//! same name as a global variable.
//!
//! A program reaches a global through `VAR_EXTERNAL`. A `VAR`,
//! `VAR_INPUT`, `VAR_OUTPUT` or other own declaration that reuses a
//! global's name is rejected rather than hiding the global. See
//! ADR-0051 for why this pair is rejected while a function, function
//! block or method local may still hide an outer name.
//!
//! ## Passes
//!
//! ```ignore
//! CONFIGURATION config
//!   VAR_GLOBAL
//!     MaxSpeed : INT := 100;
//!   END_VAR
//! END_CONFIGURATION
//!
//! PROGRAM main
//!   VAR_EXTERNAL
//!     MaxSpeed : INT;
//!   END_VAR
//!   VAR
//!     currentSpeed : INT;
//!   END_VAR
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! CONFIGURATION config
//!   VAR_GLOBAL
//!     MaxSpeed : INT := 100;
//!   END_VAR
//! END_CONFIGURATION
//!
//! PROGRAM main
//!   VAR
//!     MaxSpeed : INT;
//!   END_VAR
//! END_PROGRAM
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
    // Keyed by the global's own `Id` so the diagnostic can point at the
    // global's declaration. `Id` compares case-insensitively.
    let globals: HashSet<Id> = collect_global_var_decls(lib)
        .iter()
        .filter_map(|decl| decl.identifier.symbolic_id().cloned())
        .collect();

    run_rule(
        RuleProgramVarHidesGlobal {
            globals,
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleProgramVarHidesGlobal {
    globals: HashSet<Id>,
    diagnostics: Vec<Diagnostic>,
}

impl DiagnosticVisitor for RuleProgramVarHidesGlobal {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleProgramVarHidesGlobal {
    type Value = ();

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Infallible> {
        for decl in &node.variables {
            // `VAR_EXTERNAL` is how a program names a global, not a
            // redeclaration of it.
            if decl.var_type == VariableType::External {
                continue;
            }
            let Some(name) = decl.identifier.symbolic_id() else {
                continue;
            };
            if let Some(global) = self.globals.get(name) {
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::ProgramVariableHidesGlobal,
                        Label::span(
                            decl.identifier.span(),
                            "Program variable has the same name as a global variable",
                        ),
                    )
                    .with_context_id("variable", name)
                    .with_context_id("program", &node.name)
                    .with_secondary(Label::span(global.span(), "Global variable")),
                );
            }
        }

        // A program body cannot declare variables, so there is nothing
        // further down to visit.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ironplc_parser::options::CompilerOptions;
    use ironplc_problems::Problem;

    const CONFIG: &str = "
CONFIGURATION config
  VAR_GLOBAL
    MaxSpeed : INT := 100;
  END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION
";

    fn with_config(pou: &str) -> String {
        format!("{CONFIG}{pou}")
    }

    rule_ok!(
        apply_when_program_uses_var_external_then_ok,
        &with_config(
            "
PROGRAM main
  VAR_EXTERNAL
    MaxSpeed : INT;
  END_VAR
  VAR
    currentSpeed : INT;
  END_VAR
  currentSpeed := MaxSpeed;
END_PROGRAM"
        )
    );

    rule_err1!(
        apply_when_program_var_named_like_configuration_global_then_error,
        &with_config(
            "
PROGRAM main
  VAR
    MaxSpeed : INT;
  END_VAR
END_PROGRAM"
        ),
        Problem::ProgramVariableHidesGlobal
    );

    rule_err1!(
        apply_when_program_var_differs_only_in_case_then_error,
        &with_config(
            "
PROGRAM main
  VAR
    MAXSPEED : INT;
  END_VAR
END_PROGRAM"
        ),
        Problem::ProgramVariableHidesGlobal
    );

    rule_err1!(
        apply_when_program_input_named_like_global_then_error,
        &with_config(
            "
PROGRAM main
  VAR_INPUT
    MaxSpeed : INT;
  END_VAR
END_PROGRAM"
        ),
        Problem::ProgramVariableHidesGlobal
    );

    rule_err1!(
        apply_when_program_var_named_like_resource_global_then_error,
        "
CONFIGURATION config
  RESOURCE res ON PLC
    VAR_GLOBAL
      Limit : INT;
    END_VAR
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR
    Limit : INT;
  END_VAR
END_PROGRAM",
        Problem::ProgramVariableHidesGlobal
    );

    rule_err1_with!(
        apply_when_program_var_named_like_top_level_global_then_error,
        CompilerOptions {
            allow_top_level_var_global: true,
            ..CompilerOptions::default()
        },
        "
VAR_GLOBAL
  Limit : INT;
END_VAR

PROGRAM main
  VAR
    Limit : INT;
  END_VAR
END_PROGRAM",
        Problem::ProgramVariableHidesGlobal
    );

    rule_errn!(
        apply_when_two_program_vars_named_like_globals_then_reports_both,
        "
CONFIGURATION config
  VAR_GLOBAL
    First : INT;
    Second : INT;
  END_VAR
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
  VAR
    First : INT;
    Second : INT;
  END_VAR
END_PROGRAM",
        2,
        Problem::ProgramVariableHidesGlobal
    );

    // Hiding in a function block compiles correctly and is relied on by
    // user code that redeclares a library constant, so it stays allowed.
    rule_ok!(
        apply_when_function_block_var_named_like_global_then_ok,
        &with_config(
            "
FUNCTION_BLOCK FB_Own
  VAR
    MaxSpeed : INT := 5;
  END_VAR
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    fb : FB_Own;
  END_VAR
END_PROGRAM"
        )
    );

    rule_ok!(
        apply_when_function_var_named_like_global_then_ok,
        &with_config(
            "
FUNCTION f : INT
  VAR
    MaxSpeed : INT := 5;
  END_VAR
  f := MaxSpeed;
END_FUNCTION

PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := f();
END_PROGRAM"
        )
    );
}
