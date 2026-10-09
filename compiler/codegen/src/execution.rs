//! Builds the task table, the program instance and the global variables from
//! the execution model the analyzer resolved.
//!
//! The analyzer decides which configuration is built, which instances run
//! under which tasks, how each task is scheduled and which globals exist
//! (`ironplc_analyzer::execution_model`). This module decides none of it, and
//! never reads the configuration's declarations. It reports why the model is
//! not executable, and checks the model against what the VM and the container
//! can represent: one program instance, no event tasks, a priority that fits
//! `u16` and an interval that fits `u64` microseconds. It reaches a
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
    DebugName, Execution, ExecutionModel, GlobalKind, GlobalScope, NotExecutable, ProgramInstance,
    Schedule, SystemGlobal, Task,
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

/// Builds what the container runs from the execution model, after reporting
/// why the model is not executable and checking it against what the VM
/// supports.
pub(crate) fn runnable<'a>(analysis: &CleanAnalysis<'a>) -> Result<Runnable<'a>, Diagnostic> {
    let model = match analysis.execution() {
        Execution::Executable(model) => model,
        Execution::NotExecutable(reason) => return Err(not_executable(reason)),
    };
    let (instance, task) = single_program_instance(model)?;
    let schedule = task_schedule(task)?;
    let program = analysis
        .program_declaration(instance.program)
        .ok_or_else(|| {
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

/// The diagnostic for a model the analyzer recorded as not executable.
fn not_executable(reason: &NotExecutable) -> Diagnostic {
    match reason {
        NotExecutable::NoProgram => Diagnostic::problem(
            Problem::NoProgramDeclaration,
            Label::file(
                FileId::default(),
                "Source does not contain a PROGRAM declaration",
            ),
        ),
        NotExecutable::SeveralPrograms(programs) => {
            let names: Vec<&DebugName> = programs.iter().collect();
            multiple_programs_not_implemented("PROGRAM declaration", &names)
        }
        NotExecutable::SeveralConfigurations(configurations) => {
            several_configurations(configurations)
        }
        NotExecutable::UndeclaredPrograms(types) => undeclared_programs(types),
    }
}

/// The VM runs one program instance (#1613). Returns it with the task it runs
/// under.
///
/// Every `PROGRAM` declaration counts, bound or not, and so does every
/// instance, across every resource and task, until the VM runs more than
/// one. Lifting the limit is deleting this check.
fn single_program_instance(
    model: &ExecutionModel,
) -> Result<(&ProgramInstance, &Task), Diagnostic> {
    let programs: Vec<&DebugName> = model.programs().map(|(_, program)| &program.name).collect();
    if programs.len() > 1 {
        return Err(multiple_programs_not_implemented(
            "PROGRAM declaration",
            &programs,
        ));
    }

    let mut instances: Vec<(&ProgramInstance, &Task)> = model
        .configuration()
        .resources
        .iter()
        .flat_map(|resource| &resource.tasks)
        .flat_map(|task| task.instances.iter().map(move |instance| (instance, task)))
        .collect();
    match instances.as_slice() {
        [single] => Ok(*single),
        [] => Err(Diagnostic::internal_error_at(Label::file(
            FileId::default(),
            "Executable model has no program instance",
        ))),
        _ => {
            // A task owns its instances, so the model lists them by task;
            // a diagnostic counts them in the order they are written.
            instances.sort_by_key(|(instance, _)| {
                let span = instance_name(model, instance)
                    .map(DebugName::span)
                    .unwrap_or_default();
                (span.file_id.to_string(), span.start)
            });
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

/// The name to label a diagnostic about `instance` at: its own, or for the
/// implicit instance its program's.
fn instance_name<'m>(
    model: &'m ExecutionModel,
    instance: &'m ProgramInstance,
) -> Option<&'m DebugName> {
    instance
        .name
        .as_ref()
        .or_else(|| model.program(instance.program).map(|program| &program.name))
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
fn undeclared_programs(types: &[DebugName]) -> Diagnostic {
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
fn several_configurations(names: &[DebugName]) -> Diagnostic {
    let names: Vec<&DebugName> = names.iter().collect();
    labelled_from_second(
        &names,
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
