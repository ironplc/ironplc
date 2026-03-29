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
    /// Heterogeneous structure field slot. Used as the element type in array
    /// descriptors that back structure variables (which are treated as flat
    /// arrays of 8-byte slots). The VM does not check this value at runtime.
    Slot = 10,
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
            10 => Ok(FieldType::Slot),
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
/// Describes the element type and total number of elements for a single
/// array shape. Descriptors are deduplicated: multiple variables with
/// the same element type and size share one descriptor.
///
/// On disk this is 8 bytes:
/// `[element_type: u8] [reserved: u8] [total_elements: u32 LE] [element_extra: u16 LE]`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayDescriptor {
    /// Element type tag using the same encoding as [`FieldType`]
    /// (I32=0, U32=1, I64=2, U64=3, F32=4, F64=5, etc.).
    pub element_type: u8,
    /// Total number of elements across all dimensions.
    pub total_elements: u32,
}

/// Size of a single array descriptor on disk in bytes.
const ARRAY_DESCRIPTOR_SIZE: usize = 8;

/// A user-defined function block descriptor in the type section.
///
/// Maps a user-defined FB type ID to the compiled function that implements
/// its body, the variable table offset where its fields are mapped, and
/// the number of data-region fields in the instance.
///
/// On disk: type_id (u16 LE), function_id (u16 LE), var_offset (u16 LE),
/// num_fields (u8), reserved (u8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserFbDescriptor {
    pub type_id: u16,
    pub function_id: u16,
    pub var_offset: u16,
    pub num_fields: u8,
}

/// Size of a single user FB descriptor on disk in bytes.
const USER_FB_DESCRIPTOR_SIZE: usize = 8;

/// The type section of a bytecode container.
///
/// Contains FB type descriptors and array descriptors used by the verifier
/// and VM for type safety checking.
#[derive(Clone, Debug, Default)]
pub struct TypeSection {
    pub fb_types: Vec<FbTypeDescriptor>,
    pub array_descriptors: Vec<ArrayDescriptor>,
    pub user_fb_types: Vec<UserFbDescriptor>,
}

impl TypeSection {
    /// Returns the total serialized size of this section in bytes.
    pub fn section_size(&self) -> u32 {
        // FB types: count(2) + sum of (header(4) + fields * 4)
        let mut size: u32 = 2;
        for desc in &self.fb_types {
            size += 4 + desc.fields.len() as u32 * FIELD_ENTRY_SIZE as u32;
        }
        // Array descriptors: count(2) + descriptors * 8
        size += 2 + self.array_descriptors.len() as u32 * ARRAY_DESCRIPTOR_SIZE as u32;
        // User FB descriptors: count(2) + descriptors * 8
        size += 2 + self.user_fb_types.len() as u32 * USER_FB_DESCRIPTOR_SIZE as u32;
        size
    }

    /// Writes the type section to the given writer.
    ///
    /// Format: FB count (u16 LE), FB descriptors, array count (u16 LE), array descriptors.
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
            w.write_all(&[0u8; 2])?; // element_extra (reserved)
        }

        // User FB descriptors
        w.write_all(&(self.user_fb_types.len() as u16).to_le_bytes())?;
        for desc in &self.user_fb_types {
            w.write_all(&desc.type_id.to_le_bytes())?;
            w.write_all(&desc.function_id.to_le_bytes())?;
            w.write_all(&desc.var_offset.to_le_bytes())?;
            w.write_all(&[desc.num_fields])?;
            w.write_all(&[0u8])?; // reserved
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
        let mut buf2 = [0u8; 2];
        r.read_exact(&mut buf2)?;
        let array_count = u16::from_le_bytes(buf2) as usize;

        let mut array_descriptors = Vec::with_capacity(array_count);
        for _ in 0..array_count {
            let mut desc_buf = [0u8; ARRAY_DESCRIPTOR_SIZE];
            r.read_exact(&mut desc_buf)?;
            let element_type = desc_buf[0];
            // desc_buf[1] is reserved
            let total_elements =
                u32::from_le_bytes([desc_buf[2], desc_buf[3], desc_buf[4], desc_buf[5]]);
            // desc_buf[6..8] is element_extra (reserved)
            array_descriptors.push(ArrayDescriptor {
                element_type,
                total_elements,
            });
        }

        // User FB descriptors
        let mut buf2 = [0u8; 2];
        let user_fb_count = if r.read_exact(&mut buf2).is_ok() {
            u16::from_le_bytes(buf2) as usize
        } else {
            0
        };

        let mut user_fb_types = Vec::with_capacity(user_fb_count);
        for _ in 0..user_fb_count {
            let mut desc_buf = [0u8; USER_FB_DESCRIPTOR_SIZE];
            r.read_exact(&mut desc_buf)?;
            let type_id = u16::from_le_bytes([desc_buf[0], desc_buf[1]]);
            let function_id = u16::from_le_bytes([desc_buf[2], desc_buf[3]]);
            let var_offset = u16::from_le_bytes([desc_buf[4], desc_buf[5]]);
            let num_fields = desc_buf[6];
            // desc_buf[7] is reserved
            user_fb_types.push(UserFbDescriptor {
                type_id,
                function_id,
                var_offset,
                num_fields,
            });
        }

        Ok(TypeSection {
            fb_types,
            array_descriptors,
            user_fb_types,
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
        assert!(decoded.array_descriptors.is_empty());
    }

    #[test]
    fn type_section_write_read_when_ton_descriptor_then_roundtrips() {
        let section = TypeSection {
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
            array_descriptors: vec![],
            user_fb_types: vec![],
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
    fn type_section_write_read_when_array_descriptors_then_roundtrips() {
        let section = TypeSection {
            fb_types: vec![],
            array_descriptors: vec![
                ArrayDescriptor {
                    element_type: FieldType::I32 as u8,
                    total_elements: 10,
                },
                ArrayDescriptor {
                    element_type: FieldType::F64 as u8,
                    total_elements: 32768,
                },
            ],
            user_fb_types: vec![],
        };

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = TypeSection::read_from(&mut cursor).unwrap();

        assert!(decoded.fb_types.is_empty());
        assert_eq!(decoded.array_descriptors.len(), 2);
        assert_eq!(
            decoded.array_descriptors[0].element_type,
            FieldType::I32 as u8
        );
        assert_eq!(decoded.array_descriptors[0].total_elements, 10);
        assert_eq!(
            decoded.array_descriptors[1].element_type,
            FieldType::F64 as u8
        );
        assert_eq!(decoded.array_descriptors[1].total_elements, 32768);
    }

    #[test]
    fn type_section_write_read_when_fb_and_array_descriptors_then_roundtrips() {
        let section = TypeSection {
            fb_types: vec![FbTypeDescriptor {
                type_id: 1,
                fields: vec![FieldEntry {
                    field_type: FieldType::I32,
                    field_extra: 0,
                }],
            }],
            array_descriptors: vec![ArrayDescriptor {
                element_type: FieldType::U32 as u8,
                total_elements: 100,
            }],
            user_fb_types: vec![],
        };

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = TypeSection::read_from(&mut cursor).unwrap();

        assert_eq!(decoded.fb_types.len(), 1);
        assert_eq!(decoded.fb_types[0].type_id, 1);
        assert_eq!(decoded.array_descriptors.len(), 1);
        assert_eq!(
            decoded.array_descriptors[0].element_type,
            FieldType::U32 as u8
        );
        assert_eq!(decoded.array_descriptors[0].total_elements, 100);
    }

    #[test]
    fn section_size_when_empty_then_returns_header_counts_only() {
        let section = TypeSection::default();
        // 2 bytes for FB count + 2 bytes for array count + 2 bytes for user FB count
        assert_eq!(section.section_size(), 6);
    }

    #[test]
    fn section_size_when_array_descriptors_then_includes_descriptor_bytes() {
        let section = TypeSection {
            fb_types: vec![],
            array_descriptors: vec![
                ArrayDescriptor {
                    element_type: 0,
                    total_elements: 10,
                },
                ArrayDescriptor {
                    element_type: 4,
                    total_elements: 20,
                },
            ],
            user_fb_types: vec![],
        };
        // 2 (FB count) + 2 (array count) + 2 * 8 (descriptors) + 2 (user FB count) = 22
        assert_eq!(section.section_size(), 22);
    }

    #[test]
    fn type_section_write_read_when_user_fb_descriptors_then_roundtrips() {
        let section = TypeSection {
            fb_types: vec![],
            array_descriptors: vec![],
            user_fb_types: vec![
                UserFbDescriptor {
                    type_id: 0x1000,
                    function_id: 2,
                    var_offset: 4,
                    num_fields: 3,
                },
                UserFbDescriptor {
                    type_id: 0x1001,
                    function_id: 3,
                    var_offset: 7,
                    num_fields: 5,
                },
            ],
        };

        let mut buf = Vec::new();
        section.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = TypeSection::read_from(&mut cursor).unwrap();

        assert_eq!(decoded.user_fb_types.len(), 2);
        assert_eq!(decoded.user_fb_types[0].type_id, 0x1000);
        assert_eq!(decoded.user_fb_types[0].function_id, 2);
        assert_eq!(decoded.user_fb_types[0].var_offset, 4);
        assert_eq!(decoded.user_fb_types[0].num_fields, 3);
        assert_eq!(decoded.user_fb_types[1].type_id, 0x1001);
        assert_eq!(decoded.user_fb_types[1].function_id, 3);
        assert_eq!(decoded.user_fb_types[1].var_offset, 7);
        assert_eq!(decoded.user_fb_types[1].num_fields, 5);
    }

    #[test]
    fn field_type_from_u8_when_invalid_then_returns_error() {
        assert!(matches!(
            FieldType::from_u8(42),
            Err(ContainerError::InvalidFieldType(42))
        ));
    }
}
