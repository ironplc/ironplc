//! Builds the task table, the program instance and the global variables from
//! the execution model the analyzer resolved.
//!
//! The analyzer decides which configurations the library declares, which
//! instances run under which tasks, how each task is scheduled and which
//! globals exist (`ironplc_analyzer::execution_model`). This module decides
//! none of it, and never reads the configuration's declarations. It checks
//! the model against what the VM and the container can represent: a program
//! to run, one configuration, a body for every instance, one program instance,
//! no event tasks, a priority that fits `u16` and an interval that fits `u64`
//! microseconds. It reaches a
//! declaration only through [`CleanAnalysis::program_declaration`] and
//! [`CleanAnalysis::global_declaration`].
//!
//! See `specs/design/execution-model.md` and ADR-0065.

use ironplc_analyzer::CleanAnalysis;
use ironplc_container::{Container, TaskType};
use ironplc_dsl::common::{ProgramDeclaration, VarDecl, VariableType};
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::execution::{
    Configuration, DebugName, ExecutionModel, GlobalKind, GlobalScope, InstanceOf, ProgramId,
    ProgramInstance, Schedule, SystemGlobal, Task,
};
use ironplc_problems::Problem;

/// What the container runs, built from the execution model.
pub(crate) struct Runnable<'a> {
    /// The program whose body the one program instance runs.
    pub(crate) program: &'a ProgramDeclaration,
    /// The task table entry's scheduling fields.
    pub(crate) schedule: TaskSchedule,
    /// The global variables, in variable-table order.
    pub(crate) globals: Vec<VarDecl>,
    /// Whether the model lists the uptime global, which the VM writes each
    /// scan.
    pub(crate) has_system_uptime: bool,
}

/// Builds what the container runs from the execution model, after checking
/// the model against what the VM supports.
pub(crate) fn runnable<'a>(analysis: &CleanAnalysis<'a>) -> Result<Runnable<'a>, Diagnostic> {
    let model = analysis.execution_model();
    let configuration = single_configuration(model)?;
    check_instances_resolved(configuration)?;
    let (program_id, instance, task) = single_program_instance(model, configuration)?;
    let schedule = task_schedule(task)?;
    let program = analysis.program_declaration(program_id).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            instance_name(model, instance)
                .map(DebugName::span)
                .unwrap_or_default(),
            "Program instance has no PROGRAM declaration",
        ))
    })?;
    Ok(Runnable {
        program,
        schedule,
        globals: global_declarations(analysis, model)?,
        has_system_uptime: model
            .globals()
            .any(|(_, global)| matches!(global.kind, GlobalKind::System(SystemGlobal::UpTime))),
    })
}

/// The one configuration the container holds.
///
/// A library with no `PROGRAM` has nothing to run, whatever else it
/// declares. The container holds one configuration, and the compiler does not
/// choose one of several.
fn single_configuration(model: &ExecutionModel) -> Result<&Configuration, Diagnostic> {
    if model.programs().next().is_none() {
        return Err(Diagnostic::problem(
            Problem::NoProgramDeclaration,
            Label::file(
                FileId::default(),
                "Source does not contain a PROGRAM declaration",
            ),
        ));
    }
    match model.configurations() {
        [configuration] => Ok(configuration),
        [] => Err(Diagnostic::internal_error_at(Label::file(
            FileId::default(),
            "Library with a PROGRAM has no configuration",
        ))),
        several => {
            let names: Vec<&DebugName> = several
                .iter()
                .filter_map(|configuration| configuration.name.as_ref())
                .collect();
            Err(several_configurations(&names))
        }
    }
}

/// The instances of `configuration`, each with the task that runs it, in the
/// order they are written: a task owns its instances, so the model lists them
/// by task.
fn instances_in_source_order<'m>(
    model: &'m ExecutionModel,
    configuration: &'m Configuration,
) -> Vec<(&'m ProgramInstance, &'m Task)> {
    let mut instances: Vec<(&ProgramInstance, &Task)> = configuration
        .resources
        .iter()
        .flat_map(|resource| &resource.tasks)
        .flat_map(|task| task.instances.iter().map(move |instance| (instance, task)))
        .collect();
    instances.sort_by_key(|(instance, _)| {
        let span = instance_name(model, instance)
            .map(DebugName::span)
            .unwrap_or_default();
        (span.file_id.to_string(), span.start)
    });
    instances
}

/// Every instance needs a `PROGRAM` body to run. One whose type the library
/// does not declare as a `PROGRAM` has none.
fn check_instances_resolved(configuration: &Configuration) -> Result<(), Diagnostic> {
    let mut unresolved: Vec<&DebugName> = configuration
        .resources
        .iter()
        .flat_map(|resource| &resource.tasks)
        .flat_map(|task| &task.instances)
        .filter_map(|instance| match &instance.program {
            InstanceOf::Unresolved(type_name) => Some(type_name),
            InstanceOf::Program(_) => None,
        })
        .collect();
    if unresolved.is_empty() {
        return Ok(());
    }
    unresolved.sort_by_key(|name| {
        let span = name.span();
        (span.file_id.to_string(), span.start)
    });
    Err(undeclared_programs(&unresolved))
}

/// The VM runs one program instance (#1613). Returns it with its program and
/// the task it runs under.
///
/// Every `PROGRAM` declaration counts, bound or not, and so does every
/// instance, across every resource and task, until the VM runs more than
/// one. Lifting the limit is deleting this check.
fn single_program_instance<'m>(
    model: &'m ExecutionModel,
    configuration: &'m Configuration,
) -> Result<(ProgramId, &'m ProgramInstance, &'m Task), Diagnostic> {
    let programs: Vec<&DebugName> = model.programs().map(|(_, program)| &program.name).collect();
    if programs.len() > 1 {
        return Err(multiple_programs_not_implemented(
            "PROGRAM declaration",
            &programs,
        ));
    }

    let instances = instances_in_source_order(model, configuration);
    match instances.as_slice() {
        [(instance, task)] => match instance.program {
            InstanceOf::Program(program) => Ok((program, instance, task)),
            InstanceOf::Unresolved(_) => Err(Diagnostic::internal_error_at(Label::span(
                instance_name(model, instance)
                    .map(DebugName::span)
                    .unwrap_or_default(),
                "Unresolved program instance passed the check for one",
            ))),
        },
        [] => Err(Diagnostic::internal_error_at(Label::file(
            FileId::default(),
            "Configuration of a library with a PROGRAM has no program instance",
        ))),
        _ => {
            let names: Vec<&DebugName> = instances
                .iter()
                .filter_map(|(instance, _)| instance_name(model, instance))
                .collect();
            Err(multiple_programs_not_implemented(
                "program instance",
                &names,
            ))
        }
    }
}

/// The name to label a diagnostic about `instance` at: its own, or for an
/// implicit instance its program's.
fn instance_name<'m>(
    model: &'m ExecutionModel,
    instance: &'m ProgramInstance,
) -> Option<&'m DebugName> {
    instance.name.as_ref().or(match &instance.program {
        InstanceOf::Program(id) => model.program(*id).map(|program| &program.name),
        InstanceOf::Unresolved(type_name) => Some(type_name),
    })
}

/// The scheduling fields of a task table entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TaskSchedule {
    pub(crate) priority: u16,
    pub(crate) task_type: TaskType,
    pub(crate) interval_us: u64,
}

/// Converts the model's task into a task table entry's scheduling fields.
///
/// Only the task the program instance runs under is converted: a task no
/// instance runs under is never emitted, so its parameters do not matter to
/// the container. A problem with the task is labelled at its name.
pub(crate) fn task_schedule(task: &Task) -> Result<TaskSchedule, Diagnostic> {
    let label = |message: String| {
        Label::span(
            task.name.as_ref().map(DebugName::span).unwrap_or_default(),
            message,
        )
    };

    // The VM stubs event-triggered tasks out of `collect_ready_tasks`, so
    // emitting one would produce a program whose task body never runs.
    if let Schedule::Event { .. } = task.schedule {
        return Err(Diagnostic::problem(
            Problem::TaskSingleNotSupported,
            label("Task declares SINGLE".into()),
        ));
    }

    let priority = u16::try_from(task.priority).map_err(|_| {
        Diagnostic::problem(
            Problem::TaskParameterOutOfRange,
            label(format!(
                "Task declares PRIORITY := {}, which exceeds the maximum of {}",
                task.priority,
                u16::MAX
            )),
        )
    })?;

    let interval_us = match &task.schedule {
        Schedule::Cyclic { interval } => {
            let micros = interval.whole_microseconds();
            u64::try_from(micros).map_err(|_| {
                Diagnostic::problem(
                    Problem::TaskParameterOutOfRange,
                    label(format!(
                        "Task declares an INTERVAL of {micros} microseconds"
                    )),
                )
            })?
        }
        Schedule::Freewheeling | Schedule::Event { .. } => 0,
    };

    // The container counts the interval in whole microseconds. A cyclic
    // interval shorter than one microsecond counts as 0, and a cyclic task
    // with a period of 0 would be permanently overdue, so it runs as fast as
    // possible instead, which is what a period that short asks for.
    let task_type = if interval_us > 0 {
        TaskType::Cyclic
    } else {
        TaskType::Freewheeling
    };

    Ok(TaskSchedule {
        priority,
        task_type,
        interval_us,
    })
}

/// Writes `schedule` into the task table entry.
///
/// `ContainerBuilder` synthesizes one task and one program instance, which is
/// what the VM runs today; only the task's scheduling fields come from the
/// model.
pub(crate) fn apply_task_schedule(container: &mut Container, schedule: TaskSchedule) {
    if let Some(entry) = container.task_table.tasks.first_mut() {
        entry.priority = schedule.priority;
        entry.interval_us = schedule.interval_us;
        entry.task_type = schedule.task_type;
    }
}

/// The declarations of the globals the variable table holds, in the model's
/// order.
///
/// A declared global's type and initial value are still compiled from its
/// declaration, which is reached by id. A resource's `VAR_GLOBAL` gets no
/// storage yet, so that a program that compiles today keeps its variable
/// indexes; a program that uses one is reported when the use is compiled.
fn global_declarations(
    analysis: &CleanAnalysis<'_>,
    model: &ExecutionModel,
) -> Result<Vec<VarDecl>, Diagnostic> {
    let mut declarations = Vec::new();
    for (id, global) in model.globals() {
        match global.kind {
            GlobalKind::System(system) => declarations.push(system_global(system)),
            GlobalKind::Declared(GlobalScope::TopLevel | GlobalScope::Configuration) => {
                let decl = analysis.global_declaration(id).ok_or_else(|| {
                    Diagnostic::internal_error_at(Label::span(
                        global.name.span(),
                        "Global variable in the execution model has no declaration",
                    ))
                })?;
                declarations.push(decl.clone());
            }
            GlobalKind::Declared(GlobalScope::Resource) => {}
        }
    }
    Ok(declarations)
}

/// The declaration of a compiler-provided global, which no source declares.
///
/// The declaration names its type rather than recording a type id: recording
/// the id would let a comparison with the global take the fused
/// compare-to-constant form, and change the bytes a program compiles to.
fn system_global(global: SystemGlobal) -> VarDecl {
    let (name, type_name) = match global {
        SystemGlobal::UpTime => ("__SYSTEM_UP_TIME", "TIME"),
        SystemGlobal::UpLTime => ("__SYSTEM_UP_LTIME", "LTIME"),
    };
    VarDecl::simple(name, type_name).with_type(VariableType::Global)
}

/// Builds the P4080 diagnostic for instances of types that are not a
/// `PROGRAM` declaration, labelled at each type name.
fn undeclared_programs(types: &[&DebugName]) -> Diagnostic {
    let mut labels = types
        .iter()
        .map(|name| Label::span(name.span(), format!("{name} is not a PROGRAM declaration")));
    let Some(primary) = labels.next() else {
        return Diagnostic::internal_error_at(Label::file(
            FileId::default(),
            "No program instance type reported as undeclared",
        ));
    };
    labels.fold(
        Diagnostic::problem(Problem::ProgramInstanceTypeNotProgram, primary),
        Diagnostic::with_secondary,
    )
}

/// Builds the P4081 diagnostic for a library with several configurations.
///
/// `names` are in source order. The primary label sits on the second name;
/// the others get secondary labels.
fn several_configurations(names: &[&DebugName]) -> Diagnostic {
    labelled_from_second(
        names,
        |label| Diagnostic::problem(Problem::ConfigurationAmbiguous, label),
        "CONFIGURATION; the compiler builds one CONFIGURATION at a time",
        "CONFIGURATION",
    )
    .with_help("Compile the files of one CONFIGURATION at a time.")
}

/// Builds the P9999 diagnostic for a second (or later) program.
///
/// `names` are in source order. The primary label sits on the second name,
/// the first one the compiler cannot honour. The first name and any later
/// ones get secondary labels so the diagnostic points at every program it is
/// about, not only the one that tipped the count.
fn multiple_programs_not_implemented(what: &str, names: &[&DebugName]) -> Diagnostic {
    labelled_from_second(
        names,
        Diagnostic::not_implemented,
        &format!("{what}; the compiler currently runs only one PROGRAM"),
        what,
    )
    .with_help(
        "Compile a single PROGRAM for now. Support for more than one PROGRAM is tracked in \
         https://github.com/ironplc/ironplc/issues/1613",
    )
}

/// A diagnostic `make` builds labelled at the second of `names` with
/// `primary`, and at each other name with `secondary`, each prefixed with its
/// ordinal.
fn labelled_from_second(
    names: &[&DebugName],
    make: impl FnOnce(Label) -> Diagnostic,
    primary: &str,
    secondary: &str,
) -> Diagnostic {
    let Some(second) = names.get(1) else {
        // Callers only get here with two or more names, so the first name,
        // when there is one, is the best location for the violation.
        return Diagnostic::internal_error_at(Label::span(
            names.first().map(|name| name.span()).unwrap_or_default(),
            format!("Fewer than two of {secondary} reported as too many"),
        ));
    };
    let mut diagnostic = make(Label::span(
        second.span(),
        format!("{} {primary}", ordinal(2)),
    ));
    for (index, name) in names.iter().enumerate() {
        if index == 1 {
            continue;
        }
        diagnostic = diagnostic.with_secondary(Label::span(
            name.span(),
            format!("{} {secondary}", ordinal(index + 1)),
        ));
    }
    diagnostic
}

/// Formats a 1-based position as an English ordinal: `1st`, `2nd`, `3rd`, `11th`.
fn ordinal(position: usize) -> String {
    let suffix = if (11..=13).contains(&(position % 100)) {
        "th"
    } else {
        match position % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    format!("{position}{suffix}")
}

#[cfg(test)]
mod tests;
