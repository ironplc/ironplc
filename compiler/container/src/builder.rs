use std::collections::HashMap;
use std::vec;
use std::vec::Vec;

use crate::code_section::{CodeSection, FuncEntry};
use crate::const_type::ConstType;
use crate::constant_pool::{ConstEntry, ConstantPool};
use crate::container::Container;
use crate::debug_section::{DebugSection, FuncNameEntry, VarNameEntry};
use crate::header::FileHeader;
use crate::id_types::{FunctionId, InstanceId, TaskId, VarIndex};
use crate::task_table::{ProgramInstanceEntry, TaskEntry, TaskTable};
use crate::task_type::TaskType;
use crate::type_section::{ArrayDescriptor, FbTypeDescriptor, TypeSection, UserFbDescriptor};

/// Fluent builder for constructing a [`Container`].
pub struct ContainerBuilder {
    num_variables: u16,
    max_stack_depth: u16,
    data_region_bytes: u32,
    num_temp_bufs: u16,
    max_temp_buf_bytes: u32,
    constant_pool: ConstantPool,
    functions: Vec<FuncEntry>,
    bytecode: Vec<u8>,
    tasks: Vec<TaskEntry>,
    programs: Vec<ProgramInstanceEntry>,
    shared_globals_size: u16,
    init_function_id: FunctionId,
    entry_function_id: FunctionId,
    fb_types: Vec<FbTypeDescriptor>,
    array_descriptors: Vec<ArrayDescriptor>,
    array_descriptor_cache: HashMap<(u8, u32, u16), u16>,
    user_fb_types: Vec<UserFbDescriptor>,
    debug_var_names: Vec<VarNameEntry>,
    debug_func_names: Vec<FuncNameEntry>,
}

impl ContainerBuilder {
    pub fn new() -> Self {
        ContainerBuilder {
            num_variables: 0,
            max_stack_depth: 0,
            data_region_bytes: 0,
            num_temp_bufs: 0,
            max_temp_buf_bytes: 0,
            constant_pool: ConstantPool::default(),
            functions: Vec::new(),
            bytecode: Vec::new(),
            tasks: Vec::new(),
            programs: Vec::new(),
            shared_globals_size: 0,
            init_function_id: FunctionId::INIT,
            entry_function_id: FunctionId::INIT,
            fb_types: Vec::new(),
            array_descriptors: Vec::new(),
            array_descriptor_cache: HashMap::new(),
            user_fb_types: Vec::new(),
            debug_var_names: Vec::new(),
            debug_func_names: Vec::new(),
        }
    }

    /// Sets the total number of variable table entries.
    pub fn num_variables(mut self, n: u16) -> Self {
        self.num_variables = n;
        self
    }

    /// Sets the total size of the unified data region in bytes.
    pub fn data_region_bytes(mut self, n: u32) -> Self {
        self.data_region_bytes = n;
        self
    }

    /// Sets the number of temporary buffers for string operations.
    pub fn num_temp_bufs(mut self, n: u16) -> Self {
        self.num_temp_bufs = n;
        self
    }

    /// Sets the size of the largest temporary buffer in bytes.
    pub fn max_temp_buf_bytes(mut self, n: u32) -> Self {
        self.max_temp_buf_bytes = n;
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

    /// Adds a string constant (Latin-1 bytes) to the constant pool.
    pub fn add_str_constant(mut self, value: &[u8]) -> Self {
        self.constant_pool.push(ConstEntry {
            const_type: ConstType::Str,
            value: value.to_vec(),
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
        function_id: FunctionId,
        bytecode: &[u8],
        max_stack_depth: u16,
        num_locals: u16,
        num_params: u16,
    ) -> Self {
        let offset = self.bytecode.len() as u32;
        self.functions.push(FuncEntry {
            function_id,
            bytecode_offset: offset,
            bytecode_length: bytecode.len() as u32,
            max_stack_depth,
            num_locals,
            num_params,
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

    /// Sets the init function ID for the default synthesized program instance.
    pub fn init_function_id(mut self, id: FunctionId) -> Self {
        self.init_function_id = id;
        self
    }

    /// Sets the entry (scan) function ID for the default synthesized program instance.
    pub fn entry_function_id(mut self, id: FunctionId) -> Self {
        self.entry_function_id = id;
        self
    }

    /// Adds a variable name entry to the debug section.
    pub fn add_var_name(mut self, entry: VarNameEntry) -> Self {
        self.debug_var_names.push(entry);
        self
    }

    /// Adds a function name entry to the debug section.
    pub fn add_func_name(mut self, entry: FuncNameEntry) -> Self {
        self.debug_func_names.push(entry);
        self
    }

    /// Adds an FB type descriptor to the type section.
    pub fn add_fb_type(mut self, desc: FbTypeDescriptor) -> Self {
        self.fb_types.push(desc);
        self
    }

    /// Adds a user-defined FB descriptor to the type section.
    pub fn add_user_fb_type(mut self, desc: UserFbDescriptor) -> Self {
        self.user_fb_types.push(desc);
        self
    }

    /// Adds an array descriptor to the type section, deduplicating
    /// identical `(element_type, total_elements, element_extra)` triples.
    ///
    /// Returns the descriptor index (for use in `LOAD_ARRAY`/`STORE_ARRAY` opcodes).
    /// For STRING arrays, `element_extra` holds the max string length.
    pub fn add_array_descriptor(
        &mut self,
        element_type: u8,
        total_elements: u32,
        element_extra: u16,
    ) -> u16 {
        let key = (element_type, total_elements, element_extra);
        if let Some(&index) = self.array_descriptor_cache.get(&key) {
            return index;
        }
        let index = self.array_descriptors.len() as u16;
        self.array_descriptors.push(ArrayDescriptor {
            element_type,
            total_elements,
            element_extra,
        });
        self.array_descriptor_cache.insert(key, index);
        index
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

        // Build type section if there are any type descriptors.
        let type_section = if !self.fb_types.is_empty()
            || !self.array_descriptors.is_empty()
            || !self.user_fb_types.is_empty()
        {
            Some(TypeSection {
                fb_types: self.fb_types,
                array_descriptors: self.array_descriptors,
                user_fb_types: self.user_fb_types,
            })
        } else {
            None
        };

        let task_table = if self.tasks.is_empty() {
            // Synthesize a default freewheeling task and program instance
            let default_task = TaskEntry {
                task_id: TaskId::DEFAULT,
                priority: 0,
                task_type: TaskType::Freewheeling,
                flags: 0x01, // enabled
                interval_us: 0,
                single_var_index: VarIndex::NO_SINGLE_VAR,
                watchdog_us: 0,
                input_image_offset: 0,
                output_image_offset: 0,
                reserved: [0; 4],
            };
            let default_program = ProgramInstanceEntry {
                instance_id: InstanceId::DEFAULT,
                task_id: TaskId::DEFAULT,
                entry_function_id: self.entry_function_id,
                var_table_offset: 0,
                var_table_count: self.num_variables,
                fb_instance_offset: 0,
                fb_instance_count: 0,
                init_function_id: self.init_function_id,
            };
            TaskTable {
                shared_globals_size: self.shared_globals_size,
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

        let debug_section = if self.debug_var_names.is_empty() && self.debug_func_names.is_empty() {
            None
        } else {
            Some(DebugSection {
                var_names: self.debug_var_names,
                func_names: self.debug_func_names,
            })
        };

        let header = FileHeader {
            num_variables: self.num_variables,
            max_stack_depth: self.max_stack_depth,
            data_region_bytes: self.data_region_bytes,
            num_temp_bufs: self.num_temp_bufs,
            max_temp_buf_bytes: self.max_temp_buf_bytes,
            num_functions: code.functions.len() as u16,
            ..FileHeader::default()
        };

        Container {
            header,
            task_table,
            type_section,
            constant_pool,
            code,
            debug_section,
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
    use crate::id_types::ConstantIndex;
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
            .add_function(FunctionId::INIT, &bytecode, 2, 2, 0)
            .build();

        assert_eq!(container.header.num_variables, 2);
        assert_eq!(container.header.max_stack_depth, 2);
        assert_eq!(container.header.num_functions, 1);

        // Verify synthesized default task table
        assert_eq!(container.task_table.shared_globals_size, 0);
        assert_eq!(container.task_table.tasks.len(), 1);
        assert_eq!(container.task_table.tasks[0].task_id, TaskId::DEFAULT);
        assert_eq!(container.task_table.tasks[0].priority, 0);
        assert_eq!(
            container.task_table.tasks[0].task_type,
            TaskType::Freewheeling
        );
        assert_eq!(container.task_table.tasks[0].flags, 0x01);
        assert_eq!(container.task_table.tasks[0].interval_us, 0);
        assert_eq!(
            container.task_table.tasks[0].single_var_index,
            VarIndex::NO_SINGLE_VAR
        );
        assert_eq!(container.task_table.tasks[0].watchdog_us, 0);
        assert_eq!(container.task_table.programs.len(), 1);
        assert_eq!(
            container.task_table.programs[0].instance_id,
            InstanceId::DEFAULT
        );
        assert_eq!(container.task_table.programs[0].task_id, TaskId::DEFAULT);
        assert_eq!(
            container.task_table.programs[0].entry_function_id,
            FunctionId::INIT
        );
        assert_eq!(container.task_table.programs[0].var_table_offset, 0);
        assert_eq!(container.task_table.programs[0].var_table_count, 2);
        assert_eq!(container.task_table.programs[0].fb_instance_offset, 0);
        assert_eq!(container.task_table.programs[0].fb_instance_count, 0);

        assert_eq!(
            container
                .constant_pool
                .get_i32(ConstantIndex::new(0))
                .unwrap(),
            10
        );
        assert_eq!(
            container
                .constant_pool
                .get_i32(ConstantIndex::new(1))
                .unwrap(),
            32
        );
        assert_eq!(container.code.functions.len(), 1);
        assert_eq!(
            container
                .code
                .get_function_bytecode(FunctionId::INIT)
                .unwrap(),
            bytecode.as_slice()
        );

        // No type section when no FB types or array descriptors added
        assert!(container.type_section.is_none());
    }

    #[test]
    fn builder_add_array_descriptor_when_unique_then_assigns_sequential_indices() {
        let mut builder = ContainerBuilder::new();
        let idx0 = builder.add_array_descriptor(0, 10, 0); // I32, 10 elements
        let idx1 = builder.add_array_descriptor(5, 100, 0); // F64, 100 elements
        assert_eq!(idx0, 0);
        assert_eq!(idx1, 1);

        let container = builder.num_variables(0).build();
        let ts = container.type_section.unwrap();
        assert_eq!(ts.array_descriptors.len(), 2);
        assert_eq!(ts.array_descriptors[0].element_type, 0);
        assert_eq!(ts.array_descriptors[0].total_elements, 10);
        assert_eq!(ts.array_descriptors[1].element_type, 5);
        assert_eq!(ts.array_descriptors[1].total_elements, 100);
    }

    #[test]
    fn builder_add_array_descriptor_when_duplicate_then_returns_same_index() {
        let mut builder = ContainerBuilder::new();
        let idx0 = builder.add_array_descriptor(0, 10, 0);
        let idx1 = builder.add_array_descriptor(0, 10, 0); // identical
        let idx2 = builder.add_array_descriptor(0, 20, 0); // different size
        assert_eq!(idx0, 0);
        assert_eq!(idx1, 0); // deduplicated
        assert_eq!(idx2, 1); // new

        let container = builder.num_variables(0).build();
        let ts = container.type_section.unwrap();
        assert_eq!(ts.array_descriptors.len(), 2);
    }
}
