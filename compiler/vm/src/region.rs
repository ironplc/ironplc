//! Copies of a whole array or structure within the data region.
//!
//! `COPY_REGION` finds the destination at the data offset a variable's slot
//! holds, and `COPY_REGION_DEREF` through a reference
//! (`ironplc_container::reference`). From there the two are the same copy:
//! both ends sized from their array descriptors, which must agree, and
//! confined to the data region.

use ironplc_container::{Container, VarIndex};

use crate::error::Trap;

/// The byte size of the copy between the regions the descriptors
/// `dst_desc` and `src_desc` describe. Both sizes come from the container's
/// array descriptors rather than from an operand, so a codegen defect cannot
/// ask for a copy that does not match the objects involved. A missing
/// descriptor traps, naming `var_index`, the variable the instruction
/// reaches the destination through, and two sizes that differ trap
/// `RegionSizeMismatch`.
pub(crate) fn size(
    container: &Container,
    dst_desc: u16,
    src_desc: u16,
    var_index: VarIndex,
) -> Result<usize, Trap> {
    let descriptor_bytes = |desc_index: u16| {
        container
            .type_section
            .as_ref()
            .and_then(|ts| ts.array_descriptors.get(desc_index as usize))
            .and_then(|d| d.byte_size())
    };
    let dst_bytes = descriptor_bytes(dst_desc).ok_or(Trap::InvalidVariableIndex(var_index))?;
    let src_bytes = descriptor_bytes(src_desc).ok_or(Trap::InvalidVariableIndex(var_index))?;
    if dst_bytes != src_bytes {
        return Err(Trap::RegionSizeMismatch {
            dst_bytes,
            src_bytes,
        });
    }
    Ok(dst_bytes as usize)
}

/// Copies the `count` bytes at `src_offset` to `dst_offset`, both confined
/// to the data region.
pub(crate) fn copy(
    data_region: &mut [u8],
    dst_offset: usize,
    src_offset: usize,
    count: usize,
) -> Result<(), Trap> {
    // Checked arithmetic so an offset near u32::MAX cannot wrap into a
    // valid-looking range on a 32-bit target.
    let dst_end = dst_offset
        .checked_add(count)
        .ok_or(Trap::DataRegionOutOfBounds(dst_offset as u32))?;
    let src_end = src_offset
        .checked_add(count)
        .ok_or(Trap::DataRegionOutOfBounds(src_offset as u32))?;
    if dst_end > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(dst_offset as u32));
    }
    if src_end > data_region.len() {
        return Err(Trap::DataRegionOutOfBounds(src_offset as u32));
    }

    // copy_within is defined for overlapping ranges, so a self-assignment
    // (`x := x`, identical offsets) is a no-op rather than corruption.
    data_region.copy_within(src_offset..src_end, dst_offset);
    Ok(())
}
