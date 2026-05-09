use ironplc_container::STRING_HEADER_BYTES;

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

/// `char_width` value for STRING (Latin-1, 1 byte per character).
pub(crate) const NARROW_CHAR_WIDTH: u8 = 1;
/// `char_width` value for WSTRING (UTF-16LE, 2 bytes per code unit).
pub(crate) const WIDE_CHAR_WIDTH: u8 = 2;

/// Metadata for an allocated temp buffer slot.
pub(crate) struct TempBufferSlot {
    /// Index of this buffer slot (the value pushed onto the stack).
    pub buf_idx: u16,
    /// Byte offset where this slot starts in the temp buffer.
    pub buf_start: usize,
    /// Maximum string data length (capacity minus header), measured in code
    /// units of the slot's encoding.
    pub max_len: u16,
    /// Encoding tag for this slot: 1 for STRING, 2 for WSTRING. Defense-in-
    /// depth — every string operation cross-checks the source's header
    /// `char_width` against the consumer's expected encoding and traps on
    /// mismatch (ADR-0034). The runtime check reads the encoding tag from
    /// the temp buffer's header bytes (written by [`Self::alloc`] via
    /// [`write_string_header`]); this field is the per-slot copy of the same
    /// tag, kept for parity with the three runtime tag sites described in
    /// ADR-0034.
    #[allow(dead_code)]
    pub char_width: u8,
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
    /// `char_width` (1 or 2).
    ///
    /// The slot capacity (in code units) is computed by dividing the available
    /// payload bytes by `char_width`, so wide strings get half the code-unit
    /// capacity of a narrow string in the same byte budget.
    pub fn alloc(
        &mut self,
        temp_buf_len: usize,
        char_width: u8,
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
        let max_len = (payload_bytes / char_width as usize) as u16;
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
pub(crate) fn read_string_header(
    data_region: &[u8],
    offset: usize,
) -> Result<(usize, usize, u8), Trap> {
    if offset + STRING_HEADER_BYTES > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(offset as u32));
    }
    let cur_len = u16::from_le_bytes([
        data_region[offset + CUR_LEN_OFFSET],
        data_region[offset + CUR_LEN_OFFSET + 1],
    ]) as usize;
    let char_width = data_region[offset + CHAR_WIDTH_OFFSET];
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
    char_width: u8,
) -> (u16, usize) {
    let cur_len = (result_len as u16).min(max_len);
    temp_buf[buf_start + MAX_LEN_OFFSET..buf_start + MAX_LEN_OFFSET + 2]
        .copy_from_slice(&max_len.to_le_bytes());
    temp_buf[buf_start + CUR_LEN_OFFSET..buf_start + CUR_LEN_OFFSET + 2]
        .copy_from_slice(&cur_len.to_le_bytes());
    // char_width is stored as a u16 for alignment; the high byte is reserved.
    temp_buf[buf_start + CHAR_WIDTH_OFFSET..buf_start + CHAR_WIDTH_OFFSET + 2]
        .copy_from_slice(&(char_width as u16).to_le_bytes());
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

/// Read char_width from a string header at `offset` in `buf`.
pub(crate) fn str_read_char_width(buf: &[u8], offset: usize) -> u8 {
    buf[offset + CHAR_WIDTH_OFFSET]
}

/// Verify that `actual` matches `expected`; returns `Trap::EncodingMismatch`
/// otherwise. Used as the runtime safety check in every string opcode that
/// reads or writes a string operand.
#[inline]
pub(crate) fn check_encoding(expected: u8, actual: u8) -> Result<(), Trap> {
    if expected == actual {
        Ok(())
    } else {
        Err(Trap::EncodingMismatch { expected, actual })
    }
}

/// Write a complete string header (max_length, cur_length, char_width) at
/// `offset` in `buf`.
pub(crate) fn str_write_header(
    buf: &mut [u8],
    offset: usize,
    max_len: u16,
    cur_len: u16,
    char_width: u8,
) {
    buf[offset + MAX_LEN_OFFSET..offset + MAX_LEN_OFFSET + 2]
        .copy_from_slice(&max_len.to_le_bytes());
    buf[offset + CUR_LEN_OFFSET..offset + CUR_LEN_OFFSET + 2]
        .copy_from_slice(&cur_len.to_le_bytes());
    buf[offset + CHAR_WIDTH_OFFSET..offset + CHAR_WIDTH_OFFSET + 2]
        .copy_from_slice(&(char_width as u16).to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a string-header byte sequence with the new 6-byte layout.
    fn header_bytes(max_len: u16, cur_len: u16, char_width: u8) -> [u8; 6] {
        let mut h = [0u8; 6];
        h[0..2].copy_from_slice(&max_len.to_le_bytes());
        h[2..4].copy_from_slice(&cur_len.to_le_bytes());
        h[4..6].copy_from_slice(&(char_width as u16).to_le_bytes());
        h
    }

    #[test]
    fn read_string_header_when_valid_then_returns_len_and_start() {
        // Header: max_len=10, cur_len=5, char_width=1; followed by "Hello".
        let mut data = [0u8; 11];
        data[..6].copy_from_slice(&header_bytes(10, 5, NARROW_CHAR_WIDTH));
        data[6..11].copy_from_slice(b"Hello");
        let (cur_len, data_start, char_width) = read_string_header(&data, 0).unwrap();
        assert_eq!(cur_len, 5);
        assert_eq!(data_start, STRING_HEADER_BYTES);
        assert_eq!(char_width, NARROW_CHAR_WIDTH);
    }

    #[test]
    fn read_string_header_when_wstring_then_returns_wide_char_width() {
        // Header: max_len=4, cur_len=2, char_width=2; followed by UTF-16LE "hi".
        let mut data = [0u8; 10];
        data[..6].copy_from_slice(&header_bytes(4, 2, WIDE_CHAR_WIDTH));
        data[6..10].copy_from_slice(&[b'h', 0, b'i', 0]);
        let (cur_len, data_start, char_width) = read_string_header(&data, 0).unwrap();
        assert_eq!(cur_len, 2);
        assert_eq!(data_start, STRING_HEADER_BYTES);
        assert_eq!(char_width, WIDE_CHAR_WIDTH);
    }

    #[test]
    fn read_string_header_when_offset_nonzero_then_reads_from_offset() {
        let mut data = [0u8; 16];
        data[4..10].copy_from_slice(&header_bytes(20, 3, NARROW_CHAR_WIDTH));
        let (cur_len, data_start, char_width) = read_string_header(&data, 4).unwrap();
        assert_eq!(cur_len, 3);
        assert_eq!(data_start, 4 + STRING_HEADER_BYTES);
        assert_eq!(char_width, NARROW_CHAR_WIDTH);
    }

    #[test]
    fn read_string_header_when_out_of_bounds_then_trap() {
        let data = [0u8; 5]; // Too small for header
        let result = read_string_header(&data, 0);
        assert!(matches!(result, Err(Trap::DataRegionOutOfBounds(0))));
    }

    #[test]
    fn alloc_when_narrow_then_returns_slot_with_byte_capacity() {
        let mut alloc = TempBufAllocator::new(32);
        let slot = alloc.alloc(64, NARROW_CHAR_WIDTH).unwrap();
        assert_eq!(slot.buf_idx, 0);
        assert_eq!(slot.buf_start, 0);
        assert_eq!(slot.max_len, (32 - STRING_HEADER_BYTES) as u16);
        assert_eq!(slot.char_width, NARROW_CHAR_WIDTH);
    }

    #[test]
    fn alloc_when_wide_then_max_len_halved() {
        let mut alloc = TempBufAllocator::new(32);
        let slot = alloc.alloc(64, WIDE_CHAR_WIDTH).unwrap();
        assert_eq!(slot.char_width, WIDE_CHAR_WIDTH);
        assert_eq!(slot.max_len, ((32 - STRING_HEADER_BYTES) / 2) as u16);
    }

    #[test]
    fn alloc_when_called_twice_then_second_slot_offset_correct() {
        let mut alloc = TempBufAllocator::new(32);
        let _first = alloc.alloc(64, NARROW_CHAR_WIDTH).unwrap();
        let second = alloc.alloc(64, NARROW_CHAR_WIDTH).unwrap();
        assert_eq!(second.buf_idx, 1);
        assert_eq!(second.buf_start, 32);
    }

    #[test]
    fn alloc_when_zero_max_then_trap() {
        let mut alloc = TempBufAllocator::new(0);
        let result = alloc.alloc(64, NARROW_CHAR_WIDTH);
        assert!(matches!(result, Err(Trap::TempBufferExhausted)));
    }

    #[test]
    fn alloc_when_exceeds_len_then_trap() {
        let mut alloc = TempBufAllocator::new(32);
        let result = alloc.alloc(16, NARROW_CHAR_WIDTH);
        assert!(matches!(result, Err(Trap::TempBufferExhausted)));
    }

    #[test]
    fn write_string_header_when_fits_then_writes_exact() {
        let mut buf = [0u8; 32];
        let (cur_len, data_start) = write_string_header(&mut buf, 0, 28, 10, NARROW_CHAR_WIDTH);
        assert_eq!(cur_len, 10);
        assert_eq!(data_start, STRING_HEADER_BYTES);
        assert_eq!(u16::from_le_bytes([buf[0], buf[1]]), 28); // max_len
        assert_eq!(u16::from_le_bytes([buf[2], buf[3]]), 10); // cur_len
        assert_eq!(buf[CHAR_WIDTH_OFFSET], NARROW_CHAR_WIDTH);
    }

    #[test]
    fn write_string_header_when_exceeds_max_then_clamps() {
        let mut buf = [0u8; 32];
        let (cur_len, _) = write_string_header(&mut buf, 0, 5, 100, NARROW_CHAR_WIDTH);
        assert_eq!(cur_len, 5);
        assert_eq!(u16::from_le_bytes([buf[2], buf[3]]), 5);
    }

    #[test]
    fn write_string_header_when_wide_then_records_char_width() {
        let mut buf = [0u8; 32];
        write_string_header(&mut buf, 0, 10, 3, WIDE_CHAR_WIDTH);
        assert_eq!(buf[CHAR_WIDTH_OFFSET], WIDE_CHAR_WIDTH);
    }

    #[test]
    fn str_read_char_width_when_called_then_returns_field() {
        let mut buf = [0u8; STRING_HEADER_BYTES];
        str_write_header(&mut buf, 0, 10, 3, WIDE_CHAR_WIDTH);
        assert_eq!(str_read_char_width(&buf, 0), WIDE_CHAR_WIDTH);
    }
}
