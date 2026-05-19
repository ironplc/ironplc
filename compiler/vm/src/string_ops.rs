use ironplc_container::{CharWidth, STRING_HEADER_BYTES};

use crate::error::Trap;

/// Byte offset of the `max_length` field within a string header.
pub(crate) const MAX_LEN_OFFSET: usize = 0;
/// Byte offset of the `cur_length` field within a string header.
pub(crate) const CUR_LEN_OFFSET: usize = 2;
/// Byte offset of the `char_width` field within a string header.
///
/// `char_width` is recorded as a `u16` for alignment, but valid values are
/// `1` (STRING / Latin-1) and `2` (WSTRING / UTF-16LE) per ADR-0035.
pub(crate) const CHAR_WIDTH_OFFSET: usize = 4;

/// Metadata for an allocated temp buffer slot.
pub(crate) struct TempBufferSlot {
    /// Index of this buffer slot (the value pushed onto the stack).
    pub buf_idx: u16,
    /// Byte offset where this slot starts in the temp buffer.
    pub buf_start: usize,
    /// Maximum string data length (capacity minus header), measured in code
    /// units of the slot's encoding.
    pub max_len: u16,
    /// Encoding tag for this slot — defense-in-depth per ADR-0034. The
    /// runtime check reads the encoding from the temp buffer's header bytes
    /// (written by [`Self::alloc`] via [`write_string_header`]); this field
    /// is the per-slot copy of the same tag, kept for parity with the three
    /// runtime tag sites described in ADR-0034.
    #[allow(dead_code)]
    pub char_width: CharWidth,
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

    /// Allocate the next temp buffer slot for a string with the given
    /// encoding.
    ///
    /// The slot capacity (in code units) is computed by dividing the available
    /// payload bytes by `char_width`, so wide strings get half the code-unit
    /// capacity of a narrow string in the same byte budget.
    pub fn alloc(
        &mut self,
        temp_buf_len: usize,
        char_width: CharWidth,
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
        let payload_bytes = self.max_temp_buf_bytes - STRING_HEADER_BYTES;
        let max_len = (payload_bytes / char_width.as_usize()) as u16;
        self.next = self.next.wrapping_add(1);
        Ok(TempBufferSlot {
            buf_idx,
            buf_start,
            max_len,
            char_width,
        })
    }
}

/// Read a string's current length, data-start offset, and char_width from the
/// data region.
///
/// Returns `(cur_len, data_start, char_width)`. `cur_len` is in code units;
/// the on-disk byte span of the data is `cur_len * char_width`.
///
/// Traps with [`Trap::InvalidCharWidth`] if the header's `char_width` byte
/// is neither `1` nor `2`.
pub(crate) fn read_string_header(
    data_region: &[u8],
    offset: usize,
) -> Result<(usize, usize, CharWidth), Trap> {
    if offset + STRING_HEADER_BYTES > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(offset as u32));
    }
    let cur_len = u16::from_le_bytes([
        data_region[offset + CUR_LEN_OFFSET],
        data_region[offset + CUR_LEN_OFFSET + 1],
    ]) as usize;
    let char_width = parse_char_width(data_region[offset + CHAR_WIDTH_OFFSET])?;
    let data_start = offset + STRING_HEADER_BYTES;
    Ok((cur_len, data_start, char_width))
}

/// Write a string header into a temp buffer and return `(cur_len, data_start)`.
///
/// `cur_len` is clamped to `max_len`. All values are recorded in code units;
/// the byte span of the payload is `cur_len * char_width`.
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
    // char_width is stored as a u16 for alignment; the high byte is reserved.
    temp_buf[buf_start + CHAR_WIDTH_OFFSET..buf_start + CHAR_WIDTH_OFFSET + 2]
        .copy_from_slice(&(char_width.byte_width() as u16).to_le_bytes());
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

/// Read and validate `char_width` from a string header at `offset` in `buf`.
/// Traps if the byte is not a valid encoding.
pub(crate) fn str_read_char_width(buf: &[u8], offset: usize) -> Result<CharWidth, Trap> {
    parse_char_width(buf[offset + CHAR_WIDTH_OFFSET])
}

/// Convert a raw `u8` encoding tag from disk/bytecode into [`CharWidth`].
/// Traps with [`Trap::InvalidCharWidth`] for any value other than `1` or `2`.
pub(crate) fn parse_char_width(value: u8) -> Result<CharWidth, Trap> {
    CharWidth::from_u8(value).map_err(|_| Trap::InvalidCharWidth(value))
}

/// Verify that `actual` matches `expected`; returns `Trap::EncodingMismatch`
/// otherwise. Used as the runtime safety check in every string opcode that
/// reads or writes a string operand.
#[inline]
pub(crate) fn check_encoding(expected: CharWidth, actual: CharWidth) -> Result<(), Trap> {
    if expected == actual {
        Ok(())
    } else {
        Err(Trap::EncodingMismatch {
            expected: expected.byte_width(),
            actual: actual.byte_width(),
        })
    }
}

/// Write a complete string header (max_length, cur_length, char_width) at
/// `offset` in `buf`.
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
    buf[offset + CHAR_WIDTH_OFFSET..offset + CHAR_WIDTH_OFFSET + 2]
        .copy_from_slice(&(char_width.byte_width() as u16).to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a string-header byte sequence with the new 6-byte layout.
    fn header_bytes(max_len: u16, cur_len: u16, char_width: CharWidth) -> [u8; 6] {
        let mut h = [0u8; 6];
        h[0..2].copy_from_slice(&max_len.to_le_bytes());
        h[2..4].copy_from_slice(&cur_len.to_le_bytes());
        h[4..6].copy_from_slice(&(char_width.byte_width() as u16).to_le_bytes());
        h
    }

    #[test]
    fn read_string_header_when_valid_then_returns_len_and_start() {
        // Header: max_len=10, cur_len=5, char_width=Narrow; followed by "Hello".
        let mut data = [0u8; 11];
        data[..6].copy_from_slice(&header_bytes(10, 5, CharWidth::Narrow));
        data[6..11].copy_from_slice(b"Hello");
        let (cur_len, data_start, char_width) = read_string_header(&data, 0).unwrap();
        assert_eq!(cur_len, 5);
        assert_eq!(data_start, STRING_HEADER_BYTES);
        assert_eq!(char_width, CharWidth::Narrow);
    }

    #[test]
    fn read_string_header_when_wstring_then_returns_wide_char_width() {
        let mut data = [0u8; 10];
        data[..6].copy_from_slice(&header_bytes(4, 2, CharWidth::Wide));
        data[6..10].copy_from_slice(&[b'h', 0, b'i', 0]);
        let (cur_len, data_start, char_width) = read_string_header(&data, 0).unwrap();
        assert_eq!(cur_len, 2);
        assert_eq!(data_start, STRING_HEADER_BYTES);
        assert_eq!(char_width, CharWidth::Wide);
    }

    #[test]
    fn read_string_header_when_offset_nonzero_then_reads_from_offset() {
        let mut data = [0u8; 16];
        data[4..10].copy_from_slice(&header_bytes(20, 3, CharWidth::Narrow));
        let (cur_len, data_start, char_width) = read_string_header(&data, 4).unwrap();
        assert_eq!(cur_len, 3);
        assert_eq!(data_start, 4 + STRING_HEADER_BYTES);
        assert_eq!(char_width, CharWidth::Narrow);
    }

    #[test]
    fn read_string_header_when_out_of_bounds_then_trap() {
        let data = [0u8; 5];
        let result = read_string_header(&data, 0);
        assert!(matches!(result, Err(Trap::DataRegionOutOfBounds(0))));
    }

    #[test]
    fn read_string_header_when_invalid_char_width_then_trap() {
        // Construct a header whose char_width byte is 9 — not a valid encoding.
        let mut data = [0u8; 6];
        data[0..2].copy_from_slice(&10u16.to_le_bytes());
        data[2..4].copy_from_slice(&0u16.to_le_bytes());
        data[4] = 9;
        let result = read_string_header(&data, 0);
        assert!(matches!(result, Err(Trap::InvalidCharWidth(9))));
    }

    #[test]
    fn alloc_when_narrow_then_returns_slot_with_byte_capacity() {
        let mut alloc = TempBufAllocator::new(32);
        let slot = alloc.alloc(64, CharWidth::Narrow).unwrap();
        assert_eq!(slot.buf_idx, 0);
        assert_eq!(slot.buf_start, 0);
        assert_eq!(slot.max_len, (32 - STRING_HEADER_BYTES) as u16);
        assert_eq!(slot.char_width, CharWidth::Narrow);
    }

    #[test]
    fn alloc_when_wide_then_max_len_halved() {
        let mut alloc = TempBufAllocator::new(32);
        let slot = alloc.alloc(64, CharWidth::Wide).unwrap();
        assert_eq!(slot.char_width, CharWidth::Wide);
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
        assert_eq!(u16::from_le_bytes([buf[0], buf[1]]), 28);
        assert_eq!(u16::from_le_bytes([buf[2], buf[3]]), 10);
        assert_eq!(buf[CHAR_WIDTH_OFFSET], CharWidth::Narrow.byte_width());
    }

    #[test]
    fn write_string_header_when_exceeds_max_then_clamps() {
        let mut buf = [0u8; 32];
        let (cur_len, _) = write_string_header(&mut buf, 0, 5, 100, CharWidth::Narrow);
        assert_eq!(cur_len, 5);
        assert_eq!(u16::from_le_bytes([buf[2], buf[3]]), 5);
    }

    #[test]
    fn write_string_header_when_wide_then_records_char_width() {
        let mut buf = [0u8; 32];
        write_string_header(&mut buf, 0, 10, 3, CharWidth::Wide);
        assert_eq!(buf[CHAR_WIDTH_OFFSET], CharWidth::Wide.byte_width());
    }

    #[test]
    fn str_read_char_width_when_called_then_returns_field() {
        let mut buf = [0u8; STRING_HEADER_BYTES];
        str_write_header(&mut buf, 0, 10, 3, CharWidth::Wide);
        assert_eq!(str_read_char_width(&buf, 0).unwrap(), CharWidth::Wide);
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
}
