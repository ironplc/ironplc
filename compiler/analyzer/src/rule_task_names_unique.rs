//! Semantic rule that task names within a resource are unique.
//!
//! ## Passes
//!
//! RESOURCE resource1 ON PLC
//!   TASK task_a(INTERVAL := T#100ms,PRIORITY := 1);
//!   TASK task_b(INTERVAL := T#200ms,PRIORITY := 2);
//!   PROGRAM instance1 WITH task_a : plc_prg;
//! END_RESOURCE
//!
//! ## Fails
//!
//! RESOURCE resource1 ON PLC
//!   TASK my_task(INTERVAL := T#100ms,PRIORITY := 1);
//!   TASK my_task(INTERVAL := T#200ms,PRIORITY := 2);
//!   PROGRAM instance1 WITH my_task : plc_prg;
//! END_RESOURCE
use ironplc_dsl::{
    common::*,
    configuration::ResourceDeclaration,
    core::Located,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::collections::HashSet;

use crate::{result::SemanticResult, semantic_context::SemanticContext};

pub fn apply(lib: &Library, _context: &SemanticContext) -> SemanticResult {
    let mut visitor = RuleTaskNamesUnique::new();
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleTaskNamesUnique {
    diagnostics: Vec<Diagnostic>,
}

impl RuleTaskNamesUnique {
    fn new() -> Self {
        RuleTaskNamesUnique {
            diagnostics: Vec::new(),
        }
    }
}

impl Visitor<Diagnostic> for RuleTaskNamesUnique {
    type Value = ();

    fn visit_resource_declaration(
        &mut self,
        node: &ResourceDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let mut seen_names = HashSet::new();

        for task_config in &node.tasks {
            if !seen_names.insert(&task_config.name) {
                self.diagnostics.push(
                    Diagnostic::problem(
                        Problem::DuplicateTaskName,
                        Label::span(task_config.name.span(), "Duplicate task name"),
                    )
                    .with_context_id("resource", &node.name)
                    .with_context_id("task", &task_config.name),
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic_context::SemanticContextBuilder;
    use crate::test_helpers::parse_and_resolve_types;

    use super::*;

    #[test]
    fn apply_when_task_names_unique_then_return_ok() {
        let program = "
        CONFIGURATION config
            RESOURCE resource1 ON PLC
               TASK task_a(INTERVAL := T#100ms,PRIORITY := 1);
               TASK task_b(INTERVAL := T#200ms,PRIORITY := 2);
               PROGRAM instance1 WITH task_a : plc_prg;
            END_RESOURCE
        END_CONFIGURATION";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_task_names_duplicated_then_return_error() {
        let program = "
        CONFIGURATION config
            RESOURCE resource1 ON PLC
               TASK my_task(INTERVAL := T#100ms,PRIORITY := 1);
               TASK my_task(INTERVAL := T#200ms,PRIORITY := 2);
               PROGRAM instance1 WITH my_task : plc_prg;
            END_RESOURCE
        END_CONFIGURATION";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_single_task_then_return_ok() {
        let program = "
        CONFIGURATION config
            RESOURCE resource1 ON PLC
               TASK my_task(INTERVAL := T#100ms,PRIORITY := 1);
               PROGRAM instance1 WITH my_task : plc_prg;
            END_RESOURCE
        END_CONFIGURATION";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_no_tasks_then_return_ok() {
        let program = "
        CONFIGURATION config
            RESOURCE resource1 ON PLC
               PROGRAM instance1 : plc_prg;
            END_RESOURCE
        END_CONFIGURATION";

        let library = parse_and_resolve_types(program);
        let context = SemanticContextBuilder::new().build().unwrap();
        let result = apply(&library, &context);
        assert!(result.is_ok());
    }
}
