//! Handlers for the array opcodes.
//!
//! Extracted from `vm.rs::execute_with_hook` to keep the dispatch loop
//! tractable. All four handlers share the same shape: read a u16 var
//! index and a u16 array-descriptor index from the operand stream,
//! check bounds, compute the byte offset within the data region, and
//! load or store an 8-byte slot.

use ironplc_container::{Container, VarIndex};

use crate::error::Trap;
use crate::stack::OperandStack;
use crate::value::Slot;
use crate::variable_table::{VariableScope, VariableTable};
use crate::vm::read_u16_le;

pub(crate) fn handle_load_array(
    bytecode: &[u8],
    pc: &mut usize,
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    data_region: &[u8],
    scope: &VariableScope,
) -> Result<(), Trap> {
    let var_index = VarIndex::new(read_u16_le(bytecode, pc)?);
    let desc_index = read_u16_le(bytecode, pc)?;
    let index_slot = stack.pop()?;
    let index_i64 = index_slot.as_i64();

    let total_elements = container
        .type_section
        .as_ref()
        .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
        .map(|d| d.total_elements)
        .ok_or(Trap::InvalidVariableIndex(var_index))?;

    if index_i64 < 0 || index_i64 >= total_elements as i64 {
        return Err(Trap::ArrayIndexOutOfBounds {
            var_index,
            index: index_i64 as i32,
            total_elements,
        });
    }
    let index = index_i64 as u32;

    scope.check_access(var_index)?;

    let data_offset = variables.load(var_index)?.as_i32() as u32 as usize;
    let byte_offset = data_offset + index as usize * 8;

    if byte_offset + 8 > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(byte_offset as u32));
    }

    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data_region[byte_offset..byte_offset + 8]);
    let raw = i64::from_le_bytes(buf);
    stack.push(Slot::from_i64(raw))?;
    Ok(())
}

pub(crate) fn handle_store_array(
    bytecode: &[u8],
    pc: &mut usize,
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    data_region: &mut [u8],
    scope: &VariableScope,
) -> Result<(), Trap> {
    let var_index = VarIndex::new(read_u16_le(bytecode, pc)?);
    let desc_index = read_u16_le(bytecode, pc)?;
    let index_slot = stack.pop()?;
    let value_slot = stack.pop()?;
    let index_i64 = index_slot.as_i64();

    let total_elements = container
        .type_section
        .as_ref()
        .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
        .map(|d| d.total_elements)
        .ok_or(Trap::InvalidVariableIndex(var_index))?;

    if index_i64 < 0 || index_i64 >= total_elements as i64 {
        return Err(Trap::ArrayIndexOutOfBounds {
            var_index,
            index: index_i64 as i32,
            total_elements,
        });
    }
    let index = index_i64 as u32;

    scope.check_access(var_index)?;

    let data_offset = variables.load(var_index)?.as_i32() as u32 as usize;
    let byte_offset = data_offset + index as usize * 8;

    if byte_offset + 8 > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(byte_offset as u32));
    }

    data_region[byte_offset..byte_offset + 8]
        .copy_from_slice(&value_slot.as_i64().to_le_bytes());
    Ok(())
}

pub(crate) fn handle_load_array_deref(
    bytecode: &[u8],
    pc: &mut usize,
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    data_region: &[u8],
    scope: &VariableScope,
) -> Result<(), Trap> {
    let ref_var_index = VarIndex::new(read_u16_le(bytecode, pc)?);
    let desc_index = read_u16_le(bytecode, pc)?;
    let index_slot = stack.pop()?;
    let index_i64 = index_slot.as_i64();

    scope.check_access(ref_var_index)?;
    let target_slot = variables.load(ref_var_index)?;
    let target_raw = target_slot.as_i64() as u64;
    if target_raw == u64::MAX {
        return Err(Trap::NullDereference);
    }
    let target_var_index = VarIndex::new(target_raw as u16);

    let total_elements = container
        .type_section
        .as_ref()
        .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
        .map(|d| d.total_elements)
        .ok_or(Trap::InvalidVariableIndex(ref_var_index))?;

    if index_i64 < 0 || index_i64 >= total_elements as i64 {
        return Err(Trap::ArrayIndexOutOfBounds {
            var_index: target_var_index,
            index: index_i64 as i32,
            total_elements,
        });
    }
    let index = index_i64 as u32;

    let data_offset = variables.load(target_var_index)?.as_i32() as u32 as usize;
    let byte_offset = data_offset + index as usize * 8;

    if byte_offset + 8 > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(byte_offset as u32));
    }

    let mut buf = [0u8; 8];
    buf.copy_from_slice(&data_region[byte_offset..byte_offset + 8]);
    let raw = i64::from_le_bytes(buf);
    stack.push(Slot::from_i64(raw))?;
    Ok(())
}

pub(crate) fn handle_store_array_deref(
    bytecode: &[u8],
    pc: &mut usize,
    container: &Container,
    stack: &mut OperandStack,
    variables: &mut VariableTable,
    data_region: &mut [u8],
    scope: &VariableScope,
) -> Result<(), Trap> {
    let ref_var_index = VarIndex::new(read_u16_le(bytecode, pc)?);
    let desc_index = read_u16_le(bytecode, pc)?;
    let index_slot = stack.pop()?;
    let value_slot = stack.pop()?;
    let index_i64 = index_slot.as_i64();

    scope.check_access(ref_var_index)?;
    let target_slot = variables.load(ref_var_index)?;
    let target_raw = target_slot.as_i64() as u64;
    if target_raw == u64::MAX {
        return Err(Trap::NullDereference);
    }
    let target_var_index = VarIndex::new(target_raw as u16);

    let total_elements = container
        .type_section
        .as_ref()
        .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
        .map(|d| d.total_elements)
        .ok_or(Trap::InvalidVariableIndex(ref_var_index))?;

    if index_i64 < 0 || index_i64 >= total_elements as i64 {
        return Err(Trap::ArrayIndexOutOfBounds {
            var_index: target_var_index,
            index: index_i64 as i32,
            total_elements,
        });
    }
    let index = index_i64 as u32;

    let data_offset = variables.load(target_var_index)?.as_i32() as u32 as usize;
    let byte_offset = data_offset + index as usize * 8;

    if byte_offset + 8 > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(byte_offset as u32));
    }

    data_region[byte_offset..byte_offset + 8]
        .copy_from_slice(&value_slot.as_i64().to_le_bytes());
    Ok(())
}
