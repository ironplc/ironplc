use ironplc_container::Container;

use crate::scheduler::{ProgramInstanceState, TaskState};
use crate::value::Slot;

/// Heap-allocated buffers that back a VM instance.
///
/// Every VM execution needs a set of mutable buffers for the stack, variables,
/// data region, temporary storage, task states, program instances, and a
/// ready-list. This struct bundles them together so callers do not need to
/// repeat the allocation boilerplate.
///
/// Construct with [`VmBuffers::from_container`], which sizes each buffer
/// according to the [`Container`] header and task table.
#[derive(Debug)]
pub struct VmBuffers {
    pub stack: Vec<Slot>,
    pub vars: Vec<Slot>,
    pub data_region: Vec<u8>,
    pub temp_buf: Vec<u8>,
    pub tasks: Vec<TaskState>,
    pub programs: Vec<ProgramInstanceState>,
    pub ready: Vec<usize>,
}

impl VmBuffers {
    /// Allocate buffers sized for the given container.
    pub fn from_container(container: &Container) -> Self {
        let h = &container.header;
        let task_count = container.task_table.tasks.len();
        let program_count = container.task_table.programs.len();
        let temp_buf_total = h.num_temp_bufs as usize * h.max_temp_buf_bytes as usize;
        VmBuffers {
            stack: vec![Slot::default(); h.max_stack_depth as usize],
            vars: vec![Slot::default(); h.num_variables as usize],
            data_region: vec![0u8; h.data_region_bytes as usize],
            temp_buf: vec![0u8; temp_buf_total],
            tasks: vec![TaskState::default(); task_count],
            programs: vec![ProgramInstanceState::default(); program_count],
            ready: vec![0usize; task_count.max(1)],
        }
    }
}
