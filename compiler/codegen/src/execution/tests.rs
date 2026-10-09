//! Tests that codegen builds the container from the execution model, and
//! checks the model against what the VM supports. What the model holds is
//! the analyzer's to test (`ironplc_analyzer::execution_model`); these tests
//! check that the container follows it.

use ironplc_analyzer::CleanAnalysis;
use ironplc_container::{Container, TaskType, FLAG_HAS_SYSTEM_UPTIME};
use ironplc_dsl::core::{FileId, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_ir::execution::{DebugName, Schedule, Task, Trigger};
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_problems::Problem;
use spec_test_macro::spec_test;
use time::Duration;

use super::{ordinal, task_schedule};
use crate::compile::{compile, CodegenOptions};

/// The code of the P9999 not-implemented problem, which only its
/// `Diagnostic` constructor may name.
const NOT_IMPLEMENTED_CODE: &str = "P9999";

fn compile_with(source: &str, options: &CompilerOptions) -> Result<Container, Diagnostic> {
    let library = parse_program(source, &FileId::default(), options).unwrap();
    let (library, context) = ironplc_analyzer::stages::analyze(&[&library], options).unwrap();
    compile(
        CleanAnalysis::new(&library, &context).unwrap(),
        &CodegenOptions::from(options),
        &crate::EmptyLookup,
    )
}

fn compile_source(source: &str) -> Result<Container, Diagnostic> {
    compile_with(source, &CompilerOptions::default())
}

fn error_code(source: &str) -> String {
    compile_source(source).unwrap_err().code
}

const MAIN: &str = "
PROGRAM main
  VAR
    x : INT;
  END_VAR
  x := 1;
END_PROGRAM
";

/// `main` bound by a CONFIGURATION to one task with the given parameters.
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

/// A task named `task1` at `3..8`, as the model holds it.
fn task(priority: u32, schedule: Schedule) -> Task {
    Task {
        name: Some(DebugName::new("task1", SourceSpan::range(3, 8))),
        priority,
        schedule,
        instances: Vec::new(),
    }
}

#[spec_test(REQ_EM_codegen_001)]
fn compile_when_task_has_interval_then_cyclic_task_with_interval_us() {
    let container =
        compile_source(&program_with_task("INTERVAL := T#100ms, PRIORITY := 3")).unwrap();

    let entry = &container.task_table.tasks[0];
    assert_eq!(entry.task_type, TaskType::Cyclic);
    assert_eq!(entry.interval_us, 100_000);
    assert_eq!(entry.priority, 3);
}

#[spec_test(REQ_EM_codegen_001)]
fn compile_when_task_interval_is_sub_millisecond_then_interval_us_keeps_precision() {
    let container =
        compile_source(&program_with_task("INTERVAL := T#0.5ms, PRIORITY := 0")).unwrap();

    assert_eq!(container.task_table.tasks[0].interval_us, 500);
}

#[spec_test(REQ_EM_codegen_001)]
fn compile_when_task_interval_is_zero_then_freewheeling_task_keeps_priority() {
    let container = compile_source(&program_with_task("INTERVAL := T#0ms, PRIORITY := 4")).unwrap();

    let entry = &container.task_table.tasks[0];
    assert_eq!(entry.task_type, TaskType::Freewheeling);
    assert_eq!(entry.interval_us, 0);
    assert_eq!(entry.priority, 4);
}

#[spec_test(REQ_EM_codegen_001)]
fn compile_when_no_configuration_then_implicit_freewheeling_task() {
    let container = compile_source(MAIN).unwrap();

    let entry = &container.task_table.tasks[0];
    assert_eq!(entry.task_type, TaskType::Freewheeling);
    assert_eq!(entry.interval_us, 0);
    assert_eq!(entry.priority, 0);
}

#[spec_test(REQ_EM_codegen_001)]
fn compile_when_program_instance_has_no_task_then_implicit_freewheeling_task() {
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
    let container = compile_source(&source).unwrap();

    let entry = &container.task_table.tasks[0];
    assert_eq!(entry.task_type, TaskType::Freewheeling);
    assert_eq!(entry.interval_us, 0);
    assert_eq!(entry.priority, 0);
}

#[spec_test(REQ_EM_codegen_001)]
fn compile_when_unused_task_is_event_then_compiles_instance_task() {
    let source = format!(
        "{MAIN}
CONFIGURATION config
  VAR_GLOBAL
    Trigger : BOOL;
  END_VAR
  RESOURCE resource1 ON PLC
    TASK unused(SINGLE := Trigger, PRIORITY := 100000);
    TASK task1(INTERVAL := T#10ms, PRIORITY := 2);
    PROGRAM instance1 WITH task1 : main;
  END_RESOURCE
END_CONFIGURATION
"
    );
    let container = compile_source(&source).unwrap();

    let entry = &container.task_table.tasks[0];
    assert_eq!(entry.task_type, TaskType::Cyclic);
    assert_eq!(entry.interval_us, 10_000);
    assert_eq!(entry.priority, 2);
}

#[spec_test(REQ_EM_codegen_001)]
fn task_schedule_when_cyclic_interval_is_under_a_microsecond_then_freewheeling() {
    let schedule = task_schedule(&task(
        1,
        Schedule::Cyclic {
            interval: Duration::nanoseconds(500),
        },
    ))
    .unwrap();

    assert_eq!(schedule.task_type, TaskType::Freewheeling);
    assert_eq!(schedule.interval_us, 0);
}

#[spec_test(REQ_EM_codegen_002)]
fn compile_when_two_program_instances_then_p9999() {
    let source = format!(
        "{MAIN}
CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK task1(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM first WITH task1 : main;
    PROGRAM second WITH task1 : main;
  END_RESOURCE
END_CONFIGURATION
"
    );
    let diagnostic = compile_source(&source).unwrap_err();

    assert_eq!(diagnostic.code, NOT_IMPLEMENTED_CODE);
    assert!(diagnostic.help.iter().any(|help| help.contains("1613")));
}

#[spec_test(REQ_EM_codegen_002)]
fn compile_when_instances_on_two_tasks_then_p9999_counts_them_in_source_order() {
    let source = format!(
        "{MAIN}
CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK slow(INTERVAL := T#100ms, PRIORITY := 2);
    TASK fast(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM first WITH fast : main;
    PROGRAM second WITH slow : main;
  END_RESOURCE
END_CONFIGURATION
"
    );
    let diagnostic = compile_source(&source).unwrap_err();

    assert_eq!(diagnostic.code, NOT_IMPLEMENTED_CODE);
    assert_eq!(
        diagnostic.primary.location.start,
        source.find("second").unwrap()
    );
    assert_eq!(diagnostic.secondary.len(), 1);
    assert_eq!(
        diagnostic.secondary[0].location.start,
        source.find("first").unwrap()
    );
}

#[spec_test(REQ_EM_codegen_002)]
fn compile_when_configuration_binds_one_of_two_programs_then_p9999() {
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

    assert_eq!(error_code(&source), NOT_IMPLEMENTED_CODE);
}

#[spec_test(REQ_EM_codegen_003)]
fn compile_when_task_has_single_then_p4047() {
    let source = program_with_task("SINGLE := Trigger, PRIORITY := 1");

    assert_eq!(error_code(&source), Problem::TaskSingleNotSupported.code());
}

#[spec_test(REQ_EM_codegen_003)]
fn task_schedule_when_event_with_out_of_range_priority_then_p4047() {
    let event = task(
        100_000,
        Schedule::Event {
            trigger: Trigger::Constant,
            interval: None,
        },
    );

    let diagnostic = task_schedule(&event).unwrap_err();

    assert_eq!(diagnostic.code, Problem::TaskSingleNotSupported.code());
}

#[spec_test(REQ_EM_codegen_004)]
fn compile_when_task_priority_exceeds_u16_then_p4048_at_task_name() {
    let source = program_with_task("INTERVAL := T#100ms, PRIORITY := 100000");

    let diagnostic = compile_source(&source).unwrap_err();

    assert_eq!(diagnostic.code, Problem::TaskParameterOutOfRange.code());
    assert_eq!(
        diagnostic.primary.location.start,
        source.find("task1(").unwrap()
    );
}

#[spec_test(REQ_EM_codegen_004)]
fn task_schedule_when_interval_exceeds_u64_microseconds_then_p4048_at_task_name() {
    let huge = task(
        1,
        Schedule::Cyclic {
            interval: Duration::MAX,
        },
    );

    let diagnostic = task_schedule(&huge).unwrap_err();

    assert_eq!(diagnostic.code, Problem::TaskParameterOutOfRange.code());
    assert_eq!(diagnostic.primary.location.start, 3);
    assert_eq!(diagnostic.primary.location.end, 8);
}

#[spec_test(REQ_EM_codegen_004)]
fn task_schedule_when_priority_is_u16_max_then_fits() {
    let largest = task(
        u32::from(u16::MAX),
        Schedule::Cyclic {
            interval: Duration::seconds(1),
        },
    );

    let schedule = task_schedule(&largest).unwrap();

    assert_eq!(schedule.priority, u16::MAX);
    assert_eq!(schedule.interval_us, 1_000_000);
    assert_eq!(schedule.task_type, TaskType::Cyclic);
}

#[spec_test(REQ_EM_codegen_005)]
fn compile_when_no_program_then_p4020() {
    let source = "
FUNCTION_BLOCK MyBlock
  VAR
    x : INT;
  END_VAR
END_FUNCTION_BLOCK
";

    assert_eq!(error_code(source), Problem::NoProgramDeclaration.code());
}

#[spec_test(REQ_EM_codegen_005)]
fn compile_when_two_programs_and_no_configuration_then_p9999() {
    let source = format!(
        "{MAIN}
PROGRAM helper
  VAR y : INT; END_VAR
  y := 2;
END_PROGRAM
"
    );
    let diagnostic = compile_source(&source).unwrap_err();

    assert_eq!(diagnostic.code, NOT_IMPLEMENTED_CODE);
    assert!(diagnostic.help.iter().any(|help| help.contains("1613")));
}

#[spec_test(REQ_EM_codegen_005)]
fn compile_when_two_configurations_then_p4081_on_second() {
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
    let diagnostic = compile_source(&source).unwrap_err();

    assert_eq!(diagnostic.code, Problem::ConfigurationAmbiguous.code());
    assert_eq!(
        diagnostic.primary.location.start,
        source.find("cfgB").unwrap()
    );
    assert_eq!(diagnostic.secondary.len(), 1);
    assert_eq!(
        diagnostic.secondary[0].location.start,
        source.find("cfgA").unwrap()
    );
}

#[spec_test(REQ_EM_codegen_005)]
fn compile_when_instance_type_is_function_block_then_p4080_at_type_name() {
    let source = format!(
        "{MAIN}
FUNCTION_BLOCK counter
  VAR n : INT; END_VAR
  n := n + 1;
END_FUNCTION_BLOCK
CONFIGURATION config
  RESOURCE resource1 ON PLC
    TASK task1(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM instance1 WITH task1 : counter;
  END_RESOURCE
END_CONFIGURATION
"
    );
    let diagnostic = compile_source(&source).unwrap_err();

    assert_eq!(
        diagnostic.code,
        Problem::ProgramInstanceTypeNotProgram.code()
    );
    assert_eq!(
        diagnostic.primary.location.start,
        source.find(": counter").unwrap() + 2
    );
}

#[spec_test(REQ_EM_codegen_006)]
fn compile_when_model_has_system_globals_then_uptime_flag_set() {
    let options = CompilerOptions {
        allow_system_uptime_global: true,
        ..CompilerOptions::default()
    };
    let container = compile_with(MAIN, &options).unwrap();

    assert_ne!(container.header.flags & FLAG_HAS_SYSTEM_UPTIME, 0);
    assert_eq!(container.header.num_variables, 3);
}

#[spec_test(REQ_EM_codegen_006)]
fn compile_when_model_has_no_system_globals_then_uptime_flag_clear() {
    let container = compile_source(MAIN).unwrap();

    assert_eq!(container.header.flags & FLAG_HAS_SYSTEM_UPTIME, 0);
    assert_eq!(container.header.num_variables, 1);
}

#[spec_test(REQ_EM_codegen_007)]
fn compile_when_globals_in_every_scope_then_resource_globals_get_no_storage() {
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
        ..CompilerOptions::default()
    };
    let container = compile_with(&source, &options).unwrap();

    // `top`, `shared` and `main`'s `x`.
    assert_eq!(container.header.num_variables, 3);
}

/// The source files of this crate, with their paths.
fn codegen_sources() -> Vec<(std::path::PathBuf, String)> {
    let mut files = Vec::new();
    let mut directories = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                directories.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = std::fs::read_to_string(&path).unwrap();
                files.push((path, source));
            }
        }
    }
    files
}

#[spec_test(REQ_EM_codegen_007)]
fn codegen_sources_when_scanned_then_read_no_configuration_declaration() {
    // The configuration's declarations, and the library elements a backend
    // would search for a program or a global, are named nowhere in codegen:
    // it reaches a declaration only through `CleanAnalysis`. The words are
    // split so that this file does not name them either.
    let forbidden = [
        ["ironplc_dsl::", "configuration"].concat(),
        ["Configuration", "Declaration"].concat(),
        ["Resource", "Declaration"].concat(),
        ["Task", "Configuration"].concat(),
        ["Program", "Configuration"].concat(),
        ["LibraryElementKind::", "ProgramDeclaration"].concat(),
        ["LibraryElementKind::", "GlobalVarDeclarations"].concat(),
    ];

    let found: Vec<String> = codegen_sources()
        .iter()
        .flat_map(|(path, source)| {
            forbidden
                .iter()
                .filter(|word| source.contains(word.as_str()))
                .map(move |word| format!("{}: {word}", path.display()))
        })
        .collect();
    assert_eq!(found, Vec::<String>::new());
}

#[spec_test(REQ_EM_codegen_007)]
fn codegen_sources_when_scanned_then_declarations_reached_only_by_id() {
    let sources = codegen_sources();
    let calls = |method: &str| {
        sources
            .iter()
            .filter(|(path, _)| !path.ends_with("execution/tests.rs"))
            .map(|(_, source)| source.matches(method).count())
            .sum::<usize>()
    };

    assert_eq!(calls(".program_declaration("), 1);
    assert_eq!(calls(".global_declaration("), 1);
}

#[test]
fn ordinal_when_teens_then_th() {
    let ordinals: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 111]
        .into_iter()
        .map(ordinal)
        .collect();

    assert_eq!(
        ordinals,
        ["1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "23rd", "111th"]
    );
}
