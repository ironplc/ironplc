//! Shared test helpers for codegen integration tests.

use ironplc_codegen::compile;
use ironplc_container::Container;
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_parser::options::ParseOptions;
use ironplc_parser::parse_program;
use ironplc_vm::{ProgramInstanceState, Slot, TaskState, Vm};

/// Helper struct that allocates Vec-backed buffers for VM usage.
#[allow(dead_code)]
pub struct VmBuffers {
    pub stack: Vec<Slot>,
    pub vars: Vec<Slot>,
    pub tasks: Vec<TaskState>,
    pub programs: Vec<ProgramInstanceState>,
    pub ready: Vec<usize>,
}

impl VmBuffers {
    #[allow(dead_code)]
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

/// Parses an IEC 61131-3 source string into a Library.
#[allow(dead_code)]
pub fn parse(source: &str) -> Library {
    parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap()
}

/// Parses an IEC 61131-3 program, compiles it, and runs one scan cycle.
/// Returns the container and buffers so callers can inspect variable values.
#[allow(dead_code)]
pub fn parse_and_run(source: &str) -> (Container, VmBuffers) {
    let library = parse_program(source, &FileId::default(), &ParseOptions::default()).unwrap();
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
            .start();
        vm.run_round(0).unwrap();
    }
    (container, bufs)
}
