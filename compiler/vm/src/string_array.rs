//! Loads and stores of an element of a string array.
//!
//! `STR_LOAD_ARRAY_ELEM` and `STR_STORE_ARRAY_ELEM` find the array's first
//! element at the data offset a variable's slot holds, and
//! `STR_LOAD_ARRAY_DEREF` and `STR_STORE_ARRAY_DEREF` through a reference
//! (`ironplc_container::reference`). From there the four are the same
//! access: the element's offset from the descriptor's stride, and a copy
//! between the element and a temp buffer bounded by the descriptor's
//! capacity.

use ironplc_container::{ArrayDescriptor, VarIndex, STRING_HEADER_BYTES};

use crate::error::Trap;
use crate::string_ops::{self, TempBufAllocator};

/// The data offset of element `index` of the string array `desc` describes,
/// whose first element is at `base`. An index outside the array traps,
/// naming `var_index`, the variable the instruction reaches the array
/// through.
pub(crate) fn element_offset(
    desc: &ArrayDescriptor,
    var_index: VarIndex,
    base: usize,
    index: i64,
) -> Result<usize, Trap> {
    if index < 0 || index >= desc.total_elements as i64 {
        return Err(Trap::ArrayIndexOutOfBounds {
            var_index,
            index: index as i32,
            total_elements: desc.total_elements,
        });
    }
    Ok(base + index as usize * desc.element_stride() as usize)
}

/// Copies the string element at `elem_offset`, at most `max_len` code units
/// of it, into a fresh temp buffer, and returns the buffer's index.
pub(crate) fn load(
    data_region: &[u8],
    temp_buf: &mut [u8],
    temp_alloc: &mut TempBufAllocator,
    elem_offset: usize,
    max_len: u16,
) -> Result<i32, Trap> {
    if elem_offset + STRING_HEADER_BYTES > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
    }
    let elem_width = string_ops::str_read_char_width(data_region, elem_offset)?;
    let src_cur_len = string_ops::str_read_cur_len(data_region, elem_offset);
    let read_units = src_cur_len.min(max_len) as usize;

    let (buf_idx, buf_start, buf_max_len) = {
        let slot = temp_alloc.alloc(temp_buf.len(), elem_width)?;
        (slot.buf_idx as usize, slot.buf_start, slot.max_len)
    };

    let cur_len = (read_units as u16).min(buf_max_len);
    string_ops::str_write_header(temp_buf, buf_start, buf_max_len, cur_len, elem_width);

    let copy_bytes = cur_len as usize * elem_width.as_usize();
    if elem_offset + STRING_HEADER_BYTES + copy_bytes > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
    }
    string_ops::copy_code_units(
        temp_buf,
        buf_start + STRING_HEADER_BYTES,
        data_region,
        elem_offset + STRING_HEADER_BYTES,
        cur_len as usize,
        elem_width,
    );
    Ok(buf_idx as i32)
}

/// Copies the string in temp buffer `buf_idx` into the string element at
/// `elem_offset`, truncated to `max_len` code units per IEC 61131-3, and
/// hands the buffer back.
pub(crate) fn store(
    data_region: &mut [u8],
    temp_buf: &[u8],
    temp_alloc: &mut TempBufAllocator,
    max_temp_buf_bytes: usize,
    buf_idx: usize,
    elem_offset: usize,
    max_len: u16,
) -> Result<(), Trap> {
    // Read source from temp buffer.
    let buf_start = buf_idx * max_temp_buf_bytes;
    if buf_start + STRING_HEADER_BYTES > temp_buf.len() {
        return Err(Trap::TempBufferExhausted);
    }
    let src_width = string_ops::str_read_char_width(temp_buf, buf_start)?;
    let src_cur_len = string_ops::str_read_cur_len(temp_buf, buf_start);

    if elem_offset + STRING_HEADER_BYTES > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
    }
    // The source's encoding must match the array element's (ADR-0034).
    let dest_width = string_ops::str_read_char_width(data_region, elem_offset)?;
    string_ops::verify_encoding(dest_width, src_width)?;

    // Copy, truncating to max_len code units per IEC 61131-3.
    let copy_units = src_cur_len.min(max_len) as usize;
    let copy_bytes = copy_units * dest_width.as_usize();
    if elem_offset + STRING_HEADER_BYTES + copy_bytes > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
    }
    string_ops::copy_code_units(
        data_region,
        elem_offset + STRING_HEADER_BYTES,
        temp_buf,
        buf_start + STRING_HEADER_BYTES,
        copy_units,
        dest_width,
    );

    // Update destination cur_length (code units).
    string_ops::str_write_cur_len(data_region, elem_offset, copy_units as u16);

    // The value has been copied out, so the buffer holding it is dead: hand
    // its slot back for the next allocation.
    temp_alloc.release(buf_idx);
    Ok(())
}
