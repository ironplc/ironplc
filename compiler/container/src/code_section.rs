use std::io::{Read, Write};

use crate::ContainerError;

/// Size of a single function directory entry in bytes.
const FUNC_ENTRY_SIZE: usize = 14;

/// A function entry in the code section directory.
#[derive(Clone, Debug)]
pub struct FuncEntry {
    pub function_id: u16,
    pub bytecode_offset: u32,
    pub bytecode_length: u32,
    pub max_stack_depth: u16,
    pub num_locals: u16,
}

/// The code section of a bytecode container.
#[derive(Clone, Debug, Default)]
pub struct CodeSection {
    pub functions: Vec<FuncEntry>,
    pub bytecode: Vec<u8>,
}

impl CodeSection {
    /// Returns the serialized size of this code section in bytes.
    ///
    /// Format: function_directory + bytecode_bodies
    pub fn section_size(&self) -> u32 {
        (self.functions.len() * FUNC_ENTRY_SIZE + self.bytecode.len()) as u32
    }

    /// Returns the bytecode slice for the given function ID.
    ///
    /// Uses direct indexing (O(1)) since function IDs are compiler-assigned
    /// sequential indices starting from 0.
    pub fn get_function_bytecode(&self, function_id: u16) -> Option<&[u8]> {
        let entry = self.functions.get(function_id as usize)?;
        let start = entry.bytecode_offset as usize;
        let end = start + entry.bytecode_length as usize;
        self.bytecode.get(start..end)
    }

    /// Writes the code section to the given writer.
    pub fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        for func in &self.functions {
            w.write_all(&func.function_id.to_le_bytes())?;
            w.write_all(&func.bytecode_offset.to_le_bytes())?;
            w.write_all(&func.bytecode_length.to_le_bytes())?;
            w.write_all(&func.max_stack_depth.to_le_bytes())?;
            w.write_all(&func.num_locals.to_le_bytes())?;
        }
        w.write_all(&self.bytecode)?;
        Ok(())
    }

    /// Reads a code section from the given reader.
    ///
    /// `num_functions` comes from the file header; `section_size` is the
    /// total size of the code section so we know how many bytecode bytes
    /// to read after the function directory.
    pub fn read_from(
        r: &mut impl Read,
        num_functions: u16,
        section_size: u32,
    ) -> Result<Self, ContainerError> {
        let dir_size = num_functions as usize * FUNC_ENTRY_SIZE;
        let bytecode_size = section_size as usize - dir_size;

        let mut functions = Vec::with_capacity(num_functions as usize);
        for _ in 0..num_functions {
            let mut entry_buf = [0u8; FUNC_ENTRY_SIZE];
            r.read_exact(&mut entry_buf)?;
            functions.push(FuncEntry {
                function_id: u16::from_le_bytes([entry_buf[0], entry_buf[1]]),
                bytecode_offset: u32::from_le_bytes([
                    entry_buf[2],
                    entry_buf[3],
                    entry_buf[4],
                    entry_buf[5],
                ]),
                bytecode_length: u32::from_le_bytes([
                    entry_buf[6],
                    entry_buf[7],
                    entry_buf[8],
                    entry_buf[9],
                ]),
                max_stack_depth: u16::from_le_bytes([entry_buf[10], entry_buf[11]]),
                num_locals: u16::from_le_bytes([entry_buf[12], entry_buf[13]]),
            });
        }

        let mut bytecode = vec![0u8; bytecode_size];
        r.read_exact(&mut bytecode)?;

        Ok(CodeSection {
            functions,
            bytecode,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn code_section_write_read_when_single_function_then_roundtrips() {
        let bytecode = vec![0x01, 0x00, 0x00, 0xB5];
        let section = CodeSection {
            functions: vec![FuncEntry {
                function_id: 0,
                bytecode_offset: 0,
                bytecode_length: bytecode.len() as u32,
                max_stack_depth: 2,
                num_locals: 1,
            }],
            bytecode,
        };

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded =
            CodeSection::read_from(&mut cursor, 1, section.section_size()).unwrap();

        assert_eq!(decoded.functions.len(), 1);
        assert_eq!(decoded.functions[0].function_id, 0);
        assert_eq!(decoded.functions[0].bytecode_length, 4);
        assert_eq!(decoded.functions[0].max_stack_depth, 2);
        assert_eq!(decoded.functions[0].num_locals, 1);
        assert_eq!(decoded.bytecode, vec![0x01, 0x00, 0x00, 0xB5]);
    }

    #[test]
    fn func_entry_write_when_single_entry_then_exactly_func_entry_size_bytes() {
        let section = CodeSection {
            functions: vec![FuncEntry {
                function_id: 0,
                bytecode_offset: 0,
                bytecode_length: 0,
                max_stack_depth: 0,
                num_locals: 0,
            }],
            bytecode: vec![],
        };

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        assert_eq!(buf.len(), FUNC_ENTRY_SIZE);
    }
}
