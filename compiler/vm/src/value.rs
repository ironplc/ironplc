use ironplc_container::VarIndex;

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

    /// Creates a slot from a 64-bit signed integer.
    pub fn from_i64(v: i64) -> Self {
        Slot(v as u64)
    }

    /// Extracts a 64-bit signed integer from this slot.
    pub fn as_i64(self) -> i64 {
        self.0 as i64
    }

    /// Creates a slot from a 32-bit float (stored as bit pattern).
    pub fn from_f32(v: f32) -> Self {
        Slot(v.to_bits() as u64)
    }

    /// Extracts a 32-bit float from this slot.
    pub fn as_f32(self) -> f32 {
        f32::from_bits(self.0 as u32)
    }

    /// Creates a slot from a 64-bit float (stored as bit pattern).
    pub fn from_f64(v: f64) -> Self {
        Slot(v.to_bits())
    }

    /// Extracts a 64-bit float from this slot.
    pub fn as_f64(self) -> f64 {
        f64::from_bits(self.0)
    }

    /// Creates a slot from a raw 64-bit value.
    pub fn from_u64(v: u64) -> Self {
        Slot(v)
    }

    /// Returns the raw 64-bit representation of this slot.
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// Creates a null reference sentinel (u64::MAX).
    pub fn null_ref() -> Self {
        Slot(u64::MAX)
    }

    /// Returns true if this slot holds the null reference sentinel.
    pub fn is_null_ref(self) -> bool {
        self.0 == u64::MAX
    }

    /// Extracts a variable table index from this slot.
    /// Returns `None` if the value exceeds u16::MAX (invalid index).
    pub fn as_var_index(self) -> Option<VarIndex> {
        u16::try_from(self.0).ok().map(VarIndex::new)
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

    #[test]
    fn slot_from_i64_when_negative_then_roundtrips() {
        let slot = Slot::from_i64(-1);
        assert_eq!(slot.as_i64(), -1);
        assert_eq!(slot.0, 0xFFFFFFFFFFFFFFFF);
    }

    #[test]
    fn slot_from_i64_when_large_positive_then_roundtrips() {
        let slot = Slot::from_i64(i64::MAX);
        assert_eq!(slot.as_i64(), i64::MAX);
    }

    #[test]
    fn slot_from_f32_when_positive_then_roundtrips() {
        let slot = Slot::from_f32(std::f32::consts::PI);
        assert_eq!(slot.as_f32(), std::f32::consts::PI);
    }

    #[test]
    fn slot_from_f32_when_negative_then_roundtrips() {
        let slot = Slot::from_f32(-2.5);
        assert_eq!(slot.as_f32(), -2.5_f32);
    }

    #[test]
    fn slot_from_f64_when_positive_then_roundtrips() {
        let slot = Slot::from_f64(std::f64::consts::PI);
        assert_eq!(slot.as_f64(), std::f64::consts::PI);
    }

    #[test]
    fn slot_from_f64_when_negative_then_roundtrips() {
        let slot = Slot::from_f64(-1.23e10);
        assert_eq!(slot.as_f64(), -1.23e10_f64);
    }

    #[test]
    fn slot_from_u64_when_i32_roundtrip_then_matches() {
        let original = Slot::from_i32(-42);
        let raw = original.as_u64();
        let restored = Slot::from_u64(raw);
        assert_eq!(restored.as_i32(), -42);
    }

    #[test]
    fn slot_from_u64_when_f32_roundtrip_then_matches() {
        let original = Slot::from_f32(3.14);
        let raw = original.as_u64();
        let restored = Slot::from_u64(raw);
        assert_eq!(restored.as_f32(), 3.14_f32);
    }

    #[test]
    fn slot_from_u64_when_zero_then_default() {
        assert_eq!(Slot::from_u64(0), Slot::default());
    }

    #[test]
    fn slot_null_ref_when_created_then_is_null() {
        assert!(Slot::null_ref().is_null_ref());
    }

    #[test]
    fn slot_is_null_ref_when_not_null_then_false() {
        assert!(!Slot::from_i64(0).is_null_ref());
        assert!(!Slot::from_i64(42).is_null_ref());
    }

    #[test]
    fn slot_as_var_index_when_valid_then_some() {
        let slot = Slot::from_i64(5);
        assert_eq!(slot.as_var_index(), Some(VarIndex::new(5)));
    }

    #[test]
    fn slot_as_var_index_when_max_u16_then_some() {
        let slot = Slot::from_u64(u16::MAX as u64);
        assert_eq!(slot.as_var_index(), Some(VarIndex::new(u16::MAX)));
    }

    #[test]
    fn slot_as_var_index_when_null_ref_then_none() {
        assert_eq!(Slot::null_ref().as_var_index(), None);
    }

    #[test]
    fn slot_as_var_index_when_too_large_then_none() {
        let slot = Slot::from_u64(u16::MAX as u64 + 1);
        assert_eq!(slot.as_var_index(), None);
    }
}
