use crate::const_type::ConstType;
use crate::error::ContainerError;
use crate::header::{FileHeader, HEADER_SIZE};
use crate::task_type::TaskType;

/// Size of a single function directory entry in bytes.
const FUNC_ENTRY_SIZE: usize = 14;

/// Size of the task table header in bytes (num_tasks + num_programs + shared_globals_size).
const TASK_TABLE_HEADER_SIZE: usize = 6;

/// Size of a single task entry in bytes.
const TASK_ENTRY_SIZE: usize = 32;

/// Size of a single program instance entry in bytes.
const PROGRAM_ENTRY_SIZE: usize = 16;

/// A task entry parsed from a container's task table (no_std-compatible).
#[derive(Clone, Debug)]
pub struct TaskEntryRef {
    pub task_id: u16,
    pub priority: u16,
    pub task_type: TaskType,
    pub flags: u8,
    pub interval_us: u64,
    pub single_var_index: u16,
    pub watchdog_us: u64,
    pub input_image_offset: u16,
    pub output_image_offset: u16,
    pub reserved: [u8; 4],
}

/// A program instance entry parsed from a container's task table (no_std-compatible).
#[derive(Clone, Debug)]
pub struct ProgramEntryRef {
    pub instance_id: u16,
    pub task_id: u16,
    pub entry_function_id: u16,
    pub var_table_offset: u16,
    pub var_table_count: u16,
    pub fb_instance_offset: u16,
    pub fb_instance_count: u16,
    pub reserved: u16,
}

/// Zero-copy, `no_std`-compatible view over a serialized bytecode container.
///
/// Borrows the underlying byte slice and provides O(1) accessors for all
/// container sections. The caller provides a mutable buffer for pre-scanned
/// constant pool offsets.
#[derive(Debug)]
pub struct ContainerRef<'a> {
    header: FileHeader,
    const_pool_bytes: &'a [u8],
    const_offsets: &'a [u32],
    code_bytes: &'a [u8],
    func_dir: &'a [u8],
    task_table_bytes: &'a [u8],
}

/// Helper to read a little-endian u16 from a byte slice at the given offset.
///
/// Returns `SectionSizeMismatch` if the slice is too short.
fn read_u16(data: &[u8], offset: usize) -> Result<u16, ContainerError> {
    if offset + 2 > data.len() {
        return Err(ContainerError::SectionSizeMismatch);
    }
    Ok(u16::from_le_bytes([data[offset], data[offset + 1]]))
}

impl<'a> ContainerRef<'a> {
    /// Returns the number of constants in the constant pool without fully
    /// parsing the container.
    ///
    /// This is useful for sizing the `const_offset_buf` before calling
    /// [`from_slice`](Self::from_slice).
    pub fn const_count(data: &[u8]) -> Result<u16, ContainerError> {
        if data.len() < HEADER_SIZE {
            return Err(ContainerError::SectionSizeMismatch);
        }
        let header_bytes: &[u8; HEADER_SIZE] = data[..HEADER_SIZE]
            .try_into()
            .map_err(|_| ContainerError::SectionSizeMismatch)?;
        let header = FileHeader::from_bytes(header_bytes)?;

        if header.const_section_size == 0 {
            return Ok(0);
        }

        let offset = header.const_section_offset as usize;
        read_u16(data, offset)
    }

    /// Parses a serialized container from a byte slice, filling `const_offset_buf`
    /// with the byte offsets of each constant pool entry.
    ///
    /// The caller must provide a `const_offset_buf` with at least
    /// [`const_count`](Self::const_count) elements.
    pub fn from_slice(
        data: &'a [u8],
        const_offset_buf: &'a mut [u32],
    ) -> Result<Self, ContainerError> {
        // 1. Parse header
        if data.len() < HEADER_SIZE {
            return Err(ContainerError::SectionSizeMismatch);
        }
        let header_bytes: &[u8; HEADER_SIZE] = data[..HEADER_SIZE]
            .try_into()
            .map_err(|_| ContainerError::SectionSizeMismatch)?;
        let header = FileHeader::from_bytes(header_bytes)?;

        // 2. Slice out constant pool section
        let const_start = header.const_section_offset as usize;
        let const_end = const_start + header.const_section_size as usize;
        if const_end > data.len() {
            return Err(ContainerError::SectionSizeMismatch);
        }
        let const_section = &data[const_start..const_end];

        // Read count and skip it to get the entry bytes
        let (const_pool_bytes, num_consts) = if header.const_section_size == 0 {
            (&data[0..0], 0u16)
        } else {
            if const_section.len() < 2 {
                return Err(ContainerError::SectionSizeMismatch);
            }
            let count = u16::from_le_bytes([const_section[0], const_section[1]]);
            (&const_section[2..], count)
        };

        // 3. Pre-scan constant entries to fill const_offset_buf
        if (num_consts as usize) > const_offset_buf.len() {
            return Err(ContainerError::SectionSizeMismatch);
        }
        let mut pos: usize = 0;
        for slot in const_offset_buf.iter_mut().take(num_consts as usize) {
            *slot = pos as u32;
            // Each entry: type(1) + reserved(1) + size(2) + value(size)
            if pos + 4 > const_pool_bytes.len() {
                return Err(ContainerError::SectionSizeMismatch);
            }
            let entry_value_size =
                u16::from_le_bytes([const_pool_bytes[pos + 2], const_pool_bytes[pos + 3]]) as usize;
            pos += 4 + entry_value_size;
            if pos > const_pool_bytes.len() {
                return Err(ContainerError::SectionSizeMismatch);
            }
        }
        let const_offsets = &const_offset_buf[..num_consts as usize];

        // 4. Slice out code section, split into func_dir and code_bytes
        let code_start = header.code_section_offset as usize;
        let code_end = code_start + header.code_section_size as usize;
        if code_end > data.len() {
            return Err(ContainerError::SectionSizeMismatch);
        }
        let code_section = &data[code_start..code_end];

        let func_dir_size = header.num_functions as usize * FUNC_ENTRY_SIZE;
        if func_dir_size > code_section.len() {
            return Err(ContainerError::SectionSizeMismatch);
        }
        let func_dir = &code_section[..func_dir_size];
        let code_bytes = &code_section[func_dir_size..];

        // 5. Slice out task table section
        let task_start = header.task_section_offset as usize;
        let task_end = task_start + header.task_section_size as usize;
        if task_end > data.len() {
            return Err(ContainerError::SectionSizeMismatch);
        }
        let task_table_bytes = &data[task_start..task_end];

        // Validate task table has at least a header
        if header.task_section_size > 0 && task_table_bytes.len() < TASK_TABLE_HEADER_SIZE {
            return Err(ContainerError::SectionSizeMismatch);
        }

        Ok(ContainerRef {
            header,
            const_pool_bytes,
            const_offsets,
            code_bytes,
            func_dir,
            task_table_bytes,
        })
    }

    /// Returns a reference to the parsed file header.
    pub fn header(&self) -> &FileHeader {
        &self.header
    }

    /// Returns the i32 constant at the given pool index.
    ///
    /// Validates that the entry's type tag is `ConstType::I32` and reads
    /// 4 bytes as a little-endian i32.
    pub fn get_i32_constant(&self, index: u16) -> Result<i32, ContainerError> {
        let idx = index as usize;
        if idx >= self.const_offsets.len() {
            return Err(ContainerError::InvalidConstantIndex(index));
        }
        let offset = self.const_offsets[idx] as usize;

        // Read type tag and validate
        if offset + 4 > self.const_pool_bytes.len() {
            return Err(ContainerError::SectionSizeMismatch);
        }
        let type_tag = self.const_pool_bytes[offset];
        let const_type = ConstType::from_u8(type_tag)?;
        if const_type != ConstType::I32 {
            return Err(ContainerError::InvalidConstantType(type_tag));
        }

        // Skip type(1) + reserved(1) + size(2) = 4 bytes to get to value
        let value_offset = offset + 4;
        if value_offset + 4 > self.const_pool_bytes.len() {
            return Err(ContainerError::SectionSizeMismatch);
        }
        Ok(i32::from_le_bytes([
            self.const_pool_bytes[value_offset],
            self.const_pool_bytes[value_offset + 1],
            self.const_pool_bytes[value_offset + 2],
            self.const_pool_bytes[value_offset + 3],
        ]))
    }

    /// Returns the bytecode slice for the given function ID.
    ///
    /// Reads the function directory entry at `id * FUNC_ENTRY_SIZE` to get
    /// the offset and length, then slices the code bytes.
    pub fn get_function_bytecode(&self, id: u16) -> Option<&'a [u8]> {
        let entry_offset = id as usize * FUNC_ENTRY_SIZE;
        if entry_offset + FUNC_ENTRY_SIZE > self.func_dir.len() {
            return None;
        }
        let entry = &self.func_dir[entry_offset..entry_offset + FUNC_ENTRY_SIZE];

        let bytecode_offset = u32::from_le_bytes([entry[2], entry[3], entry[4], entry[5]]) as usize;
        let bytecode_length = u32::from_le_bytes([entry[6], entry[7], entry[8], entry[9]]) as usize;

        let end = bytecode_offset + bytecode_length;
        if end > self.code_bytes.len() {
            return None;
        }
        Some(&self.code_bytes[bytecode_offset..end])
    }

    /// Returns the number of tasks in the task table.
    pub fn num_tasks(&self) -> u16 {
        if self.task_table_bytes.len() < 2 {
            return 0;
        }
        u16::from_le_bytes([self.task_table_bytes[0], self.task_table_bytes[1]])
    }

    /// Returns the number of program instances in the task table.
    pub fn num_programs(&self) -> u16 {
        if self.task_table_bytes.len() < 4 {
            return 0;
        }
        u16::from_le_bytes([self.task_table_bytes[2], self.task_table_bytes[3]])
    }

    /// Returns the shared globals size from the task table header.
    pub fn shared_globals_size(&self) -> u16 {
        if self.task_table_bytes.len() < 6 {
            return 0;
        }
        u16::from_le_bytes([self.task_table_bytes[4], self.task_table_bytes[5]])
    }

    /// Parses and returns the task entry at the given index.
    pub fn task_entry(&self, index: u16) -> Result<TaskEntryRef, ContainerError> {
        let start = TASK_TABLE_HEADER_SIZE + index as usize * TASK_ENTRY_SIZE;
        let end = start + TASK_ENTRY_SIZE;
        if end > self.task_table_bytes.len() {
            return Err(ContainerError::SectionSizeMismatch);
        }
        let buf = &self.task_table_bytes[start..end];

        let task_type = TaskType::from_u8(buf[4])?;

        Ok(TaskEntryRef {
            task_id: u16::from_le_bytes([buf[0], buf[1]]),
            priority: u16::from_le_bytes([buf[2], buf[3]]),
            task_type,
            flags: buf[5],
            interval_us: u64::from_le_bytes([
                buf[6], buf[7], buf[8], buf[9], buf[10], buf[11], buf[12], buf[13],
            ]),
            single_var_index: u16::from_le_bytes([buf[14], buf[15]]),
            watchdog_us: u64::from_le_bytes([
                buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23],
            ]),
            input_image_offset: u16::from_le_bytes([buf[24], buf[25]]),
            output_image_offset: u16::from_le_bytes([buf[26], buf[27]]),
            reserved: [buf[28], buf[29], buf[30], buf[31]],
        })
    }

    /// Parses and returns the program instance entry at the given index.
    pub fn program_entry(&self, index: u16) -> Result<ProgramEntryRef, ContainerError> {
        let tasks_end = TASK_TABLE_HEADER_SIZE + self.num_tasks() as usize * TASK_ENTRY_SIZE;
        let start = tasks_end + index as usize * PROGRAM_ENTRY_SIZE;
        let end = start + PROGRAM_ENTRY_SIZE;
        if end > self.task_table_bytes.len() {
            return Err(ContainerError::SectionSizeMismatch);
        }
        let buf = &self.task_table_bytes[start..end];

        Ok(ProgramEntryRef {
            instance_id: u16::from_le_bytes([buf[0], buf[1]]),
            task_id: u16::from_le_bytes([buf[2], buf[3]]),
            entry_function_id: u16::from_le_bytes([buf[4], buf[5]]),
            var_table_offset: u16::from_le_bytes([buf[6], buf[7]]),
            var_table_count: u16::from_le_bytes([buf[8], buf[9]]),
            fb_instance_offset: u16::from_le_bytes([buf[10], buf[11]]),
            fb_instance_count: u16::from_le_bytes([buf[12], buf[13]]),
            reserved: u16::from_le_bytes([buf[14], buf[15]]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use std::vec::Vec;

    fn steel_thread_bytes() -> Vec<u8> {
        use crate::ContainerBuilder;
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,       // LOAD_CONST_I32 pool[0]  (10)
            0x18, 0x00, 0x00,       // STORE_VAR_I32  var[0]
            0x10, 0x00, 0x00,       // LOAD_VAR_I32   var[0]
            0x01, 0x01, 0x00,       // LOAD_CONST_I32 pool[1]  (32)
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
        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn container_ref_from_slice_when_valid_bytes_then_parses() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        assert_eq!(count, 2);
        let mut offsets = vec![0u32; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        assert_eq!(cref.header().num_variables, 2);
        assert_eq!(cref.header().num_functions, 1);
        assert_eq!(cref.header().max_stack_depth, 2);
    }

    #[test]
    fn container_ref_from_slice_when_invalid_magic_then_error() {
        let mut data = steel_thread_bytes();
        // Corrupt magic bytes
        data[0] = 0xFF;
        data[1] = 0xFF;
        data[2] = 0xFF;
        data[3] = 0xFF;
        let mut offsets = vec![0u32; 16];
        let result = ContainerRef::from_slice(&data, &mut offsets);
        assert!(matches!(result, Err(ContainerError::InvalidMagic)));
    }

    #[test]
    fn container_ref_from_slice_when_truncated_then_error() {
        let data = vec![0u8; 100];
        let mut offsets = vec![0u32; 16];
        let result = ContainerRef::from_slice(&data, &mut offsets);
        assert!(matches!(result, Err(ContainerError::SectionSizeMismatch)));
    }

    #[test]
    fn container_ref_get_i32_constant_when_valid_index_then_returns_value() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![0u32; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        assert_eq!(cref.get_i32_constant(0).unwrap(), 10);
        assert_eq!(cref.get_i32_constant(1).unwrap(), 32);
    }

    #[test]
    fn container_ref_get_i32_constant_when_out_of_bounds_then_error() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![0u32; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        let result = cref.get_i32_constant(99);
        assert!(matches!(
            result,
            Err(ContainerError::InvalidConstantIndex(99))
        ));
    }

    #[test]
    fn container_ref_get_function_bytecode_when_valid_id_then_returns_slice() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![0u32; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        let bytecode = cref.get_function_bytecode(0).unwrap();
        // First byte: LOAD_CONST_I32 (0x01), last byte: RET_VOID (0xB5)
        assert_eq!(bytecode[0], 0x01);
        assert_eq!(*bytecode.last().unwrap(), 0xB5);
    }

    #[test]
    fn container_ref_task_entry_when_valid_index_then_returns_fields() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![0u32; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        let task = cref.task_entry(0).unwrap();
        assert_eq!(task.task_id, 0);
        assert_eq!(task.task_type, TaskType::Freewheeling);
        assert_eq!(task.flags, 0x01);
    }

    #[test]
    fn container_ref_program_entry_when_valid_index_then_returns_fields() {
        let data = steel_thread_bytes();
        let count = ContainerRef::const_count(&data).unwrap();
        let mut offsets = vec![0u32; count as usize];
        let cref = ContainerRef::from_slice(&data, &mut offsets).unwrap();

        let prog = cref.program_entry(0).unwrap();
        assert_eq!(prog.instance_id, 0);
        assert_eq!(prog.task_id, 0);
        assert_eq!(prog.var_table_count, 2);
    }
}
