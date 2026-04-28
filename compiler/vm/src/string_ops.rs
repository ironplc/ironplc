use ironplc_container::STRING_HEADER_BYTES;

use crate::error::Trap;

/// Metadata for an allocated temp buffer slot.
pub(crate) struct TempBufferSlot {
    /// Index of this buffer slot (the value pushed onto the stack).
    pub buf_idx: u16,
    /// Byte offset where this slot starts in the temp buffer.
    pub buf_start: usize,
    /// Maximum string data length (capacity minus header).
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

    /// Allocate the next temp buffer slot.
    ///
    /// Returns a [`TempBufferSlot`] with the slot index, byte offset,
    /// and max data length. The internal counter is advanced automatically.
    pub fn alloc(&mut self, temp_buf_len: usize) -> Result<TempBufferSlot, Trap> {
        if self.max_temp_buf_bytes == 0 {
            return Err(Trap::TempBufferExhausted);
        }
        let buf_idx = self.next;
        let buf_start = buf_idx as usize * self.max_temp_buf_bytes;
        let buf_end = buf_start + self.max_temp_buf_bytes;
        if buf_end > temp_buf_len {
            return Err(Trap::TempBufferExhausted);
        }
        let max_len = (self.max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
        self.next = self.next.wrapping_add(1);
        Ok(TempBufferSlot {
            buf_idx,
            buf_start,
            max_len,
        })
    }
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
    let cur_len = u16::from_le_bytes([data_region[offset + 2], data_region[offset + 3]]) as usize;
    let data_start = offset + STRING_HEADER_BYTES;
    Ok((cur_len, data_start))
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

// ============================================================================
// Opcode handlers
// ============================================================================
//
// Each `handle_*` function corresponds to one string-related opcode. Extracted
// from `vm.rs::execute_with_hook` to keep the dispatch loop tractable. The
// dispatch loop calls these via `string_ops::handle_X(...)?` instead of
// inlining the body.

use ironplc_container::{Container, ConstantIndex, VarIndex};

use crate::stack::OperandStack;
use crate::value::Slot;
use crate::variable_table::{VariableScope, VariableTable};
use crate::dispatch::{read_u16_le, read_u32_le};

pub(crate) fn handle_str_init(
    bytecode: &[u8],
    pc: &mut usize,
    data_region: &mut [u8],
) -> Result<(), Trap> {
    let data_offset = read_u32_le(bytecode, pc)? as usize;
    let max_length = read_u16_le(bytecode, pc)?;
    if data_offset + STRING_HEADER_BYTES > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
    }
    str_write_header(data_region, data_offset, max_length, 0);
    Ok(())
}

pub(crate) fn handle_load_const_str(
    bytecode: &[u8],
    pc: &mut usize,
    container: &Container,
    stack: &mut OperandStack,
    temp_buf: &mut [u8],
    max_temp_buf_bytes: usize,
    temp_alloc: &mut TempBufAllocator,
) -> Result<(), Trap> {
    let index = read_u16_le(bytecode, pc)?;
    let str_bytes = container
        .constant_pool
        .get_str(ConstantIndex::new(index))
        .map_err(|_| Trap::InvalidConstantIndex(ConstantIndex::new(index)))?;

    let (buf_idx, buf_start) = {
        let slot = temp_alloc.alloc(temp_buf.len())?;
        (slot.buf_idx as usize, slot.buf_start)
    };

    let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
    let cur_len = (str_bytes.len() as u16).min(max_len);
    str_write_header(temp_buf, buf_start, max_len, cur_len);
    temp_buf[buf_start + STRING_HEADER_BYTES
        ..buf_start + STRING_HEADER_BYTES + cur_len as usize]
        .copy_from_slice(&str_bytes[..cur_len as usize]);
    stack.push(Slot::from_i32(buf_idx as i32))?;
    Ok(())
}

pub(crate) fn handle_str_store_var(
    bytecode: &[u8],
    pc: &mut usize,
    stack: &mut OperandStack,
    data_region: &mut [u8],
    temp_buf: &[u8],
    max_temp_buf_bytes: usize,
) -> Result<(), Trap> {
    let data_offset = read_u32_le(bytecode, pc)? as usize;
    let buf_idx = stack.pop()?.as_i32() as usize;

    let buf_start = buf_idx * max_temp_buf_bytes;
    if buf_start + STRING_HEADER_BYTES > temp_buf.len() {
        return Err(Trap::TempBufferExhausted);
    }
    let src_cur_len = str_read_cur_len(temp_buf, buf_start);

    if data_offset + STRING_HEADER_BYTES > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
    }
    let dest_max_len = str_read_max_len(data_region, data_offset);

    let copy_len = src_cur_len.min(dest_max_len) as usize;
    if data_offset + STRING_HEADER_BYTES + copy_len > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
    }
    let dst_start = data_offset + STRING_HEADER_BYTES;
    let src_start = buf_start + STRING_HEADER_BYTES;
    data_region[dst_start..dst_start + copy_len]
        .copy_from_slice(&temp_buf[src_start..src_start + copy_len]);
    data_region[data_offset + 2..data_offset + STRING_HEADER_BYTES]
        .copy_from_slice(&(copy_len as u16).to_le_bytes());
    Ok(())
}

pub(crate) fn handle_str_load_var(
    bytecode: &[u8],
    pc: &mut usize,
    stack: &mut OperandStack,
    data_region: &[u8],
    temp_buf: &mut [u8],
    max_temp_buf_bytes: usize,
    temp_alloc: &mut TempBufAllocator,
) -> Result<(), Trap> {
    let data_offset = read_u32_le(bytecode, pc)? as usize;
    if data_offset + STRING_HEADER_BYTES > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
    }
    let src_max_len = str_read_max_len(data_region, data_offset);
    let src_cur_len = str_read_cur_len(data_region, data_offset);
    let read_len = src_cur_len.min(src_max_len) as usize;

    let (buf_idx, buf_start) = {
        let slot = temp_alloc.alloc(temp_buf.len())?;
        (slot.buf_idx as usize, slot.buf_start)
    };

    let max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
    let cur_len = (read_len as u16).min(max_len);
    str_write_header(temp_buf, buf_start, max_len, cur_len);
    if data_offset + STRING_HEADER_BYTES + cur_len as usize > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
    }
    let dst_start = buf_start + STRING_HEADER_BYTES;
    let src_start = data_offset + STRING_HEADER_BYTES;
    temp_buf[dst_start..dst_start + cur_len as usize]
        .copy_from_slice(&data_region[src_start..src_start + cur_len as usize]);
    stack.push(Slot::from_i32(buf_idx as i32))?;
    Ok(())
}

pub(crate) fn handle_len_str(
    bytecode: &[u8],
    pc: &mut usize,
    stack: &mut OperandStack,
    data_region: &[u8],
) -> Result<(), Trap> {
    let data_offset = read_u32_le(bytecode, pc)? as usize;
    if data_offset + STRING_HEADER_BYTES > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(data_offset as u32));
    }
    let cur_len = u16::from_le_bytes([
        data_region[data_offset + 2],
        data_region[data_offset + 3],
    ]);
    stack.push(Slot::from_i32(cur_len as i32))?;
    Ok(())
}

pub(crate) fn handle_find_str(
    bytecode: &[u8],
    pc: &mut usize,
    stack: &mut OperandStack,
    data_region: &[u8],
) -> Result<(), Trap> {
    let in1_offset = read_u32_le(bytecode, pc)? as usize;
    let in2_offset = read_u32_le(bytecode, pc)? as usize;

    if in1_offset + STRING_HEADER_BYTES > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(in1_offset as u32));
    }
    let in1_len =
        u16::from_le_bytes([data_region[in1_offset + 2], data_region[in1_offset + 3]]) as usize;

    if in2_offset + STRING_HEADER_BYTES > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(in2_offset as u32));
    }
    let in2_len =
        u16::from_le_bytes([data_region[in2_offset + 2], data_region[in2_offset + 3]]) as usize;

    let result = if in2_len == 0 || in2_len > in1_len {
        0i32
    } else {
        let in1_start = in1_offset + STRING_HEADER_BYTES;
        let in2_start = in2_offset + STRING_HEADER_BYTES;
        let in1_data = &data_region[in1_start..in1_start + in1_len];
        let in2_data = &data_region[in2_start..in2_start + in2_len];
        let mut found = 0i32;
        for i in 0..=(in1_len - in2_len) {
            if in1_data[i..i + in2_len] == *in2_data {
                found = (i + 1) as i32;
                break;
            }
        }
        found
    };
    stack.push(Slot::from_i32(result))?;
    Ok(())
}

pub(crate) fn handle_replace_str(
    bytecode: &[u8],
    pc: &mut usize,
    stack: &mut OperandStack,
    data_region: &[u8],
    temp_buf: &mut [u8],
    temp_alloc: &mut TempBufAllocator,
) -> Result<(), Trap> {
    let in1_offset = read_u32_le(bytecode, pc)? as usize;
    let in2_offset = read_u32_le(bytecode, pc)? as usize;
    let p_val = stack.pop()?.as_i32();
    let l_val = stack.pop()?.as_i32();

    let (in1_len, in1_start) = read_string_header(data_region, in1_offset)?;
    let (in2_len, in2_start) = read_string_header(data_region, in2_offset)?;

    let p = if p_val < 1 { 1usize } else { p_val as usize };
    let l = if l_val < 0 { 0usize } else { l_val as usize };
    let start_idx = (p - 1).min(in1_len);
    let delete_len = l.min(in1_len - start_idx);

    let prefix_len = start_idx;
    let suffix_start = start_idx + delete_len;
    let suffix_len = in1_len - suffix_start;
    let result_len = prefix_len + in2_len + suffix_len;

    let slot = temp_alloc.alloc(temp_buf.len())?;

    let (cur_len, data_start) =
        write_string_header(temp_buf, slot.buf_start, slot.max_len, result_len);

    let mut write_pos = 0usize;
    let prefix_copy = prefix_len.min(cur_len as usize);
    temp_buf[data_start..data_start + prefix_copy]
        .copy_from_slice(&data_region[in1_start..in1_start + prefix_copy]);
    write_pos += prefix_copy;
    let in2_copy = in2_len.min((cur_len as usize).saturating_sub(write_pos));
    temp_buf[data_start + write_pos..data_start + write_pos + in2_copy]
        .copy_from_slice(&data_region[in2_start..in2_start + in2_copy]);
    write_pos += in2_copy;
    let suffix_copy = suffix_len.min((cur_len as usize).saturating_sub(write_pos));
    temp_buf[data_start + write_pos..data_start + write_pos + suffix_copy].copy_from_slice(
        &data_region[in1_start + suffix_start..in1_start + suffix_start + suffix_copy],
    );

    stack.push(Slot::from_i32(slot.buf_idx as i32))?;
    Ok(())
}

pub(crate) fn handle_insert_str(
    bytecode: &[u8],
    pc: &mut usize,
    stack: &mut OperandStack,
    data_region: &[u8],
    temp_buf: &mut [u8],
    temp_alloc: &mut TempBufAllocator,
) -> Result<(), Trap> {
    let in1_offset = read_u32_le(bytecode, pc)? as usize;
    let in2_offset = read_u32_le(bytecode, pc)? as usize;
    let p_val = stack.pop()?.as_i32();

    let (in1_len, in1_start) = read_string_header(data_region, in1_offset)?;
    let (in2_len, in2_start) = read_string_header(data_region, in2_offset)?;

    let p = if p_val < 0 { 0usize } else { p_val as usize };
    let insert_idx = p.min(in1_len);

    let prefix_len = insert_idx;
    let suffix_len = in1_len - insert_idx;
    let result_len = prefix_len + in2_len + suffix_len;

    let slot = temp_alloc.alloc(temp_buf.len())?;

    let (cur_len, data_start) =
        write_string_header(temp_buf, slot.buf_start, slot.max_len, result_len);

    let mut write_pos = 0usize;
    let prefix_copy = prefix_len.min(cur_len as usize);
    temp_buf[data_start..data_start + prefix_copy]
        .copy_from_slice(&data_region[in1_start..in1_start + prefix_copy]);
    write_pos += prefix_copy;
    let in2_copy = in2_len.min((cur_len as usize).saturating_sub(write_pos));
    temp_buf[data_start + write_pos..data_start + write_pos + in2_copy]
        .copy_from_slice(&data_region[in2_start..in2_start + in2_copy]);
    write_pos += in2_copy;
    let suffix_copy = suffix_len.min((cur_len as usize).saturating_sub(write_pos));
    temp_buf[data_start + write_pos..data_start + write_pos + suffix_copy].copy_from_slice(
        &data_region[in1_start + insert_idx..in1_start + insert_idx + suffix_copy],
    );

    stack.push(Slot::from_i32(slot.buf_idx as i32))?;
    Ok(())
}

pub(crate) fn handle_delete_str(
    bytecode: &[u8],
    pc: &mut usize,
    stack: &mut OperandStack,
    data_region: &[u8],
    temp_buf: &mut [u8],
    temp_alloc: &mut TempBufAllocator,
) -> Result<(), Trap> {
    let in1_offset = read_u32_le(bytecode, pc)? as usize;
    let p_val = stack.pop()?.as_i32();
    let l_val = stack.pop()?.as_i32();

    let (in1_len, in1_start) = read_string_header(data_region, in1_offset)?;

    let p = if p_val < 1 { 1usize } else { p_val as usize };
    let l = if l_val < 0 { 0usize } else { l_val as usize };
    let start_idx = (p - 1).min(in1_len);
    let delete_len = l.min(in1_len - start_idx);

    let prefix_len = start_idx;
    let suffix_start = start_idx + delete_len;
    let suffix_len = in1_len - suffix_start;
    let result_len = prefix_len + suffix_len;

    let slot = temp_alloc.alloc(temp_buf.len())?;

    let (cur_len, data_start) =
        write_string_header(temp_buf, slot.buf_start, slot.max_len, result_len);

    let mut write_pos = 0usize;
    let prefix_copy = prefix_len.min(cur_len as usize);
    temp_buf[data_start..data_start + prefix_copy]
        .copy_from_slice(&data_region[in1_start..in1_start + prefix_copy]);
    write_pos += prefix_copy;
    let suffix_copy = suffix_len.min((cur_len as usize).saturating_sub(write_pos));
    temp_buf[data_start + write_pos..data_start + write_pos + suffix_copy].copy_from_slice(
        &data_region[in1_start + suffix_start..in1_start + suffix_start + suffix_copy],
    );

    stack.push(Slot::from_i32(slot.buf_idx as i32))?;
    Ok(())
}

pub(crate) fn handle_left_str(
    bytecode: &[u8],
    pc: &mut usize,
    stack: &mut OperandStack,
    data_region: &[u8],
    temp_buf: &mut [u8],
    temp_alloc: &mut TempBufAllocator,
) -> Result<(), Trap> {
    let in_offset = read_u32_le(bytecode, pc)? as usize;
    let l_val = stack.pop()?.as_i32();

    let (in_len, in_start) = read_string_header(data_region, in_offset)?;

    let l = if l_val < 0 { 0usize } else { l_val as usize };
    let result_len = l.min(in_len);

    let slot = temp_alloc.alloc(temp_buf.len())?;

    let (cur_len, data_start) =
        write_string_header(temp_buf, slot.buf_start, slot.max_len, result_len);

    let copy_len = cur_len as usize;
    temp_buf[data_start..data_start + copy_len]
        .copy_from_slice(&data_region[in_start..in_start + copy_len]);

    stack.push(Slot::from_i32(slot.buf_idx as i32))?;
    Ok(())
}

pub(crate) fn handle_right_str(
    bytecode: &[u8],
    pc: &mut usize,
    stack: &mut OperandStack,
    data_region: &[u8],
    temp_buf: &mut [u8],
    temp_alloc: &mut TempBufAllocator,
) -> Result<(), Trap> {
    let in_offset = read_u32_le(bytecode, pc)? as usize;
    let l_val = stack.pop()?.as_i32();

    let (in_len, in_start) = read_string_header(data_region, in_offset)?;

    let l = if l_val < 0 { 0usize } else { l_val as usize };
    let result_len = l.min(in_len);
    let src_start = in_len - result_len;

    let slot = temp_alloc.alloc(temp_buf.len())?;

    let (cur_len, data_start) =
        write_string_header(temp_buf, slot.buf_start, slot.max_len, result_len);

    let copy_len = cur_len as usize;
    let src = in_start + src_start;
    temp_buf[data_start..data_start + copy_len]
        .copy_from_slice(&data_region[src..src + copy_len]);

    stack.push(Slot::from_i32(slot.buf_idx as i32))?;
    Ok(())
}

pub(crate) fn handle_mid_str(
    bytecode: &[u8],
    pc: &mut usize,
    stack: &mut OperandStack,
    data_region: &[u8],
    temp_buf: &mut [u8],
    temp_alloc: &mut TempBufAllocator,
) -> Result<(), Trap> {
    let in_offset = read_u32_le(bytecode, pc)? as usize;
    let p_val = stack.pop()?.as_i32();
    let l_val = stack.pop()?.as_i32();

    let (in_len, in_start) = read_string_header(data_region, in_offset)?;

    let p = if p_val < 1 { 1usize } else { p_val as usize };
    let l = if l_val < 0 { 0usize } else { l_val as usize };
    let start_idx = (p - 1).min(in_len);
    let result_len = l.min(in_len - start_idx);

    let slot = temp_alloc.alloc(temp_buf.len())?;

    let (cur_len, data_start) =
        write_string_header(temp_buf, slot.buf_start, slot.max_len, result_len);

    let copy_len = cur_len as usize;
    let src = in_start + start_idx;
    temp_buf[data_start..data_start + copy_len]
        .copy_from_slice(&data_region[src..src + copy_len]);

    stack.push(Slot::from_i32(slot.buf_idx as i32))?;
    Ok(())
}

pub(crate) fn handle_concat_str(
    bytecode: &[u8],
    pc: &mut usize,
    stack: &mut OperandStack,
    data_region: &[u8],
    temp_buf: &mut [u8],
    temp_alloc: &mut TempBufAllocator,
) -> Result<(), Trap> {
    let in1_offset = read_u32_le(bytecode, pc)? as usize;
    let in2_offset = read_u32_le(bytecode, pc)? as usize;

    let (in1_len, in1_start) = read_string_header(data_region, in1_offset)?;
    let (in2_len, in2_start) = read_string_header(data_region, in2_offset)?;

    let result_len = in1_len + in2_len;

    let slot = temp_alloc.alloc(temp_buf.len())?;

    let (cur_len, data_start) =
        write_string_header(temp_buf, slot.buf_start, slot.max_len, result_len);

    let mut write_pos = 0usize;
    let in1_copy = in1_len.min(cur_len as usize);
    for i in 0..in1_copy {
        temp_buf[data_start + write_pos] = data_region[in1_start + i];
        write_pos += 1;
    }
    let in2_copy = in2_len.min((cur_len as usize).saturating_sub(write_pos));
    for i in 0..in2_copy {
        temp_buf[data_start + write_pos] = data_region[in2_start + i];
        write_pos += 1;
    }

    stack.push(Slot::from_i32(slot.buf_idx as i32))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_str_init_array(
    bytecode: &[u8],
    pc: &mut usize,
    container: &Container,
    variables: &mut VariableTable,
    data_region: &mut [u8],
    scope: &VariableScope,
) -> Result<(), Trap> {
    let var_index = VarIndex::new(read_u16_le(bytecode, pc)?);
    let desc_index = read_u16_le(bytecode, pc)?;

    let desc = container
        .type_section
        .as_ref()
        .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
        .ok_or(Trap::InvalidVariableIndex(var_index))?;
    let total_elements = desc.total_elements;
    let max_str_len = desc.element_extra;
    let stride = STRING_HEADER_BYTES + max_str_len as usize;

    scope.check_access(var_index)?;
    let base_offset = variables.load(var_index)?.as_i32() as u32 as usize;

    for i in 0..total_elements as usize {
        let elem_offset = base_offset + i * stride;
        if elem_offset + STRING_HEADER_BYTES > data_region.len() {
            return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
        }
        str_write_header(data_region, elem_offset, max_str_len, 0);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_str_load_array_elem(
    bytecode: &[u8],
    pc: &mut usize,
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    data_region: &[u8],
    temp_buf: &mut [u8],
    max_temp_buf_bytes: usize,
    temp_alloc: &mut TempBufAllocator,
    scope: &VariableScope,
) -> Result<(), Trap> {
    let var_index = VarIndex::new(read_u16_le(bytecode, pc)?);
    let desc_index = read_u16_le(bytecode, pc)?;
    let index_slot = stack.pop()?;
    let index_i64 = index_slot.as_i64();

    let desc = container
        .type_section
        .as_ref()
        .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
        .ok_or(Trap::InvalidVariableIndex(var_index))?;
    let total_elements = desc.total_elements;
    let max_str_len = desc.element_extra;
    let stride = STRING_HEADER_BYTES + max_str_len as usize;

    if index_i64 < 0 || index_i64 >= total_elements as i64 {
        return Err(Trap::ArrayIndexOutOfBounds {
            var_index,
            index: index_i64 as i32,
            total_elements,
        });
    }
    let index = index_i64 as usize;

    scope.check_access(var_index)?;
    let base_offset = variables.load(var_index)?.as_i32() as u32 as usize;
    let elem_offset = base_offset + index * stride;

    if elem_offset + STRING_HEADER_BYTES > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
    }
    let src_cur_len = str_read_cur_len(data_region, elem_offset);
    let read_len = src_cur_len.min(max_str_len) as usize;

    let (buf_idx, buf_start) = {
        let slot = temp_alloc.alloc(temp_buf.len())?;
        (slot.buf_idx as usize, slot.buf_start)
    };

    let buf_max_len = (max_temp_buf_bytes - STRING_HEADER_BYTES) as u16;
    let cur_len = (read_len as u16).min(buf_max_len);
    str_write_header(temp_buf, buf_start, buf_max_len, cur_len);

    if elem_offset + STRING_HEADER_BYTES + cur_len as usize > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
    }
    let dst_start = buf_start + STRING_HEADER_BYTES;
    let src_start = elem_offset + STRING_HEADER_BYTES;
    temp_buf[dst_start..dst_start + cur_len as usize]
        .copy_from_slice(&data_region[src_start..src_start + cur_len as usize]);

    stack.push(Slot::from_i32(buf_idx as i32))?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn handle_str_store_array_elem(
    bytecode: &[u8],
    pc: &mut usize,
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    data_region: &mut [u8],
    temp_buf: &[u8],
    max_temp_buf_bytes: usize,
    scope: &VariableScope,
) -> Result<(), Trap> {
    let var_index = VarIndex::new(read_u16_le(bytecode, pc)?);
    let desc_index = read_u16_le(bytecode, pc)?;
    let index_slot = stack.pop()?;
    let value_slot = stack.pop()?;
    let index_i64 = index_slot.as_i64();
    let buf_idx = value_slot.as_i32() as usize;

    let desc = container
        .type_section
        .as_ref()
        .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
        .ok_or(Trap::InvalidVariableIndex(var_index))?;
    let total_elements = desc.total_elements;
    let max_str_len = desc.element_extra;
    let stride = STRING_HEADER_BYTES + max_str_len as usize;

    if index_i64 < 0 || index_i64 >= total_elements as i64 {
        return Err(Trap::ArrayIndexOutOfBounds {
            var_index,
            index: index_i64 as i32,
            total_elements,
        });
    }
    let index = index_i64 as usize;

    scope.check_access(var_index)?;
    let base_offset = variables.load(var_index)?.as_i32() as u32 as usize;
    let elem_offset = base_offset + index * stride;

    let buf_start = buf_idx * max_temp_buf_bytes;
    if buf_start + STRING_HEADER_BYTES > temp_buf.len() {
        return Err(Trap::TempBufferExhausted);
    }
    let src_cur_len = str_read_cur_len(temp_buf, buf_start);

    if elem_offset + STRING_HEADER_BYTES > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
    }

    let copy_len = src_cur_len.min(max_str_len) as usize;
    if elem_offset + STRING_HEADER_BYTES + copy_len > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(elem_offset as u32));
    }
    let dst_start = elem_offset + STRING_HEADER_BYTES;
    let src_start = buf_start + STRING_HEADER_BYTES;
    data_region[dst_start..dst_start + copy_len]
        .copy_from_slice(&temp_buf[src_start..src_start + copy_len]);

    data_region[elem_offset + 2..elem_offset + STRING_HEADER_BYTES]
        .copy_from_slice(&(copy_len as u16).to_le_bytes());
    Ok(())
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
    fn alloc_when_valid_then_returns_slot() {
        let mut alloc = TempBufAllocator::new(32);
        let slot = alloc.alloc(64).unwrap();
        assert_eq!(slot.buf_idx, 0);
        assert_eq!(slot.buf_start, 0);
        assert_eq!(slot.max_len, (32 - STRING_HEADER_BYTES) as u16);
    }

    #[test]
    fn alloc_when_called_twice_then_second_slot_offset_correct() {
        let mut alloc = TempBufAllocator::new(32);
        let _first = alloc.alloc(64).unwrap();
        let second = alloc.alloc(64).unwrap();
        assert_eq!(second.buf_idx, 1);
        assert_eq!(second.buf_start, 32);
    }

    #[test]
    fn alloc_when_zero_max_then_trap() {
        let mut alloc = TempBufAllocator::new(0);
        let result = alloc.alloc(64);
        assert!(matches!(result, Err(Trap::TempBufferExhausted)));
    }

    #[test]
    fn alloc_when_exceeds_len_then_trap() {
        let mut alloc = TempBufAllocator::new(32);
        let result = alloc.alloc(16);
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
