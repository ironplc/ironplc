/// A 64-bit slot that holds any VM value.
///
/// I32 values are sign-extended into the slot so that negative values
/// roundtrip correctly: `v as i64 as u64`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Slot(u64);

impl Slot {
    /// Creates a slot from a 32-bit signed integer (sign-extended).
    pub fn from_i32(v: i32) -> Self {
        Slot(v as i64 as u64)
    }

    /// Extracts a 32-bit signed integer from this slot.
    pub fn as_i32(self) -> i32 {
        self.0 as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_from_i32_when_negative_then_roundtrips() {
        let slot = Slot::from_i32(-1);
        assert_eq!(slot.as_i32(), -1);
        // Sign extension: -1 should be 0xFFFFFFFFFFFFFFFF
        assert_eq!(slot.0, 0xFFFFFFFFFFFFFFFF);
    }

    #[test]
    fn slot_from_i32_when_positive_then_roundtrips() {
        let slot = Slot::from_i32(42);
        assert_eq!(slot.as_i32(), 42);
        assert_eq!(slot.0, 42);
    }
}
