use std::io::{Cursor, Read, Write};

use crate::code_section::CodeSection;
use crate::constant_pool::ConstantPool;
use crate::header::{FileHeader, HEADER_SIZE};
use crate::ContainerError;

/// A complete bytecode container: header + constant pool + code section.
#[derive(Clone, Debug)]
pub struct Container {
    pub header: FileHeader,
    pub constant_pool: ConstantPool,
    pub code: CodeSection,
}

impl Container {
    /// Writes the container to the given writer.
    ///
    /// Computes section offsets and fills the header before writing
    /// sections in file-layout order.
    pub fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        let const_section_offset = HEADER_SIZE as u32;
        let const_section_size = self.constant_pool.section_size();
        let code_section_offset = const_section_offset + const_section_size;
        let code_section_size = self.code.section_size();

        let mut header = self.header.clone();
        header.const_section_offset = const_section_offset;
        header.const_section_size = const_section_size;
        header.code_section_offset = code_section_offset;
        header.code_section_size = code_section_size;
        header.num_functions = self.code.functions.len() as u16;

        header.write_to(w)?;
        self.constant_pool.write_to(w)?;
        self.code.write_to(w)?;
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

        Ok(Container {
            header,
            constant_pool,
            code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            .add_function(0, &bytecode, 2, 2)
            .build();

        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();

        let decoded = Container::read_from(&mut Cursor::new(&buf)).unwrap();

        assert_eq!(decoded.constant_pool.get_i32(0).unwrap(), 10);
        assert_eq!(decoded.constant_pool.get_i32(1).unwrap(), 32);
        assert_eq!(decoded.code.functions.len(), 1);
        assert_eq!(decoded.code.functions[0].function_id, 0);

        let code = decoded.code.get_function_bytecode(0).unwrap();
        assert_eq!(code, bytecode.as_slice());
    }
}
