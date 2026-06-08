use ironplc_container::{CharWidth, STRING_HEADER_BYTES};

use crate::error::Trap;

/// Byte offset of the `max_length` field within a string header.
pub(crate) const MAX_LEN_OFFSET: usize = 0;
/// Byte offset of the `cur_length` field within a string header.
pub(crate) const CUR_LEN_OFFSET: usize = 2;
/// Byte offset of the `char_width` field within a string header (ADR-0035).
///
/// The field is a `u16` (bytes 4-5 of the 6-byte header) carrying the
/// per-code-unit byte width: 1 for STRING (Latin-1), 2 for WSTRING
/// (UTF-16LE). `max_length` and `cur_length` are counts of code units; a
/// span of `n` code units occupies `n * char_width` bytes.
pub(crate) const CHAR_WIDTH_OFFSET: usize = 4;

/// Verifies that an operand's actual encoding matches the expected one,
/// trapping with [`Trap::EncodingMismatch`] otherwise (ADR-0034).
pub(crate) fn verify_encoding(expected: CharWidth, actual: CharWidth) -> Result<(), Trap> {
    if expected != actual {
        return Err(Trap::EncodingMismatch {
            expected: expected.byte_width(),
            actual: actual.byte_width(),
        });
    }
    Ok(())
}

/// Metadata for an allocated temp buffer slot.
///
/// The slot's encoding is not stored here: it is written into the buffer's
/// string header (read back via [`str_read_char_width`]) by the allocating
/// opcode, which already knows the width it requested.
pub(crate) struct TempBufferSlot {
    /// Index of this buffer slot (the value pushed onto the stack).
    pub buf_idx: u16,
    /// Byte offset where this slot starts in the temp buffer.
    pub buf_start: usize,
    /// Maximum string data length in code units (capacity minus header,
    /// divided by the requested encoding's byte width).
    pub max_len: u16,
}

/// Bump allocator for temporary string buffers.
///
/// Wraps the raw `u16` counter so that callers cannot manually
/// increment it — all allocations must go through [`Self::alloc`].
pub(crate) struct TempBufAllocator {
    next: u16,
    max_temp_buf_bytes: usize,
}

impl TempBufAllocator {
    /// Create a new allocator starting at slot 0.
    pub fn new(max_temp_buf_bytes: usize) -> Self {
        Self {
            next: 0,
            max_temp_buf_bytes,
        }
    }

    /// Allocate the next temp buffer slot for a string of the given
    /// `encoding`.
    ///
    /// Returns a [`TempBufferSlot`] with the slot index, byte offset, max
    /// data length (in code units, so `capacity / char_width`), and the
    /// recorded encoding. The internal counter is advanced automatically.
    pub fn alloc(
        &mut self,
        temp_buf_len: usize,
        encoding: CharWidth,
    ) -> Result<TempBufferSlot, Trap> {
        if self.max_temp_buf_bytes == 0 {
            return Err(Trap::TempBufferExhausted);
        }
        let buf_idx = self.next;
        let buf_start = buf_idx as usize * self.max_temp_buf_bytes;
        let buf_end = buf_start + self.max_temp_buf_bytes;
        if buf_end > temp_buf_len {
            return Err(Trap::TempBufferExhausted);
        }
        let max_len =
            ((self.max_temp_buf_bytes - STRING_HEADER_BYTES) / encoding.as_usize()) as u16;
        self.next = self.next.wrapping_add(1);
        Ok(TempBufferSlot {
            buf_idx,
            buf_start,
            max_len,
        })
    }
}

/// Read a string's current length (code units), data-start byte offset, and
/// encoding from a data region. Returns `(cur_len, data_start, char_width)`.
///
/// The `char_width` field is validated (trapping [`Trap::InvalidCharWidth`]
/// on a malformed value) and the read is bounds-checked.
pub(crate) fn read_string_header(
    data_region: &[u8],
    offset: usize,
) -> Result<(usize, usize, CharWidth), Trap> {
    // str_read_char_width bounds-checks the whole header.
    let char_width = str_read_char_width(data_region, offset)?;
    let cur_len = str_read_cur_len(data_region, offset) as usize;
    let data_start = offset + STRING_HEADER_BYTES;
    Ok((cur_len, data_start, char_width))
}

/// Copy `units` code units (`units * char_width` bytes) from `src` starting at
/// byte offset `src_byte` into `dst` starting at byte offset `dst_byte`.
pub(crate) fn copy_code_units(
    dst: &mut [u8],
    dst_byte: usize,
    src: &[u8],
    src_byte: usize,
    units: usize,
    char_width: CharWidth,
) {
    let n = units * char_width.as_usize();
    dst[dst_byte..dst_byte + n].copy_from_slice(&src[src_byte..src_byte + n]);
}

/// Read the per-code-unit [`CharWidth`] from a string header at `offset`.
///
/// Validates the stored byte, trapping with [`Trap::InvalidCharWidth`] for
/// any value other than 1 or 2 (malformed or tampered bytecode).
pub(crate) fn str_read_char_width(buf: &[u8], offset: usize) -> Result<CharWidth, Trap> {
    if offset + STRING_HEADER_BYTES > buf.len() {
        return Err(Trap::DataRegionOutOfBounds(offset as u32));
    }
    let value = u16::from_le_bytes([
        buf[offset + CHAR_WIDTH_OFFSET],
        buf[offset + CHAR_WIDTH_OFFSET + 1],
    ]);
    CharWidth::from_u8(value as u8).map_err(|_| Trap::InvalidCharWidth(value as u8))
}

/// Write the `char_width` field of a string header at `offset` in `buf`.
pub(crate) fn str_write_char_width(buf: &mut [u8], offset: usize, char_width: CharWidth) {
    buf[offset + CHAR_WIDTH_OFFSET..offset + CHAR_WIDTH_OFFSET + 2]
        .copy_from_slice(&(char_width.byte_width() as u16).to_le_bytes());
}

/// Write a string header into a temp buffer and return `(cur_len, data_start)`.
///
/// `max_len`, `result_len`, and the returned `cur_len` are counts of code
/// units; `cur_len` is clamped to `max_len`. The data span is
/// `cur_len * char_width` bytes, starting at the returned `data_start`.
pub(crate) fn write_string_header(
    temp_buf: &mut [u8],
    buf_start: usize,
    max_len: u16,
    result_len: usize,
    char_width: CharWidth,
) -> (u16, usize) {
    let cur_len = (result_len as u16).min(max_len);
    temp_buf[buf_start + MAX_LEN_OFFSET..buf_start + MAX_LEN_OFFSET + 2]
        .copy_from_slice(&max_len.to_le_bytes());
    temp_buf[buf_start + CUR_LEN_OFFSET..buf_start + CUR_LEN_OFFSET + 2]
        .copy_from_slice(&cur_len.to_le_bytes());
    str_write_char_width(temp_buf, buf_start, char_width);
    let data_start = buf_start + STRING_HEADER_BYTES;
    (cur_len, data_start)
}

/// Read max_length from a string header at `offset` in `buf`.
pub(crate) fn str_read_max_len(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([
        buf[offset + MAX_LEN_OFFSET],
        buf[offset + MAX_LEN_OFFSET + 1],
    ])
}

/// Read cur_length from a string header at `offset` in `buf`.
pub(crate) fn str_read_cur_len(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([
        buf[offset + CUR_LEN_OFFSET],
        buf[offset + CUR_LEN_OFFSET + 1],
    ])
}

/// Write cur_length into a string header at `offset` in `buf`, leaving the
/// `max_length` and `char_width` fields untouched.
pub(crate) fn str_write_cur_len(buf: &mut [u8], offset: usize, cur_len: u16) {
    buf[offset + CUR_LEN_OFFSET..offset + CUR_LEN_OFFSET + 2]
        .copy_from_slice(&cur_len.to_le_bytes());
}

/// Write a full string header (max_length, cur_length, char_width) at
/// `offset` in `buf`. `max_len` and `cur_len` are counts of code units.
pub(crate) fn str_write_header(
    buf: &mut [u8],
    offset: usize,
    max_len: u16,
    cur_len: u16,
    char_width: CharWidth,
) {
    buf[offset + MAX_LEN_OFFSET..offset + MAX_LEN_OFFSET + 2]
        .copy_from_slice(&max_len.to_le_bytes());
    buf[offset + CUR_LEN_OFFSET..offset + CUR_LEN_OFFSET + 2]
        .copy_from_slice(&cur_len.to_le_bytes());
    str_write_char_width(buf, offset, char_width);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_string_header_when_valid_then_returns_len_start_and_width() {
        // Header: max_len=10, cur_len=5, char_width=1, then data.
        let data = [10, 0, 5, 0, 1, 0, b'H', b'e', b'l', b'l', b'o'];
        let (cur_len, data_start, width) = read_string_header(&data, 0).unwrap();
        assert_eq!(cur_len, 5);
        assert_eq!(data_start, STRING_HEADER_BYTES);
        assert_eq!(width, CharWidth::Narrow);
    }

    #[test]
    fn read_string_header_when_offset_nonzero_then_reads_from_offset() {
        let mut data = [0u8; 14];
        // Place header at offset 4.
        data[4] = 20; // max_len low byte
        data[6] = 3; // cur_len low byte
        data[8] = 2; // char_width low byte (wide)
        let (cur_len, data_start, width) = read_string_header(&data, 4).unwrap();
        assert_eq!(cur_len, 3);
        assert_eq!(data_start, 4 + STRING_HEADER_BYTES);
        assert_eq!(width, CharWidth::Wide);
    }

    #[test]
    fn read_string_header_when_out_of_bounds_then_trap() {
        let data = [0u8; 3]; // Too small for header
        let result = read_string_header(&data, 0);
        assert!(matches!(result, Err(Trap::DataRegionOutOfBounds(0))));
    }

    #[test]
    fn copy_code_units_when_wide_then_copies_scaled_bytes() {
        let src = [0u8, 0, 0, 0, 0, 0, 0x41, 0x00, 0x42, 0x00];
        let mut dst = [0u8; 4];
        // Copy 2 wide code units (4 bytes) from src byte 6 to dst byte 0.
        copy_code_units(&mut dst, 0, &src, 6, 2, CharWidth::Wide);
        assert_eq!(dst, [0x41, 0x00, 0x42, 0x00]);
    }

    #[test]
    fn alloc_when_narrow_then_max_len_is_capacity_minus_header() {
        let mut alloc = TempBufAllocator::new(32);
        let slot = alloc.alloc(64, CharWidth::Narrow).unwrap();
        assert_eq!(slot.buf_idx, 0);
        assert_eq!(slot.buf_start, 0);
        assert_eq!(slot.max_len, (32 - STRING_HEADER_BYTES) as u16);
    }

    #[test]
    fn alloc_when_wide_then_max_len_is_half_capacity_in_code_units() {
        let mut alloc = TempBufAllocator::new(32);
        let slot = alloc.alloc(64, CharWidth::Wide).unwrap();
        // 32 - 6 header = 26 data bytes / 2 bytes per code unit = 13 units.
        assert_eq!(slot.max_len, ((32 - STRING_HEADER_BYTES) / 2) as u16);
    }

    #[test]
    fn alloc_when_called_twice_then_second_slot_offset_correct() {
        let mut alloc = TempBufAllocator::new(32);
        let _first = alloc.alloc(64, CharWidth::Narrow).unwrap();
        let second = alloc.alloc(64, CharWidth::Narrow).unwrap();
        assert_eq!(second.buf_idx, 1);
        assert_eq!(second.buf_start, 32);
    }

    #[test]
    fn alloc_when_zero_max_then_trap() {
        let mut alloc = TempBufAllocator::new(0);
        let result = alloc.alloc(64, CharWidth::Narrow);
        assert!(matches!(result, Err(Trap::TempBufferExhausted)));
    }

    #[test]
    fn alloc_when_exceeds_len_then_trap() {
        let mut alloc = TempBufAllocator::new(32);
        let result = alloc.alloc(16, CharWidth::Narrow);
        assert!(matches!(result, Err(Trap::TempBufferExhausted)));
    }

    #[test]
    fn write_string_header_when_fits_then_writes_exact() {
        let mut buf = [0u8; 32];
        let (cur_len, data_start) = write_string_header(&mut buf, 0, 28, 10, CharWidth::Narrow);
        assert_eq!(cur_len, 10);
        assert_eq!(data_start, STRING_HEADER_BYTES);
        assert_eq!(u16::from_le_bytes([buf[0], buf[1]]), 28); // max_len
        assert_eq!(u16::from_le_bytes([buf[2], buf[3]]), 10); // cur_len
        assert_eq!(u16::from_le_bytes([buf[4], buf[5]]), 1); // char_width
    }

    #[test]
    fn write_string_header_when_exceeds_max_then_clamps() {
        let mut buf = [0u8; 32];
        let (cur_len, _) = write_string_header(&mut buf, 0, 5, 100, CharWidth::Narrow);
        assert_eq!(cur_len, 5);
        assert_eq!(u16::from_le_bytes([buf[2], buf[3]]), 5);
    }

    #[test]
    fn write_string_header_when_wide_then_writes_char_width_two() {
        let mut buf = [0u8; 32];
        write_string_header(&mut buf, 0, 10, 4, CharWidth::Wide);
        assert_eq!(u16::from_le_bytes([buf[4], buf[5]]), 2);
    }

    #[test]
    fn str_read_char_width_when_valid_then_returns_encoding() {
        let mut buf = [0u8; STRING_HEADER_BYTES];
        str_write_char_width(&mut buf, 0, CharWidth::Wide);
        assert_eq!(str_read_char_width(&buf, 0).unwrap(), CharWidth::Wide);
        str_write_char_width(&mut buf, 0, CharWidth::Narrow);
        assert_eq!(str_read_char_width(&buf, 0).unwrap(), CharWidth::Narrow);
    }

    #[test]
    fn str_read_char_width_when_invalid_byte_then_trap() {
        let mut buf = [0u8; STRING_HEADER_BYTES];
        buf[CHAR_WIDTH_OFFSET] = 7;
        assert!(matches!(
            str_read_char_width(&buf, 0),
            Err(Trap::InvalidCharWidth(7))
        ));
    }

    #[test]
    fn str_read_char_width_when_out_of_bounds_then_trap() {
        let buf = [0u8; 3];
        assert!(matches!(
            str_read_char_width(&buf, 0),
            Err(Trap::DataRegionOutOfBounds(0))
        ));
    }

    #[test]
    fn verify_encoding_when_match_then_ok_else_trap() {
        assert!(verify_encoding(CharWidth::Narrow, CharWidth::Narrow).is_ok());
        assert!(verify_encoding(CharWidth::Wide, CharWidth::Wide).is_ok());
        assert_eq!(
            verify_encoding(CharWidth::Narrow, CharWidth::Wide),
            Err(Trap::EncodingMismatch {
                expected: 1,
                actual: 2,
            })
        );
    }
}
