//! The execution model: what runs, and when.
//!
//! The model says which configurations the library declares, the resources
//! each holds, the
//! tasks each resource schedules, the program instances each task runs, and the
//! globals in scope. Every decision is already made, so a backend lowers the
//! model and decides nothing. See `specs/design/execution-model.md`.
//!
//! The model follows two rules:
//!
//! - **No source identity is a key.** It holds no declaration node, and no
//!   name a backend looks anything up by. A name is a [`DebugName`], which
//!   cannot be compared, and is kept only to label a diagnostic or debug
//!   information.
//! - **A relationship is ownership where it can be, and an id the model
//!   allocated where it cannot.** A configuration owns its resources, a
//!   resource its tasks, and a task its instances. An instance names its
//!   program by [`ProgramId`] and an event task its trigger by [`GlobalId`],
//!   both allocated by [`ExecutionModelBuilder`].

use std::fmt;

use ironplc_dsl::core::SourceSpan;

/// What runs, and when, as the library declares it.
///
/// The model describes the library; it does not say whether a backend can
/// build it. A library with no `PROGRAM`, several configurations, several
/// programs, or an instance of a type that is not a `PROGRAM` is a valid
/// library, and each backend checks the model against what it can run.
///
/// Every id the model holds names one of its own entries. The fields are
/// private so that [`ExecutionModelBuilder`] is the only way to make one.
#[derive(Debug, Clone)]
pub struct ExecutionModel {
    configurations: Vec<Configuration>,
    programs: Vec<ProgramType>,
    globals: Vec<Global>,
}

impl ExecutionModel {
    /// The configurations, in source order: those the library declares, or
    /// the implicit one of a library that declares none but has a `PROGRAM`.
    pub fn configurations(&self) -> &[Configuration] {
        &self.configurations
    }

    /// The program type `id` names.
    ///
    /// `None` means `id` was allocated by the builder of another model, a
    /// compiler defect the caller reports as an internal error. Such an id
    /// can also name the wrong entry: nothing in an id tells models apart.
    pub fn program(&self, id: ProgramId) -> Option<&ProgramType> {
        self.programs.get(id.0)
    }

    /// The global `id` names.
    ///
    /// `None` means `id` was allocated by the builder of another model, a
    /// compiler defect the caller reports as an internal error. Such an id
    /// can also name the wrong entry: nothing in an id tells models apart.
    pub fn global(&self, id: GlobalId) -> Option<&Global> {
        self.globals.get(id.0)
    }

    /// The program types, in the order they were added.
    pub fn programs(&self) -> impl Iterator<Item = (ProgramId, &ProgramType)> {
        self.programs
            .iter()
            .enumerate()
            .map(|(index, program)| (ProgramId(index), program))
    }

    /// The globals in scope, in variable-table order: the order they were
    /// added.
    pub fn globals(&self) -> impl Iterator<Item = (GlobalId, &Global)> {
        self.globals
            .iter()
            .enumerate()
            .map(|(index, global)| (GlobalId(index), global))
    }
}

/// A configuration, declared or implicit.
#[derive(Debug, Clone)]
pub struct Configuration {
    /// `None` for the implicit configuration of a library that declares none.
    pub name: Option<DebugName>,
    /// The resources, in declaration order, with an implicit one last when no
    /// declared resource runs a program instance.
    pub resources: Vec<Resource>,
}

/// A resource of the configuration.
#[derive(Debug, Clone)]
pub struct Resource {
    /// `None` for the implicit resource.
    pub name: Option<DebugName>,
    /// The declared tasks, in declaration order, then the implicit one.
    pub tasks: Vec<Task>,
}

/// A task of a resource, and the program instances it runs.
#[derive(Debug, Clone)]
pub struct Task {
    /// `None` for an implicit task.
    pub name: Option<DebugName>,
    /// The priority as declared. Whether a backend can represent it is the
    /// backend's check.
    pub priority: u32,
    pub schedule: Schedule,
    /// The instances this task runs, in order.
    pub instances: Vec<ProgramInstance>,
}

/// When a task runs.
///
/// Each kind carries the data it needs, so a cyclic task without an interval,
/// or an event task without a trigger, cannot be written.
#[derive(Debug, Clone)]
pub enum Schedule {
    /// A positive `INTERVAL` and no `SINGLE`.
    Cyclic { interval: time::Duration },
    /// An absent or zero `INTERVAL` and no `SINGLE`: the task runs as fast as
    /// possible.
    Freewheeling,
    /// A `SINGLE`, whatever the interval.
    Event {
        trigger: Trigger,
        interval: Option<time::Duration>,
    },
}

/// What a `SINGLE` names.
#[derive(Debug, Clone)]
pub enum Trigger {
    /// `SINGLE := <global>`.
    Global(GlobalId),
    /// `SINGLE := <constant>`.
    Constant,
}

/// One instance of a program, owned by the task that runs it.
#[derive(Debug, Clone)]
pub struct ProgramInstance {
    /// `None` for an implicit instance.
    pub name: Option<DebugName>,
    pub program: InstanceOf,
}

/// What a program instance instantiates.
#[derive(Debug, Clone)]
pub enum InstanceOf {
    /// A `PROGRAM` declaration of the library.
    Program(ProgramId),
    /// A type that is not a `PROGRAM` declaration of the library: a function
    /// block, or a name declared nowhere, as written. A configuration is often
    /// checked without the files that declare its programs, so this is not an
    /// error in the library; a backend has no body to compile for it.
    Unresolved(DebugName),
}

/// A `PROGRAM` declaration that a program instance instantiates.
#[derive(Debug, Clone)]
pub struct ProgramType {
    pub name: DebugName,
}

/// A global in scope.
///
/// A global carries no type: what a backend stores for a declared global is
/// its type in the lowered program's type table, which does not exist yet.
#[derive(Debug, Clone)]
pub struct Global {
    pub name: DebugName,
    pub kind: GlobalKind,
}

/// Where a global comes from.
#[derive(Debug, Clone)]
pub enum GlobalKind {
    /// Provided by the compiler.
    System(SystemGlobal),
    /// Declared in the source.
    Declared(GlobalScope),
}

/// A global the compiler provides, named by what the runtime writes into it,
/// so a backend does not recognise it by its name or its type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemGlobal {
    /// `__SYSTEM_UP_TIME`, the time since the runtime started, as `TIME`.
    UpTime,
    /// `__SYSTEM_UP_LTIME`, the time since the runtime started, as `LTIME`.
    UpLTime,
}

/// The `VAR_GLOBAL` block a declared global is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalScope {
    /// A `VAR_GLOBAL` outside any configuration.
    TopLevel,
    /// A `VAR_GLOBAL` of the configuration.
    Configuration,
    /// A `VAR_GLOBAL` of a resource.
    Resource,
}

/// A program type of one model, allocated by [`ExecutionModelBuilder`].
///
/// The id is the program's position in the model, so it is valid only for the
/// model whose builder allocated it. The position is held as a `usize`, the
/// type a vector is indexed by, so allocating an id needs no conversion that
/// could fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProgramId(usize);

/// A global of one model, allocated by [`ExecutionModelBuilder`].
///
/// The id is the global's position in the variable table, so it is valid only
/// for the model whose builder allocated it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlobalId(usize);

/// The only way to make an [`ExecutionModel`].
///
/// Adding a program or a global returns its id, so every id names an entry
/// the model holds.
#[derive(Debug, Default)]
pub struct ExecutionModelBuilder {
    programs: Vec<ProgramType>,
    globals: Vec<Global>,
}

impl ExecutionModelBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a program type and returns the id that names it.
    pub fn add_program(&mut self, program: ProgramType) -> ProgramId {
        let id = ProgramId(self.programs.len());
        self.programs.push(program);
        id
    }

    /// Adds a global at the end of the variable table and returns the id that
    /// names it.
    pub fn add_global(&mut self, global: Global) -> GlobalId {
        let id = GlobalId(self.globals.len());
        self.globals.push(global);
        id
    }

    /// Builds the model of `configurations`, whose ids this builder
    /// allocated.
    pub fn build(self, configurations: Vec<Configuration>) -> ExecutionModel {
        ExecutionModel {
            configurations,
            programs: self.programs,
            globals: self.globals,
        }
    }
}

/// A source name, for a diagnostic or debug information.
///
/// It implements neither `PartialEq`, `Eq`, `Hash` nor `Ord`, so it cannot be
/// compared or used as a key: nothing in the model is looked up by name.
#[derive(Debug, Clone)]
pub struct DebugName {
    text: String,
    span: SourceSpan,
}

impl DebugName {
    /// A name as written, at `span`.
    pub fn new(text: impl Into<String>, span: SourceSpan) -> Self {
        Self {
            text: text.into(),
            span,
        }
    }

    /// Where the name is written, to label a diagnostic.
    pub fn span(&self) -> SourceSpan {
        self.span.clone()
    }
}

impl fmt::Display for DebugName {
    /// The name as written.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::core::SourceSpan;

    use super::*;

    fn name(text: &str) -> DebugName {
        DebugName::new(text, SourceSpan::range(0, text.len()))
    }

    fn program(text: &str) -> ProgramType {
        ProgramType { name: name(text) }
    }

    fn global(text: &str) -> Global {
        Global {
            name: name(text),
            kind: GlobalKind::Declared(GlobalScope::TopLevel),
        }
    }

    fn no_configurations() -> Vec<Configuration> {
        vec![]
    }

    #[test]
    fn add_program_when_added_then_model_returns_it_by_id() {
        let mut builder = ExecutionModelBuilder::new();
        let first = builder.add_program(program("first"));
        let second = builder.add_program(program("second"));
        let model = builder.build(no_configurations());

        assert_eq!(
            model.program(first).map(|p| p.name.to_string()),
            Some("first".to_string())
        );
        assert_eq!(
            model.program(second).map(|p| p.name.to_string()),
            Some("second".to_string())
        );
    }

    #[test]
    fn add_global_when_added_then_model_returns_it_by_id() {
        let mut builder = ExecutionModelBuilder::new();
        let first = builder.add_global(global("first"));
        let second = builder.add_global(global("second"));
        let model = builder.build(no_configurations());

        assert_eq!(
            model.global(first).map(|g| g.name.to_string()),
            Some("first".to_string())
        );
        assert_eq!(
            model.global(second).map(|g| g.name.to_string()),
            Some("second".to_string())
        );
    }

    #[test]
    fn program_when_id_from_another_model_then_none() {
        let mut other = ExecutionModelBuilder::new();
        other.add_program(program("a"));
        let foreign = other.add_program(program("b"));
        let mut builder = ExecutionModelBuilder::new();
        builder.add_program(program("only"));
        let model = builder.build(no_configurations());

        assert!(model.program(foreign).is_none());
    }

    #[test]
    fn global_when_id_from_another_model_then_none() {
        let mut other = ExecutionModelBuilder::new();
        other.add_global(global("a"));
        let foreign = other.add_global(global("b"));
        let model = ExecutionModelBuilder::new().build(no_configurations());

        assert!(model.global(foreign).is_none());
    }

    #[test]
    fn globals_when_added_then_iterates_in_added_order_with_ids() {
        let mut builder = ExecutionModelBuilder::new();
        let up_time = builder.add_global(Global {
            name: name("__SYSTEM_UP_TIME"),
            kind: GlobalKind::System(SystemGlobal::UpTime),
        });
        let declared = builder.add_global(global("counter"));
        let model = builder.build(no_configurations());

        let globals: Vec<(GlobalId, String)> = model
            .globals()
            .map(|(id, g)| (id, g.name.to_string()))
            .collect();
        assert_eq!(
            globals,
            vec![
                (up_time, "__SYSTEM_UP_TIME".to_string()),
                (declared, "counter".to_string())
            ]
        );
    }

    #[test]
    fn programs_when_added_then_iterates_in_added_order_with_ids() {
        let mut builder = ExecutionModelBuilder::new();
        let first = builder.add_program(program("first"));
        let second = builder.add_program(program("second"));
        let model = builder.build(no_configurations());

        let programs: Vec<(ProgramId, String)> = model
            .programs()
            .map(|(id, p)| (id, p.name.to_string()))
            .collect();
        assert_eq!(
            programs,
            vec![(first, "first".to_string()), (second, "second".to_string())]
        );
    }

    #[test]
    fn build_when_configuration_given_then_model_holds_it() {
        let mut builder = ExecutionModelBuilder::new();
        let main = builder.add_program(program("main"));
        let configuration = Configuration {
            name: Some(name("config")),
            resources: vec![Resource {
                name: Some(name("resource1")),
                tasks: vec![Task {
                    name: Some(name("fast")),
                    priority: 70000,
                    schedule: Schedule::Cyclic {
                        interval: time::Duration::milliseconds(10),
                    },
                    instances: vec![ProgramInstance {
                        name: Some(name("instance1")),
                        program: InstanceOf::Program(main),
                    }],
                }],
            }],
        };
        let model = builder.build(vec![configuration]);

        let task = &model.configurations()[0].resources[0].tasks[0];
        assert_eq!(task.priority, 70000);
        assert!(matches!(
            task.schedule,
            Schedule::Cyclic { interval } if interval == time::Duration::milliseconds(10)
        ));
        assert!(matches!(task.instances[0].program, InstanceOf::Program(id) if id == main));
    }

    #[test]
    fn build_when_instance_unresolved_then_model_keeps_its_type_name() {
        let configuration = Configuration {
            name: Some(name("config")),
            resources: vec![Resource {
                name: Some(name("resource1")),
                tasks: vec![Task {
                    name: None,
                    priority: 0,
                    schedule: Schedule::Freewheeling,
                    instances: vec![ProgramInstance {
                        name: Some(name("instance1")),
                        program: InstanceOf::Unresolved(name("elsewhere")),
                    }],
                }],
            }],
        };
        let model = ExecutionModelBuilder::new().build(vec![configuration]);

        let instance = &model.configurations()[0].resources[0].tasks[0].instances[0];
        assert!(matches!(
            &instance.program,
            InstanceOf::Unresolved(type_name) if type_name.to_string() == "elsewhere"
        ));
    }

    #[test]
    fn debug_name_when_displayed_then_name_as_written() {
        let debug_name = DebugName::new("Main_Task", SourceSpan::range(3, 12));

        assert_eq!(debug_name.to_string(), "Main_Task");
        assert_eq!(debug_name.span().start, 3);
        assert_eq!(debug_name.span().end, 12);
    }
}
