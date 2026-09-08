#[cfg(feature = "std")]
use std::io::{Read, Write};
#[cfg(feature = "std")]
use std::vec::Vec;

use crate::id_types::{FunctionId, InstanceId, TaskId, VarIndex};
use crate::task_type::TaskType;
use crate::ContainerError;

/// A single task entry in the task table (32 bytes fixed).
#[derive(Clone, Copy, Debug)]
pub struct TaskEntry {
    pub task_id: TaskId,
    pub priority: u16,
    pub task_type: TaskType,
    pub flags: u8,
    pub interval_us: u64,
    pub single_var_index: VarIndex,
    pub watchdog_us: u64,
    pub input_image_offset: u16,
    pub output_image_offset: u16,
    pub reserved: [u8; 4],
}

impl TaskEntry {
    /// Serialized size of one entry in bytes.
    pub const SIZE: usize = 32;

    /// Decodes an entry from its serialized bytes.
    ///
    /// The only byte that can be invalid is the task type tag.
    pub fn from_bytes(buf: &[u8; Self::SIZE]) -> Result<Self, ContainerError> {
        let task_type = TaskType::from_u8(buf[4])?;
        Ok(TaskEntry {
            task_id: TaskId::new(u16::from_le_bytes([buf[0], buf[1]])),
            priority: u16::from_le_bytes([buf[2], buf[3]]),
            task_type,
            flags: buf[5],
            interval_us: u64::from_le_bytes([
                buf[6], buf[7], buf[8], buf[9], buf[10], buf[11], buf[12], buf[13],
            ]),
            single_var_index: VarIndex::new(u16::from_le_bytes([buf[14], buf[15]])),
            watchdog_us: u64::from_le_bytes([
                buf[16], buf[17], buf[18], buf[19], buf[20], buf[21], buf[22], buf[23],
            ]),
            input_image_offset: u16::from_le_bytes([buf[24], buf[25]]),
            output_image_offset: u16::from_le_bytes([buf[26], buf[27]]),
            reserved: [buf[28], buf[29], buf[30], buf[31]],
        })
    }

    /// Encodes the entry into its serialized bytes.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..2].copy_from_slice(&self.task_id.to_le_bytes());
        buf[2..4].copy_from_slice(&self.priority.to_le_bytes());
        buf[4] = self.task_type as u8;
        buf[5] = self.flags;
        buf[6..14].copy_from_slice(&self.interval_us.to_le_bytes());
        buf[14..16].copy_from_slice(&self.single_var_index.to_le_bytes());
        buf[16..24].copy_from_slice(&self.watchdog_us.to_le_bytes());
        buf[24..26].copy_from_slice(&self.input_image_offset.to_le_bytes());
        buf[26..28].copy_from_slice(&self.output_image_offset.to_le_bytes());
        buf[28..32].copy_from_slice(&self.reserved);
        buf
    }
}

/// A single program instance entry in the task table (16 bytes fixed).
#[derive(Clone, Copy, Debug)]
pub struct ProgramInstanceEntry {
    pub instance_id: InstanceId,
    pub task_id: TaskId,
    pub entry_function_id: FunctionId,
    pub var_table_offset: u16,
    pub var_table_count: u16,
    pub fb_instance_offset: u16,
    pub fb_instance_count: u16,
    pub init_function_id: FunctionId,
}

impl ProgramInstanceEntry {
    /// Serialized size of one entry in bytes.
    pub const SIZE: usize = 16;

    /// Decodes an entry from its serialized bytes. Every field is a plain
    /// integer, so any 16 bytes decode.
    pub fn from_bytes(buf: &[u8; Self::SIZE]) -> Self {
        ProgramInstanceEntry {
            instance_id: InstanceId::new(u16::from_le_bytes([buf[0], buf[1]])),
            task_id: TaskId::new(u16::from_le_bytes([buf[2], buf[3]])),
            entry_function_id: FunctionId::new(u16::from_le_bytes([buf[4], buf[5]])),
            var_table_offset: u16::from_le_bytes([buf[6], buf[7]]),
            var_table_count: u16::from_le_bytes([buf[8], buf[9]]),
            fb_instance_offset: u16::from_le_bytes([buf[10], buf[11]]),
            fb_instance_count: u16::from_le_bytes([buf[12], buf[13]]),
            init_function_id: FunctionId::new(u16::from_le_bytes([buf[14], buf[15]])),
        }
    }

    /// Encodes the entry into its serialized bytes.
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..2].copy_from_slice(&self.instance_id.to_le_bytes());
        buf[2..4].copy_from_slice(&self.task_id.to_le_bytes());
        buf[4..6].copy_from_slice(&self.entry_function_id.to_le_bytes());
        buf[6..8].copy_from_slice(&self.var_table_offset.to_le_bytes());
        buf[8..10].copy_from_slice(&self.var_table_count.to_le_bytes());
        buf[10..12].copy_from_slice(&self.fb_instance_offset.to_le_bytes());
        buf[12..14].copy_from_slice(&self.fb_instance_count.to_le_bytes());
        buf[14..16].copy_from_slice(&self.init_function_id.to_le_bytes());
        buf
    }
}

/// Size of the task table header in bytes: `num_tasks` (u16),
/// `num_programs` (u16) and `shared_globals_size` (u16).
pub const TASK_TABLE_HEADER_SIZE: usize = 6;

/// The task table section of a bytecode container.
#[cfg(feature = "std")]
#[derive(Clone, Debug, Default)]
pub struct TaskTable {
    pub shared_globals_size: u16,
    pub tasks: Vec<TaskEntry>,
    pub programs: Vec<ProgramInstanceEntry>,
}

#[cfg(feature = "std")]
impl TaskTable {
    /// Returns the serialized size of this task table section in bytes.
    ///
    /// Format: header (6 bytes) + task entries + program instance entries
    pub fn section_size(&self) -> u32 {
        let tasks_size = (self.tasks.len() * TaskEntry::SIZE) as u32;
        let programs_size = (self.programs.len() * ProgramInstanceEntry::SIZE) as u32;
        TASK_TABLE_HEADER_SIZE as u32 + tasks_size + programs_size
    }

    /// Writes the task table to the given writer.
    pub fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        // Header: num_tasks, num_programs, shared_globals_size
        w.write_all(&(self.tasks.len() as u16).to_le_bytes())?;
        w.write_all(&(self.programs.len() as u16).to_le_bytes())?;
        w.write_all(&self.shared_globals_size.to_le_bytes())?;

        for task in &self.tasks {
            w.write_all(&task.to_bytes())?;
        }
        for prog in &self.programs {
            w.write_all(&prog.to_bytes())?;
        }

        Ok(())
    }

    /// Reads a task table from the given reader.
    pub fn read_from(r: &mut impl Read) -> Result<Self, ContainerError> {
        // Read header
        let mut hdr = [0u8; TASK_TABLE_HEADER_SIZE];
        r.read_exact(&mut hdr)?;
        let num_tasks = u16::from_le_bytes([hdr[0], hdr[1]]) as usize;
        let num_programs = u16::from_le_bytes([hdr[2], hdr[3]]) as usize;
        let shared_globals_size = u16::from_le_bytes([hdr[4], hdr[5]]);

        let mut tasks = Vec::with_capacity(num_tasks);
        for _ in 0..num_tasks {
            let mut buf = [0u8; TaskEntry::SIZE];
            r.read_exact(&mut buf)?;
            tasks.push(TaskEntry::from_bytes(&buf)?);
        }

        let mut programs = Vec::with_capacity(num_programs);
        for _ in 0..num_programs {
            let mut buf = [0u8; ProgramInstanceEntry::SIZE];
            r.read_exact(&mut buf)?;
            programs.push(ProgramInstanceEntry::from_bytes(&buf));
        }

        Ok(TaskTable {
            shared_globals_size,
            tasks,
            programs,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::vec;
    use std::vec::Vec;

    #[test]
    fn section_size_when_empty_then_returns_header_only() {
        let table = TaskTable::default();
        assert_eq!(table.section_size(), 6);
    }

    #[test]
    fn section_size_when_tasks_and_programs_then_returns_correct_size() {
        let table = TaskTable {
            shared_globals_size: 0,
            tasks: vec![TaskEntry {
                task_id: TaskId::new(0),
                priority: 0,
                task_type: TaskType::Cyclic,
                flags: 0,
                interval_us: 0,
                single_var_index: VarIndex::new(0),
                watchdog_us: 0,
                input_image_offset: 0,
                output_image_offset: 0,
                reserved: [0; 4],
            }],
            programs: vec![ProgramInstanceEntry {
                instance_id: InstanceId::new(0),
                task_id: TaskId::new(0),
                entry_function_id: FunctionId::new(0),
                var_table_offset: 0,
                var_table_count: 0,
                fb_instance_offset: 0,
                fb_instance_count: 0,
                init_function_id: FunctionId::new(0),
            }],
        };
        // 6 (header) + 32 (1 task) + 16 (1 program) = 54
        assert_eq!(table.section_size(), 54);
    }

    #[test]
    fn task_table_write_read_when_cyclic_task_then_roundtrips() {
        let table = TaskTable {
            shared_globals_size: 128,
            tasks: vec![TaskEntry {
                task_id: TaskId::new(1),
                priority: 10,
                task_type: TaskType::Cyclic,
                flags: 0x01,
                interval_us: 10_000,
                single_var_index: VarIndex::NO_SINGLE_VAR,
                watchdog_us: 50_000,
                input_image_offset: 0,
                output_image_offset: 64,
                reserved: [0; 4],
            }],
            programs: vec![ProgramInstanceEntry {
                instance_id: InstanceId::new(0),
                task_id: TaskId::new(1),
                entry_function_id: FunctionId::new(0),
                var_table_offset: 0,
                var_table_count: 5,
                fb_instance_offset: 0,
                fb_instance_count: 0,
                init_function_id: FunctionId::new(0),
            }],
        };

        let mut buf = Vec::new();
        table.write_to(&mut buf).unwrap();
        assert_eq!(buf.len(), table.section_size() as usize);

        let mut cursor = Cursor::new(&buf);
        let decoded = TaskTable::read_from(&mut cursor).unwrap();

        assert_eq!(decoded.shared_globals_size, 128);
        assert_eq!(decoded.tasks.len(), 1);
        assert_eq!(decoded.tasks[0].task_id, TaskId::new(1));
        assert_eq!(decoded.tasks[0].priority, 10);
        assert_eq!(decoded.tasks[0].task_type, TaskType::Cyclic);
        assert_eq!(decoded.tasks[0].flags, 0x01);
        assert_eq!(decoded.tasks[0].interval_us, 10_000);
        assert_eq!(decoded.tasks[0].single_var_index, VarIndex::NO_SINGLE_VAR);
        assert_eq!(decoded.tasks[0].watchdog_us, 50_000);
        assert_eq!(decoded.tasks[0].input_image_offset, 0);
        assert_eq!(decoded.tasks[0].output_image_offset, 64);
        assert_eq!(decoded.tasks[0].reserved, [0; 4]);

        assert_eq!(decoded.programs.len(), 1);
        assert_eq!(decoded.programs[0].instance_id, InstanceId::new(0));
        assert_eq!(decoded.programs[0].task_id, TaskId::new(1));
        assert_eq!(decoded.programs[0].entry_function_id, FunctionId::new(0));
        assert_eq!(decoded.programs[0].var_table_offset, 0);
        assert_eq!(decoded.programs[0].var_table_count, 5);
        assert_eq!(decoded.programs[0].fb_instance_offset, 0);
        assert_eq!(decoded.programs[0].fb_instance_count, 0);
        assert_eq!(decoded.programs[0].init_function_id, FunctionId::new(0));
    }

    #[test]
    fn task_table_write_read_when_empty_then_roundtrips() {
        let table = TaskTable::default();

        let mut buf = Vec::new();
        table.write_to(&mut buf).unwrap();
        assert_eq!(buf.len(), 6);

        let mut cursor = Cursor::new(&buf);
        let decoded = TaskTable::read_from(&mut cursor).unwrap();

        assert_eq!(decoded.shared_globals_size, 0);
        assert_eq!(decoded.tasks.len(), 0);
        assert_eq!(decoded.programs.len(), 0);
    }

    #[test]
    fn task_table_read_when_invalid_task_type_then_error() {
        // Build a buffer with header (1 task, 0 programs, 0 globals size)
        // followed by a task entry with an invalid task type byte (0xFF).
        let mut buf = Vec::new();
        buf.extend_from_slice(&1u16.to_le_bytes()); // num_tasks
        buf.extend_from_slice(&0u16.to_le_bytes()); // num_programs
        buf.extend_from_slice(&0u16.to_le_bytes()); // shared_globals_size
                                                    // 32-byte task entry with invalid task_type at offset 4
        let mut entry = [0u8; TaskEntry::SIZE];
        entry[4] = 0xFF; // invalid task type
        buf.extend_from_slice(&entry);

        let mut cursor = Cursor::new(&buf);
        let result = TaskTable::read_from(&mut cursor);

        assert!(matches!(result, Err(ContainerError::InvalidTaskType(0xFF))));
    }

    #[test]
    fn task_type_as_str_when_cyclic_then_returns_cyclic_string() {
        assert_eq!(TaskType::Cyclic.as_str(), "Cyclic");
    }

    #[test]
    fn task_type_as_str_when_event_then_returns_event_string() {
        assert_eq!(TaskType::Event.as_str(), "Event");
    }

    #[test]
    fn task_type_as_str_when_freewheeling_then_returns_freewheeling_string() {
        assert_eq!(TaskType::Freewheeling.as_str(), "Freewheeling");
    }
}
