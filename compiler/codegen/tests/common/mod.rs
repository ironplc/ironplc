//! Shared test helpers for codegen integration tests.

#![allow(dead_code)]

use ironplc_codegen::compile;
use ironplc_container::Container;
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::ParseOptions;
use ironplc_parser::parse_program;
use ironplc_vm::{FaultContext, ProgramInstanceState, Slot, TaskState, Vm};

/// Helper struct that allocates Vec-backed buffers for VM usage.
#[derive(Debug)]
pub struct VmBuffers {
    pub stack: Vec<Slot>,
    pub vars: Vec<Slot>,
    pub tasks: Vec<TaskState>,
    pub programs: Vec<ProgramInstanceState>,
    pub ready: Vec<usize>,
}

impl VmBuffers {
    pub fn from_container(c: &Container) -> Self {
        let h = &c.header;
        let task_count = c.task_table.tasks.len();
        let program_count = c.task_table.programs.len();
        VmBuffers {
            stack: vec![Slot::default(); h.max_stack_depth as usize],
            vars: vec![Slot::default(); h.num_variables as usize],
            tasks: vec![TaskState::default(); task_count],
            programs: vec![ProgramInstanceState::default(); program_count],
            ready: vec![0usize; task_count.max(1)],
        }
    }
}

/// Parses an IEC 61131-3 source string and runs type resolution via the analyzer.
///
/// The analyzer populates `Expr.resolved_type` and resolves type aliases in
/// variable declarations, which codegen requires.
pub fn parse(source: &str) -> Library {
    let library = parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap();
    let (analyzed, _ctx) = ironplc_analyzer::stages::resolve_types(&[&library]).unwrap();
    analyzed
}

/// Parses, analyzes, compiles, and runs one scan cycle.
/// Returns the container and buffers so callers can inspect variable values.
pub fn parse_and_run(source: &str) -> (Container, VmBuffers) {
    let (container, bufs) = parse_and_try_run(source).expect("VM execution trapped unexpectedly");
    (container, bufs)
}

/// Parses, analyzes, compiles, and runs one scan cycle, returning `Err` on VM trap.
/// Use this to test that certain programs produce runtime traps.
pub fn parse_and_try_run(source: &str) -> Result<(Container, VmBuffers), FaultContext> {
    let library = parse(source);
    let container = compile(&library).unwrap();
    let mut bufs = VmBuffers::from_container(&container);
    {
        let mut vm = Vm::new()
            .load(
                &container,
                &mut bufs.stack,
                &mut bufs.vars,
                &mut bufs.tasks,
                &mut bufs.programs,
                &mut bufs.ready,
            )
            .start()?;
        vm.run_round(0)?;
    }
    Ok((container, bufs))
}
