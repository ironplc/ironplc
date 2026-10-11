//! Semantic rule that a task's parameters mean something.
//!
//! The `INTERVAL` of a cyclic task is a period, so it cannot be negative. The
//! `SINGLE` parameter of an event task names a `BOOL` global whose rising edge
//! triggers the task, so it must name a declared global, and that global must
//! be `BOOL`. These are errors whatever backend compiles the library, so they
//! are reported here rather than by codegen (see
//! `specs/design/execution-model.md`).
//!
//! ## Passes
//!
//! ```ignore
//! CONFIGURATION config
//!   VAR_GLOBAL
//!     trigger : BOOL;
//!   END_VAR
//!   RESOURCE resource1 ON PLC
//!     TASK cyclic(INTERVAL := T#10ms, PRIORITY := 1);
//!     TASK event(SINGLE := trigger, PRIORITY := 2);
//!     PROGRAM instance1 WITH cyclic : main;
//!   END_RESOURCE
//! END_CONFIGURATION
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! CONFIGURATION config
//!   VAR_GLOBAL
//!     count : INT;
//!   END_VAR
//!   RESOURCE resource1 ON PLC
//!     TASK backwards(INTERVAL := T#-10ms, PRIORITY := 1);
//!     TASK event(SINGLE := count, PRIORITY := 2);
//!     PROGRAM instance1 WITH backwards : main;
//!   END_RESOURCE
//! END_CONFIGURATION
//! ```
use ironplc_dsl::{
    common::*,
    configuration::{DataSourceKind, GlobalVarReference, TaskConfiguration},
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
    semantic_type::SemanticType,
    symbol_environment::{ScopeKind, SymbolKind},
    variable_type,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    run_rule(
        RuleTaskConfiguration {
            context,
            diagnostics: Vec::new(),
        },
        lib,
    )
}

struct RuleTaskConfiguration<'a> {
    context: &'a SemanticContext,
    diagnostics: Vec<Diagnostic>,
}

impl RuleTaskConfiguration<'_> {
    /// Checks that `reference` names a declared `BOOL` global.
    fn check_single(&mut self, task: &TaskConfiguration, reference: &GlobalVarReference) {
        let name = &reference.global_var_name;
        let declared = self
            .context
            .symbols()
            .find(name, &ScopeKind::Global)
            .filter(|symbol| symbol.kind == SymbolKind::Variable);
        if declared.is_none() {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::VariableUndefined,
                    Label::span(name.span(), "Undefined variable"),
                )
                .with_context_id("task", &task.name)
                .with_context_id("variable", name),
            );
            return;
        }

        let declared_type = variable_type::declared(name, self.context, &ScopeKind::Global);
        let trigger_type = match &reference.structure_element_name {
            None => declared_type.cloned(),
            Some(field) => {
                declared_type.and_then(|parent| variable_type::struct_field_type(parent, field))
            }
        };
        // A global whose type did not resolve is reported where it is
        // declared; repeating that here would add nothing.
        if trigger_type
            .as_ref()
            .is_some_and(|t| *t != SemanticType::Bool)
        {
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::TaskSingleNotBool,
                    Label::span(name.span(), "SINGLE variable"),
                )
                .with_context_id("task", &task.name)
                .with_context_id("variable", name),
            );
        }
    }
}

impl Visitor<Infallible> for RuleTaskConfiguration<'_> {
    type Value = ();

    fn visit_task_configuration(
        &mut self,
        node: &TaskConfiguration,
    ) -> Result<Self::Value, Infallible> {
        if let Some(interval) = &node.interval {
            if interval.interval.is_negative() {
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::TaskIntervalNegative,
                        Label::span(interval.span.clone(), "Task INTERVAL"),
                    )
                    .with_context_id("task", &node.name),
                );
            }
        }

        // A constant SINGLE is what the grammar's `data_source` allows; the
        // parser has already checked that it is a constant.
        if let Some(DataSourceKind::GlobalVarReference(reference)) = &node.single {
            self.check_single(node, reference);
        }

        Ok(())
    }
}

impl DiagnosticVisitor for RuleTaskConfiguration<'_> {
    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

#[cfg(test)]
mod tests {
    use ironplc_problems::Problem;
    use spec_test_macro::spec_test;

    /// A configuration that declares one task with the given parameters,
    /// and the globals `flag : BOOL` and `count : INT`.
    macro_rules! task {
        ($init:literal) => {
            concat!(
                "
PROGRAM main
  VAR x : INT; END_VAR
  x := 1;
END_PROGRAM
CONFIGURATION config
  VAR_GLOBAL
    flag : BOOL;
    count : INT;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK task1(",
                $init,
                ");
    PROGRAM instance1 WITH task1 : main;
  END_RESOURCE
END_CONFIGURATION"
            )
        };
    }

    rule_ok!(
        apply_when_interval_positive_then_ok,
        task!("INTERVAL := T#10ms, PRIORITY := 1")
    );

    rule_ok!(
        apply_when_interval_zero_then_ok,
        task!("INTERVAL := T#0ms, PRIORITY := 1")
    );

    rule_err!(
        #[spec_test(REQ_EM_analyzer_060)]
        apply_when_interval_negative_then_task_interval_negative,
        task!("INTERVAL := T#-10ms, PRIORITY := 1"),
        [Problem::TaskIntervalNegative]
    );

    rule_err_at!(
        apply_when_interval_negative_then_error_at_interval,
        task!("INTERVAL := T#-10ms, PRIORITY := 1"),
        Problem::TaskIntervalNegative,
        "T#-10ms"
    );

    rule_ok!(
        apply_when_single_names_bool_global_then_ok,
        task!("SINGLE := flag, PRIORITY := 1")
    );

    rule_ok!(
        apply_when_single_is_constant_then_ok,
        task!("SINGLE := TRUE, PRIORITY := 1")
    );

    rule_err!(
        #[spec_test(REQ_EM_analyzer_061)]
        apply_when_single_names_undeclared_variable_then_variable_undefined,
        task!("SINGLE := missing, PRIORITY := 1"),
        [Problem::VariableUndefined]
    );

    rule_err!(
        apply_when_single_names_program_then_variable_undefined,
        task!("SINGLE := main, PRIORITY := 1"),
        [Problem::VariableUndefined]
    );

    rule_err!(
        #[spec_test(REQ_EM_analyzer_062)]
        apply_when_single_names_int_global_then_task_single_not_bool,
        task!("SINGLE := count, PRIORITY := 1"),
        [Problem::TaskSingleNotBool]
    );
}
