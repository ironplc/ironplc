//! Tests of the execution model `resolve` builds from a library.

use ironplc_dsl::common::{Library, LibraryElementKind, TypeName};
use ironplc_dsl::configuration::ConfigurationDeclaration;
use ironplc_dsl::core::{FileId, Id};
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use spec_test_macro::spec_test;
use time::Duration;

use super::*;
use crate::stages::analyze;
use crate::SemanticContext;

/// Analyzes `source`, asserts analysis reported nothing, and returns the
/// execution model and the context it was resolved into.
fn resolve_with(source: &str, options: &CompilerOptions) -> (ExecutionModel, SemanticContext) {
    let library = parse_program(source, &FileId::default(), options).unwrap();
    let (_, context) = analyze(&[&library], options).unwrap();
    assert!(
        context.diagnostics().is_empty(),
        "{:?}",
        context.diagnostics()
    );
    (context.execution_model().clone(), context)
}

fn resolve_model(source: &str) -> ExecutionModel {
    resolve_with(source, &CompilerOptions::default()).0
}

/// The first program instance and the task it runs under.
fn only_instance(model: &ExecutionModel) -> (ProgramInstance, Task) {
    let bound = model.instances().next().unwrap();
    (bound.instance.clone(), bound.task.clone())
}

fn names(ids: &[Id]) -> Vec<String> {
    ids.iter().map(Id::to_string).collect()
}

const MAIN: &str = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 1;
END_PROGRAM
";

/// A program `main` whose CONFIGURATION declares one task with the given
/// initialization parameters and binds the program to it.
fn program_with_task(task_init: &str) -> String {
    format!(
        "{MAIN}
CONFIGURATION config
  VAR_GLOBAL
    Trigger : BOOL;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK task1({task_init});
    PROGRAM instance1 WITH task1 : main;
  END_RESOURCE
END_CONFIGURATION
"
    )
}

#[spec_test(REQ_EM_analyzer_001)]
fn resolve_when_library_dropped_then_model_still_readable() {
    let model = {
        let source = program_with_task("INTERVAL := T#100ms, PRIORITY := 3");
        resolve_model(&source)
    };

    let (instance, task) = only_instance(&model);

    assert_eq!(model.configuration().unwrap().to_string(), "config");
    assert_eq!(instance.name.unwrap().to_string(), "instance1");
    assert_eq!(task.name.unwrap().to_string(), "task1");
}

#[spec_test(REQ_EM_analyzer_010)]
fn resolve_when_no_configuration_then_implicit_instance_under_implicit_freewheeling_task() {
    let model = resolve_model(MAIN);

    assert_eq!(model.configuration(), None);
    assert_eq!(model.not_executable(), None);
    assert_eq!(model.resources().len(), 1);
    assert_eq!(model.resources()[0].name(), None);
    let (instance, task) = only_instance(&model);
    assert_eq!(instance.name, None);
    assert_eq!(instance.program, Id::from("main"));
    assert_eq!(task, Task::implicit());
    assert_eq!(task.kind, TaskKind::Freewheeling);
}

#[spec_test(REQ_EM_analyzer_011)]
fn resolve_when_program_instance_has_no_task_then_implicit_freewheeling_task_after_declared() {
    let source = format!(
        "{MAIN}
CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK task1(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM instance1 : main;
  END_RESOURCE
END_CONFIGURATION
"
    );
    let model = resolve_model(&source);

    let resource = &model.resources()[0];
    assert_eq!(resource.tasks().len(), 2);
    assert_eq!(resource.tasks()[1], Task::implicit());
    let (instance, task) = only_instance(&model);
    assert_eq!(instance.task, TaskIndex(1));
    assert_eq!(task.kind, TaskKind::Freewheeling);
}

#[spec_test(REQ_EM_analyzer_011)]
fn resolve_when_two_instances_have_no_task_then_they_share_one_implicit_task() {
    let source = format!(
        "{MAIN}
CONFIGURATION config
  RESOURCE resource1 ON PLC
    PROGRAM instance1 : main;
    PROGRAM instance2 : main;
  END_RESOURCE
END_CONFIGURATION
"
    );
    let model = resolve_model(&source);

    let resource = &model.resources()[0];
    assert_eq!(resource.tasks(), &[Task::implicit()]);
    assert_eq!(resource.instances()[0].task, TaskIndex(0));
    assert_eq!(resource.instances()[1].task, TaskIndex(0));
}

#[spec_test(REQ_EM_analyzer_020)]
fn resolve_when_task_has_single_then_event_task_with_global_trigger() {
    let source = program_with_task("SINGLE := Trigger, PRIORITY := 1");
    let model = resolve_model(&source);

    let (_, task) = only_instance(&model);
    assert_eq!(task.kind, TaskKind::Event);
    assert_eq!(task.single, Some(EventTrigger::Global(Id::from("Trigger"))));
}

#[spec_test(REQ_EM_analyzer_020)]
fn resolve_when_task_has_single_and_interval_then_event_task() {
    let source = program_with_task("SINGLE := Trigger, INTERVAL := T#10ms, PRIORITY := 1");
    let model = resolve_model(&source);

    let (_, task) = only_instance(&model);
    assert_eq!(task.kind, TaskKind::Event);
}

#[spec_test(REQ_EM_analyzer_021)]
fn resolve_when_task_has_interval_then_cyclic_task_with_duration() {
    let source = program_with_task("INTERVAL := T#100ms, PRIORITY := 3");
    let model = resolve_model(&source);

    let (_, task) = only_instance(&model);
    assert_eq!(task.kind, TaskKind::Cyclic);
    assert_eq!(task.interval.unwrap().duration, Duration::milliseconds(100));
    assert_eq!(task.priority, 3);
}

#[spec_test(REQ_EM_analyzer_021)]
fn resolve_when_task_interval_is_sub_millisecond_then_duration_keeps_precision() {
    let source = program_with_task("INTERVAL := T#0.5ms, PRIORITY := 0");
    let model = resolve_model(&source);

    let (_, task) = only_instance(&model);
    assert_eq!(task.interval.unwrap().duration, Duration::microseconds(500));
}

#[spec_test(REQ_EM_analyzer_022)]
fn resolve_when_task_interval_is_zero_then_freewheeling_task() {
    let source = program_with_task("INTERVAL := T#0ms, PRIORITY := 1");
    let model = resolve_model(&source);

    let (_, task) = only_instance(&model);
    assert_eq!(task.kind, TaskKind::Freewheeling);
    assert_eq!(task.priority, 1);
}

#[spec_test(REQ_EM_analyzer_022)]
fn resolve_when_task_has_no_interval_then_freewheeling_task() {
    let source = program_with_task("PRIORITY := 2");
    let model = resolve_model(&source);

    let (_, task) = only_instance(&model);
    assert_eq!(task.kind, TaskKind::Freewheeling);
    assert_eq!(task.interval, None);
}

#[spec_test(REQ_EM_analyzer_023)]
fn resolve_when_task_priority_exceeds_u16_then_priority_recorded_unchanged() {
    let source = program_with_task("INTERVAL := T#100ms, PRIORITY := 100000");
    let model = resolve_model(&source);

    let (_, task) = only_instance(&model);
    assert_eq!(task.priority, 100_000);
}

/// The configuration a library declares.
fn configuration_mut(library: &mut Library) -> &mut ConfigurationDeclaration {
    library
        .elements
        .iter_mut()
        .find_map(|element| match element {
            LibraryElementKind::ConfigurationDeclaration(config) => Some(config),
            _ => None,
        })
        .unwrap()
}

#[spec_test(REQ_EM_analyzer_030)]
fn resolve_when_two_resources_with_several_tasks_and_instances_then_resolves_all() {
    let source = "
PROGRAM fast_prg
  VAR x : INT; END_VAR
  x := 1;
END_PROGRAM

PROGRAM slow_prg
  VAR y : INT; END_VAR
  y := 2;
END_PROGRAM

CONFIGURATION plant
  RESOURCE cpu1 ON PLC
    TASK fast(INTERVAL := T#10ms, PRIORITY := 0);
    TASK slow(INTERVAL := T#100ms, PRIORITY := 5);
    PROGRAM fast1 WITH fast : fast_prg;
    PROGRAM slow1 WITH slow : slow_prg;
    PROGRAM fast2 WITH fast : fast_prg;
  END_RESOURCE
END_CONFIGURATION
";
    // The structured text grammar takes one RESOURCE per configuration; the
    // PLCopen XML front end gives a configuration as many as it declares. So
    // the second resource is parsed on its own and added to the first.
    let second = "
CONFIGURATION other
  RESOURCE cpu2 ON PLC
    TASK background(PRIORITY := 9);
    PROGRAM slow2 WITH background : slow_prg;
    PROGRAM loose : fast_prg;
  END_RESOURCE
END_CONFIGURATION
";
    let options = CompilerOptions::default();
    let mut library = parse_program(source, &FileId::default(), &options).unwrap();
    let mut other = parse_program(second, &FileId::default(), &options).unwrap();
    let cpu2 = configuration_mut(&mut other).resource_decl.remove(0);
    configuration_mut(&mut library).resource_decl.push(cpu2);
    let (_, context) = analyze(&[&library], &options).unwrap();
    assert!(context.diagnostics().is_empty());
    let model = context.execution_model();

    assert_eq!(model.not_executable(), None);
    assert_eq!(names(model.programs()), ["fast_prg", "slow_prg"]);
    let bound: Vec<(String, String, String, String, TaskKind)> = model
        .instances()
        .map(|bound| {
            (
                bound.resource.name().unwrap().to_string(),
                bound.instance.name.as_ref().unwrap().to_string(),
                bound.instance.program.to_string(),
                bound
                    .task
                    .name
                    .as_ref()
                    .map_or("<implicit>".to_string(), Id::to_string),
                bound.task.kind,
            )
        })
        .collect();
    let expected = [
        ("cpu1", "fast1", "fast_prg", "fast", TaskKind::Cyclic),
        ("cpu1", "slow1", "slow_prg", "slow", TaskKind::Cyclic),
        ("cpu1", "fast2", "fast_prg", "fast", TaskKind::Cyclic),
        (
            "cpu2",
            "slow2",
            "slow_prg",
            "background",
            TaskKind::Freewheeling,
        ),
        (
            "cpu2",
            "loose",
            "fast_prg",
            "<implicit>",
            TaskKind::Freewheeling,
        ),
    ]
    .map(|(r, i, p, t, k)| (r.into(), i.into(), p.into(), t.into(), k));
    assert_eq!(bound, expected);
    assert_eq!(model.resources()[0].tasks().len(), 2);
    assert_eq!(model.resources()[1].tasks().len(), 2);
}

#[spec_test(REQ_EM_analyzer_040)]
fn resolve_when_globals_in_every_scope_then_ordered_by_scope_with_type_ids() {
    let source = format!(
        "
VAR_GLOBAL
  top : INT;
END_VAR
{MAIN}
CONFIGURATION config
  VAR_GLOBAL
    shared : DINT;
  END_VAR
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      local : BOOL;
    END_VAR
    PROGRAM instance1 : main;
  END_RESOURCE
END_CONFIGURATION
"
    );
    let options = CompilerOptions {
        allow_top_level_var_global: true,
        ..CompilerOptions::default()
    };
    let (model, context) = resolve_with(&source, &options);

    let expected = vec![
        GlobalVariable {
            name: Id::from("top"),
            type_id: context.types().id_of(&TypeName::from("INT")),
            scope: GlobalScope::TopLevel,
        },
        GlobalVariable {
            name: Id::from("shared"),
            type_id: context.types().id_of(&TypeName::from("DINT")),
            scope: GlobalScope::Configuration,
        },
        GlobalVariable {
            name: Id::from("local"),
            type_id: context.types().id_of(&TypeName::from("BOOL")),
            scope: GlobalScope::Resource(Id::from("resource1")),
        },
    ];
    assert_eq!(model.globals(), expected.as_slice());
    assert!(model.globals().iter().all(|g| g.type_id.is_some()));
}

#[spec_test(REQ_EM_analyzer_041)]
fn resolve_when_system_uptime_global_allowed_then_uptime_globals_typed_and_first() {
    let source = "
VAR_GLOBAL
  top : INT;
END_VAR
PROGRAM main
  VAR x : TIME; END_VAR
  x := __SYSTEM_UP_TIME;
END_PROGRAM
";
    let options = CompilerOptions {
        allow_system_uptime_global: true,
        allow_top_level_var_global: true,
        ..CompilerOptions::default()
    };
    let (model, context) = resolve_with(source, &options);

    let globals: Vec<(String, Option<TypeId>, GlobalScope)> = model
        .globals()
        .iter()
        .map(|g| (g.name.to_string(), g.type_id, g.scope.clone()))
        .collect();
    assert_eq!(
        globals,
        vec![
            (
                "__SYSTEM_UP_TIME".to_string(),
                context.types().id_of(&TypeName::from("TIME")),
                GlobalScope::System
            ),
            (
                "__SYSTEM_UP_LTIME".to_string(),
                context.types().id_of(&TypeName::from("LTIME")),
                GlobalScope::System
            ),
            (
                "top".to_string(),
                context.types().id_of(&TypeName::from("INT")),
                GlobalScope::TopLevel
            ),
        ]
    );
    assert!(model.globals()[0].type_id.is_some());
    assert!(model.globals()[1].type_id.is_some());
}

#[spec_test(REQ_EM_analyzer_041)]
fn resolve_when_system_uptime_global_not_allowed_then_no_system_globals() {
    let model = resolve_model(MAIN);

    assert!(model.globals().is_empty());
}

#[spec_test(REQ_EM_analyzer_050)]
fn resolve_when_no_program_then_not_executable_no_program() {
    let source = "
FUNCTION_BLOCK MyBlock
  VAR
    x : INT;
  END_VAR
END_FUNCTION_BLOCK
";
    let model = resolve_model(source);

    assert_eq!(model.not_executable(), Some(&NotExecutable::NoProgram));
    assert_eq!(model.instances().count(), 0);
}

#[spec_test(REQ_EM_analyzer_051)]
fn resolve_when_two_configurations_then_not_executable_several_configurations() {
    let source = format!(
        "{MAIN}
CONFIGURATION cfgA
  RESOURCE resA ON PLC
    TASK t(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM instA WITH t : main;
  END_RESOURCE
END_CONFIGURATION

CONFIGURATION cfgB
  RESOURCE resB ON PLC
    TASK t(INTERVAL := T#500ms, PRIORITY := 1);
    PROGRAM instB WITH t : main;
  END_RESOURCE
END_CONFIGURATION
"
    );
    let model = resolve_model(&source);

    assert_eq!(
        model.not_executable(),
        Some(&NotExecutable::SeveralConfigurations(vec![
            Id::from("cfgA"),
            Id::from("cfgB")
        ]))
    );
    assert_eq!(model.configuration(), None);
    assert_eq!(model.instances().count(), 0);
}

#[spec_test(REQ_EM_analyzer_052)]
fn resolve_when_two_programs_and_no_configuration_then_not_executable_several_programs() {
    let source = "
PROGRAM first
  VAR x : INT; END_VAR
  x := 1;
END_PROGRAM

PROGRAM second
  VAR y : INT; END_VAR
  y := 2;
END_PROGRAM
";
    let model = resolve_model(source);

    assert_eq!(
        model.not_executable(),
        Some(&NotExecutable::SeveralPrograms(vec![
            Id::from("first"),
            Id::from("second")
        ]))
    );
    assert_eq!(model.instances().count(), 0);
}

#[spec_test(REQ_EM_analyzer_052)]
fn resolve_when_two_programs_and_configuration_binds_one_then_executable_model() {
    let source = format!(
        "{MAIN}
PROGRAM helper
  VAR y : INT; END_VAR
  y := 2;
END_PROGRAM

CONFIGURATION config
  RESOURCE resource1 ON PLC
    PROGRAM instance1 : main;
  END_RESOURCE
END_CONFIGURATION
"
    );
    let model = resolve_model(&source);

    assert_eq!(model.not_executable(), None);
    assert_eq!(model.programs().len(), 2);
    assert_eq!(model.instances().count(), 1);
}

#[spec_test(REQ_EM_analyzer_053)]
fn resolve_when_instance_type_is_not_a_program_then_not_executable_undeclared_programs() {
    let source = format!(
        "{MAIN}
FUNCTION_BLOCK counter
  VAR n : INT; END_VAR
  n := n + 1;
END_FUNCTION_BLOCK

CONFIGURATION config
  RESOURCE resource1 ON PLC
    PROGRAM first : counter;
    PROGRAM second : main;
    PROGRAM third : elsewhere;
  END_RESOURCE
END_CONFIGURATION
"
    );
    let model = resolve_model(&source);

    assert_eq!(
        model.not_executable(),
        Some(&NotExecutable::UndeclaredPrograms(vec![
            Id::from("counter"),
            Id::from("elsewhere")
        ]))
    );
    assert_eq!(model.instances().count(), 3);
}
