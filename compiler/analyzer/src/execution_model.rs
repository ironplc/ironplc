//! Resolves the execution model: which configuration is built, which program
//! instances run under which tasks, how each task is scheduled, and which
//! globals are in scope.
//!
//! These are language decisions, so they are made once, here, and a backend
//! lowers the answer. [`resolve`] builds an [`Execution`] of `ironplc-ir`'s
//! types from an analyzed library, and
//! [`SemanticContext::execution`](crate::SemanticContext::execution) hands it
//! to codegen through [`CleanAnalysis`](crate::CleanAnalysis).
//!
//! `resolve` reads only what a clean analysis holds (the library and the
//! compiler options), never the analyzer's internals, so that the module can
//! move unchanged into a lowering crate when one exists. It reports nothing:
//! a library that cannot be built into an executable is a valid library, and
//! [`NotExecutable`] says why as data.
//!
//! See `specs/design/execution-model.md` and ADR-0065.

use std::collections::HashMap;

use ironplc_dsl::common::{Library, LibraryElementKind, ProgramDeclaration, VarDecl};
use ironplc_dsl::configuration::{
    ConfigurationDeclaration, DataSourceKind, ResourceDeclaration, TaskConfiguration,
};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_ir::execution::{
    Configuration, DebugName, Execution, ExecutionModelBuilder, Global, GlobalId, GlobalKind,
    GlobalScope, NotExecutable, ProgramId, ProgramInstance, ProgramType, Resource, Schedule,
    SystemGlobal, Task, Trigger,
};
use ironplc_parser::options::CompilerOptions;

use crate::system_globals::SYSTEM_UPTIME_GLOBALS;

/// Resolves the execution model of an analyzed library.
pub fn resolve(library: &Library, options: &CompilerOptions) -> Execution {
    resolve_with_declarations(library, options).0
}

/// Resolves the execution model, and records where the declaration behind
/// each id the model allocated is in `library`.
///
/// Both come from one walk of the library, so the declarations are those of
/// the ids the model holds. See [`Declarations`].
pub(crate) fn resolve_with_declarations(
    library: &Library,
    options: &CompilerOptions,
) -> (Execution, Declarations) {
    let mut walk = Walk::new(library);
    let execution = walk.resolve(options);
    (execution, walk.declarations)
}

/// Where, in the library the model was resolved from, the declaration behind
/// each id is.
///
/// Codegen still compiles a program's body, and a declared global's type and
/// initial value, from the library. It reaches them by id, through
/// [`CleanAnalysis`](crate::CleanAnalysis), from this record of the walk that
/// allocated the ids. It goes away when the lowered program carries bodies,
/// types and initial values.
#[derive(Clone, Debug, Default)]
pub(crate) struct Declarations {
    /// The position in `library.elements` of each program's declaration.
    programs: HashMap<ProgramId, usize>,
    /// Where each declared global's declaration is. A system global has none.
    globals: HashMap<GlobalId, GlobalDeclaration>,
}

/// Where a declared global's `VarDecl` is in the library.
#[derive(Clone, Copy, Debug)]
enum GlobalDeclaration {
    /// `library.elements[element]` is a top-level `VAR_GLOBAL` block.
    TopLevel { element: usize, index: usize },
    /// `library.elements[element]` is the configuration.
    Configuration { element: usize, index: usize },
    /// `library.elements[element]` is the configuration, and `resource` its
    /// resource.
    Resource {
        element: usize,
        resource: usize,
        index: usize,
    },
}

impl Declarations {
    /// The declaration of the program `id` names, at `span`.
    ///
    /// `None` when `library` is not the one the ids were allocated from: the
    /// element is not a `PROGRAM`, or not the one whose name is at `span`.
    pub(crate) fn program<'a>(
        &self,
        library: &'a Library,
        id: ProgramId,
        span: &SourceSpan,
    ) -> Option<&'a ProgramDeclaration> {
        match library.elements.get(*self.programs.get(&id)?)? {
            LibraryElementKind::ProgramDeclaration(program)
                if same_position(&program.name.span, span) =>
            {
                Some(program)
            }
            _ => None,
        }
    }

    /// The declaration of the declared global `id` names, at `span`.
    ///
    /// `None` for a system global, which has no declaration, and when
    /// `library` is not the one the ids were allocated from.
    pub(crate) fn global<'a>(
        &self,
        library: &'a Library,
        id: GlobalId,
        span: &SourceSpan,
    ) -> Option<&'a VarDecl> {
        let decl = match *self.globals.get(&id)? {
            GlobalDeclaration::TopLevel { element, index } => {
                match library.elements.get(element)? {
                    LibraryElementKind::GlobalVarDeclarations(decls) => decls.get(index)?,
                    _ => return None,
                }
            }
            GlobalDeclaration::Configuration { element, index } => {
                configuration_at(library, element)?.global_var.get(index)?
            }
            GlobalDeclaration::Resource {
                element,
                resource,
                index,
            } => configuration_at(library, element)?
                .resource_decl
                .get(resource)?
                .global_vars
                .get(index)?,
        };
        same_position(&decl.identifier.span(), span).then_some(decl)
    }
}

/// Whether `a` and `b` are the same place in the same file. `SourceSpan`'s
/// own equality holds for any two spans.
fn same_position(a: &SourceSpan, b: &SourceSpan) -> bool {
    a.file_id == b.file_id && a.start == b.start && a.end == b.end
}

fn configuration_at(library: &Library, element: usize) -> Option<&ConfigurationDeclaration> {
    match library.elements.get(element)? {
        LibraryElementKind::ConfigurationDeclaration(config) => Some(config),
        _ => None,
    }
}

/// The one walk of the library that builds the model and records where each
/// id's declaration is.
struct Walk<'a> {
    library: &'a Library,
    builder: ExecutionModelBuilder,
    declarations: Declarations,
    /// The declared programs and their ids, in source order.
    programs: Vec<(&'a ProgramDeclaration, ProgramId)>,
    /// The globals a `SINGLE` can name outside its resource, nearest scope
    /// first: the configuration's, the top-level ones, the system ones.
    outer_scopes: Vec<Scope>,
    /// The type names of program instances that name no declared `PROGRAM`,
    /// in declaration order.
    undeclared: Vec<DebugName>,
}

impl<'a> Walk<'a> {
    fn new(library: &'a Library) -> Self {
        Self {
            library,
            builder: ExecutionModelBuilder::new(),
            declarations: Declarations::default(),
            programs: Vec::new(),
            outer_scopes: Vec::new(),
            undeclared: Vec::new(),
        }
    }

    fn resolve(&mut self, options: &CompilerOptions) -> Execution {
        let mut programs = Vec::new();
        let mut configurations = Vec::new();
        let mut top_level = Vec::new();
        for (element, kind) in self.library.elements.iter().enumerate() {
            match kind {
                LibraryElementKind::ProgramDeclaration(program) => {
                    programs.push((element, program))
                }
                LibraryElementKind::ConfigurationDeclaration(config) => {
                    configurations.push((element, config))
                }
                LibraryElementKind::GlobalVarDeclarations(decls) => {
                    top_level.push((element, decls))
                }
                _ => {}
            }
        }
        // The toposort reorders POUs and configurations, and for declarations
        // with no dependency between them the order comes out reversed. Top-
        // level `VAR_GLOBAL` blocks stay first, in source order, so their
        // order is kept as it is.
        sort_by_source_position(&mut programs, |(_, program)| &program.name);
        sort_by_source_position(&mut configurations, |(_, config)| &config.name);

        if programs.is_empty() {
            return Execution::NotExecutable(NotExecutable::NoProgram);
        }
        if configurations.len() > 1 {
            return Execution::NotExecutable(NotExecutable::SeveralConfigurations(
                configurations
                    .iter()
                    .map(|(_, config)| debug_name(&config.name))
                    .collect(),
            ));
        }

        for (element, program) in programs {
            let id = self.builder.add_program(ProgramType {
                name: debug_name(&program.name),
            });
            self.declarations.programs.insert(id, element);
            self.programs.push((program, id));
        }

        let mut system = Scope::new();
        if options.allow_system_uptime_global {
            for (global, kind) in SYSTEM_UPTIME_GLOBALS
                .iter()
                .zip([SystemGlobal::UpTime, SystemGlobal::UpLTime])
            {
                let id = self.builder.add_global(Global {
                    name: DebugName::new(global.name, SourceSpan::default()),
                    kind: GlobalKind::System(kind),
                });
                system.insert(Id::from(global.name), id);
            }
        }

        let mut top_level_scope = Scope::new();
        for (element, decls) in top_level {
            for (index, decl) in decls.iter().enumerate() {
                let id = self.add_global(
                    decl,
                    GlobalScope::TopLevel,
                    GlobalDeclaration::TopLevel { element, index },
                );
                if let Some(name) = decl.identifier.symbolic_id() {
                    top_level_scope.insert(name.clone(), id);
                }
            }
        }

        let configuration = match configurations.first() {
            Some((element, config)) => {
                self.outer_scopes = vec![top_level_scope, system];
                self.resolve_configuration(*element, config)
            }
            None => Configuration {
                name: None,
                resources: Vec::new(),
            },
        };
        self.finish(configuration)
    }

    /// Builds the model of `configuration`, binding the only `PROGRAM`
    /// implicitly when no instance runs one.
    fn finish(&mut self, mut configuration: Configuration) -> Execution {
        if !self.undeclared.is_empty() {
            return Execution::NotExecutable(NotExecutable::UndeclaredPrograms(std::mem::take(
                &mut self.undeclared,
            )));
        }

        let has_instance = configuration
            .resources
            .iter()
            .flat_map(|resource| &resource.tasks)
            .any(|task| !task.instances.is_empty());
        if !has_instance {
            match self.programs.as_slice() {
                [(_, program)] => configuration.resources.push(Resource {
                    name: None,
                    tasks: vec![implicit_task(vec![ProgramInstance {
                        name: None,
                        program: *program,
                    }])],
                }),
                several => {
                    return Execution::NotExecutable(NotExecutable::SeveralPrograms(
                        several
                            .iter()
                            .map(|(program, _)| debug_name(&program.name))
                            .collect(),
                    ))
                }
            }
        }

        let builder = std::mem::take(&mut self.builder);
        Execution::Executable(builder.build(configuration))
    }

    /// Adds the globals of `config` and its resources, then resolves its
    /// resources.
    fn resolve_configuration(
        &mut self,
        element: usize,
        config: &ConfigurationDeclaration,
    ) -> Configuration {
        let mut configuration_scope = Scope::new();
        for (index, decl) in config.global_var.iter().enumerate() {
            let id = self.add_global(
                decl,
                GlobalScope::Configuration,
                GlobalDeclaration::Configuration { element, index },
            );
            if let Some(name) = decl.identifier.symbolic_id() {
                configuration_scope.insert(name.clone(), id);
            }
        }

        let mut resource_scopes = Vec::new();
        for (resource, declaration) in config.resource_decl.iter().enumerate() {
            let mut scope = Scope::new();
            for (index, decl) in declaration.global_vars.iter().enumerate() {
                let id = self.add_global(
                    decl,
                    GlobalScope::Resource,
                    GlobalDeclaration::Resource {
                        element,
                        resource,
                        index,
                    },
                );
                if let Some(name) = decl.identifier.symbolic_id() {
                    scope.insert(name.clone(), id);
                }
            }
            resource_scopes.push(scope);
        }

        self.outer_scopes.insert(0, configuration_scope);

        let resources = config
            .resource_decl
            .iter()
            .zip(&resource_scopes)
            .map(|(resource, scope)| self.resolve_resource(resource, scope))
            .collect();

        Configuration {
            name: Some(debug_name(&config.name)),
            resources,
        }
    }

    /// Resolves a resource's tasks and gives each program instance to one.
    fn resolve_resource(&mut self, resource: &ResourceDeclaration, scope: &Scope) -> Resource {
        let mut tasks: Vec<Task> = resource
            .tasks
            .iter()
            .map(|task| self.resolve_task(task, scope))
            .collect();
        let mut implicit = Vec::new();
        for instance in &resource.programs {
            let Some(program) = self.program_named(&instance.type_name) else {
                self.undeclared.push(debug_name(&instance.type_name));
                continue;
            };
            let instance_model = ProgramInstance {
                name: Some(debug_name(&instance.name)),
                program,
            };
            // A `WITH` naming a task the resource does not declare is
            // reported by `rule_program_task_definition_exists`; the instance
            // runs under the implicit task, so every instance has a task.
            let declared = instance.task_name.as_ref().and_then(|task_name| {
                resource
                    .tasks
                    .iter()
                    .position(|task| &task.name == task_name)
            });
            match declared {
                Some(task) => tasks[task].instances.push(instance_model),
                None => implicit.push(instance_model),
            }
        }
        if !implicit.is_empty() {
            tasks.push(implicit_task(implicit));
        }
        Resource {
            name: Some(debug_name(&resource.name)),
            tasks,
        }
    }

    /// Resolves a task's parameters into its schedule.
    fn resolve_task(&self, task: &TaskConfiguration, scope: &Scope) -> Task {
        let interval = task.interval.as_ref().map(|interval| interval.interval);
        let schedule = match &task.single {
            Some(single) => Schedule::Event {
                trigger: match single {
                    DataSourceKind::GlobalVarReference(reference) => self
                        .global_named(&reference.global_var_name, scope)
                        .map(Trigger::Global)
                        // A `SINGLE` that names no global is reported by
                        // `rule_task_configuration`, so no clean analysis
                        // holds this model; the trigger is recorded as a
                        // constant only to keep the model well formed.
                        .unwrap_or(Trigger::Constant),
                    DataSourceKind::Constant(_) => Trigger::Constant,
                },
                interval,
            },
            // A zero interval means "as fast as possible", which is what a
            // freewheeling task does: scheduling it as cyclic with a zero
            // period would leave it permanently overdue. A negative interval
            // is reported by `rule_task_configuration`.
            None => match interval {
                Some(interval) if interval.is_positive() => Schedule::Cyclic { interval },
                _ => Schedule::Freewheeling,
            },
        };
        Task {
            name: Some(debug_name(&task.name)),
            priority: task.priority,
            schedule,
            instances: Vec::new(),
        }
    }

    /// Adds a declared global and records where its declaration is.
    fn add_global(
        &mut self,
        decl: &VarDecl,
        scope: GlobalScope,
        declaration: GlobalDeclaration,
    ) -> GlobalId {
        let id = self.builder.add_global(Global {
            name: DebugName::new(decl.identifier.to_string(), decl.identifier.span()),
            kind: GlobalKind::Declared(scope),
        });
        self.declarations.globals.insert(id, declaration);
        id
    }

    /// The id of the declared `PROGRAM` named `name`.
    fn program_named(&self, name: &Id) -> Option<ProgramId> {
        self.programs
            .iter()
            .find(|(program, _)| &program.name == name)
            .map(|(_, id)| *id)
    }

    /// The global `name` names from a task of a resource whose globals are
    /// `scope`: the resource's, then the configuration's, then the top-level
    /// ones, then the system ones.
    fn global_named(&self, name: &Id, scope: &Scope) -> Option<GlobalId> {
        std::iter::once(scope)
            .chain(&self.outer_scopes)
            .find_map(|scope| scope.get(name).copied())
    }
}

/// The globals of one scope, by name: what a `SINGLE` names a global by.
type Scope = HashMap<Id, GlobalId>;

/// The freewheeling task of priority 0 that runs the instances no `WITH`
/// binds, and the instance of a program no configuration binds.
fn implicit_task(instances: Vec<ProgramInstance>) -> Task {
    Task {
        name: None,
        priority: 0,
        schedule: Schedule::Freewheeling,
        instances,
    }
}

fn debug_name(id: &Id) -> DebugName {
    DebugName::new(id.original().clone(), id.span())
}

/// Orders `items` by where their identifier appears in the source: by file,
/// then by offset.
fn sort_by_source_position<T>(items: &mut [T], id: impl Fn(&T) -> &Id) {
    items.sort_by_key(|item| {
        let span = &id(item).span;
        (span.file_id.to_string(), span.start)
    });
}

#[cfg(test)]
mod tests;
