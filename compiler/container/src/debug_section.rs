use std::io::{Read, Write};
use std::string::String;
use std::vec;
use std::vec::Vec;

use crate::ContainerError;

// Sub-table tag constants.
const TAG_VAR_NAME: u16 = 2;
const TAG_FUNC_NAME: u16 = 3;

/// Size of each directory entry: tag(2) + reserved(2) + size(4) = 8 bytes.
const DIR_ENTRY_SIZE: u32 = 8;

/// IEC 61131-3 type tag for debug display interpretation.
///
/// See ADR-0019 for the full encoding table and rationale.
pub mod iec_type_tag {
    pub const BOOL: u8 = 0;
    pub const SINT: u8 = 1;
    pub const INT: u8 = 2;
    pub const DINT: u8 = 3;
    pub const LINT: u8 = 4;
    pub const USINT: u8 = 5;
    pub const UINT: u8 = 6;
    pub const UDINT: u8 = 7;
    pub const ULINT: u8 = 8;
    pub const REAL: u8 = 9;
    pub const LREAL: u8 = 10;
    pub const BYTE: u8 = 11;
    pub const WORD: u8 = 12;
    pub const DWORD: u8 = 13;
    pub const LWORD: u8 = 14;
    pub const STRING: u8 = 15;
    pub const WSTRING: u8 = 16;
    pub const TIME: u8 = 17;
    pub const LTIME: u8 = 18;
    pub const DATE: u8 = 19;
    pub const LDATE: u8 = 20;
    pub const TIME_OF_DAY: u8 = 21;
    pub const LTOD: u8 = 22;
    pub const DATE_AND_TIME: u8 = 23;
    pub const LDT: u8 = 24;
    pub const OTHER: u8 = 255;
}

/// Function ID constants for debug variable ownership.
pub mod function_id {
    /// Indicates a variable belongs to program/global scope (not a specific function).
    pub const GLOBAL_SCOPE: u16 = 0xFFFF;
}

/// IEC 61131-3 variable section encoding.
pub mod var_section {
    pub const VAR: u8 = 0;
    pub const VAR_TEMP: u8 = 1;
    pub const VAR_INPUT: u8 = 2;
    pub const VAR_OUTPUT: u8 = 3;
    pub const VAR_IN_OUT: u8 = 4;
    pub const VAR_EXTERNAL: u8 = 5;
    pub const VAR_GLOBAL: u8 = 6;
}

/// A variable name entry (debug section Tag 2).
#[derive(Clone, Debug, PartialEq)]
pub struct VarNameEntry {
    pub var_index: u16,
    pub function_id: u16,
    pub var_section: u8,
    pub iec_type_tag: u8,
    pub name: String,
    pub type_name: String,
}

/// A function name entry (debug section Tag 3).
#[derive(Clone, Debug, PartialEq)]
pub struct FuncNameEntry {
    pub function_id: u16,
    pub name: String,
}

/// The debug section of a bytecode container.
#[derive(Clone, Debug, Default)]
pub struct DebugSection {
    pub var_names: Vec<VarNameEntry>,
    pub func_names: Vec<FuncNameEntry>,
}

impl DebugSection {
    /// Returns the serialized size of this debug section in bytes.
    pub fn section_size(&self) -> u32 {
        let sub_table_count: u32 = self.num_sub_tables() as u32;
        // Header: sub_table_count(2) + directory entries
        let header_size = 2 + sub_table_count * DIR_ENTRY_SIZE;
        header_size + self.var_name_payload_size() + self.func_name_payload_size()
    }

    /// Writes the debug section to the given writer.
    pub fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        let sub_table_count = self.num_sub_tables();
        w.write_all(&(sub_table_count as u16).to_le_bytes())?;

        // Write directory entries for present sub-tables.
        if !self.var_names.is_empty() {
            w.write_all(&TAG_VAR_NAME.to_le_bytes())?;
            w.write_all(&0u16.to_le_bytes())?; // reserved
            w.write_all(&self.var_name_payload_size().to_le_bytes())?;
        }
        if !self.func_names.is_empty() {
            w.write_all(&TAG_FUNC_NAME.to_le_bytes())?;
            w.write_all(&0u16.to_le_bytes())?; // reserved
            w.write_all(&self.func_name_payload_size().to_le_bytes())?;
        }

        // Write payloads in directory order.
        if !self.var_names.is_empty() {
            self.write_var_names(w)?;
        }
        if !self.func_names.is_empty() {
            self.write_func_names(w)?;
        }

        Ok(())
    }

    /// Reads a debug section from the given reader.
    pub fn read_from(r: &mut impl Read) -> Result<Self, ContainerError> {
        let mut buf2 = [0u8; 2];
        r.read_exact(&mut buf2)?;
        let sub_table_count = u16::from_le_bytes(buf2) as usize;

        // Read directory entries.
        let mut directory = Vec::with_capacity(sub_table_count);
        for _ in 0..sub_table_count {
            let mut entry_buf = [0u8; 8];
            r.read_exact(&mut entry_buf)?;
            let tag = u16::from_le_bytes([entry_buf[0], entry_buf[1]]);
            // entry_buf[2..4] is reserved
            let size = u32::from_le_bytes([entry_buf[4], entry_buf[5], entry_buf[6], entry_buf[7]]);
            directory.push((tag, size));
        }

        let mut var_names = Vec::new();
        let mut func_names = Vec::new();

        // Read payloads in directory order, skipping unknown tags.
        for (tag, size) in &directory {
            match *tag {
                TAG_VAR_NAME => {
                    var_names = Self::read_var_names(r, *size)?;
                }
                TAG_FUNC_NAME => {
                    func_names = Self::read_func_names(r, *size)?;
                }
                _ => {
                    // Skip unknown tags by reading and discarding their payload.
                    let mut skip_buf = vec![0u8; *size as usize];
                    r.read_exact(&mut skip_buf)?;
                }
            }
        }

        Ok(DebugSection {
            var_names,
            func_names,
        })
    }

    fn num_sub_tables(&self) -> usize {
        let mut count = 0;
        if !self.var_names.is_empty() {
            count += 1;
        }
        if !self.func_names.is_empty() {
            count += 1;
        }
        count
    }

    fn var_name_payload_size(&self) -> u32 {
        if self.var_names.is_empty() {
            return 0;
        }
        let mut size: u32 = 2; // count
        for entry in &self.var_names {
            // var_index(2) + function_id(2) + var_section(1) + iec_type_tag(1)
            // + name_len(1) + name + type_name_len(1) + type_name
            size += 8 + entry.name.len() as u32 + entry.type_name.len() as u32;
        }
        size
    }

    fn func_name_payload_size(&self) -> u32 {
        if self.func_names.is_empty() {
            return 0;
        }
        let mut size: u32 = 2; // count
        for entry in &self.func_names {
            // function_id(2) + name_len(1) + name
            size += 3 + entry.name.len() as u32;
        }
        size
    }

    fn write_var_names(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        w.write_all(&(self.var_names.len() as u16).to_le_bytes())?;
        for entry in &self.var_names {
            w.write_all(&entry.var_index.to_le_bytes())?;
            w.write_all(&entry.function_id.to_le_bytes())?;
            w.write_all(&[entry.var_section])?;
            w.write_all(&[entry.iec_type_tag])?;
            w.write_all(&[entry.name.len() as u8])?;
            w.write_all(entry.name.as_bytes())?;
            w.write_all(&[entry.type_name.len() as u8])?;
            w.write_all(entry.type_name.as_bytes())?;
        }
        Ok(())
    }

    fn write_func_names(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        w.write_all(&(self.func_names.len() as u16).to_le_bytes())?;
        for entry in &self.func_names {
            w.write_all(&entry.function_id.to_le_bytes())?;
            w.write_all(&[entry.name.len() as u8])?;
            w.write_all(entry.name.as_bytes())?;
        }
        Ok(())
    }

    fn read_var_names(r: &mut impl Read, _size: u32) -> Result<Vec<VarNameEntry>, ContainerError> {
        let mut buf2 = [0u8; 2];
        r.read_exact(&mut buf2)?;
        let count = u16::from_le_bytes(buf2) as usize;

        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let mut hdr = [0u8; 6];
            r.read_exact(&mut hdr)?;
            let var_index = u16::from_le_bytes([hdr[0], hdr[1]]);
            let function_id = u16::from_le_bytes([hdr[2], hdr[3]]);
            let var_section_val = hdr[4];
            let iec_type_tag_val = hdr[5];

            let mut len_buf = [0u8; 1];
            r.read_exact(&mut len_buf)?;
            let name_len = len_buf[0] as usize;
            let mut name_buf = vec![0u8; name_len];
            r.read_exact(&mut name_buf)?;
            let name =
                String::from_utf8(name_buf).map_err(|_| ContainerError::InvalidDebugSection)?;

            r.read_exact(&mut len_buf)?;
            let type_name_len = len_buf[0] as usize;
            let mut type_name_buf = vec![0u8; type_name_len];
            r.read_exact(&mut type_name_buf)?;
            let type_name = String::from_utf8(type_name_buf)
                .map_err(|_| ContainerError::InvalidDebugSection)?;

            entries.push(VarNameEntry {
                var_index,
                function_id,
                var_section: var_section_val,
                iec_type_tag: iec_type_tag_val,
                name,
                type_name,
            });
        }
        Ok(entries)
    }

    fn read_func_names(
        r: &mut impl Read,
        _size: u32,
    ) -> Result<Vec<FuncNameEntry>, ContainerError> {
        let mut buf2 = [0u8; 2];
        r.read_exact(&mut buf2)?;
        let count = u16::from_le_bytes(buf2) as usize;

        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let mut id_buf = [0u8; 2];
            r.read_exact(&mut id_buf)?;
            let function_id = u16::from_le_bytes(id_buf);

            let mut len_buf = [0u8; 1];
            r.read_exact(&mut len_buf)?;
            let name_len = len_buf[0] as usize;
            let mut name_buf = vec![0u8; name_len];
            r.read_exact(&mut name_buf)?;
            let name =
                String::from_utf8(name_buf).map_err(|_| ContainerError::InvalidDebugSection)?;

            entries.push(FuncNameEntry { function_id, name });
        }
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn debug_section_write_read_when_var_names_then_roundtrips() {
        let section = DebugSection {
            var_names: vec![
                VarNameEntry {
                    var_index: 0,
                    function_id: function_id::GLOBAL_SCOPE,
                    var_section: var_section::VAR,
                    iec_type_tag: iec_type_tag::DINT,
                    name: "counter".into(),
                    type_name: "DINT".into(),
                },
                VarNameEntry {
                    var_index: 1,
                    function_id: function_id::GLOBAL_SCOPE,
                    var_section: var_section::VAR,
                    iec_type_tag: iec_type_tag::REAL,
                    name: "temp".into(),
                    type_name: "REAL".into(),
                },
            ],
            func_names: vec![],
        };

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        let decoded = DebugSection::read_from(&mut Cursor::new(&buf)).unwrap();
        assert_eq!(decoded.var_names.len(), 2);
        assert_eq!(decoded.var_names[0].name, "counter");
        assert_eq!(decoded.var_names[0].iec_type_tag, iec_type_tag::DINT);
        assert_eq!(decoded.var_names[0].type_name, "DINT");
        assert_eq!(decoded.var_names[1].name, "temp");
        assert_eq!(decoded.var_names[1].iec_type_tag, iec_type_tag::REAL);
        assert!(decoded.func_names.is_empty());
    }

    #[test]
    fn debug_section_write_read_when_func_names_then_roundtrips() {
        let section = DebugSection {
            var_names: vec![],
            func_names: vec![
                FuncNameEntry {
                    function_id: 0,
                    name: "MAIN_init".into(),
                },
                FuncNameEntry {
                    function_id: 1,
                    name: "MAIN".into(),
                },
            ],
        };

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        let decoded = DebugSection::read_from(&mut Cursor::new(&buf)).unwrap();
        assert!(decoded.var_names.is_empty());
        assert_eq!(decoded.func_names.len(), 2);
        assert_eq!(decoded.func_names[0].function_id, 0);
        assert_eq!(decoded.func_names[0].name, "MAIN_init");
        assert_eq!(decoded.func_names[1].function_id, 1);
        assert_eq!(decoded.func_names[1].name, "MAIN");
    }

    #[test]
    fn debug_section_write_read_when_both_tables_then_roundtrips() {
        let section = DebugSection {
            var_names: vec![VarNameEntry {
                var_index: 0,
                function_id: 1,
                var_section: var_section::VAR_INPUT,
                iec_type_tag: iec_type_tag::BOOL,
                name: "active".into(),
                type_name: "BOOL".into(),
            }],
            func_names: vec![FuncNameEntry {
                function_id: 1,
                name: "MAIN".into(),
            }],
        };

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        let decoded = DebugSection::read_from(&mut Cursor::new(&buf)).unwrap();
        assert_eq!(decoded.var_names.len(), 1);
        assert_eq!(decoded.func_names.len(), 1);
        assert_eq!(decoded.var_names[0], section.var_names[0]);
        assert_eq!(decoded.func_names[0], section.func_names[0]);
    }

    #[test]
    fn debug_section_read_when_unknown_tag_then_skips() {
        // Build a debug section with an unknown tag (tag=99) followed by FUNC_NAME.
        let mut buf = Vec::new();
        // sub_table_count = 2
        buf.extend_from_slice(&2u16.to_le_bytes());
        // Directory entry 1: unknown tag 99, 4 bytes of payload
        buf.extend_from_slice(&99u16.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.extend_from_slice(&4u32.to_le_bytes());
        // Directory entry 2: FUNC_NAME
        let func_payload_size: u32 = 2 + 3 + 4; // count(2) + id(2)+len(1)+name(4)
        buf.extend_from_slice(&TAG_FUNC_NAME.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.extend_from_slice(&func_payload_size.to_le_bytes());
        // Unknown payload: 4 garbage bytes
        buf.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
        // FUNC_NAME payload: count=1, function_id=0, name="MAIN"
        buf.extend_from_slice(&1u16.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes());
        buf.push(4); // name_len
        buf.extend_from_slice(b"MAIN");

        let decoded = DebugSection::read_from(&mut Cursor::new(&buf)).unwrap();
        assert!(decoded.var_names.is_empty());
        assert_eq!(decoded.func_names.len(), 1);
        assert_eq!(decoded.func_names[0].name, "MAIN");
    }

    #[test]
    fn debug_section_read_when_empty_then_empty_tables() {
        // sub_table_count = 0
        let buf: Vec<u8> = vec![0, 0];
        let decoded = DebugSection::read_from(&mut Cursor::new(&buf)).unwrap();
        assert!(decoded.var_names.is_empty());
        assert!(decoded.func_names.is_empty());
    }

    #[test]
    fn debug_section_section_size_when_both_tables_then_correct() {
        let section = DebugSection {
            var_names: vec![VarNameEntry {
                var_index: 0,
                function_id: function_id::GLOBAL_SCOPE,
                var_section: var_section::VAR,
                iec_type_tag: iec_type_tag::DINT,
                name: "x".into(),
                type_name: "DINT".into(),
            }],
            func_names: vec![FuncNameEntry {
                function_id: 0,
                name: "MAIN".into(),
            }],
        };

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();
        assert_eq!(section.section_size(), buf.len() as u32);
    }
}
