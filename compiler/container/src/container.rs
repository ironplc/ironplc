use std::io::{Cursor, Read, Write};
use std::vec::Vec;

use crate::code_section::CodeSection;
use crate::constant_pool::ConstantPool;
use crate::debug_section::DebugSection;
use crate::header::{FileHeader, HEADER_SIZE};
use crate::task_table::TaskTable;
use crate::type_section::TypeSection;
use crate::ContainerError;

/// A complete bytecode container: header + task table + constant pool + code section
/// + optional debug section.
#[derive(Clone, Debug)]
pub struct Container {
    pub header: FileHeader,
    pub task_table: TaskTable,
    pub type_section: Option<TypeSection>,
    pub constant_pool: ConstantPool,
    pub code: CodeSection,
    pub debug_section: Option<DebugSection>,
}

impl Container {
    /// Writes the container to the given writer.
    ///
    /// Computes section offsets and fills the header before writing
    /// sections in file-layout order.
    pub fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        let task_section_offset = HEADER_SIZE as u32;
        let task_section_size = self.task_table.section_size();

        let mut next_offset = task_section_offset + task_section_size;

        let mut header = self.header.clone();
        header.task_section_offset = task_section_offset;
        header.task_section_size = task_section_size;

        // Type section (optional, between task table and constant pool)
        if let Some(type_section) = &self.type_section {
            let type_section_size = type_section.section_size();
            header.type_section_offset = next_offset;
            header.type_section_size = type_section_size;
            header.flags |= 0x04; // bit 2: type section present
            next_offset += type_section_size;
        }

        let const_section_offset = next_offset;
        let const_section_size = self.constant_pool.section_size();
        header.const_section_offset = const_section_offset;
        header.const_section_size = const_section_size;
        next_offset = const_section_offset + const_section_size;

        let code_section_offset = next_offset;
        let code_section_size = self.code.section_size();
        header.code_section_offset = code_section_offset;
        header.code_section_size = code_section_size;
        header.num_functions = self.code.functions.len() as u16;
        next_offset = code_section_offset + code_section_size;

        if let Some(debug) = &self.debug_section {
            let debug_section_size = debug.section_size();
            header.debug_section_offset = next_offset;
            header.debug_section_size = debug_section_size;
            header.flags |= 0x02; // bit 1: debug section present
        }

        header.write_to(w)?;
        self.task_table.write_to(w)?;

        if let Some(type_section) = &self.type_section {
            type_section.write_to(w)?;
        }

        self.constant_pool.write_to(w)?;
        self.code.write_to(w)?;

        if let Some(debug) = &self.debug_section {
            debug.write_to(w)?;
        }

        Ok(())
    }

    /// Reads a container from the given reader.
    pub fn read_from(r: &mut impl Read) -> Result<Self, ContainerError> {
        let header = FileHeader::read_from(r)?;

        // Read remaining bytes after the header so we can seek to
        // section offsets within them.
        let mut rest = Vec::new();
        r.read_to_end(&mut rest)?;

        let base = HEADER_SIZE as u32;

        let task_start = (header.task_section_offset - base) as usize;
        let task_end = task_start + header.task_section_size as usize;
        let task_table = TaskTable::read_from(&mut Cursor::new(&rest[task_start..task_end]))?;

        // Parse type section if present (flag bit 2).
        let type_section = if (header.flags & 0x04) != 0 && header.type_section_size > 0 {
            let ts_start = (header.type_section_offset - base) as usize;
            let ts_end = ts_start + header.type_section_size as usize;
            if ts_end <= rest.len() {
                Some(TypeSection::read_from(&mut Cursor::new(
                    &rest[ts_start..ts_end],
                ))?)
            } else {
                None
            }
        } else {
            None
        };

        let const_start = (header.const_section_offset - base) as usize;
        let const_end = const_start + header.const_section_size as usize;
        let constant_pool =
            ConstantPool::read_from(&mut Cursor::new(&rest[const_start..const_end]))?;

        let code_start = (header.code_section_offset - base) as usize;
        let code_end = code_start + header.code_section_size as usize;
        let code = CodeSection::read_from(
            &mut Cursor::new(&rest[code_start..code_end]),
            header.num_functions,
            header.code_section_size,
        )?;

        // Parse debug section if present (non-fatal on error).
        let debug_section = if header.debug_section_size > 0 {
            let debug_start = (header.debug_section_offset - base) as usize;
            let debug_end = debug_start + header.debug_section_size as usize;
            if debug_end <= rest.len() {
                DebugSection::read_from(&mut Cursor::new(&rest[debug_start..debug_end])).ok()
            } else {
                None
            }
        } else {
            None
        };

        Ok(Container {
            header,
            task_table,
            type_section,
            constant_pool,
            code,
            debug_section,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use std::vec::Vec;

    use crate::debug_section::{
        function_id, iec_type_tag, var_section, FuncNameEntry, VarNameEntry,
    };
    use crate::id_types::{ConstantIndex, FunctionId, InstanceId, TaskId, VarIndex};
    use crate::ContainerBuilder;

    #[test]
    fn container_write_read_when_steel_thread_program_then_roundtrips() {
        // x := 10; y := x + 32;
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
            0x18, 0x00, 0x00,       // STORE_VAR_I32  var[0]   (x := 10)
            0x10, 0x00, 0x00,       // LOAD_VAR_I32   var[0]   (push x)
            0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (32)
            0x30,                   // ADD_I32
            0x18, 0x01, 0x00,       // STORE_VAR_I32  var[1]   (y := 42)
            0xB5,                   // RET_VOID
        ];

        let container = ContainerBuilder::new()
            .num_variables(2)
            .add_i32_constant(10)
            .add_i32_constant(32)
            .add_function(FunctionId::INIT, &bytecode, 2, 2, 0)
            .build();

        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();

        let decoded = Container::read_from(&mut Cursor::new(&buf)).unwrap();

        // Verify synthesized default task table
        assert_eq!(decoded.task_table.tasks.len(), 1);
        assert_eq!(decoded.task_table.tasks[0].task_id, TaskId::DEFAULT);
        assert_eq!(
            decoded.task_table.tasks[0].task_type,
            crate::TaskType::Freewheeling
        );
        assert_eq!(decoded.task_table.tasks[0].flags, 0x01);
        assert_eq!(decoded.task_table.programs.len(), 1);
        assert_eq!(
            decoded.task_table.programs[0].instance_id,
            InstanceId::DEFAULT
        );
        assert_eq!(decoded.task_table.programs[0].task_id, TaskId::DEFAULT);
        assert_eq!(decoded.task_table.programs[0].var_table_count, 2);

        assert_eq!(
            decoded
                .constant_pool
                .get_i32(ConstantIndex::new(0))
                .unwrap(),
            10
        );
        assert_eq!(
            decoded
                .constant_pool
                .get_i32(ConstantIndex::new(1))
                .unwrap(),
            32
        );
        assert_eq!(decoded.code.functions.len(), 1);
        assert_eq!(decoded.code.functions[0].function_id, FunctionId::INIT);

        let code = decoded
            .code
            .get_function_bytecode(FunctionId::INIT)
            .unwrap();
        assert_eq!(code, bytecode.as_slice());

        // No debug section in this container.
        assert!(decoded.debug_section.is_none());
    }

    #[test]
    fn container_write_read_when_debug_section_then_roundtrips() {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]
            0x18, 0x00, 0x00,       // STORE_VAR_I32  var[0]
            0xB5,                   // RET_VOID
        ];

        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_i32_constant(42)
            .add_function(FunctionId::INIT, &bytecode, 1, 1, 0)
            .add_var_name(VarNameEntry {
                var_index: VarIndex::new(0),
                function_id: function_id::GLOBAL_SCOPE,
                var_section: var_section::VAR,
                iec_type_tag: iec_type_tag::DINT,
                name: "x".into(),
                type_name: "DINT".into(),
            })
            .add_func_name(FuncNameEntry {
                function_id: FunctionId::INIT,
                name: "MAIN".into(),
            })
            .build();

        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();

        let decoded = Container::read_from(&mut Cursor::new(&buf)).unwrap();

        // Verify debug section flag is set.
        assert_eq!(decoded.header.flags & 0x02, 0x02);

        let debug = decoded.debug_section.unwrap();
        assert_eq!(debug.var_names.len(), 1);
        assert_eq!(debug.var_names[0].name, "x");
        assert_eq!(debug.var_names[0].type_name, "DINT");
        assert_eq!(debug.var_names[0].iec_type_tag, iec_type_tag::DINT);
        assert_eq!(debug.func_names.len(), 1);
        assert_eq!(debug.func_names[0].name, "MAIN");
    }

    #[test]
    fn container_write_read_when_type_section_with_array_then_roundtrips() {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]
            0x18, 0x00, 0x00,       // STORE_VAR_I32  var[0]
            0xB5,                   // RET_VOID
        ];

        let mut builder = ContainerBuilder::new();
        let desc_idx = builder.add_array_descriptor(0, 10); // I32, 10 elements
        assert_eq!(desc_idx, 0);

        let container = builder
            .num_variables(1)
            .add_i32_constant(42)
            .add_function(FunctionId::INIT, &bytecode, 1, 1, 0)
            .build();

        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();

        let decoded = Container::read_from(&mut Cursor::new(&buf)).unwrap();

        // Verify type section flag is set.
        assert_eq!(decoded.header.flags & 0x04, 0x04);

        let ts = decoded.type_section.unwrap();
        assert!(ts.fb_types.is_empty());
        assert_eq!(ts.array_descriptors.len(), 1);
        assert_eq!(ts.array_descriptors[0].element_type, 0);
        assert_eq!(ts.array_descriptors[0].total_elements, 10);

        // Verify other sections still roundtrip correctly.
        assert_eq!(
            decoded
                .constant_pool
                .get_i32(ConstantIndex::new(0))
                .unwrap(),
            42
        );
        assert_eq!(decoded.code.functions.len(), 1);
    }
}
