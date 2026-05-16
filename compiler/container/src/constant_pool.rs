use std::boxed::Box;
use std::io::{Read, Write};
use std::vec;
use std::vec::Vec;

use crate::char_width::CharWidth;
use crate::const_type::ConstType;
use crate::id_types::ConstantIndex;
use crate::ContainerError;

/// A single entry in the constant pool.
///
/// Primitive values (I32/U32/I64/U64/F32/F64) are stored inline as
/// little-endian bytes in `primitive`, so that VM lookups need only a single
/// pointer chase from the entry vector. String constants live in `str_value`;
/// for non-string entries `str_value` is an empty (non-allocating) slice.
#[derive(Clone, Debug)]
pub struct ConstEntry {
    pub const_type: ConstType,
    primitive: [u8; 8],
    str_value: Box<[u8]>,
}

impl ConstEntry {
    /// Constructs a primitive entry from little-endian bytes.
    ///
    /// `bytes` must be at most 8 bytes; the source slice is interpreted as
    /// the native little-endian encoding of the primitive.
    pub fn primitive_le(const_type: ConstType, bytes: &[u8]) -> Self {
        debug_assert!(!matches!(const_type, ConstType::Str | ConstType::WStr));
        debug_assert!(bytes.len() <= 8);
        let mut primitive = [0u8; 8];
        primitive[..bytes.len()].copy_from_slice(bytes);
        Self {
            const_type,
            primitive,
            str_value: Box::default(),
        }
    }

    /// Constructs a narrow string entry (Latin-1, 1 byte per character).
    pub fn string(bytes: impl Into<Box<[u8]>>) -> Self {
        Self {
            const_type: ConstType::Str,
            primitive: [0u8; 8],
            str_value: bytes.into(),
        }
    }

    /// Constructs a wide string entry (UTF-16LE, 2 bytes per code unit).
    pub fn wstring(bytes: impl Into<Box<[u8]>>) -> Self {
        Self {
            const_type: ConstType::WStr,
            primitive: [0u8; 8],
            str_value: bytes.into(),
        }
    }

    /// Returns the on-wire bytes for this entry (little-endian for primitives,
    /// raw bytes for strings).
    pub fn bytes(&self) -> &[u8] {
        match self.const_type {
            ConstType::I32 | ConstType::U32 | ConstType::F32 => &self.primitive[..4],
            ConstType::I64 | ConstType::U64 | ConstType::F64 => &self.primitive[..8],
            ConstType::Str | ConstType::WStr => &self.str_value,
        }
    }

    /// Returns the per-code-unit byte width for string entries, or `None`
    /// for non-string entries.
    pub fn char_width(&self) -> Option<CharWidth> {
        self.const_type.char_width()
    }
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
            // type(1) + char_width(1) + size(2) + value
            size += 4 + entry.bytes().len() as u32;
        }
        size
    }

    /// Reads the first `N` bytes of the entry at `index`, after verifying its
    /// type matches `expected`.
    fn get_le_bytes<const N: usize>(
        &self,
        index: ConstantIndex,
        expected: ConstType,
    ) -> Result<[u8; N], ContainerError> {
        let entry = self
            .entries
            .get(index.raw() as usize)
            .ok_or(ContainerError::InvalidConstantIndex(index))?;
        if entry.const_type != expected {
            return Err(ContainerError::InvalidConstantType(entry.const_type as u8));
        }
        let mut bytes = [0u8; N];
        bytes.copy_from_slice(&entry.primitive[..N]);
        Ok(bytes)
    }

    /// Gets an i32 value from the constant pool at the given index.
    pub fn get_i32(&self, index: ConstantIndex) -> Result<i32, ContainerError> {
        self.get_le_bytes::<4>(index, ConstType::I32)
            .map(i32::from_le_bytes)
    }

    /// Gets an f32 value from the constant pool at the given index.
    pub fn get_f32(&self, index: ConstantIndex) -> Result<f32, ContainerError> {
        self.get_le_bytes::<4>(index, ConstType::F32)
            .map(f32::from_le_bytes)
    }

    /// Gets an f64 value from the constant pool at the given index.
    pub fn get_f64(&self, index: ConstantIndex) -> Result<f64, ContainerError> {
        self.get_le_bytes::<8>(index, ConstType::F64)
            .map(f64::from_le_bytes)
    }

    /// Gets an i64 value from the constant pool at the given index.
    pub fn get_i64(&self, index: ConstantIndex) -> Result<i64, ContainerError> {
        self.get_le_bytes::<8>(index, ConstType::I64)
            .map(i64::from_le_bytes)
    }

    /// Gets a string value (raw bytes) from the constant pool at the given
    /// index. Accepts both [`ConstType::Str`] and [`ConstType::WStr`] entries.
    pub fn get_str(&self, index: ConstantIndex) -> Result<&[u8], ContainerError> {
        let entry = self
            .entries
            .get(index.raw() as usize)
            .ok_or(ContainerError::InvalidConstantIndex(index))?;
        match entry.const_type {
            ConstType::Str | ConstType::WStr => Ok(&entry.str_value),
            other => Err(ContainerError::InvalidConstantType(other as u8)),
        }
    }

    /// Returns the encoding of the string-typed entry at `index`. Returns an
    /// error if the entry is not a string type.
    pub fn get_str_char_width(&self, index: ConstantIndex) -> Result<CharWidth, ContainerError> {
        let entry = self
            .entries
            .get(index.raw() as usize)
            .ok_or(ContainerError::InvalidConstantIndex(index))?;
        entry
            .char_width()
            .ok_or(ContainerError::InvalidConstantType(entry.const_type as u8))
    }

    /// Writes the constant pool to the given writer.
    ///
    /// Each entry is written as `type(1) + char_width(1) + size(2) + value`.
    /// For string entries, `char_width` is 1 (STRING) or 2 (WSTRING); for
    /// non-string entries it is 0.
    pub fn write_to(&self, w: &mut impl Write) -> Result<(), ContainerError> {
        w.write_all(&(self.entries.len() as u16).to_le_bytes())?;
        for entry in &self.entries {
            let bytes = entry.bytes();
            w.write_all(&[entry.const_type as u8])?;
            w.write_all(&[entry.char_width().map(CharWidth::byte_width).unwrap_or(0)])?;
            w.write_all(&(bytes.len() as u16).to_le_bytes())?;
            w.write_all(bytes)?;
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
            let on_disk_char_width = hdr[1];
            let size = u16::from_le_bytes([hdr[2], hdr[3]]) as usize;
            let entry = match const_type {
                ConstType::Str | ConstType::WStr => {
                    // Validate the per-entry encoding tag matches the type
                    // tag. The tag is parsed (not just compared) so a stray
                    // value yields InvalidCharWidth rather than silent
                    // misinterpretation of payload bytes.
                    let parsed = CharWidth::from_u8(on_disk_char_width)?;
                    if Some(parsed) != const_type.char_width() {
                        return Err(ContainerError::InvalidConstantType(const_type as u8));
                    }
                    let mut value = vec![0u8; size];
                    r.read_exact(&mut value)?;
                    ConstEntry {
                        const_type,
                        primitive: [0u8; 8],
                        str_value: value.into_boxed_slice(),
                    }
                }
                _ => {
                    if size > 8 {
                        return Err(ContainerError::InvalidConstantType(const_type as u8));
                    }
                    let mut buf = [0u8; 8];
                    r.read_exact(&mut buf[..size])?;
                    ConstEntry {
                        const_type,
                        primitive: buf,
                        str_value: Box::default(),
                    }
                }
            };
            entries.push(entry);
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
        pool.push(ConstEntry::primitive_le(
            ConstType::I32,
            &10i32.to_le_bytes(),
        ));
        pool.push(ConstEntry::primitive_le(
            ConstType::I32,
            &32i32.to_le_bytes(),
        ));

        let mut buf = Vec::new();
        pool.write_to(&mut buf).unwrap();

        let mut cursor = Cursor::new(&buf);
        let decoded = ConstantPool::read_from(&mut cursor).unwrap();

        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded.get_i32(ConstantIndex::new(0)).unwrap(), 10);
        assert_eq!(decoded.get_i32(ConstantIndex::new(1)).unwrap(), 32);
    }

    #[test]
    fn constant_pool_get_i32_when_valid_index_then_returns_value() {
        let mut pool = ConstantPool::default();
        pool.push(ConstEntry::primitive_le(
            ConstType::I32,
            &42i32.to_le_bytes(),
        ));

        assert_eq!(pool.get_i32(ConstantIndex::new(0)).unwrap(), 42);
    }

    #[test]
    fn constant_pool_get_i32_when_out_of_bounds_then_error() {
        let pool = ConstantPool::default();

        assert!(matches!(
            pool.get_i32(ConstantIndex::new(0)),
            Err(ContainerError::InvalidConstantIndex(idx)) if idx == ConstantIndex::new(0)
        ));
    }

    #[test]
    fn constant_pool_iter_when_two_entries_then_returns_both() {
        let mut pool = ConstantPool::default();
        pool.push(ConstEntry::primitive_le(
            ConstType::I32,
            &10i32.to_le_bytes(),
        ));
        pool.push(ConstEntry::primitive_le(
            ConstType::F64,
            &2.72f64.to_le_bytes(),
        ));

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

    #[test]
    fn constant_pool_when_empty_then_is_empty_returns_true() {
        let pool = ConstantPool::default();
        assert!(pool.is_empty());
    }

    #[test]
    fn constant_pool_when_push_then_is_empty_returns_false() {
        let mut pool = ConstantPool::default();
        pool.push(ConstEntry::primitive_le(
            ConstType::I32,
            &1i32.to_le_bytes(),
        ));
        assert!(!pool.is_empty());
    }

    #[test]
    fn constant_pool_get_i32_when_type_mismatch_then_returns_error() {
        let mut pool = ConstantPool::default();
        pool.push(ConstEntry::primitive_le(
            ConstType::F32,
            &1.0f32.to_le_bytes(),
        ));

        assert!(matches!(
            pool.get_i32(ConstantIndex::new(0)),
            Err(ContainerError::InvalidConstantType(_))
        ));
    }

    #[test]
    fn constant_pool_get_f32_when_type_mismatch_then_returns_error() {
        let mut pool = ConstantPool::default();
        pool.push(ConstEntry::primitive_le(
            ConstType::I32,
            &1i32.to_le_bytes(),
        ));

        assert!(matches!(
            pool.get_f32(ConstantIndex::new(0)),
            Err(ContainerError::InvalidConstantType(_))
        ));
    }

    #[test]
    fn constant_pool_get_f64_when_type_mismatch_then_returns_error() {
        let mut pool = ConstantPool::default();
        pool.push(ConstEntry::primitive_le(
            ConstType::I32,
            &1i32.to_le_bytes(),
        ));

        assert!(matches!(
            pool.get_f64(ConstantIndex::new(0)),
            Err(ContainerError::InvalidConstantType(_))
        ));
    }

    #[test]
    fn constant_pool_get_i64_when_type_mismatch_then_returns_error() {
        let mut pool = ConstantPool::default();
        pool.push(ConstEntry::primitive_le(
            ConstType::F64,
            &1.0f64.to_le_bytes(),
        ));

        assert!(matches!(
            pool.get_i64(ConstantIndex::new(0)),
            Err(ContainerError::InvalidConstantType(_))
        ));
    }

    #[test]
    fn constant_pool_get_str_when_type_mismatch_then_returns_error() {
        let mut pool = ConstantPool::default();
        pool.push(ConstEntry::primitive_le(
            ConstType::I32,
            &1i32.to_le_bytes(),
        ));

        assert!(matches!(
            pool.get_str(ConstantIndex::new(0)),
            Err(ContainerError::InvalidConstantType(_))
        ));
    }

    #[test]
    fn constant_pool_write_read_when_str_constant_then_roundtrips() {
        let mut pool = ConstantPool::default();
        pool.push(ConstEntry::string(b"hello".to_vec()));

        let mut buf = Vec::new();
        pool.write_to(&mut buf).unwrap();
        let decoded = ConstantPool::read_from(&mut Cursor::new(&buf)).unwrap();

        assert_eq!(decoded.get_str(ConstantIndex::new(0)).unwrap(), b"hello");
        assert_eq!(
            decoded.get_str_char_width(ConstantIndex::new(0)).unwrap(),
            CharWidth::Narrow
        );
    }

    #[test]
    fn constant_pool_write_read_when_wstr_constant_then_roundtrips() {
        let mut pool = ConstantPool::default();
        // UTF-16LE encoding of "hi"
        pool.push(ConstEntry::wstring(vec![b'h', 0, b'i', 0]));

        let mut buf = Vec::new();
        pool.write_to(&mut buf).unwrap();
        let decoded = ConstantPool::read_from(&mut Cursor::new(&buf)).unwrap();

        assert_eq!(
            decoded.get_str(ConstantIndex::new(0)).unwrap(),
            &[b'h', 0, b'i', 0]
        );
        assert_eq!(
            decoded.get_str_char_width(ConstantIndex::new(0)).unwrap(),
            CharWidth::Wide
        );
    }

    #[test]
    fn constant_pool_bytes_when_primitive_then_returns_typed_length() {
        let i32_entry = ConstEntry::primitive_le(ConstType::I32, &7i32.to_le_bytes());
        assert_eq!(i32_entry.bytes(), &7i32.to_le_bytes());
        let f64_entry = ConstEntry::primitive_le(ConstType::F64, &1.5f64.to_le_bytes());
        assert_eq!(f64_entry.bytes(), &1.5f64.to_le_bytes());
    }
}
