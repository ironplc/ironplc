use ironplc_container::STRING_HEADER_BYTES;

use crate::error::Trap;

/// Metadata for an allocated temp buffer slot.
pub(crate) struct TempBufferSlot {
    /// Index of this buffer slot (the value pushed onto the stack).
    pub buf_idx: u16,
    /// Updated next_temp_buf value (caller must assign back).
    pub next_temp_buf: u16,
    /// Byte offset where this slot starts in the temp buffer.
    pub buf_start: usize,
    /// Maximum string data length (capacity minus header).
    pub max_len: u16,
}

/// Read a string's current length and data-start offset from the data region.
///
/// Returns `(cur_len, data_start)`.
pub(crate) fn read_string_header(
    data_region: &[u8],
    offset: usize,
) -> Result<(usize, usize), Trap> {
    if offset + STRING_HEADER_BYTES > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(offset as u32));
    }
    let cur_len =
        u16::from_le_bytes([data_region[offset + 2], data_region[offset + 3]]) as usize;
    let data_start = offset + STRING_HEADER_BYTES;
    Ok((cur_len, data_start))
}

/// Allocate the next temp buffer slot.
///
/// Returns a [`TempBufferSlot`] with the slot index, updated counter,
/// byte offset, and max data length.
pub(crate) fn allocate_temp_buffer(
    temp_buf: &[u8],
    next_temp_buf: u16,
    max_temp_buf_bytes: usize,
) -> Result<TempBufferSlot, Trap> {
    if max_temp_buf_bytes == 0 {
        return Err(Trap::TempBufferExhausted);
    }
    let buf_idx = next_temp_buf;
    let buf_start = buf_idx as usize * max_temp_buf_bytes;
    let buf_end = buf_start + max_temp_buf_bytes;
    if buf_end > temp_buf.len() {
        return Err(Trap::TempBufferExhausted);
    }
    let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
    Ok(TempBufferSlot {
        buf_idx,
        next_temp_buf: next_temp_buf.wrapping_add(1),
        buf_start,
        max_len,
    })
}

/// Write a string header into a temp buffer and return `(cur_len, data_start)`.
///
/// `cur_len` is clamped to `max_len`.
pub(crate) fn write_string_header(
    temp_buf: &mut [u8],
    buf_start: usize,
    max_len: u16,
    result_len: usize,
) -> (u16, usize) {
    let cur_len = (result_len as u16).min(max_len);
    temp_buf[buf_start..buf_start + 2].copy_from_slice(&max_len.to_le_bytes());
    temp_buf[buf_start + 2..buf_start + STRING_HEADER_BYTES]
        .copy_from_slice(&cur_len.to_le_bytes());
    let data_start = buf_start + STRING_HEADER_BYTES;
    (cur_len, data_start)
}

/// Read max_length from a string header at `offset` in `buf`.
pub(crate) fn str_read_max_len(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset], buf[offset + 1]])
}

/// Read cur_length from a string header at `offset` in `buf`.
pub(crate) fn str_read_cur_len(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset + 2], buf[offset + 3]])
}

/// Write a string header (max_length, cur_length) at `offset` in `buf`.
pub(crate) fn str_write_header(buf: &mut [u8], offset: usize, max_len: u16, cur_len: u16) {
    buf[offset..offset + 2].copy_from_slice(&max_len.to_le_bytes());
    buf[offset + 2..offset + STRING_HEADER_BYTES].copy_from_slice(&cur_len.to_le_bytes());
}

/// Allocate the next temp buffer slot. Returns (buf_idx, buf_start).
///
/// This is the legacy interface used by non-string-op callers.
pub(crate) fn str_alloc_temp(
    next_temp_buf: &mut u16,
    max_temp_buf_bytes: usize,
    temp_buf_len: usize,
) -> Result<(usize, usize), Trap> {
    if max_temp_buf_bytes == 0 {
        return Err(Trap::TempBufferExhausted);
    }
    let buf_idx = *next_temp_buf as usize;
    let buf_start = buf_idx * max_temp_buf_bytes;
    if buf_start + max_temp_buf_bytes > temp_buf_len {
        return Err(Trap::TempBufferExhausted);
    }
    *next_temp_buf = next_temp_buf.wrapping_add(1);
    Ok((buf_idx, buf_start))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_string_header_when_valid_then_returns_len_and_start() {
        // Header: max_len=10 (bytes 0-1), cur_len=5 (bytes 2-3)
        let data = [10, 0, 5, 0, b'H', b'e', b'l', b'l', b'o'];
        let (cur_len, data_start) = read_string_header(&data, 0).unwrap();
        assert_eq!(cur_len, 5);
        assert_eq!(data_start, STRING_HEADER_BYTES);
    }

    #[test]
    fn read_string_header_when_offset_nonzero_then_reads_from_offset() {
        let mut data = [0u8; 12];
        // Place header at offset 4
        data[4] = 20; // max_len low byte
        data[6] = 3; // cur_len low byte
        let (cur_len, data_start) = read_string_header(&data, 4).unwrap();
        assert_eq!(cur_len, 3);
        assert_eq!(data_start, 8);
    }

    #[test]
    fn read_string_header_when_out_of_bounds_then_trap() {
        let data = [0u8; 3]; // Too small for header
        let result = read_string_header(&data, 0);
        assert!(matches!(result, Err(Trap::DataRegionOutOfBounds(0))));
    }

    #[test]
    fn allocate_temp_buffer_when_valid_then_returns_slot() {
        let buf = [0u8; 64];
        let slot = allocate_temp_buffer(&buf, 0, 32).unwrap();
        assert_eq!(slot.buf_idx, 0);
        assert_eq!(slot.next_temp_buf, 1);
        assert_eq!(slot.buf_start, 0);
        assert_eq!(slot.max_len, (32 - STRING_HEADER_BYTES) as u16);
    }

    #[test]
    fn allocate_temp_buffer_when_second_slot_then_offset_correct() {
        let buf = [0u8; 64];
        let slot = allocate_temp_buffer(&buf, 1, 32).unwrap();
        assert_eq!(slot.buf_idx, 1);
        assert_eq!(slot.next_temp_buf, 2);
        assert_eq!(slot.buf_start, 32);
    }

    #[test]
    fn allocate_temp_buffer_when_zero_max_then_trap() {
        let buf = [0u8; 64];
        let result = allocate_temp_buffer(&buf, 0, 0);
        assert!(matches!(result, Err(Trap::TempBufferExhausted)));
    }

    #[test]
    fn allocate_temp_buffer_when_exceeds_len_then_trap() {
        let buf = [0u8; 16];
        let result = allocate_temp_buffer(&buf, 0, 32);
        assert!(matches!(result, Err(Trap::TempBufferExhausted)));
    }

    #[test]
    fn write_string_header_when_fits_then_writes_exact() {
        let mut buf = [0u8; 32];
        let (cur_len, data_start) = write_string_header(&mut buf, 0, 28, 10);
        assert_eq!(cur_len, 10);
        assert_eq!(data_start, STRING_HEADER_BYTES);
        assert_eq!(u16::from_le_bytes([buf[0], buf[1]]), 28); // max_len
        assert_eq!(u16::from_le_bytes([buf[2], buf[3]]), 10); // cur_len
    }

    #[test]
    fn write_string_header_when_exceeds_max_then_clamps() {
        let mut buf = [0u8; 32];
        let (cur_len, _) = write_string_header(&mut buf, 0, 5, 100);
        assert_eq!(cur_len, 5);
        assert_eq!(u16::from_le_bytes([buf[2], buf[3]]), 5);
    }
}
