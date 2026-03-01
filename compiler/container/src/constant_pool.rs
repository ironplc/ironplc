use std::io::{Read, Write};
use std::vec;
use std::vec::Vec;

use crate::const_type::ConstType;
use crate::ContainerError;

/// A single entry in the constant pool.
#[derive(Clone, Debug)]
pub struct ConstEntry {
    pub const_type: ConstType,
    pub value: Vec<u8>,
}

/// The constant pool section of a bytecode container.
#[derive(Clone, Debug, Default)]
pub struct ConstantPool {
    entries: Vec<ConstEntry>,
}

impl ConstantPool {
    /// Adds a constant entry.
    pub fn push(&mut self, entry: ConstEntry) {
        self.entries.push(entry);
    }

    /// Returns the number of entries in the constant pool.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if the constant pool has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns an iterator over the constant pool entries.
    pub fn iter(&self) -> std::slice::Iter<'_, ConstEntry> {
        self.entries.iter()
    }

    /// Returns the serialized size of this constant pool section in bytes.
    ///
    /// Only called at construction/save time, not during execution.
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
            return Err(ContainerError::InvalidConstantType(entry.const_type as u8));
        }
        Ok(i32::from_le_bytes([
            entry.value[0],
            entry.value[1],
            entry.value[2],
            entry.value[3],
        ]))
    }

    /// Gets an i64 value from the constant pool at the given index.
    pub fn get_i64(&self, index: u16) -> Result<i64, ContainerError> {
        let entry = self
            .entries
            .get(index as usize)
            .ok_or(ContainerError::InvalidConstantIndex(index))?;
        if entry.const_type != ConstType::I64 {
            return Err(ContainerError::InvalidConstantType(entry.const_type as u8));
        }
        Ok(i64::from_le_bytes([
            entry.value[0],
            entry.value[1],
            entry.value[2],
            entry.value[3],
            entry.value[4],
            entry.value[5],
            entry.value[6],
            entry.value[7],
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
    use std::vec::Vec;

    #[test]
    fn constant_pool_write_read_when_i32_constants_then_roundtrips() {
        let mut pool = ConstantPool::default();
        pool.push(ConstEntry {
            const_type: ConstType::I32,
            value: 10i32.to_le_bytes().to_vec(),
        });
        pool.push(ConstEntry {
            const_type: ConstType::I32,
            value: 32i32.to_le_bytes().to_vec(),
        });

        let mut buf = Vec::new();
        pool.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = ConstantPool::read_from(&mut cursor).unwrap();

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded.get_i32(0).unwrap(), 10);
        assert_eq!(decoded.get_i32(1).unwrap(), 32);
    }

    #[test]
    fn constant_pool_get_i32_when_valid_index_then_returns_value() {
        let mut pool = ConstantPool::default();
        pool.push(ConstEntry {
            const_type: ConstType::I32,
            value: 42i32.to_le_bytes().to_vec(),
        });

        assert_eq!(pool.get_i32(0).unwrap(), 42);
    }

    #[test]
    fn constant_pool_get_i32_when_out_of_bounds_then_error() {
        let pool = ConstantPool::default();

        assert!(matches!(
            pool.get_i32(0),
            Err(ContainerError::InvalidConstantIndex(0))
        ));
    }

    #[test]
    fn constant_pool_iter_when_two_entries_then_returns_both() {
        let mut pool = ConstantPool::default();
        pool.push(ConstEntry {
            const_type: ConstType::I32,
            value: 10i32.to_le_bytes().to_vec(),
        });
        pool.push(ConstEntry {
            const_type: ConstType::F64,
            value: 3.14f64.to_le_bytes().to_vec(),
        });

        let entries: Vec<_> = pool.iter().collect();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].const_type, ConstType::I32);
        assert_eq!(entries[1].const_type, ConstType::F64);
    }

    #[test]
    fn const_type_as_str_when_i32_then_returns_i32_string() {
        assert_eq!(ConstType::I32.as_str(), "I32");
    }

    #[test]
    fn const_type_as_str_when_f64_then_returns_f64_string() {
        assert_eq!(ConstType::F64.as_str(), "F64");
    }
}
