use ironplc_container::{Container, FunctionId};

use crate::frame_stack::Frame;
use crate::scheduler::{ProgramInstanceState, TaskState};
use crate::value::Slot;
use crate::variable_table::VariableScope;
use crate::vm::MAX_CALL_DEPTH;

/// Heap-allocated buffers that back a VM instance.
///
/// Every VM execution needs a set of mutable buffers for the stack, variables,
/// data region, temporary storage, task states, program instances, a
/// ready-list, and a call-frame stack. This struct bundles them together so
/// callers do not need to repeat the allocation boilerplate.
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
    pub frames: Vec<Frame>,
}

impl VmBuffers {
    /// Allocate buffers sized for the given container.
    pub fn from_container(container: &Container) -> Self {
        let h = &container.header;
        let task_count = container.task_table.tasks.len();
        let program_count = container.task_table.programs.len();
        let temp_buf_total = h.num_temp_bufs as usize * h.max_temp_buf_bytes as usize;
        let zero_frame = Frame {
            function_id: FunctionId::new(0),
            pc: 0,
            scope: VariableScope {
                shared_globals_size: 0,
                instance_offset: 0,
                instance_count: 0,
            },
            temp_alloc_mark: 0,
            fb_return: None,
        };
        VmBuffers {
            stack: vec![Slot::default(); h.max_stack_depth as usize],
            vars: vec![Slot::default(); h.num_variables as usize],
            data_region: vec![0u8; h.data_region_bytes as usize],
            temp_buf: vec![0u8; temp_buf_total],
            tasks: vec![TaskState::default(); task_count],
            programs: vec![ProgramInstanceState::default(); program_count],
            ready: vec![0usize; task_count.max(1)],
            frames: vec![zero_frame; MAX_CALL_DEPTH as usize],
        }
    }
}
