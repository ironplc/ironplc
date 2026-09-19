use std::io::{Cursor, Read, Write};
use std::vec::Vec;

use crate::code_section::CodeSection;
use crate::constant_pool::ConstantPool;
use crate::debug_section::DebugSection;
use crate::header::{FileHeader, FLAG_HAS_DEBUG_SECTION, FLAG_HAS_TYPE_SECTION, HEADER_SIZE};
use crate::task_table::TaskTable;
use crate::type_section::TypeSection;
use crate::ContainerError;

/// A complete bytecode container, in file order: header, task table,
/// optional type section, constant pool, code section, optional debug
/// section.
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
    /// Each section is serialized to a buffer first, so the header is
    /// derived from the bytes that actually reach the file — the section
    /// directory from their lengths — before anything is written.
    pub fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        let task_bytes = serialize(|buf| self.task_table.write_to(buf))?;
        let type_bytes = match &self.type_section {
            Some(type_section) => serialize(|buf| type_section.write_to(buf))?,
            None => Vec::new(),
        };
        let const_bytes = serialize(|buf| self.constant_pool.write_to(buf))?;
        let code_bytes = serialize(|buf| self.code.write_to(buf))?;
        let debug_bytes = match &self.debug_section {
            Some(debug) => serialize(|buf| debug.write_to(buf))?,
            None => Vec::new(),
        };

        let mut header = self.header.clone();
        let mut next_offset = HEADER_SIZE as u32;

        header.task_section_offset = next_offset;
        header.task_section_size = task_bytes.len() as u32;
        next_offset += header.task_section_size;

        // Type section (optional, between task table and constant pool)
        if self.type_section.is_some() {
            header.type_section_offset = next_offset;
            header.type_section_size = type_bytes.len() as u32;
            header.flags |= FLAG_HAS_TYPE_SECTION;
            next_offset += header.type_section_size;
        }

        header.const_section_offset = next_offset;
        header.const_section_size = const_bytes.len() as u32;
        next_offset += header.const_section_size;

        header.code_section_offset = next_offset;
        header.code_section_size = code_bytes.len() as u32;
        header.num_functions = self.code.functions.len() as u16;
        next_offset += header.code_section_size;

        if self.debug_section.is_some() {
            header.debug_section_offset = next_offset;
            header.debug_section_size = debug_bytes.len() as u32;
            header.flags |= FLAG_HAS_DEBUG_SECTION;
        }

        header.write_to(w)?;
        w.write_all(&task_bytes)?;
        w.write_all(&type_bytes)?;
        w.write_all(&const_bytes)?;
        w.write_all(&code_bytes)?;
        w.write_all(&debug_bytes)?;

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

        // Parse type section if present.
        let type_section =
            if (header.flags & FLAG_HAS_TYPE_SECTION) != 0 && header.type_section_size > 0 {
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

/// Runs a section writer against a fresh buffer and returns the bytes.
fn serialize(
    write: impl FnOnce(&mut Vec<u8>) -> Result<(), ContainerError>,
) -> Result<Vec<u8>, ContainerError> {
    let mut buf = Vec::new();
    write(&mut buf)?;
    Ok(buf)
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
    use crate::test_support::{
        round_trip, steel_thread_bytecode, steel_thread_single_function_container,
        with_tampered_header,
    };
    use crate::ContainerBuilder;

    #[test]
    fn container_write_read_when_steel_thread_program_then_roundtrips() {
        // x := 10; y := x + 32;
        let bytecode = steel_thread_bytecode();
        let decoded = round_trip(&steel_thread_single_function_container());

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
            0x00, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]
            0x10, 0x00, 0x00,       // STORE_VAR_I32  var[0]
            0x8C,                   // RET_VOID
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
            0x00, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]
            0x10, 0x00, 0x00,       // STORE_VAR_I32  var[0]
            0x8C,                   // RET_VOID
        ];

        let mut builder = ContainerBuilder::new();
        let desc_idx = builder.add_array_descriptor(0, 10, 0); // I32, 10 elements
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

    #[test]
    fn container_read_from_when_type_section_truncated_then_type_section_is_none() {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,
            0x18, 0x00, 0x00,
            0x8C,
        ];

        let mut builder = ContainerBuilder::new();
        builder.add_array_descriptor(0, 4, 0);
        let container = builder
            .num_variables(1)
            .add_i32_constant(1)
            .add_function(FunctionId::INIT, &bytecode, 1, 1, 0)
            .build();

        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();

        // Inflate the declared type_section_size so ts_end exceeds available
        // bytes, forcing the bounds check in read_from to return None.
        let n = buf.len() as u32;
        let tampered = with_tampered_header(&buf, |h| h.type_section_size = n * 2);

        let decoded = Container::read_from(&mut Cursor::new(&tampered)).unwrap();
        assert!(decoded.type_section.is_none());
    }

    #[test]
    fn container_read_from_when_debug_section_truncated_then_debug_section_is_none() {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,
            0x18, 0x00, 0x00,
            0x8C,
        ];

        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_i32_constant(1)
            .add_function(FunctionId::INIT, &bytecode, 1, 1, 0)
            .add_func_name(FuncNameEntry {
                function_id: FunctionId::INIT,
                name: "MAIN".into(),
            })
            .build();

        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();

        // Inflate the declared debug_section_size past the end of the buffer,
        // triggering the bounds check in read_from that returns None.
        let n = buf.len() as u32;
        let tampered = with_tampered_header(&buf, |h| h.debug_section_size = n * 2);

        let decoded = Container::read_from(&mut Cursor::new(&tampered)).unwrap();
        assert!(decoded.debug_section.is_none());
    }

    #[test]
    fn container_read_from_when_no_debug_section_then_debug_section_is_none() {
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,
            0x18, 0x00, 0x00,
            0x8C,
        ];

        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_i32_constant(1)
            .add_function(FunctionId::INIT, &bytecode, 1, 1, 0)
            .build();

        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();

        let decoded = Container::read_from(&mut Cursor::new(&buf)).unwrap();
        assert_eq!(decoded.header.debug_section_size, 0);
        assert!(decoded.debug_section.is_none());
    }
}
