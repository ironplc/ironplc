//! The execution model: which configuration is built, which program instances
//! run under which tasks, what each task's scheduling means, and which global
//! variables exist.
//!
//! These are language decisions, so the analyzer makes them once and a backend
//! lowers the answer. [`resolve`] builds the model from the type-resolved
//! library, and [`SemanticContext::execution_model`](crate::SemanticContext::execution_model)
//! hands it to codegen through [`CleanAnalysis`](crate::CleanAnalysis).
//!
//! The model is plain data: names, type ids, durations, integers and enums. It
//! holds no configuration, resource, task or program configuration node, so a
//! backend that reads it cannot make the decisions again from the source. A
//! name is an [`Id`], which carries where it was written so that a backend's
//! diagnostic can point at it.
//!
//! The model holds every resource, task and program instance the source
//! declares. A backend that runs fewer, such as the VM's single program
//! instance, says so as a check against the model; the model does not.
//!
//! See `specs/design/execution-model.md` and ADR-0057.

use ironplc_dsl::common::{Library, LibraryElementKind, TypeName, VarDecl};
use ironplc_dsl::configuration::{
    ConfigurationDeclaration, DataSourceKind, ResourceDeclaration, TaskConfiguration,
};
use ironplc_dsl::core::{Id, SourceSpan};
use ironplc_dsl::type_id::TypeId;
use ironplc_parser::options::CompilerOptions;
use time::Duration;

use crate::system_globals::SYSTEM_UPTIME_GLOBALS;
use crate::type_environment::TypeEnvironment;

/// The resolved execution model of a library.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExecutionModel {
    configuration: Option<Id>,
    resources: Vec<Resource>,
    programs: Vec<Id>,
    globals: Vec<GlobalVariable>,
    not_executable: Option<NotExecutable>,
}

impl ExecutionModel {
    /// The name of the configuration the model is built from, or `None` when
    /// the library declares none (or several, see [`Self::not_executable`]).
    pub fn configuration(&self) -> Option<&Id> {
        self.configuration.as_ref()
    }

    /// The resources, in declaration order. When the library binds a program
    /// implicitly, the last resource is the implicit one that holds it.
    pub fn resources(&self) -> &[Resource] {
        &self.resources
    }

    /// The names of the `PROGRAM` declarations in the library, in source order.
    pub fn programs(&self) -> &[Id] {
        &self.programs
    }

    /// The global variables in scope, in variable-table order (see
    /// [`GlobalScope`] for what the order is).
    pub fn globals(&self) -> &[GlobalVariable] {
        &self.globals
    }

    /// Why no executable can be built from the library, or `None` when one
    /// can. This is not a problem with the library: a library of functions,
    /// or of several programs, is a valid library that `check` accepts.
    pub fn not_executable(&self) -> Option<&NotExecutable> {
        self.not_executable.as_ref()
    }

    /// Every program instance with the resource and task it runs under, in
    /// resource order and then declaration order.
    pub fn instances(&self) -> impl Iterator<Item = BoundInstance<'_>> {
        self.resources.iter().flat_map(|resource| {
            resource.instances.iter().filter_map(move |instance| {
                Some(BoundInstance {
                    resource,
                    instance,
                    // Always found: `resolve` is the only place an instance
                    // is bound, and it binds each to a task of its resource.
                    task: resource.task(instance.task)?,
                })
            })
        })
    }
}

/// Why a library cannot be built into an executable.
#[derive(Clone, Debug, PartialEq)]
pub enum NotExecutable {
    /// The library declares no `PROGRAM`, so nothing would run.
    NoProgram,
    /// The library declares more than one `CONFIGURATION`, so which one to
    /// build is ambiguous. The names are in source order.
    SeveralConfigurations(Vec<Id>),
    /// A program instance names a type that is not a `PROGRAM` declaration
    /// of the library, such as a function block or nothing at all. The
    /// type names are in declaration order. This is not a problem with the
    /// library on its own: a configuration may be checked without the file
    /// that declares its programs.
    UndeclaredPrograms(Vec<Id>),
    /// No configuration binds a program, and the library declares more than
    /// one `PROGRAM`, so which one to run is ambiguous. The names are in
    /// source order.
    SeveralPrograms(Vec<Id>),
}

/// A resource: the tasks it schedules and the program instances it runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Resource {
    name: Option<Id>,
    tasks: Vec<Task>,
    instances: Vec<ProgramInstance>,
}

impl Resource {
    /// The resource's name, or `None` for the implicit resource that holds a
    /// program no configuration binds.
    pub fn name(&self) -> Option<&Id> {
        self.name.as_ref()
    }

    /// The tasks, in declaration order. A resource with an instance that has
    /// no `WITH` clause gets one implicit freewheeling task, after its
    /// declared ones.
    pub fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    /// The program instances, in declaration order.
    pub fn instances(&self) -> &[ProgramInstance] {
        &self.instances
    }

    /// The task at `index` in [`Self::tasks`].
    pub fn task(&self, index: TaskIndex) -> Option<&Task> {
        self.tasks.get(index.0)
    }
}

/// The position of a task in its resource's [`Resource::tasks`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskIndex(usize);

/// A task: when it runs the program instances bound to it.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    /// The task's name, or `None` for an implicit freewheeling task.
    pub name: Option<Id>,
    /// The declared `PRIORITY` (0 is the highest). An implicit task has 0.
    pub priority: u32,
    /// The declared `INTERVAL`, when there is one.
    pub interval: Option<TaskInterval>,
    /// The declared `SINGLE` trigger, when there is one.
    pub single: Option<EventTrigger>,
    /// How the task is scheduled, decided from the parameters above.
    pub kind: TaskKind,
}

impl Task {
    /// The freewheeling task a program instance runs under when nothing binds
    /// it to a declared one.
    fn implicit() -> Self {
        Task {
            name: None,
            priority: 0,
            interval: None,
            single: None,
            kind: TaskKind::Freewheeling,
        }
    }
}

/// A task's `INTERVAL`.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskInterval {
    pub duration: Duration,
    /// Where the interval is written, for a diagnostic about its value.
    pub span: SourceSpan,
}

/// What a task's `SINGLE` parameter names.
#[derive(Clone, Debug, PartialEq)]
pub enum EventTrigger {
    /// A global variable, by name.
    Global(Id),
    /// A constant.
    Constant,
}

/// How a task is scheduled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskKind {
    /// Runs every `INTERVAL`. A task with a positive interval and no `SINGLE`.
    Cyclic,
    /// Runs again as soon as it finishes. A task with no `SINGLE` and an
    /// absent or zero interval, and the implicit task.
    Freewheeling,
    /// Runs on the rising edge of its `SINGLE` trigger. Any task with `SINGLE`.
    Event,
}

/// A program instance.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgramInstance {
    /// The instance's name, or `None` for the implicit instance of a program
    /// no configuration binds.
    pub name: Option<Id>,
    /// The name of the `PROGRAM` type it instantiates, as written.
    pub program: Id,
    /// The task it runs under, in its resource.
    pub task: TaskIndex,
}

impl ProgramInstance {
    /// The name to point a diagnostic at: the instance's name, or for an
    /// implicit instance the program's.
    pub fn label(&self) -> &Id {
        self.name.as_ref().unwrap_or(&self.program)
    }
}

/// A program instance with the resource and task it runs under.
#[derive(Clone, Copy, Debug)]
pub struct BoundInstance<'a> {
    pub resource: &'a Resource,
    pub instance: &'a ProgramInstance,
    pub task: &'a Task,
}

/// A global variable in scope.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalVariable {
    pub name: Id,
    /// The type it declares. `None` only when the declaration's type did not
    /// resolve, which analysis reports.
    pub type_id: Option<TypeId>,
    /// Where it was declared.
    pub scope: GlobalScope,
}

/// Where a global variable was declared.
///
/// [`ExecutionModel::globals`] lists them in the order of these variants, and
/// within one variant in declaration order: the system globals in
/// [`SYSTEM_UPTIME_GLOBALS`] order, top-level `VAR_GLOBAL` blocks in the
/// order the files were given, then the configuration's, then each
/// resource's. The VM writes the system globals by slot, so they come first.
#[derive(Clone, Debug, PartialEq)]
pub enum GlobalScope {
    /// Provided by the compiler (`allow_system_uptime_global`).
    System,
    /// A `VAR_GLOBAL` outside any configuration.
    TopLevel,
    /// The configuration's `VAR_GLOBAL`.
    Configuration,
    /// A resource's `VAR_GLOBAL`, with the resource's name.
    Resource(Id),
}

/// Resolves the execution model of a type-resolved library.
///
/// `types` gives the system globals their type ids; every other global takes
/// the id its declaration records.
pub fn resolve(
    library: &Library,
    types: &TypeEnvironment,
    options: &CompilerOptions,
) -> ExecutionModel {
    let mut programs = Vec::new();
    let mut configurations = Vec::new();
    let mut globals = system_globals(types, options);
    for element in &library.elements {
        match element {
            LibraryElementKind::ProgramDeclaration(program) => programs.push(program.name.clone()),
            LibraryElementKind::ConfigurationDeclaration(config) => configurations.push(config),
            LibraryElementKind::GlobalVarDeclarations(decls) => {
                globals.extend(declared_globals(decls, &GlobalScope::TopLevel));
            }
            _ => {}
        }
    }
    sort_by_source_position(&mut programs, |name| name);
    sort_by_source_position(&mut configurations, |config| &config.name);

    let mut model = ExecutionModel {
        programs,
        globals,
        ..ExecutionModel::default()
    };

    match configurations.as_slice() {
        [] => {}
        [config] => resolve_configuration(&mut model, config),
        _ => {
            model.not_executable = Some(NotExecutable::SeveralConfigurations(
                configurations.iter().map(|c| c.name.clone()).collect(),
            ));
        }
    }

    // A program no configuration binds runs on its own, when it is the only
    // one: a library with no configuration, or a configuration that
    // instantiates nothing.
    if model.not_executable.is_none() && model.instances().next().is_none() {
        match model.programs.as_slice() {
            [] => {}
            [program] => {
                let program = program.clone();
                model.resources.push(Resource {
                    name: None,
                    tasks: vec![Task::implicit()],
                    instances: vec![ProgramInstance {
                        name: None,
                        program,
                        task: TaskIndex(0),
                    }],
                });
            }
            several => {
                model.not_executable = Some(NotExecutable::SeveralPrograms(several.to_vec()));
            }
        }
    }

    // An instance of anything but a declared PROGRAM has no body to run.
    if model.not_executable.is_none() {
        let undeclared: Vec<Id> = model
            .instances()
            .map(|bound| &bound.instance.program)
            .filter(|program| !model.programs.contains(program))
            .cloned()
            .collect();
        if !undeclared.is_empty() {
            model.not_executable = Some(NotExecutable::UndeclaredPrograms(undeclared));
        }
    }

    // With no program there is nothing to run, whatever else is ambiguous.
    if model.programs.is_empty() {
        model.not_executable = Some(NotExecutable::NoProgram);
    }

    model
}

/// The globals the compiler provides, when the options ask for them.
fn system_globals(types: &TypeEnvironment, options: &CompilerOptions) -> Vec<GlobalVariable> {
    if !options.allow_system_uptime_global {
        return Vec::new();
    }
    SYSTEM_UPTIME_GLOBALS
        .iter()
        .map(|global| GlobalVariable {
            name: Id::from(global.name),
            type_id: types.id_of(&TypeName::from(global.type_name)),
            scope: GlobalScope::System,
        })
        .collect()
}

/// The globals `decls` declares, in declaration order.
fn declared_globals<'a>(
    decls: &'a [VarDecl],
    scope: &'a GlobalScope,
) -> impl Iterator<Item = GlobalVariable> + 'a {
    decls.iter().filter_map(move |decl| {
        Some(GlobalVariable {
            name: decl.identifier.symbolic_id()?.clone(),
            type_id: decl.type_id,
            scope: scope.clone(),
        })
    })
}

/// Records the one configuration the model is built from: its globals, then
/// its resources.
fn resolve_configuration(model: &mut ExecutionModel, config: &ConfigurationDeclaration) {
    model.configuration = Some(config.name.clone());
    model.globals.extend(declared_globals(
        &config.global_var,
        &GlobalScope::Configuration,
    ));
    for resource in &config.resource_decl {
        model.globals.extend(declared_globals(
            &resource.global_vars,
            &GlobalScope::Resource(resource.name.clone()),
        ));
    }
    model.resources = config.resource_decl.iter().map(resolve_resource).collect();
}

/// Resolves a resource's tasks and binds each program instance to one.
fn resolve_resource(resource: &ResourceDeclaration) -> Resource {
    let mut tasks: Vec<Task> = resource.tasks.iter().map(resolve_task).collect();
    let mut implicit_task = None;
    let instances = resource
        .programs
        .iter()
        .map(|program| {
            // A `WITH` naming a task the resource does not declare is
            // reported by `rule_program_task_definition_exists`; binding the
            // instance to the implicit task keeps every instance bound.
            let declared = program.task_name.as_ref().and_then(|task_name| {
                resource
                    .tasks
                    .iter()
                    .position(|task| &task.name == task_name)
            });
            let task = declared.unwrap_or_else(|| {
                *implicit_task.get_or_insert_with(|| {
                    tasks.push(Task::implicit());
                    tasks.len() - 1
                })
            });
            ProgramInstance {
                name: Some(program.name.clone()),
                program: program.type_name.clone(),
                task: TaskIndex(task),
            }
        })
        .collect();
    Resource {
        name: Some(resource.name.clone()),
        tasks,
        instances,
    }
}

/// Resolves a task's parameters and decides how it is scheduled.
fn resolve_task(task: &TaskConfiguration) -> Task {
    let interval = task.interval.as_ref().map(|interval| TaskInterval {
        duration: interval.interval,
        span: interval.span.clone(),
    });
    let single = task.single.as_ref().map(|single| match single {
        DataSourceKind::GlobalVarReference(reference) => {
            EventTrigger::Global(reference.global_var_name.clone())
        }
        DataSourceKind::Constant(_) => EventTrigger::Constant,
    });
    // A zero interval means "as fast as possible", which is what a
    // freewheeling task does. Scheduling it as cyclic with a zero period
    // would leave it permanently overdue. A negative interval is reported by
    // `rule_task_configuration`.
    let kind = if single.is_some() {
        TaskKind::Event
    } else if interval
        .as_ref()
        .is_some_and(|interval| interval.duration.is_positive())
    {
        TaskKind::Cyclic
    } else {
        TaskKind::Freewheeling
    };
    Task {
        name: Some(task.name.clone()),
        priority: task.priority,
        interval,
        single,
        kind,
    }
}

/// Orders `items` by where their identifier appears in the source.
///
/// The toposort reorders library elements, and for declarations with no
/// dependency edges between them the order comes out reversed. Sorting by file
/// and offset makes "the second configuration" the second one in the file.
fn sort_by_source_position<T>(items: &mut [T], id: impl Fn(&T) -> &Id) {
    items.sort_by_key(|item| {
        let span = &id(item).span;
        (span.file_id.to_string(), span.start)
    });
}

#[cfg(test)]
mod tests;
