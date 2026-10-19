//! How a reference is held in a 64-bit slot, shared by the code generator,
//! which makes references, and the VM, which follows them.
//!
//! A reference takes one of three forms
//! (`specs/design/var-in-out-parameters.md`, "References"):
//!
//! | Form | Encoding | Names |
//! |---|---|---|
//! | null | `u64::MAX` | nothing |
//! | variable reference | `0 ≤ r < 2^16` | slot `r` of the variable table |
//! | data-region reference | `2^32 + offset`, `offset < 2^32` | byte `offset` of the data region |
//!
//! The forms cannot collide: a variable index is 16 bits, and a data-region
//! reference has bit 32 set and bits 33 to 63 clear, so it is never
//! `u64::MAX`.

/// The bit every data-region reference has set and no variable reference
/// has.
const DATA_REGION_TAG: u64 = 1 << 32;

/// The data-region reference to byte `offset` of the data region.
pub fn data_region_reference(offset: u32) -> u64 {
    DATA_REGION_TAG | u64::from(offset)
}

/// The byte of the data region `reference` names, when it is a data-region
/// reference. `None` for the null reference, a variable reference, and any
/// value of neither form.
pub fn data_region_offset(reference: u64) -> Option<u32> {
    (reference >> 32 == 1).then_some(reference as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_region_offset_when_data_region_reference_then_its_offset() {
        assert_eq!(data_region_offset(data_region_reference(0)), Some(0));
        assert_eq!(data_region_offset(data_region_reference(1234)), Some(1234));
        assert_eq!(
            data_region_offset(data_region_reference(u32::MAX)),
            Some(u32::MAX)
        );
    }

    #[test]
    fn data_region_offset_when_null_or_variable_reference_then_none() {
        assert_eq!(data_region_offset(u64::MAX), None);
        assert_eq!(data_region_offset(0), None);
        assert_eq!(data_region_offset(u64::from(u16::MAX)), None);
        assert_eq!(data_region_offset(1 << 33), None);
    }
}
