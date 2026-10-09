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
fn resolve_source_with(source: &str, options: &CompilerOptions) -> Execution {
    let library = parse_program(source, &FileId::default(), options).unwrap();
    let (library, context) = analyze_clean(&library, options);
    let execution = resolve(&library, options);
    // The context holds what one walk of the same library built.
    assert_eq!(
        format!("{execution:?}"),
        format!("{:?}", context.execution())
    );
    execution
}

fn resolve_source(source: &str) -> Execution {
    resolve_source_with(source, &CompilerOptions::default())
}

fn model(execution: Execution) -> ExecutionModel {
    match execution {
        Execution::Executable(model) => model,
        Execution::NotExecutable(reason) => panic!("not executable: {reason:?}"),
    }
}

fn reason(execution: Execution) -> NotExecutable {
    match execution {
        Execution::NotExecutable(reason) => reason,
        Execution::Executable(model) => panic!("executable: {model:?}"),
    }
}

fn names(names: &[DebugName]) -> Vec<String> {
    names.iter().map(DebugName::to_string).collect()
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

/// Every instance of `model` as `resource/task(schedule, priority):
/// instance=program`, in the order the model lists them.
fn instances(model: &ExecutionModel) -> Vec<String> {
    let mut lines = Vec::new();
    for resource in &model.configuration().resources {
        for task in &resource.tasks {
            for instance in &task.instances {
                let program = model.program(instance.program).unwrap();
                lines.push(format!(
                    "{}/{}({}, {}): {}={}",
                    name(&resource.name),
                    name(&task.name),
                    schedule(&task.schedule),
                    task.priority,
                    name(&instance.name),
                    program.name
                ));
            }
        }
    }
    lines
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
    let mut tasks = model
        .configuration()
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
    let model = model(resolve_source(&program_with_task(
        "INTERVAL := T#100ms, PRIORITY := 3",
    )));

    assert_eq!(name(&model.configuration().name), "config");
    assert_eq!(
        instances(&model),
        ["resource1/task1(cyclic 100ms, 3): instance1=main"]
    );
}

#[spec_test(REQ_EM_analyzer_010)]
fn resolve_when_no_configuration_then_implicit_instance_under_implicit_freewheeling_task() {
    let model = model(resolve_source(MAIN));

    assert!(model.configuration().name.is_none());
    assert_eq!(model.configuration().resources.len(), 1);
    assert_eq!(
        instances(&model),
        ["<implicit>/<implicit>(freewheeling, 0): <implicit>=main"]
    );
}

#[spec_test(REQ_EM_analyzer_010)]
fn resolve_when_configuration_instantiates_nothing_then_only_program_bound_implicitly() {
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

    let model = model(resolve(&library, &options));

    assert_eq!(name(&model.configuration().name), "config");
    let resources: Vec<String> = model
        .configuration()
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
    let model = model(resolve_source(&source));

    let tasks: Vec<String> = model.configuration().resources[0]
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
    let model = model(resolve_source(&source));

    assert_eq!(model.configuration().resources[0].tasks.len(), 1);
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
    let model = model(resolve_source(&program_with_task(
        "SINGLE := Trigger, PRIORITY := 1",
    )));

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
    let model = model(resolve_source(&program_with_task(
        "SINGLE := Trigger, INTERVAL := T#10ms, PRIORITY := 1",
    )));

    assert_eq!(schedule(&only_task(&model).schedule), "event global 10ms");
}

#[spec_test(REQ_EM_analyzer_020)]
fn resolve_when_task_single_is_constant_then_event_task_with_constant_trigger() {
    let model = model(resolve_source(&program_with_task(
        "SINGLE := TRUE, PRIORITY := 1",
    )));

    assert_eq!(schedule(&only_task(&model).schedule), "event constant");
}

#[spec_test(REQ_EM_analyzer_021)]
fn resolve_when_task_has_interval_then_cyclic_task_with_duration() {
    let model = model(resolve_source(&program_with_task(
        "INTERVAL := T#100ms, PRIORITY := 3",
    )));

    assert!(matches!(
        only_task(&model).schedule,
        Schedule::Cyclic { interval } if interval == Duration::milliseconds(100)
    ));
}

#[spec_test(REQ_EM_analyzer_021)]
fn resolve_when_task_interval_is_sub_millisecond_then_duration_keeps_precision() {
    let model = model(resolve_source(&program_with_task(
        "INTERVAL := T#0.5ms, PRIORITY := 0",
    )));

    assert!(matches!(
        only_task(&model).schedule,
        Schedule::Cyclic { interval } if interval == Duration::microseconds(500)
    ));
}

#[spec_test(REQ_EM_analyzer_022)]
fn resolve_when_task_interval_is_zero_then_freewheeling_task() {
    let model = model(resolve_source(&program_with_task(
        "INTERVAL := T#0ms, PRIORITY := 1",
    )));

    assert!(matches!(only_task(&model).schedule, Schedule::Freewheeling));
    assert_eq!(only_task(&model).priority, 1);
}

#[spec_test(REQ_EM_analyzer_022)]
fn resolve_when_task_has_no_interval_then_freewheeling_task() {
    let model = model(resolve_source(&program_with_task("PRIORITY := 2")));

    assert!(matches!(only_task(&model).schedule, Schedule::Freewheeling));
}

#[spec_test(REQ_EM_analyzer_023)]
fn resolve_when_task_priority_exceeds_u16_then_priority_recorded_unchanged() {
    let model = model(resolve_source(&program_with_task(
        "INTERVAL := T#100ms, PRIORITY := 100000",
    )));

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

    let model = model(resolve(&library, &options));

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
    let model = model(resolve_source_with(&source, &options));

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
    let Execution::Executable(model) = analysis.execution() else {
        panic!("not executable");
    };

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
fn program_declaration_when_library_is_not_the_one_resolved_then_none() {
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
    let Execution::Executable(model) = analysis.execution() else {
        panic!("not executable");
    };

    let (id, _) = model.programs().next().unwrap();
    assert!(analysis.program_declaration(id).is_none());
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
    let model = model(resolve_source_with(source, &options));

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
    let model = model(resolve_source(MAIN));

    assert_eq!(model.globals().count(), 0);
}

#[spec_test(REQ_EM_analyzer_050)]
fn resolve_when_no_program_then_not_executable_no_program() {
    let source = "
FUNCTION_BLOCK MyBlock
  VAR
    x : INT;
  END_VAR
END_FUNCTION_BLOCK

CONFIGURATION cfgA
  RESOURCE resA ON PLC
    PROGRAM instA : MyBlock;
  END_RESOURCE
END_CONFIGURATION

CONFIGURATION cfgB
  RESOURCE resB ON PLC
    PROGRAM instB : MyBlock;
  END_RESOURCE
END_CONFIGURATION
";

    assert!(matches!(
        reason(resolve_source(source)),
        NotExecutable::NoProgram
    ));
}

#[spec_test(REQ_EM_analyzer_051)]
fn resolve_when_two_configurations_then_not_executable_with_names_in_source_order() {
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

    let NotExecutable::SeveralConfigurations(configurations) = reason(resolve_source(&source))
    else {
        panic!("not several configurations");
    };
    assert_eq!(names(&configurations), ["cfgA", "cfgB"]);
}

#[spec_test(REQ_EM_analyzer_052)]
fn resolve_when_two_programs_and_no_configuration_then_not_executable_with_names_in_source_order() {
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

    let NotExecutable::SeveralPrograms(programs) = reason(resolve_source(source)) else {
        panic!("not several programs");
    };
    assert_eq!(names(&programs), ["first", "second"]);
}

#[spec_test(REQ_EM_analyzer_052)]
fn resolve_when_two_programs_and_configuration_binds_one_then_executable() {
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
    let model = model(resolve_source(&source));

    assert_eq!(model.programs().count(), 2);
    assert_eq!(
        instances(&model),
        ["resource1/<implicit>(freewheeling, 0): instance1=main"]
    );
}

#[spec_test(REQ_EM_analyzer_053)]
fn resolve_when_instance_type_is_not_a_program_then_not_executable_with_type_names() {
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

    let NotExecutable::UndeclaredPrograms(types) = reason(resolve_source(&source)) else {
        panic!("not undeclared programs");
    };
    assert_eq!(names(&types), ["counter", "elsewhere"]);
}
