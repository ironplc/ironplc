//! Semantic rule that a task name referenced from a program configuration exists.
//!
//! ## Passes
//!
//! RESOURCE resource1 ON PLC
//!   TASK plc_task(INTERVAL := T#100ms,PRIORITY := 1);
//!   PROGRAM plc_task_instance WITH plc_task : plc_prg;
//! END_RESOURCE
//!
//! ## Fails
//!
//! RESOURCE resource1 ON PLC
//!   PROGRAM plc_task_instance WITH plc_task : plc_prg;
//! END_RESOURCE
use ironplc_dsl::{
    common::*,
    configuration::ResourceDeclaration,
    core::SourcePosition,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::collections::HashSet;

pub fn apply(lib: &Library) -> Result<(), Diagnostic> {
    let mut visitor = RuleProgramTaskDefinitionExists::new();
    visitor.walk(lib)
}

struct RuleProgramTaskDefinitionExists {}
impl RuleProgramTaskDefinitionExists {
    fn new() -> Self {
        RuleProgramTaskDefinitionExists {}
    }
}

impl Visitor<Diagnostic> for RuleProgramTaskDefinitionExists {
    type Value = ();

    fn visit_resource_declaration(
        &mut self,
        node: &ResourceDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let mut task_names = HashSet::new();

        // Collect all task names for easy lookup
        for task_config in &node.tasks {
            task_names.insert(&task_config.name);
        }

        // Check for any task name that is not defined
        for program in &node.programs {
            if let Some(task_name) = &program.task_name {
                if !task_names.contains(&task_name) {
                    return Err(Diagnostic::problem(
                        Problem::ProgramMissingTaskConfig,
                        Label::source_loc(task_name.position(), "Reference to task configuration"),
                    )
                    .with_context_id("program", &program.name)
                    .with_context_id("task name", task_name));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::core::FileId;

    use super::*;

    use crate::stages::parse;

    #[test]
    fn apply_when_task_not_defined_then_return_error() {
        let program = "
        CONFIGURATION config
            RESOURCE resource1 ON PLC
               PROGRAM plc_task_instance WITH plc_task : plc_prg;
            END_RESOURCE
        END_CONFIGURATION";

        let library = parse(program, &FileId::default()).unwrap();
        let result = apply(&library);
        assert!(result.is_err());
    }

    #[test]
    fn apply_when_task_defined_then_return_ok() {
        let program = "
        CONFIGURATION config
            RESOURCE resource1 ON PLC
               TASK plc_task(INTERVAL := T#100ms,PRIORITY := 1);
               PROGRAM plc_task_instance WITH plc_task : plc_prg;
            END_RESOURCE
        END_CONFIGURATION";

        let library = parse(program, &FileId::default()).unwrap();
        let result = apply(&library);
        assert!(result.is_ok());
    }
}
