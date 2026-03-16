use std::io::{Read, Write};
use std::vec::Vec;

use crate::ContainerError;

/// Type tags for FB field entries.
///
/// These match the `var_type` encoding used in the variable table
/// (see the bytecode container format spec).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FieldType {
    I32 = 0,
    U32 = 1,
    I64 = 2,
    U64 = 3,
    F32 = 4,
    F64 = 5,
    String = 6,
    WString = 7,
    FbInstance = 8,
    Time = 9,
}

impl FieldType {
    /// Converts a raw `u8` to a `FieldType`, returning an error for unknown tags.
    pub fn from_u8(v: u8) -> Result<Self, ContainerError> {
        match v {
            0 => Ok(FieldType::I32),
            1 => Ok(FieldType::U32),
            2 => Ok(FieldType::I64),
            3 => Ok(FieldType::U64),
            4 => Ok(FieldType::F32),
            5 => Ok(FieldType::F64),
            6 => Ok(FieldType::String),
            7 => Ok(FieldType::WString),
            8 => Ok(FieldType::FbInstance),
            9 => Ok(FieldType::Time),
            _ => Err(ContainerError::InvalidFieldType(v)),
        }
    }
}

/// A single field entry within an FB type descriptor.
///
/// On disk this is 4 bytes: field_type (u8), reserved (u8), field_extra (u16 LE).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldEntry {
    pub field_type: FieldType,
    pub field_extra: u16,
}

/// Size of a single field entry on disk in bytes.
const FIELD_ENTRY_SIZE: usize = 4;

/// An FB type descriptor in the type section.
///
/// On disk: type_id (u16 LE), num_fields (u8), reserved (u8), then field entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FbTypeDescriptor {
    pub type_id: u16,
    pub fields: Vec<FieldEntry>,
}

/// An array descriptor in the type section.
///
/// On disk: element_type (u8), reserved (u8), total_elements (u32 LE),
/// element_extra (u16 LE). Total: 8 bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayDescriptor {
    /// Element type (same encoding as VarEntry.var_type):
    /// 0=I32, 1=U32, 2=I64, 3=U64, 4=F32, 5=F64.
    pub element_type: u8,
    /// Total number of elements across all dimensions.
    pub total_elements: u32,
    /// For STRING/WSTRING elements: max length. For FB elements: fb_type_id.
    /// For other types: 0.
    pub element_extra: u16,
}

/// Size of a single array descriptor on disk in bytes.
const ARRAY_DESCRIPTOR_SIZE: usize = 8;

/// The type section of a bytecode container.
///
/// Contains FB type descriptors and array descriptors used by the verifier
/// for type safety checking. The interpreter does not read this section.
#[derive(Clone, Debug, Default)]
pub struct TypeSection {
    pub fb_types: Vec<FbTypeDescriptor>,
    pub array_descriptors: Vec<ArrayDescriptor>,
}

impl TypeSection {
    /// Returns the on-disk size of the type section in bytes.
    pub fn section_size(&self) -> u32 {
        // FB types: count (2) + each descriptor header (4) + fields (4 each)
        let mut size: u32 = 2;
        for desc in &self.fb_types {
            size += 4 + desc.fields.len() as u32 * FIELD_ENTRY_SIZE as u32;
        }
        // Array descriptors: count (2) + each descriptor (8)
        size += 2 + self.array_descriptors.len() as u32 * ARRAY_DESCRIPTOR_SIZE as u32;
        size
    }

    /// Writes the type section to the given writer.
    ///
    /// Format: FB type count (u16 LE) + FB type descriptors,
    /// then array descriptor count (u16 LE) + array descriptors.
    pub fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        // FB type descriptors
        w.write_all(&(self.fb_types.len() as u16).to_le_bytes())?;
        for desc in &self.fb_types {
            w.write_all(&desc.type_id.to_le_bytes())?;
            w.write_all(&[desc.fields.len() as u8])?;
            w.write_all(&[0u8])?; // reserved
            for field in &desc.fields {
                w.write_all(&[field.field_type as u8])?;
                w.write_all(&[0u8])?; // reserved
                w.write_all(&field.field_extra.to_le_bytes())?;
            }
        }
        // Array descriptors
        w.write_all(&(self.array_descriptors.len() as u16).to_le_bytes())?;
        for desc in &self.array_descriptors {
            w.write_all(&[desc.element_type])?;
            w.write_all(&[0u8])?; // reserved
            w.write_all(&desc.total_elements.to_le_bytes())?;
            w.write_all(&desc.element_extra.to_le_bytes())?;
        }
        Ok(())
    }

    /// Reads a type section from the given reader.
    pub fn read_from(r: &mut impl Read) -> Result<Self, ContainerError> {
        let mut buf2 = [0u8; 2];
        r.read_exact(&mut buf2)?;
        let count = u16::from_le_bytes(buf2) as usize;

        let mut fb_types = Vec::with_capacity(count);
        for _ in 0..count {
            let mut hdr = [0u8; 4];
            r.read_exact(&mut hdr)?;
            let type_id = u16::from_le_bytes([hdr[0], hdr[1]]);
            let num_fields = hdr[2] as usize;
            // hdr[3] is reserved

            let mut fields = Vec::with_capacity(num_fields);
            for _ in 0..num_fields {
                let mut entry_buf = [0u8; FIELD_ENTRY_SIZE];
                r.read_exact(&mut entry_buf)?;
                let field_type = FieldType::from_u8(entry_buf[0])?;
                // entry_buf[1] is reserved
                let field_extra = u16::from_le_bytes([entry_buf[2], entry_buf[3]]);
                fields.push(FieldEntry {
                    field_type,
                    field_extra,
                });
            }

            fb_types.push(FbTypeDescriptor { type_id, fields });
        }

        // Array descriptors
        r.read_exact(&mut buf2)?;
        let num_arrays = u16::from_le_bytes(buf2) as usize;

        let mut array_descriptors = Vec::with_capacity(num_arrays);
        for _ in 0..num_arrays {
            let mut desc_buf = [0u8; ARRAY_DESCRIPTOR_SIZE];
            r.read_exact(&mut desc_buf)?;
            let element_type = desc_buf[0];
            // desc_buf[1] is reserved
            let total_elements =
                u32::from_le_bytes([desc_buf[2], desc_buf[3], desc_buf[4], desc_buf[5]]);
            let element_extra = u16::from_le_bytes([desc_buf[6], desc_buf[7]]);
            array_descriptors.push(ArrayDescriptor {
                element_type,
                total_elements,
                element_extra,
            });
        }

        Ok(TypeSection {
            fb_types,
            array_descriptors,
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
    fn type_section_write_read_when_empty_then_roundtrips() {
        let section = TypeSection::default();

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = TypeSection::read_from(&mut cursor).unwrap();

        assert!(decoded.fb_types.is_empty());
    }

    #[test]
    fn type_section_write_read_when_ton_descriptor_then_roundtrips() {
        let section = TypeSection {
            array_descriptors: vec![],
            fb_types: vec![FbTypeDescriptor {
                type_id: 0x0010,
                fields: vec![
                    FieldEntry {
                        field_type: FieldType::I32,
                        field_extra: 0,
                    },
                    FieldEntry {
                        field_type: FieldType::Time,
                        field_extra: 0,
                    },
                    FieldEntry {
                        field_type: FieldType::I32,
                        field_extra: 0,
                    },
                    FieldEntry {
                        field_type: FieldType::Time,
                        field_extra: 0,
                    },
                    FieldEntry {
                        field_type: FieldType::Time,
                        field_extra: 0,
                    },
                    FieldEntry {
                        field_type: FieldType::I32,
                        field_extra: 0,
                    },
                ],
            }],
        };

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = TypeSection::read_from(&mut cursor).unwrap();

        assert_eq!(decoded.fb_types.len(), 1);
        let desc = &decoded.fb_types[0];
        assert_eq!(desc.type_id, 0x0010);
        assert_eq!(desc.fields.len(), 6);
        assert_eq!(desc.fields[0].field_type, FieldType::I32);
        assert_eq!(desc.fields[1].field_type, FieldType::Time);
        assert_eq!(desc.fields[2].field_type, FieldType::I32);
        assert_eq!(desc.fields[3].field_type, FieldType::Time);
        assert_eq!(desc.fields[4].field_type, FieldType::Time);
        assert_eq!(desc.fields[5].field_type, FieldType::I32);
    }

    #[test]
    fn field_type_from_u8_when_invalid_then_returns_error() {
        assert!(matches!(
            FieldType::from_u8(42),
            Err(ContainerError::InvalidFieldType(42))
        ));
    }

    #[test]
    fn type_section_write_read_when_array_descriptors_then_roundtrips() {
        let section = TypeSection {
            fb_types: vec![],
            array_descriptors: vec![
                ArrayDescriptor {
                    element_type: 0, // I32
                    total_elements: 10,
                    element_extra: 0,
                },
                ArrayDescriptor {
                    element_type: 2, // I64
                    total_elements: 100,
                    element_extra: 0,
                },
            ],
        };

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = TypeSection::read_from(&mut cursor).unwrap();

        assert!(decoded.fb_types.is_empty());
        assert_eq!(decoded.array_descriptors.len(), 2);
        assert_eq!(decoded.array_descriptors[0].element_type, 0);
        assert_eq!(decoded.array_descriptors[0].total_elements, 10);
        assert_eq!(decoded.array_descriptors[0].element_extra, 0);
        assert_eq!(decoded.array_descriptors[1].element_type, 2);
        assert_eq!(decoded.array_descriptors[1].total_elements, 100);
    }

    #[test]
    fn type_section_write_read_when_fb_and_array_descriptors_then_roundtrips() {
        let section = TypeSection {
            fb_types: vec![FbTypeDescriptor {
                type_id: 0x0010,
                fields: vec![FieldEntry {
                    field_type: FieldType::I32,
                    field_extra: 0,
                }],
            }],
            array_descriptors: vec![ArrayDescriptor {
                element_type: 5, // F64
                total_elements: 32768,
                element_extra: 0,
            }],
        };

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = TypeSection::read_from(&mut cursor).unwrap();

        assert_eq!(decoded.fb_types.len(), 1);
        assert_eq!(decoded.fb_types[0].type_id, 0x0010);
        assert_eq!(decoded.array_descriptors.len(), 1);
        assert_eq!(decoded.array_descriptors[0].element_type, 5);
        assert_eq!(decoded.array_descriptors[0].total_elements, 32768);
    }

    #[test]
    fn type_section_section_size_when_empty_then_returns_header_only() {
        let section = TypeSection::default();
        // 2 bytes for FB count + 2 bytes for array count
        assert_eq!(section.section_size(), 4);
    }

    #[test]
    fn type_section_section_size_when_descriptors_then_returns_correct_size() {
        let section = TypeSection {
            fb_types: vec![FbTypeDescriptor {
                type_id: 0x0010,
                fields: vec![
                    FieldEntry {
                        field_type: FieldType::I32,
                        field_extra: 0,
                    },
                    FieldEntry {
                        field_type: FieldType::I64,
                        field_extra: 0,
                    },
                ],
            }],
            array_descriptors: vec![ArrayDescriptor {
                element_type: 0,
                total_elements: 10,
                element_extra: 0,
            }],
        };
        // FB: 2 (count) + 4 (header) + 2*4 (fields) = 14
        // Arrays: 2 (count) + 1*8 (descriptors) = 10
        assert_eq!(section.section_size(), 24);

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();
        assert_eq!(buf.len(), section.section_size() as usize);
    }
}
