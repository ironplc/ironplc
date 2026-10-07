//! Lowers the analyzer's execution model: the program instance that runs, the
//! task table entry it runs under, and the global variables.
//!
//! The analyzer decides which configuration is built, which instances run
//! under which tasks, what each task's scheduling means and which globals
//! exist (`ironplc_analyzer::execution_model`). This module makes no language
//! decision of its own. It reports why the model is not executable, and checks
//! the model against what the VM and the container can represent: one program
//! instance, no event tasks, a priority that fits `u16` and an interval that
//! fits `u64` microseconds. See `specs/design/execution-model.md` and
//! ADR-0057.

use std::collections::HashMap;

use ironplc_analyzer::execution_model::{
    ExecutionModel, GlobalScope, GlobalVariable, NotExecutable, Task, TaskKind,
};
use ironplc_analyzer::TypeEnvironment;
use ironplc_container::{Container, TaskType};
use ironplc_dsl::common::{Library, LibraryElementKind, ProgramDeclaration, VarDecl, VariableType};
use ironplc_dsl::core::{FileId, Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

/// The program the container runs and the task it runs under.
pub(crate) struct Runnable<'a> {
    pub(crate) program: &'a ProgramDeclaration,
    pub(crate) schedule: TaskSchedule,
}

/// Selects the one program instance the VM runs, after checking the model
/// against what the VM and the container support.
///
/// Reads the program's declaration from the library to compile its body;
/// that is the POU body, not the execution model.
pub(crate) fn runnable<'a>(
    model: &ExecutionModel,
    library: &'a Library,
) -> Result<Runnable<'a>, Diagnostic> {
    if let Some(reason) = model.not_executable() {
        return Err(not_executable(reason));
    }
    let (program_name, task) = single_program_instance(model)?;
    let schedule = task_schedule(task)?;
    let program = library
        .elements
        .iter()
        .find_map(|element| match element {
            LibraryElementKind::ProgramDeclaration(program) if &program.name == program_name => {
                Some(program)
            }
            _ => None,
        })
        .ok_or_else(|| {
            // The model records an instance of anything but a declared
            // PROGRAM as not executable (P4075), so one is always found.
            Diagnostic::internal_error_at(Label::span(
                program_name.span(),
                "Program instance names no PROGRAM declaration",
            ))
        })?;
    Ok(Runnable { program, schedule })
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
            multiple_programs_not_implemented("PROGRAM declaration", programs)
        }
        NotExecutable::SeveralConfigurations(configurations) => {
            several_configurations(configurations)
        }
        NotExecutable::UndeclaredPrograms(types) => undeclared_programs(types),
    }
}

/// The VM runs one program instance (#1613). Returns its program and task.
///
/// Every instance of the model counts, across every resource, and so does
/// every `PROGRAM` declaration, bound or not, until the VM runs more than
/// one. Lifting the limit is deleting this check.
fn single_program_instance(model: &ExecutionModel) -> Result<(&Id, &Task), Diagnostic> {
    if model.programs().len() > 1 {
        return Err(multiple_programs_not_implemented(
            "PROGRAM declaration",
            model.programs(),
        ));
    }
    let instances: Vec<_> = model.instances().collect();
    match instances.as_slice() {
        [bound] => Ok((&bound.instance.program, bound.task)),
        _ => {
            let names: Vec<Id> = instances
                .iter()
                .map(|bound| bound.instance.label().clone())
                .collect();
            Err(multiple_programs_not_implemented(
                "program instance",
                &names,
            ))
        }
    }
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
/// the container.
pub(crate) fn task_schedule(task: &Task) -> Result<TaskSchedule, Diagnostic> {
    let label = |message: String| {
        Label::span(
            task.name
                .as_ref()
                .map(|name| name.span())
                .unwrap_or_default(),
            message,
        )
    };

    // The VM stubs event-triggered tasks out of `collect_ready_tasks`, so
    // emitting one would produce a program whose task body never runs.
    let task_type = match task.kind {
        TaskKind::Cyclic => TaskType::Cyclic,
        TaskKind::Freewheeling => TaskType::Freewheeling,
        TaskKind::Event => {
            return Err(Diagnostic::problem(
                Problem::TaskSingleNotSupported,
                label("Task declares SINGLE".into()),
            ))
        }
    };

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

    // A freewheeling task runs again as soon as it finishes, so it has no
    // interval, whatever (zero) interval it declares.
    let interval_us = match (&task.interval, task_type) {
        (Some(interval), TaskType::Cyclic) => {
            let micros = interval.duration.whole_microseconds();
            u64::try_from(micros).map_err(|_| {
                Diagnostic::problem(
                    Problem::TaskParameterOutOfRange,
                    Label::span(
                        interval.span.clone(),
                        format!("Task declares an INTERVAL of {micros} microseconds"),
                    ),
                )
            })?
        }
        _ => 0,
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

/// Whether the model lists a system global, which the VM writes each scan.
pub(crate) fn has_system_globals(model: &ExecutionModel) -> bool {
    model
        .globals()
        .iter()
        .any(|global| global.scope == GlobalScope::System)
}

/// The declarations of the globals the variable table holds, in the model's
/// order.
///
/// The model gives each global's name, type and scope, but not yet its
/// initial value. Until it does, a declared global's declaration, which holds
/// its initializer, is looked up by name and scope in the analyzed library.
/// This is the one place codegen reads a declaration to build the execution
/// model.
///
/// A resource's `VAR_GLOBAL` gets no storage yet, as before the model; a
/// program that uses one is reported when the use is compiled.
pub(crate) fn global_declarations(
    model: &ExecutionModel,
    library: &Library,
    types: &TypeEnvironment,
) -> Result<Vec<VarDecl>, Diagnostic> {
    let mut top_level: HashMap<&Id, &VarDecl> = HashMap::new();
    let mut configuration: HashMap<&Id, &VarDecl> = HashMap::new();
    for element in &library.elements {
        match element {
            LibraryElementKind::GlobalVarDeclarations(decls) => by_name(&mut top_level, decls),
            LibraryElementKind::ConfigurationDeclaration(config)
                if Some(&config.name) == model.configuration() =>
            {
                by_name(&mut configuration, &config.global_var)
            }
            _ => {}
        }
    }

    let declared = |table: &HashMap<&Id, &VarDecl>, global: &GlobalVariable| {
        table
            .get(&global.name)
            .map(|decl| (*decl).clone())
            .ok_or_else(|| {
                Diagnostic::internal_error_at(Label::span(
                    global.name.span(),
                    "Global variable in the execution model has no declaration",
                ))
            })
    };

    let mut declarations = Vec::new();
    for global in model.globals() {
        match &global.scope {
            GlobalScope::System => declarations.push(system_global(global, types)?),
            GlobalScope::TopLevel => declarations.push(declared(&top_level, global)?),
            GlobalScope::Configuration => declarations.push(declared(&configuration, global)?),
            GlobalScope::Resource(_) => {}
        }
    }
    Ok(declarations)
}

fn by_name<'a>(table: &mut HashMap<&'a Id, &'a VarDecl>, decls: &'a [VarDecl]) {
    for decl in decls {
        if let Some(name) = decl.identifier.symbolic_id() {
            table.insert(name, decl);
        }
    }
}

/// The declaration of a compiler-provided global, which no source declares.
///
/// The declaration names its type rather than recording the type id, as the
/// declaration codegen synthesized before the model did: recording the id
/// would let a comparison with the global take the fused compare-to-constant
/// form and change the bytes a program compiles to.
fn system_global(global: &GlobalVariable, types: &TypeEnvironment) -> Result<VarDecl, Diagnostic> {
    let type_name = global
        .type_id
        .and_then(|id| types.name_of(id))
        .ok_or_else(|| {
            Diagnostic::internal_error_at(Label::span(
                global.name.span(),
                "System global has no named type",
            ))
        })?;
    Ok(
        VarDecl::simple(&global.name.to_string(), &type_name.to_string())
            .with_type(VariableType::Global),
    )
}

/// Builds the P4075 diagnostic for instances of types that are not a
/// `PROGRAM` declaration, labelled at each type name.
fn undeclared_programs(types: &[Id]) -> Diagnostic {
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

/// Builds the P4076 diagnostic for a library with several configurations.
///
/// `names` are in source order. The primary label sits on the second name;
/// the others get secondary labels.
fn several_configurations(names: &[Id]) -> Diagnostic {
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
fn multiple_programs_not_implemented(what: &str, names: &[Id]) -> Diagnostic {
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
    names: &[Id],
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
