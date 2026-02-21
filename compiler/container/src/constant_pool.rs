use std::io::{Read, Write};

use crate::ContainerError;

/// Type tags for constant pool entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ConstType {
    I32 = 0,
    U32 = 1,
    I64 = 2,
    U64 = 3,
    F32 = 4,
    F64 = 5,
}

impl ConstType {
    fn from_u8(v: u8) -> Result<Self, ContainerError> {
        match v {
            0 => Ok(ConstType::I32),
            1 => Ok(ConstType::U32),
            2 => Ok(ConstType::I64),
            3 => Ok(ConstType::U64),
            4 => Ok(ConstType::F32),
            5 => Ok(ConstType::F64),
            _ => Err(ContainerError::InvalidConstantType(v)),
        }
    }
}

/// A single entry in the constant pool.
#[derive(Clone, Debug)]
pub struct ConstEntry {
    pub const_type: ConstType,
    pub value: Vec<u8>,
}

/// The constant pool section of a bytecode container.
#[derive(Clone, Debug, Default)]
pub struct ConstantPool {
    pub entries: Vec<ConstEntry>,
}

impl ConstantPool {
    /// Returns the serialized size of this constant pool section in bytes.
    ///
    /// Format: count(u16) + for each entry: type(u8) + reserved(u8) + size(u16) + value
    pub fn section_size(&self) -> u32 {
        let mut size: u32 = 2; // count
        for entry in &self.entries {
            // type(1) + reserved(1) + size(2) + value
            size += 4 + entry.value.len() as u32;
        }
        size
    }

    /// Gets an i32 value from the constant pool at the given index.
    pub fn get_i32(&self, index: u16) -> Result<i32, ContainerError> {
        let entry = self
            .entries
            .get(index as usize)
            .ok_or(ContainerError::InvalidConstantIndex(index))?;
        if entry.const_type != ConstType::I32 {
            return Err(ContainerError::InvalidConstantType(
                entry.const_type as u8,
            ));
        }
        Ok(i32::from_le_bytes([
            entry.value[0],
            entry.value[1],
            entry.value[2],
            entry.value[3],
        ]))
    }

    /// Writes the constant pool to the given writer.
    pub fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        w.write_all(&(self.entries.len() as u16).to_le_bytes())?;
        for entry in &self.entries {
            w.write_all(&[entry.const_type as u8])?;
            w.write_all(&[0u8])?; // reserved
            w.write_all(&(entry.value.len() as u16).to_le_bytes())?;
            w.write_all(&entry.value)?;
        }
        Ok(())
    }

    /// Reads a constant pool from the given reader.
    pub fn read_from(r: &mut impl Read) -> Result<Self, ContainerError> {
        let mut buf2 = [0u8; 2];
        r.read_exact(&mut buf2)?;
        let count = u16::from_le_bytes(buf2) as usize;

        let mut entries = Vec::with_capacity(count);
        for _ in 0..count {
            let mut hdr = [0u8; 4];
            r.read_exact(&mut hdr)?;
            let const_type = ConstType::from_u8(hdr[0])?;
            // hdr[1] is reserved
            let size = u16::from_le_bytes([hdr[2], hdr[3]]) as usize;
            let mut value = vec![0u8; size];
            r.read_exact(&mut value)?;
            entries.push(ConstEntry { const_type, value });
        }

        Ok(ConstantPool { entries })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn constant_pool_write_read_when_i32_constants_then_roundtrips() {
        let pool = ConstantPool {
            entries: vec![
                ConstEntry {
                    const_type: ConstType::I32,
                    value: 10i32.to_le_bytes().to_vec(),
                },
                ConstEntry {
                    const_type: ConstType::I32,
                    value: 32i32.to_le_bytes().to_vec(),
                },
            ],
        };

        let mut buf = Vec::new();
        pool.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = ConstantPool::read_from(&mut cursor).unwrap();

        assert_eq!(decoded.entries.len(), 2);
        assert_eq!(decoded.entries[0].const_type, ConstType::I32);
        assert_eq!(decoded.entries[0].value, 10i32.to_le_bytes());
        assert_eq!(decoded.entries[1].const_type, ConstType::I32);
        assert_eq!(decoded.entries[1].value, 32i32.to_le_bytes());
    }

    #[test]
    fn constant_pool_get_i32_when_valid_index_then_returns_value() {
        let pool = ConstantPool {
            entries: vec![
                ConstEntry {
                    const_type: ConstType::I32,
                    value: 42i32.to_le_bytes().to_vec(),
                },
            ],
        };

        assert_eq!(pool.get_i32(0).unwrap(), 42);
    }

    #[test]
    fn constant_pool_get_i32_when_out_of_bounds_then_error() {
        let pool = ConstantPool {
            entries: vec![],
        };

        assert!(matches!(
            pool.get_i32(0),
            Err(ContainerError::InvalidConstantIndex(0))
        ));
    }
}
