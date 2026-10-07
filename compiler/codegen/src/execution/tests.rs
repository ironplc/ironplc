//! Tests that codegen lowers the execution model, and checks it against what
//! the VM supports. What the model holds is the analyzer's to test
//! (`ironplc_analyzer::execution_model`); these tests only check that the
//! container follows it.

use ironplc_analyzer::execution_model::{Task, TaskInterval, TaskKind};
use ironplc_analyzer::CleanAnalysis;
use ironplc_container::{Container, TaskType, FLAG_HAS_SYSTEM_UPTIME};
use ironplc_dsl::core::{FileId, Id, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::CompilerOptions;
use ironplc_parser::parse_program;
use ironplc_problems::Problem;
use spec_test_macro::spec_test;
use time::Duration;

use super::task_schedule;
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

/// A declared task of the given kind and parameters, as the model holds it.
fn task(kind: TaskKind, priority: u32, interval: Option<Duration>) -> Task {
    Task {
        name: Some(Id::from("task1")),
        priority,
        interval: interval.map(|duration| TaskInterval {
            duration,
            span: SourceSpan::default(),
        }),
        single: None,
        kind,
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

#[spec_test(REQ_EM_codegen_004)]
fn compile_when_task_priority_exceeds_u16_then_p4048() {
    let source = program_with_task("INTERVAL := T#100ms, PRIORITY := 100000");

    assert_eq!(error_code(&source), Problem::TaskParameterOutOfRange.code());
}

#[spec_test(REQ_EM_codegen_004)]
fn task_schedule_when_interval_exceeds_u64_microseconds_then_p4048() {
    let huge = task(TaskKind::Cyclic, 1, Some(Duration::MAX));

    let diagnostic = task_schedule(&huge).unwrap_err();

    assert_eq!(diagnostic.code, Problem::TaskParameterOutOfRange.code());
}

#[spec_test(REQ_EM_codegen_004)]
fn task_schedule_when_priority_is_u16_max_then_fits() {
    let largest = task(
        TaskKind::Cyclic,
        u32::from(u16::MAX),
        Some(Duration::seconds(1)),
    );

    let schedule = task_schedule(&largest).unwrap();

    assert_eq!(schedule.priority, u16::MAX);
    assert_eq!(schedule.interval_us, 1_000_000);
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

    assert_eq!(error_code(&source), NOT_IMPLEMENTED_CODE);
}

#[spec_test(REQ_EM_codegen_005)]
fn compile_when_two_configurations_then_p4076_on_second() {
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

#[spec_test(REQ_EM_codegen_005)]
fn compile_when_instance_type_is_function_block_then_p4075() {
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

    assert_eq!(
        error_code(&source),
        Problem::ProgramInstanceTypeNotProgram.code()
    );
}
