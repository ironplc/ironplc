use std::vec;
use std::vec::Vec;

use crate::code_section::{CodeSection, FuncEntry};
use crate::const_type::ConstType;
use crate::constant_pool::{ConstEntry, ConstantPool};
use crate::container::Container;
use crate::header::FileHeader;
use crate::task_table::{ProgramInstanceEntry, TaskEntry, TaskTable};
use crate::task_type::TaskType;

/// Fluent builder for constructing a [`Container`] in tests.
pub struct ContainerBuilder {
    num_variables: u16,
    max_stack_depth: u16,
    constant_pool: ConstantPool,
    functions: Vec<FuncEntry>,
    bytecode: Vec<u8>,
    tasks: Vec<TaskEntry>,
    programs: Vec<ProgramInstanceEntry>,
    shared_globals_size: u16,
}

impl ContainerBuilder {
    pub fn new() -> Self {
        ContainerBuilder {
            num_variables: 0,
            max_stack_depth: 0,
            constant_pool: ConstantPool::default(),
            functions: Vec::new(),
            bytecode: Vec::new(),
            tasks: Vec::new(),
            programs: Vec::new(),
            shared_globals_size: 0,
        }
    }

    /// Sets the total number of variable table entries.
    pub fn num_variables(mut self, n: u16) -> Self {
        self.num_variables = n;
        self
    }

    /// Adds an i32 constant to the constant pool.
    pub fn add_i32_constant(mut self, value: i32) -> Self {
        self.constant_pool.push(ConstEntry {
            const_type: ConstType::I32,
            value: value.to_le_bytes().to_vec(),
        });
        self
    }

    /// Adds an f32 constant to the constant pool.
    pub fn add_f32_constant(mut self, value: f32) -> Self {
        self.constant_pool.push(ConstEntry {
            const_type: ConstType::F32,
            value: value.to_le_bytes().to_vec(),
        });
        self
    }

    /// Adds an f64 constant to the constant pool.
    pub fn add_f64_constant(mut self, value: f64) -> Self {
        self.constant_pool.push(ConstEntry {
            const_type: ConstType::F64,
            value: value.to_le_bytes().to_vec(),
        });
        self
    }

    /// Adds an i64 constant to the constant pool.
    pub fn add_i64_constant(mut self, value: i64) -> Self {
        self.constant_pool.push(ConstEntry {
            const_type: ConstType::I64,
            value: value.to_le_bytes().to_vec(),
        });
        self
    }

    /// Adds a function with the given bytecode.
    pub fn add_function(
        mut self,
        function_id: u16,
        bytecode: &[u8],
        max_stack_depth: u16,
        num_locals: u16,
    ) -> Self {
        let offset = self.bytecode.len() as u32;
        self.functions.push(FuncEntry {
            function_id,
            bytecode_offset: offset,
            bytecode_length: bytecode.len() as u32,
            max_stack_depth,
            num_locals,
        });
        self.bytecode.extend_from_slice(bytecode);

        if max_stack_depth > self.max_stack_depth {
            self.max_stack_depth = max_stack_depth;
        }
        self
    }

    /// Adds a task entry to the task table.
    pub fn add_task(mut self, task: TaskEntry) -> Self {
        self.tasks.push(task);
        self
    }

    /// Adds a program instance entry to the task table.
    pub fn add_program_instance(mut self, program: ProgramInstanceEntry) -> Self {
        self.programs.push(program);
        self
    }

    /// Sets the shared globals size in the task table.
    pub fn shared_globals_size(mut self, size: u16) -> Self {
        self.shared_globals_size = size;
        self
    }

    /// Builds the container, computing header fields from the added data.
    ///
    /// If no tasks have been added, synthesizes a default freewheeling task
    /// with a single program instance that covers all variables.
    pub fn build(self) -> Container {
        let constant_pool = self.constant_pool;
        let code = CodeSection {
            functions: self.functions,
            bytecode: self.bytecode,
        };

        let task_table = if self.tasks.is_empty() {
            // Synthesize a default freewheeling task and program instance
            let default_task = TaskEntry {
                task_id: 0,
                priority: 0,
                task_type: TaskType::Freewheeling,
                flags: 0x01, // enabled
                interval_us: 0,
                single_var_index: 0xFFFF,
                watchdog_us: 0,
                input_image_offset: 0,
                output_image_offset: 0,
                reserved: [0; 4],
            };
            let default_program = ProgramInstanceEntry {
                instance_id: 0,
                task_id: 0,
                entry_function_id: 0,
                var_table_offset: 0,
                var_table_count: self.num_variables,
                fb_instance_offset: 0,
                fb_instance_count: 0,
                reserved: 0,
            };
            TaskTable {
                shared_globals_size: 0,
                tasks: vec![default_task],
                programs: vec![default_program],
            }
        } else {
            TaskTable {
                shared_globals_size: self.shared_globals_size,
                tasks: self.tasks,
                programs: self.programs,
            }
        };

        let header = FileHeader {
            num_variables: self.num_variables,
            max_stack_depth: self.max_stack_depth,
            num_functions: code.functions.len() as u16,
            ..FileHeader::default()
        };

        Container {
            header,
            task_table,
            constant_pool,
            code,
        }
    }
}

impl Default for ContainerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use std::vec::Vec;

    #[test]
    fn builder_when_steel_thread_program_then_builds_valid_container() {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]
            0x18, 0x00, 0x00,       // STORE_VAR_I32  var[0]
            0x10, 0x00, 0x00,       // LOAD_VAR_I32   var[0]
            0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]
            0x30,                   // ADD_I32
            0x18, 0x01, 0x00,       // STORE_VAR_I32  var[1]
            0xB5,                   // RET_VOID
        ];

        let container = ContainerBuilder::new()
            .num_variables(2)
            .add_i32_constant(10)
            .add_i32_constant(32)
            .add_function(0, &bytecode, 2, 2)
            .build();

        assert_eq!(container.header.num_variables, 2);
        assert_eq!(container.header.max_stack_depth, 2);
        assert_eq!(container.header.num_functions, 1);

        // Verify synthesized default task table
        assert_eq!(container.task_table.shared_globals_size, 0);
        assert_eq!(container.task_table.tasks.len(), 1);
        assert_eq!(container.task_table.tasks[0].task_id, 0);
        assert_eq!(container.task_table.tasks[0].priority, 0);
        assert_eq!(
            container.task_table.tasks[0].task_type,
            TaskType::Freewheeling
        );
        assert_eq!(container.task_table.tasks[0].flags, 0x01);
        assert_eq!(container.task_table.tasks[0].interval_us, 0);
        assert_eq!(container.task_table.tasks[0].single_var_index, 0xFFFF);
        assert_eq!(container.task_table.tasks[0].watchdog_us, 0);
        assert_eq!(container.task_table.programs.len(), 1);
        assert_eq!(container.task_table.programs[0].instance_id, 0);
        assert_eq!(container.task_table.programs[0].task_id, 0);
        assert_eq!(container.task_table.programs[0].entry_function_id, 0);
        assert_eq!(container.task_table.programs[0].var_table_offset, 0);
        assert_eq!(container.task_table.programs[0].var_table_count, 2);
        assert_eq!(container.task_table.programs[0].fb_instance_offset, 0);
        assert_eq!(container.task_table.programs[0].fb_instance_count, 0);

        assert_eq!(container.constant_pool.get_i32(0).unwrap(), 10);
        assert_eq!(container.constant_pool.get_i32(1).unwrap(), 32);
        assert_eq!(container.code.functions.len(), 1);
        assert_eq!(
            container.code.get_function_bytecode(0).unwrap(),
            bytecode.as_slice()
        );
    }
}
