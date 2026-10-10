//! Resolves the execution model: the configurations the library declares,
//! which program instances run under which tasks, how each task is scheduled,
//! and which globals are in scope.
//!
//! These are language decisions, so they are made once, here, and a backend
//! lowers the answer. [`resolve`] builds an [`ExecutionModel`] of
//! `ironplc-ir`'s types from an analyzed library. The
//! [`SemanticContext`](crate::SemanticContext) stores it for `check` and the
//! editor, and [`CleanAnalysis::new`](crate::CleanAnalysis::new) resolves it
//! again, with the declaration behind each id, from the library it hands to
//! codegen.
//!
//! `resolve` reads only what a clean analysis holds (the library and the
//! compiler options), never the analyzer's internals, so that the module can
//! move unchanged into a lowering crate when one exists. It reports nothing:
//! the model describes the library as it is, and whether a backend can build
//! it (one configuration, one program, a body for every instance) is that
//! backend's check.
//!
//! See `specs/design/execution-model.md` and ADR-0065.

use std::collections::HashMap;

use ironplc_dsl::common::{Library, LibraryElementKind, ProgramDeclaration, VarDecl};
use ironplc_dsl::configuration::{
    ConfigurationDeclaration, DataSourceKind, ResourceDeclaration, TaskConfiguration,
};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_ir::execution::{
    Configuration, DebugName, ExecutionModel, ExecutionModelBuilder, Global, GlobalId, GlobalKind,
    GlobalScope, InstanceOf, ProgramId, ProgramInstance, ProgramType, Resource, Schedule,
    SystemGlobal, Task, Trigger,
};
use ironplc_parser::options::CompilerOptions;

use crate::system_globals::SYSTEM_UPTIME_GLOBALS;

/// Resolves the execution model of an analyzed library.
pub fn resolve(library: &Library, options: &CompilerOptions) -> ExecutionModel {
    resolve_with_declarations(library, options).0
}

/// Resolves the execution model, and the declaration behind each id the
/// model allocated.
///
/// Both come from one walk of `library`, so the declarations are those of the
/// ids the model holds, and they borrow from `library`. See [`Declarations`].
pub(crate) fn resolve_with_declarations<'a>(
    library: &'a Library,
    options: &CompilerOptions,
) -> (ExecutionModel, Declarations<'a>) {
    let mut walk = Walk::new(library);
    let model = walk.resolve(options);
    (model, walk.declarations)
}

/// The declaration behind each id of a model, borrowed from the library the
/// model was resolved from.
///
/// Codegen still compiles a program's body, and a declared global's type and
/// initial value, from the library. It reaches them by id, through
/// [`CleanAnalysis`](crate::CleanAnalysis), from the walk that allocated the
/// ids. It goes away when the lowered program carries bodies, types and
/// initial values.
#[derive(Clone, Debug, Default)]
pub(crate) struct Declarations<'a> {
    programs: HashMap<ProgramId, &'a ProgramDeclaration>,
    /// A declared global's declaration. A system global has none.
    globals: HashMap<GlobalId, &'a VarDecl>,
}

impl<'a> Declarations<'a> {
    /// The declaration of the program `id` names.
    pub(crate) fn program(&self, id: ProgramId) -> Option<&'a ProgramDeclaration> {
        self.programs.get(&id).copied()
    }

    /// The declaration of the declared global `id` names. `None` for a system
    /// global, which has no declaration.
    pub(crate) fn global(&self, id: GlobalId) -> Option<&'a VarDecl> {
        self.globals.get(&id).copied()
    }
}

/// The one walk of the library that builds the model and records where each
/// id's declaration is.
struct Walk<'a> {
    library: &'a Library,
    builder: ExecutionModelBuilder,
    declarations: Declarations<'a>,
    /// The declared programs and their ids, in source order.
    programs: Vec<(&'a ProgramDeclaration, ProgramId)>,
    /// The globals a `SINGLE` can name outside its configuration, nearest
    /// scope first: the top-level ones, then the system ones.
    outer_scopes: Vec<Scope>,
}

impl<'a> Walk<'a> {
    fn new(library: &'a Library) -> Self {
        Self {
            library,
            builder: ExecutionModelBuilder::new(),
            declarations: Declarations::default(),
            programs: Vec::new(),
            outer_scopes: Vec::new(),
        }
    }

    fn resolve(&mut self, options: &CompilerOptions) -> ExecutionModel {
        let mut programs = Vec::new();
        let mut configurations = Vec::new();
        let mut top_level = Vec::new();
        for kind in &self.library.elements {
            match kind {
                LibraryElementKind::ProgramDeclaration(program) => programs.push(program),
                LibraryElementKind::ConfigurationDeclaration(config) => configurations.push(config),
                LibraryElementKind::GlobalVarDeclarations(decls) => top_level.push(decls),
                _ => {}
            }
        }
        // The toposort reorders POUs and configurations, and for declarations
        // with no dependency between them the order comes out reversed. Top-
        // level `VAR_GLOBAL` blocks stay first, in source order, so their
        // order is kept as it is.
        sort_by_source_position(&mut programs, |program| &program.name);
        sort_by_source_position(&mut configurations, |config| &config.name);

        for program in programs {
            let id = self.builder.add_program(ProgramType {
                name: debug_name(&program.name),
            });
            self.declarations.programs.insert(id, program);
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
        for decl in top_level.into_iter().flatten() {
            let id = self.add_global(decl, GlobalScope::TopLevel);
            if let Some(name) = decl.identifier.symbolic_id() {
                top_level_scope.insert(name.clone(), id);
            }
        }
        self.outer_scopes = vec![top_level_scope, system];

        let mut resolved: Vec<Configuration> = configurations
            .into_iter()
            .map(|config| self.resolve_configuration(config))
            .collect();
        // A library with no configuration runs its programs on their own.
        if resolved.is_empty() && !self.programs.is_empty() {
            resolved.push(Configuration {
                name: None,
                resources: Vec::new(),
            });
        }
        for configuration in &mut resolved {
            self.bind_programs_implicitly(configuration);
        }

        std::mem::take(&mut self.builder).build(resolved)
    }

    /// Gives `configuration` an implicit resource that runs an implicit
    /// instance of every `PROGRAM`, when none of its resources runs one.
    fn bind_programs_implicitly(&self, configuration: &mut Configuration) {
        let has_instance = configuration
            .resources
            .iter()
            .flat_map(|resource| &resource.tasks)
            .any(|task| !task.instances.is_empty());
        if has_instance || self.programs.is_empty() {
            return;
        }
        let instances = self
            .programs
            .iter()
            .map(|(_, program)| ProgramInstance {
                name: None,
                program: InstanceOf::Program(*program),
            })
            .collect();
        configuration.resources.push(Resource {
            name: None,
            tasks: vec![implicit_task(instances)],
        });
    }

    /// Adds the globals of `config` and its resources, then resolves its
    /// resources.
    fn resolve_configuration(&mut self, config: &'a ConfigurationDeclaration) -> Configuration {
        let mut configuration_scope = Scope::new();
        for decl in &config.global_var {
            let id = self.add_global(decl, GlobalScope::Configuration);
            if let Some(name) = decl.identifier.symbolic_id() {
                configuration_scope.insert(name.clone(), id);
            }
        }

        let mut resource_scopes = Vec::new();
        for resource in &config.resource_decl {
            let mut scope = Scope::new();
            for decl in &resource.global_vars {
                let id = self.add_global(decl, GlobalScope::Resource);
                if let Some(name) = decl.identifier.symbolic_id() {
                    scope.insert(name.clone(), id);
                }
            }
            resource_scopes.push(scope);
        }

        let resources = config
            .resource_decl
            .iter()
            .zip(&resource_scopes)
            .map(|(resource, scope)| self.resolve_resource(resource, scope, &configuration_scope))
            .collect();

        Configuration {
            name: Some(debug_name(&config.name)),
            resources,
        }
    }

    /// Resolves a resource's tasks and gives each program instance to one.
    fn resolve_resource(
        &self,
        resource: &ResourceDeclaration,
        scope: &Scope,
        configuration_scope: &Scope,
    ) -> Resource {
        let mut tasks: Vec<Task> = resource
            .tasks
            .iter()
            .map(|task| self.resolve_task(task, &[scope, configuration_scope]))
            .collect();
        let mut implicit = Vec::new();
        for instance in &resource.programs {
            let instance_model = ProgramInstance {
                name: Some(debug_name(&instance.name)),
                program: match self.program_named(&instance.type_name) {
                    Some(program) => InstanceOf::Program(program),
                    None => InstanceOf::Unresolved(debug_name(&instance.type_name)),
                },
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
    fn resolve_task(&self, task: &TaskConfiguration, scopes: &[&Scope]) -> Task {
        let interval = task.interval.as_ref().map(|interval| interval.interval);
        let schedule = match &task.single {
            Some(single) => Schedule::Event {
                trigger: match single {
                    DataSourceKind::GlobalVarReference(reference) => self
                        .global_named(&reference.global_var_name, scopes)
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

    /// Adds a declared global and records its declaration.
    fn add_global(&mut self, decl: &'a VarDecl, scope: GlobalScope) -> GlobalId {
        let id = self.builder.add_global(Global {
            name: DebugName::new(decl.identifier.to_string(), decl.identifier.span()),
            kind: GlobalKind::Declared(scope),
        });
        self.declarations.globals.insert(id, decl);
        id
    }

    /// The id of the declared `PROGRAM` named `name`.
    fn program_named(&self, name: &Id) -> Option<ProgramId> {
        self.programs
            .iter()
            .find(|(program, _)| &program.name == name)
            .map(|(_, id)| *id)
    }

    /// The global `name` names from a task whose resource and configuration
    /// globals are `scopes`, nearest first: then the top-level ones, then the
    /// system ones.
    fn global_named(&self, name: &Id, scopes: &[&Scope]) -> Option<GlobalId> {
        scopes
            .iter()
            .copied()
            .chain(&self.outer_scopes)
            .find_map(|scope| scope.get(name).copied())
    }
}

/// The globals of one scope, by name: what a `SINGLE` names a global by.
type Scope = HashMap<Id, GlobalId>;

/// The freewheeling task of priority 0 that runs the instances no `WITH`
/// binds, and the instances of the programs no configuration binds.
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
