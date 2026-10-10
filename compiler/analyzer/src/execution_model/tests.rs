//! Tests of the execution model `resolve` builds from a library.

use ironplc_dsl::common::{Library, LibraryElementKind};
use ironplc_dsl::configuration::ConfigurationDeclaration;
use ironplc_dsl::core::FileId;
use ironplc_ir::execution::ExecutionModel;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use spec_test_macro::spec_test;
use time::Duration;

use super::*;
use crate::stages::analyze;
use crate::{CleanAnalysis, SemanticContext};

/// Analyzes `library`, and asserts analysis reported nothing.
fn analyze_clean(library: &Library, options: &CompilerOptions) -> (Library, SemanticContext) {
    let (library, context) = analyze(&[library], options).unwrap();
    assert!(
        context.diagnostics().is_empty(),
        "{:?}",
        context.diagnostics()
    );
    (library, context)
}

/// Analyzes `source` and returns what `resolve` built from it.
fn resolve_source_with(source: &str, options: &CompilerOptions) -> ExecutionModel {
    let library = parse_program(source, &FileId::default(), options).unwrap();
    let (library, context) = analyze_clean(&library, options);
    let model = resolve(&library, options);
    // The context holds what one walk of the same library built.
    assert_eq!(
        format!("{model:?}"),
        format!("{:?}", context.execution_model())
    );
    model
}

fn resolve_source(source: &str) -> ExecutionModel {
    resolve_source_with(source, &CompilerOptions::default())
}

fn name(name: &Option<DebugName>) -> String {
    name.as_ref()
        .map_or("<implicit>".to_string(), DebugName::to_string)
}

fn schedule(schedule: &Schedule) -> String {
    match schedule {
        Schedule::Cyclic { interval } => format!("cyclic {interval}"),
        Schedule::Freewheeling => "freewheeling".to_string(),
        Schedule::Event { trigger, interval } => {
            let trigger = match trigger {
                Trigger::Global(_) => "global",
                Trigger::Constant => "constant",
            };
            match interval {
                Some(interval) => format!("event {trigger} {interval}"),
                None => format!("event {trigger}"),
            }
        }
    }
}

/// What `instance` instantiates: its program's name, or `?` and the type
/// name it names.
fn instance_of(model: &ExecutionModel, instance: &ProgramInstance) -> String {
    match &instance.program {
        InstanceOf::Program(id) => model.program(*id).unwrap().name.to_string(),
        InstanceOf::Unresolved(type_name) => format!("?{type_name}"),
    }
}

/// Every instance of `configuration` as `resource/task(schedule, priority):
/// instance=program`, in the order the model lists them.
fn instances_of(model: &ExecutionModel, configuration: &Configuration) -> Vec<String> {
    let mut lines = Vec::new();
    for resource in &configuration.resources {
        for task in &resource.tasks {
            for instance in &task.instances {
                lines.push(format!(
                    "{}/{}({}, {}): {}={}",
                    name(&resource.name),
                    name(&task.name),
                    schedule(&task.schedule),
                    task.priority,
                    name(&instance.name),
                    instance_of(model, instance)
                ));
            }
        }
    }
    lines
}

/// The only configuration of `model`.
fn only_configuration(model: &ExecutionModel) -> &Configuration {
    match model.configurations() {
        [configuration] => configuration,
        configurations => panic!("{} configurations", configurations.len()),
    }
}

/// Every instance of the only configuration of `model`; see [`instances_of`].
fn instances(model: &ExecutionModel) -> Vec<String> {
    instances_of(model, only_configuration(model))
}

/// The globals of `model` as `name: kind`, in variable-table order.
fn globals(model: &ExecutionModel) -> Vec<String> {
    model
        .globals()
        .map(|(_, global)| format!("{}: {:?}", global.name, global.kind))
        .collect()
}

/// The one task of the only instance.
fn only_task(model: &ExecutionModel) -> &Task {
    let mut tasks = only_configuration(model)
        .resources
        .iter()
        .flat_map(|resource| &resource.tasks)
        .filter(|task| !task.instances.is_empty());
    let task = tasks.next().unwrap();
    assert!(tasks.next().is_none());
    task
}

const MAIN: &str = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 1;
END_PROGRAM
";

/// `main`, bound by a CONFIGURATION to one task with the given parameters.
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

#[test]
fn resolve_when_configuration_then_names_every_part() {
    let model = resolve_source(&program_with_task("INTERVAL := T#100ms, PRIORITY := 3"));

    assert_eq!(name(&only_configuration(&model).name), "config");
    assert_eq!(
        instances(&model),
        ["resource1/task1(cyclic 100ms, 3): instance1=main"]
    );
}

#[spec_test(REQ_EM_analyzer_012)]
fn resolve_when_no_configuration_then_implicit_instance_under_implicit_freewheeling_task() {
    let model = resolve_source(MAIN);

    assert!(only_configuration(&model).name.is_none());
    assert_eq!(only_configuration(&model).resources.len(), 1);
    assert_eq!(
        instances(&model),
        ["<implicit>/<implicit>(freewheeling, 0): <implicit>=main"]
    );
}

#[spec_test(REQ_EM_analyzer_013)]
fn resolve_when_configuration_instantiates_nothing_then_programs_bound_implicitly() {
    let source = format!(
        "{MAIN}
CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK task1(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM instance1 WITH task1 : main;
  END_RESOURCE
END_CONFIGURATION
"
    );
    // The structured text grammar needs a program instance in a resource;
    // the PLCopen XML front end does not. So the instance is removed after
    // parsing.
    let options = CompilerOptions::default();
    let mut library = parse_program(&source, &FileId::default(), &options).unwrap();
    configuration_mut(&mut library).resource_decl[0]
        .programs
        .clear();
    let (library, _) = analyze_clean(&library, &options);

    let model = resolve(&library, &options);

    assert_eq!(name(&only_configuration(&model).name), "config");
    let resources: Vec<String> = only_configuration(&model)
        .resources
        .iter()
        .map(|resource| name(&resource.name))
        .collect();
    assert_eq!(resources, ["resource1", "<implicit>"]);
    assert_eq!(
        instances(&model),
        ["<implicit>/<implicit>(freewheeling, 0): <implicit>=main"]
    );
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
    let model = resolve_source(&source);

    let tasks: Vec<String> = only_configuration(&model).resources[0]
        .tasks
        .iter()
        .map(|task| name(&task.name))
        .collect();
    assert_eq!(tasks, ["task1", "<implicit>"]);
    assert_eq!(
        instances(&model),
        ["resource1/<implicit>(freewheeling, 0): instance1=main"]
    );
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
    let model = resolve_source(&source);

    assert_eq!(only_configuration(&model).resources[0].tasks.len(), 1);
    assert_eq!(
        instances(&model),
        [
            "resource1/<implicit>(freewheeling, 0): instance1=main",
            "resource1/<implicit>(freewheeling, 0): instance2=main",
        ]
    );
}

#[spec_test(REQ_EM_analyzer_020)]
fn resolve_when_task_has_single_then_event_task_triggered_by_global_id() {
    let model = resolve_source(&program_with_task("SINGLE := Trigger, PRIORITY := 1"));

    let Schedule::Event {
        trigger: Trigger::Global(id),
        interval: None,
    } = only_task(&model).schedule
    else {
        panic!("{:?}", only_task(&model).schedule);
    };
    assert_eq!(model.global(id).unwrap().name.to_string(), "Trigger");
}

#[spec_test(REQ_EM_analyzer_020)]
fn resolve_when_task_has_single_and_interval_then_event_task() {
    let model = resolve_source(&program_with_task(
        "SINGLE := Trigger, INTERVAL := T#10ms, PRIORITY := 1",
    ));

    assert_eq!(schedule(&only_task(&model).schedule), "event global 10ms");
}

#[spec_test(REQ_EM_analyzer_020)]
fn resolve_when_task_single_is_constant_then_event_task_with_constant_trigger() {
    let model = resolve_source(&program_with_task("SINGLE := TRUE, PRIORITY := 1"));

    assert_eq!(schedule(&only_task(&model).schedule), "event constant");
}

#[spec_test(REQ_EM_analyzer_021)]
fn resolve_when_task_has_interval_then_cyclic_task_with_duration() {
    let model = resolve_source(&program_with_task("INTERVAL := T#100ms, PRIORITY := 3"));

    assert!(matches!(
        only_task(&model).schedule,
        Schedule::Cyclic { interval } if interval == Duration::milliseconds(100)
    ));
}

#[spec_test(REQ_EM_analyzer_021)]
fn resolve_when_task_interval_is_sub_millisecond_then_duration_keeps_precision() {
    let model = resolve_source(&program_with_task("INTERVAL := T#0.5ms, PRIORITY := 0"));

    assert!(matches!(
        only_task(&model).schedule,
        Schedule::Cyclic { interval } if interval == Duration::microseconds(500)
    ));
}

#[spec_test(REQ_EM_analyzer_022)]
fn resolve_when_task_interval_is_zero_then_freewheeling_task() {
    let model = resolve_source(&program_with_task("INTERVAL := T#0ms, PRIORITY := 1"));

    assert!(matches!(only_task(&model).schedule, Schedule::Freewheeling));
    assert_eq!(only_task(&model).priority, 1);
}

#[spec_test(REQ_EM_analyzer_022)]
fn resolve_when_task_has_no_interval_then_freewheeling_task() {
    let model = resolve_source(&program_with_task("PRIORITY := 2"));

    assert!(matches!(only_task(&model).schedule, Schedule::Freewheeling));
}

#[spec_test(REQ_EM_analyzer_023)]
fn resolve_when_task_priority_exceeds_u16_then_priority_recorded_unchanged() {
    let model = resolve_source(&program_with_task(
        "INTERVAL := T#100ms, PRIORITY := 100000",
    ));

    assert_eq!(only_task(&model).priority, 100_000);
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
    let (library, _) = analyze_clean(&library, &options);

    let model = resolve(&library, &options);

    let programs: Vec<String> = model
        .programs()
        .map(|(_, program)| program.name.to_string())
        .collect();
    assert_eq!(programs, ["fast_prg", "slow_prg"]);
    assert_eq!(
        instances(&model),
        [
            "cpu1/fast(cyclic 10ms, 0): fast1=fast_prg",
            "cpu1/fast(cyclic 10ms, 0): fast2=fast_prg",
            "cpu1/slow(cyclic 100ms, 5): slow1=slow_prg",
            "cpu2/background(freewheeling, 9): slow2=slow_prg",
            "cpu2/<implicit>(freewheeling, 0): loose=fast_prg",
        ]
    );
}

#[spec_test(REQ_EM_analyzer_040)]
fn resolve_when_globals_in_every_scope_then_ordered_by_scope() {
    let source = format!(
        "
VAR_GLOBAL
  top : INT;
  top2 : INT;
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
    let model = resolve_source_with(&source, &options);

    assert_eq!(
        globals(&model),
        [
            "top: Declared(TopLevel)",
            "top2: Declared(TopLevel)",
            "shared: Declared(Configuration)",
            "local: Declared(Resource)",
        ]
    );
}

#[spec_test(REQ_EM_analyzer_040)]
fn resolve_when_globals_then_clean_analysis_answers_each_declaration_by_id() {
    let source = format!(
        "
VAR_GLOBAL
  top : INT := 7;
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
        allow_system_uptime_global: true,
        ..CompilerOptions::default()
    };
    let library = parse_program(&source, &FileId::default(), &options).unwrap();
    let (library, context) = analyze_clean(&library, &options);
    let analysis = CleanAnalysis::new(&library, &context).unwrap();
    let model = analysis.execution_model();

    let declarations: Vec<Option<String>> = model
        .globals()
        .map(|(id, _)| {
            analysis
                .global_declaration(id)
                .map(|decl| decl.identifier.to_string())
        })
        .collect();
    assert_eq!(
        declarations,
        [
            None,
            None,
            Some("top".to_string()),
            Some("shared".to_string()),
            Some("local".to_string()),
        ]
    );
    let (id, _) = model.programs().next().unwrap();
    assert_eq!(
        analysis.program_declaration(id).unwrap().name.to_string(),
        "main"
    );
}

#[test]
fn clean_analysis_when_paired_with_another_library_then_model_is_that_librarys() {
    let options = CompilerOptions::default();
    let library = parse_program(MAIN, &FileId::default(), &options).unwrap();
    let (_, context) = analyze_clean(&library, &options);
    let other = parse_program(
        "PROGRAM other VAR y : INT; END_VAR y := 2; END_PROGRAM",
        &FileId::default(),
        &options,
    )
    .unwrap();
    let analysis = CleanAnalysis::new(&other, &context).unwrap();
    let model = analysis.execution_model();

    let (id, program) = model.programs().next().unwrap();
    assert_eq!(program.name.to_string(), "other");
    assert!(std::ptr::eq(
        analysis.program_declaration(id).unwrap(),
        match &other.elements[0] {
            LibraryElementKind::ProgramDeclaration(program) => program,
            _ => panic!("not a program"),
        }
    ));
}

#[test]
fn program_declaration_when_id_is_another_models_then_none() {
    let options = CompilerOptions::default();
    let two = format!(
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
    let library = parse_program(&two, &FileId::default(), &options).unwrap();
    let (library, context) = analyze_clean(&library, &options);
    let analysis = CleanAnalysis::new(&library, &context).unwrap();
    let model = analysis.execution_model();
    let (second, _) = model.programs().nth(1).unwrap();

    let one = parse_program(MAIN, &FileId::default(), &options).unwrap();
    let (one, one_context) = analyze_clean(&one, &options);
    let one_analysis = CleanAnalysis::new(&one, &one_context).unwrap();

    assert!(one_analysis.program_declaration(second).is_none());
}

#[spec_test(REQ_EM_analyzer_041)]
fn resolve_when_system_uptime_global_allowed_then_system_globals_first() {
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
    let model = resolve_source_with(source, &options);

    assert_eq!(
        globals(&model),
        [
            "__SYSTEM_UP_TIME: System(UpTime)",
            "__SYSTEM_UP_LTIME: System(UpLTime)",
            "top: Declared(TopLevel)",
        ]
    );
}

#[spec_test(REQ_EM_analyzer_041)]
fn resolve_when_system_uptime_global_not_allowed_then_no_system_globals() {
    let model = resolve_source(MAIN);

    assert_eq!(model.globals().count(), 0);
}

#[spec_test(REQ_EM_analyzer_012)]
fn resolve_when_no_program_and_no_configuration_then_no_configuration() {
    let source = "
FUNCTION_BLOCK MyBlock
  VAR
    x : INT;
  END_VAR
END_FUNCTION_BLOCK
";
    let model = resolve_source(source);

    assert!(model.configurations().is_empty());
    assert_eq!(model.programs().count(), 0);
}

#[spec_test(REQ_EM_analyzer_012)]
fn resolve_when_two_programs_and_no_configuration_then_implicit_instance_of_each_in_source_order() {
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
    let model = resolve_source(source);

    assert_eq!(
        instances(&model),
        [
            "<implicit>/<implicit>(freewheeling, 0): <implicit>=first",
            "<implicit>/<implicit>(freewheeling, 0): <implicit>=second",
        ]
    );
}

#[spec_test(REQ_EM_analyzer_031)]
fn resolve_when_two_configurations_then_both_in_source_order() {
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
    let model = resolve_source(&source);

    let configurations: Vec<(String, Vec<String>)> = model
        .configurations()
        .iter()
        .map(|configuration| {
            (
                name(&configuration.name),
                instances_of(&model, configuration),
            )
        })
        .collect();
    assert_eq!(
        configurations,
        [
            (
                "cfgA".to_string(),
                vec!["resA/t(cyclic 10ms, 1): instA=main".to_string()]
            ),
            (
                "cfgB".to_string(),
                vec!["resB/t(cyclic 500ms, 1): instB=main".to_string()]
            ),
        ]
    );
}

#[spec_test(REQ_EM_analyzer_031)]
fn resolve_when_two_configurations_then_each_single_names_its_own_global() {
    let source = format!(
        "{MAIN}
CONFIGURATION cfgA
  VAR_GLOBAL
    go_a : BOOL;
  END_VAR
  RESOURCE resA ON PLC
    TASK t(SINGLE := go_a, PRIORITY := 1);
    PROGRAM instA WITH t : main;
  END_RESOURCE
END_CONFIGURATION

CONFIGURATION cfgB
  VAR_GLOBAL
    go_b : BOOL;
  END_VAR
  RESOURCE resB ON PLC
    TASK t(SINGLE := go_b, PRIORITY := 1);
    PROGRAM instB WITH t : main;
  END_RESOURCE
END_CONFIGURATION
"
    );
    let model = resolve_source(&source);

    let triggers: Vec<String> = model
        .configurations()
        .iter()
        .map(|configuration| {
            let Schedule::Event {
                trigger: Trigger::Global(id),
                ..
            } = configuration.resources[0].tasks[0].schedule
            else {
                panic!("not an event task");
            };
            model.global(id).unwrap().name.to_string()
        })
        .collect();
    assert_eq!(triggers, ["go_a", "go_b"]);
    assert_eq!(
        globals(&model),
        [
            "go_a: Declared(Configuration)",
            "go_b: Declared(Configuration)"
        ]
    );
}

#[spec_test(REQ_EM_analyzer_032)]
fn resolve_when_two_programs_and_configuration_binds_one_then_both_are_program_types() {
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
    let model = resolve_source(&source);

    let programs: Vec<String> = model
        .programs()
        .map(|(_, program)| program.name.to_string())
        .collect();
    assert_eq!(programs, ["main", "helper"]);
    assert_eq!(
        instances(&model),
        ["resource1/<implicit>(freewheeling, 0): instance1=main"]
    );
}

#[spec_test(REQ_EM_analyzer_054)]
fn resolve_when_instance_type_is_not_a_program_then_instance_unresolved_with_type_name() {
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
    let model = resolve_source(&source);

    assert_eq!(
        instances(&model),
        [
            "resource1/<implicit>(freewheeling, 0): first=?counter",
            "resource1/<implicit>(freewheeling, 0): second=main",
            "resource1/<implicit>(freewheeling, 0): third=?elsewhere",
        ]
    );
}

#[spec_test(REQ_EM_analyzer_054)]
fn resolve_when_configuration_alone_then_every_instance_unresolved() {
    let source = "
CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK task1(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM instance1 WITH task1 : main;
  END_RESOURCE
END_CONFIGURATION
";
    let model = resolve_source(source);

    assert_eq!(model.programs().count(), 0);
    assert_eq!(
        instances(&model),
        ["resource1/task1(cyclic 10ms, 1): instance1=?main"]
    );
}
