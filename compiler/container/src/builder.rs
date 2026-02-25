use crate::code_section::{CodeSection, FuncEntry};
use crate::constant_pool::{ConstEntry, ConstType, ConstantPool};
use crate::container::Container;
use crate::header::FileHeader;

/// Fluent builder for constructing a [`Container`] in tests.
pub struct ContainerBuilder {
    num_variables: u16,
    max_stack_depth: u16,
    constant_pool: ConstantPool,
    functions: Vec<FuncEntry>,
    bytecode: Vec<u8>,
}

impl ContainerBuilder {
    pub fn new() -> Self {
        ContainerBuilder {
            num_variables: 0,
            max_stack_depth: 0,
            constant_pool: ConstantPool::default(),
            functions: Vec::new(),
            bytecode: Vec::new(),
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

    /// Builds the container, computing header fields from the added data.
    pub fn build(self) -> Container {
        let constant_pool = self.constant_pool;
        let code = CodeSection {
            functions: self.functions,
            bytecode: self.bytecode,
        };

        let header = FileHeader {
            num_variables: self.num_variables,
            max_stack_depth: self.max_stack_depth,
            num_functions: code.functions.len() as u16,
            ..FileHeader::default()
        };

        Container {
            header,
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
        assert_eq!(container.constant_pool.get_i32(0).unwrap(), 10);
        assert_eq!(container.constant_pool.get_i32(1).unwrap(), 32);
        assert_eq!(container.code.functions.len(), 1);
        assert_eq!(
            container.code.get_function_bytecode(0).unwrap(),
            bytecode.as_slice()
        );
    }
}
