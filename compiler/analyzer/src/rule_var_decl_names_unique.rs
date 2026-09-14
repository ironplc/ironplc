//! Semantic rule that a variable name is declared at most once in a scope.
//!
//! The scopes are the global scope -- every `VAR_GLOBAL` in the merged
//! library, plus the compiler-provided uptime globals when
//! `allow_system_uptime_global` is on -- and each program, function,
//! function block and method, across all of its `VAR*` blocks and edge
//! variables. A `VAR_EXTERNAL` names a global declared in the global scope,
//! so it does not duplicate that global; two `VAR_EXTERNAL` of one name in
//! one unit do duplicate each other.
//!
//! Which *cross-scope* pairs are rejected is decided by ADR-0051 and enforced
//! by `rule_program_var_hides_global` and `rule_extends_field_duplicated`;
//! this rule never compares two scopes.
//!
//! A duplicate is diagnosed here, on the AST, rather than when the symbol
//! environment is built: that happens inside a transform which reverts the
//! whole library on `Err`, so one duplicate would have discarded every other
//! resolution. Left undiagnosed, the later declaration silently wins the
//! variable slot and the earlier one is never read (issue #1525).
//!
//! ## Passes
//!
//! ```ignore
//! PROGRAM main
//!   VAR_INPUT
//!     start : BOOL;
//!   END_VAR
//!   VAR
//!     running : BOOL;
//!   END_VAR
//! END_PROGRAM
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! PROGRAM main
//!   VAR_INPUT
//!     start : BOOL;
//!   END_VAR
//!   VAR
//!     start : INT;
//!   END_VAR
//! END_PROGRAM
//! ```
use std::collections::HashMap;
use std::convert::Infallible;

use ironplc_dsl::{
    common::*,
    core::{Id, Located, SourceSpan},
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;

use crate::{
    intermediates::global_vars::collect_global_var_decls,
    result::SemanticResult,
    rule_support::{run_rule, DiagnosticVisitor},
    semantic_context::SemanticContext,
    system_globals::SYSTEM_UPTIME_GLOBALS,
};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: &Library,
    _context: &SemanticContext,
    options: &CompilerOptions,
) -> SemanticResult {
    let mut rule = RuleVarDeclNamesUnique {
        diagnostics: Vec::new(),
    };

    // The global scope is one scope however many blocks and files declare
    // into it, so it is checked once here rather than per block in the walk.
    let mut globals = HashMap::new();
    if options.allow_system_uptime_global {
        for global in &SYSTEM_UPTIME_GLOBALS {
            globals.insert(Id::from(global.name), First::CompilerProvided);
        }
    }
    let global_decls = collect_global_var_decls(lib);
    let global_names = global_decls
        .iter()
        .filter_map(|decl| decl.identifier.symbolic_id());
    rule.check_scope(&mut globals, global_names);

    run_rule(rule, lib)
}

/// The earlier declaration a later one collides with.
enum First {
    /// Declared in source, at this span.
    Declared(SourceSpan),
    /// Declared by the compiler; there is no source to point at.
    CompilerProvided,
}

struct RuleVarDeclNamesUnique {
    diagnostics: Vec<Diagnostic>,
}

impl RuleVarDeclNamesUnique {
    /// Reports every name in `names` that `seen` already holds, and records
    /// the rest. `names` must be in declaration order so the earliest
    /// declaration is the one later ones are reported against.
    fn check_scope<'a>(
        &mut self,
        seen: &mut HashMap<Id, First>,
        names: impl Iterator<Item = &'a Id>,
    ) {
        for name in names {
            match seen.get(name) {
                Some(First::Declared(first)) => self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::SymbolDeclDuplicated,
                        Label::span(name.span(), "Variable is already declared in this scope"),
                    )
                    .with_context_id("variable", name)
                    .with_secondary(Label::span(first.clone(), "First declaration")),
                ),
                Some(First::CompilerProvided) => self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::SymbolDeclDuplicated,
                        Label::span(
                            name.span(),
                            "Variable name is reserved for a compiler-provided global",
                        ),
                    )
                    .with_context_id("variable", name)
                    .with_help(
                        "The compiler declares this global when --allow-system-uptime-global \
                         is on. Remove the declaration to read the compiler's value, or rename \
                         the variable.",
                    ),
                ),
                None => {
                    seen.insert(name.clone(), First::Declared(name.span()));
                }
            }
        }
    }

    /// Checks one program organization unit or method as a single scope.
    ///
    /// Ordinary and edge variables are kept on separate lists by the parser,
    /// so the names are put back into source order first: the diagnostic
    /// points at the later declaration and names the earlier one.
    fn check_unit(&mut self, variables: &[VarDecl], edge_variables: &[EdgeVarDecl]) {
        let mut names: Vec<&Id> = variables
            .iter()
            .filter_map(|decl| decl.identifier.symbolic_id())
            .chain(edge_variables.iter().map(|decl| &decl.identifier))
            .collect();
        names.sort_by_key(|name| name.span().start);
        self.check_scope(&mut HashMap::new(), names.into_iter());
    }
}

impl DiagnosticVisitor for RuleVarDeclNamesUnique {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl Visitor<Infallible> for RuleVarDeclNamesUnique {
    type Value = ();

    fn visit_function_declaration(
        &mut self,
        node: &FunctionDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.check_unit(&node.variables, &node.edge_variables);
        Ok(())
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.check_unit(&node.variables, &node.edge_variables);
        // Each method is its own scope, visited below.
        node.recurse_visit(self)
    }

    fn visit_method_declaration(
        &mut self,
        node: &MethodDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.check_unit(&node.variables, &node.edge_variables);
        Ok(())
    }

    fn visit_program_declaration(
        &mut self,
        node: &ProgramDeclaration,
    ) -> Result<Self::Value, Infallible> {
        self.check_unit(&node.variables, &[]);
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

    fn top_level_globals() -> CompilerOptions {
        CompilerOptions {
            allow_top_level_var_global: true,
            ..CompilerOptions::default()
        }
    }

    fn top_level_globals_and_uptime() -> CompilerOptions {
        CompilerOptions {
            allow_system_uptime_global: true,
            ..top_level_globals()
        }
    }

    rule_ok!(
        apply_when_names_unique_across_blocks_then_ok,
        "
PROGRAM main
  VAR_INPUT
    start : BOOL;
  END_VAR
  VAR
    running : BOOL;
  END_VAR
END_PROGRAM"
    );

    // The second declaration differs in case so the expected location is
    // unambiguous in the source text; identifiers compare case-insensitively.
    rule_err1_at!(
        apply_when_name_duplicated_in_one_block_then_error_at_second,
        "
PROGRAM main
  VAR
    count : INT;
    second : INT;
    COUNT : BOOL;
  END_VAR
END_PROGRAM",
        Problem::SymbolDeclDuplicated,
        "COUNT"
    );

    rule_err1!(
        apply_when_name_duplicated_across_blocks_then_error,
        "
FUNCTION_BLOCK fb
  VAR_INPUT
    start : BOOL;
  END_VAR
  VAR
    start : INT;
  END_VAR
END_FUNCTION_BLOCK",
        Problem::SymbolDeclDuplicated
    );

    rule_err1!(
        apply_when_names_differ_only_in_case_then_error,
        "
FUNCTION f : INT
  VAR_INPUT
    Count : INT;
  END_VAR
  VAR_OUTPUT
    COUNT : INT;
  END_VAR
  f := Count;
END_FUNCTION",
        Problem::SymbolDeclDuplicated
    );

    rule_errn!(
        apply_when_name_declared_three_times_then_reports_each_later_one,
        "
PROGRAM main
  VAR
    x : INT;
    x : INT;
    x : INT;
  END_VAR
END_PROGRAM",
        2,
        Problem::SymbolDeclDuplicated
    );

    rule_err1!(
        apply_when_edge_variable_named_like_variable_then_error,
        "
FUNCTION_BLOCK fb
  VAR_INPUT
    trigger : BOOL R_EDGE;
  END_VAR
  VAR
    trigger : INT;
  END_VAR
END_FUNCTION_BLOCK",
        Problem::SymbolDeclDuplicated
    );

    rule_err1!(
        apply_when_global_duplicated_across_configuration_and_resource_then_error,
        "
CONFIGURATION config
  VAR_GLOBAL
    Limit : INT;
  END_VAR
  RESOURCE res ON PLC
    VAR_GLOBAL
      Limit : INT;
    END_VAR
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
END_PROGRAM",
        Problem::SymbolDeclDuplicated
    );

    rule_err1_with!(
        apply_when_global_duplicated_across_top_level_and_configuration_then_error,
        top_level_globals(),
        &with_config(
            "
VAR_GLOBAL
  MaxSpeed : INT;
END_VAR

PROGRAM main
END_PROGRAM"
        ),
        Problem::SymbolDeclDuplicated
    );

    rule_ok!(
        apply_when_program_names_global_through_var_external_then_ok,
        &with_config(
            "
PROGRAM main
  VAR_EXTERNAL
    MaxSpeed : INT;
  END_VAR
END_PROGRAM"
        )
    );

    rule_err1!(
        apply_when_var_external_declared_twice_then_error,
        &with_config(
            "
PROGRAM main
  VAR_EXTERNAL
    MaxSpeed : INT;
  END_VAR
  VAR_EXTERNAL
    MaxSpeed : INT;
  END_VAR
END_PROGRAM"
        ),
        Problem::SymbolDeclDuplicated
    );

    // Cross-scope hiding is ADR-0051's question, not this rule's.
    rule_ok!(
        apply_when_function_local_named_like_global_then_ok,
        &with_config(
            "
FUNCTION f : INT
  VAR
    MaxSpeed : INT := 5;
  END_VAR
  f := MaxSpeed;
END_FUNCTION

PROGRAM main
END_PROGRAM"
        )
    );

    rule_ok_with!(
        apply_when_method_local_named_like_function_block_field_then_ok,
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        },
        "
FUNCTION_BLOCK fb
  VAR
    speed : INT;
  END_VAR
  METHOD get : INT
    VAR
      speed : INT;
    END_VAR
    get := speed;
  END_METHOD
END_FUNCTION_BLOCK"
    );

    rule_err1_with!(
        apply_when_method_variable_duplicated_then_error,
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        },
        "
FUNCTION_BLOCK fb
  METHOD get : INT
    VAR_INPUT
      speed : INT;
    END_VAR
    VAR
      speed : INT;
    END_VAR
    get := speed;
  END_METHOD
END_FUNCTION_BLOCK",
        Problem::SymbolDeclDuplicated
    );

    rule_ok!(
        apply_when_located_variables_have_no_name_then_ok,
        "
PROGRAM main
  VAR
    AT %IX0.0 : BOOL;
    AT %IX0.1 : BOOL;
  END_VAR
END_PROGRAM"
    );

    // Issue #1525: redeclaring a compiler-provided global.
    rule_err1_with!(
        apply_when_global_redeclares_system_uptime_with_flag_on_then_error,
        top_level_globals_and_uptime(),
        "
VAR_GLOBAL
  __SYSTEM_UP_TIME : TIME;
END_VAR

PROGRAM main
  VAR
    seen : TIME;
  END_VAR
  seen := __SYSTEM_UP_TIME;
END_PROGRAM",
        Problem::SymbolDeclDuplicated
    );

    rule_ok_with!(
        apply_when_global_named_system_uptime_with_flag_off_then_ok,
        top_level_globals(),
        "
VAR_GLOBAL
  __SYSTEM_UP_TIME : TIME;
END_VAR

PROGRAM main
  VAR
    seen : TIME;
  END_VAR
  seen := __SYSTEM_UP_TIME;
END_PROGRAM"
    );
}
