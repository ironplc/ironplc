//! Newtype wrappers for semantically distinct identifiers and indices
//! used throughout the bytecode container format.
//!
//! These types prevent accidentally mixing up values that share the same
//! underlying representation (e.g., passing a `TaskId` where a `FunctionId`
//! is expected).

/// A function identifier within a bytecode container.
///
/// Function IDs are compiler-assigned sequential indices starting from 0.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FunctionId(u16);

impl FunctionId {
    /// The init function (ID 0).
    pub const INIT: FunctionId = FunctionId(0);
    /// The scan function (ID 1).
    pub const SCAN: FunctionId = FunctionId(1);
    /// The first user-defined function (ID 2).
    pub const FIRST_USER: FunctionId = FunctionId(2);
    /// Indicates global/program scope (not a specific function).
    pub const GLOBAL_SCOPE: FunctionId = FunctionId(0xFFFF);

    /// Creates a new `FunctionId` from a raw `u16`.
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }
    /// Returns the raw `u16` value.
    pub const fn raw(self) -> u16 {
        self.0
    }
    /// Returns the little-endian byte representation.
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
}

impl core::fmt::Display for FunctionId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A task identifier within a bytecode container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TaskId(u16);

impl TaskId {
    /// The default task (ID 0).
    pub const DEFAULT: TaskId = TaskId(0);

    /// Creates a new `TaskId` from a raw `u16`.
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }
    /// Returns the raw `u16` value.
    pub const fn raw(self) -> u16 {
        self.0
    }
    /// Returns the little-endian byte representation.
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
}

impl core::fmt::Display for TaskId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A program instance identifier within a bytecode container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct InstanceId(u16);

impl InstanceId {
    /// The default instance (ID 0).
    pub const DEFAULT: InstanceId = InstanceId(0);

    /// Creates a new `InstanceId` from a raw `u16`.
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }
    /// Returns the raw `u16` value.
    pub const fn raw(self) -> u16 {
        self.0
    }
    /// Returns the little-endian byte representation.
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
}

impl core::fmt::Display for InstanceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A function block type identifier within a bytecode container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct FbTypeId(u16);

impl FbTypeId {
    /// Creates a new `FbTypeId` from a raw `u16`.
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }
    /// Returns the raw `u16` value.
    pub const fn raw(self) -> u16 {
        self.0
    }
    /// Returns the little-endian byte representation.
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
}

impl core::fmt::Display for FbTypeId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A variable table index within a bytecode container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VarIndex(u16);

impl VarIndex {
    /// Sentinel value indicating no SINGLE trigger variable.
    pub const NO_SINGLE_VAR: VarIndex = VarIndex(0xFFFF);

    /// Creates a new `VarIndex` from a raw `u16`.
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }
    /// Returns the raw `u16` value.
    pub const fn raw(self) -> u16 {
        self.0
    }
    /// Returns the little-endian byte representation.
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
}

impl core::fmt::Display for VarIndex {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::ops::Add<u16> for VarIndex {
    type Output = VarIndex;
    fn add(self, rhs: u16) -> VarIndex {
        VarIndex(self.0 + rhs)
    }
}

impl core::ops::AddAssign<u16> for VarIndex {
    fn add_assign(&mut self, rhs: u16) {
        self.0 += rhs;
    }
}

impl core::ops::Sub for VarIndex {
    type Output = u16;
    fn sub(self, rhs: VarIndex) -> u16 {
        self.0 - rhs.0
    }
}

impl From<VarIndex> for i64 {
    fn from(v: VarIndex) -> i64 {
        v.0 as i64
    }
}

/// A constant pool index within a bytecode container.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ConstantIndex(u16);

impl ConstantIndex {
    /// Creates a new `ConstantIndex` from a raw `u16`.
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }
    /// Returns the raw `u16` value.
    pub const fn raw(self) -> u16 {
        self.0
    }
    /// Returns the little-endian byte representation.
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
}

impl core::fmt::Display for ConstantIndex {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An index into the debug section's `SOURCE_FILE_TABLE` (tag 6).
///
/// `LineMapEntry.file_id` is a `SourceFileId`; the entry at that index
/// in `DebugSection.source_files` carries the path and BLAKE3 content
/// hash. Containers without a source file table use the default
/// (`SourceFileId(0)`), which is also the first valid index — readers
/// distinguish "no table" from "table with one entry" by checking
/// `DebugSection.source_files.is_empty()`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SourceFileId(u16);

impl SourceFileId {
    /// Creates a new `SourceFileId` from a raw `u16`.
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }
    /// Returns the raw `u16` value.
    pub const fn raw(self) -> u16 {
        self.0
    }
    /// Returns the little-endian byte representation.
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
}

impl core::fmt::Display for SourceFileId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A 1-based source line number.
///
/// `0` is reserved and indicates "unknown line" — a debugger should
/// not jump to a `SourceLine(0)` entry. The newtype prevents
/// accidentally swapping line and column arguments at call sites like
/// `Emitter::set_source_position`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SourceLine(u16);

impl SourceLine {
    /// Creates a new `SourceLine` from a raw `u16`. `0` means "unknown".
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }
    /// Returns the raw `u16` value.
    pub const fn raw(self) -> u16 {
        self.0
    }
    /// Returns the little-endian byte representation.
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
}

impl core::fmt::Display for SourceLine {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A 1-based source column number.
///
/// `0` is reserved and indicates "unknown column" — column-level
/// precision is optional, line-level precision is not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SourceColumn(u16);

impl SourceColumn {
    /// Creates a new `SourceColumn` from a raw `u16`. `0` means "unknown".
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }
    /// Returns the raw `u16` value.
    pub const fn raw(self) -> u16 {
        self.0
    }
    /// Returns the little-endian byte representation.
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
}

impl core::fmt::Display for SourceColumn {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A slot offset within a structure in the bytecode container.
///
/// Slot offsets identify positions within flattened structure layouts,
/// where each slot is 8 bytes. Wraps `u32` (unlike the `u16`-based
/// identifiers) because structures can contain many slots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SlotIndex(u32);

impl SlotIndex {
    /// Creates a new `SlotIndex` from a raw `u32`.
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }
    /// Returns the raw `u32` value.
    pub const fn raw(self) -> u32 {
        self.0
    }
    /// Returns the little-endian byte representation.
    pub const fn to_le_bytes(self) -> [u8; 4] {
        self.0.to_le_bytes()
    }
}

impl core::fmt::Display for SlotIndex {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl core::ops::Add<u32> for SlotIndex {
    type Output = SlotIndex;
    fn add(self, rhs: u32) -> SlotIndex {
        SlotIndex(self.0 + rhs)
    }
}

impl core::ops::Add for SlotIndex {
    type Output = SlotIndex;
    fn add(self, rhs: SlotIndex) -> SlotIndex {
        SlotIndex(self.0 + rhs.0)
    }
}

impl core::ops::AddAssign<u32> for SlotIndex {
    fn add_assign(&mut self, rhs: u32) {
        self.0 += rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::ToString;

    #[test]
    fn fb_type_id_display_when_formatted_then_writes_number() {
        assert_eq!(FbTypeId::new(123).to_string(), "123");
    }

    #[test]
    fn var_index_display_when_formatted_then_writes_number() {
        assert_eq!(VarIndex::new(456).to_string(), "456");
    }

    #[test]
    fn var_index_when_add_u16_then_sum() {
        assert_eq!(VarIndex::new(10) + 5, VarIndex::new(15));
    }

    #[test]
    fn var_index_when_sub_var_index_then_delta() {
        assert_eq!(VarIndex::new(20) - VarIndex::new(5), 15);
    }

    #[test]
    fn var_index_when_add_assign_u16_then_mutated() {
        let mut v = VarIndex::new(10);
        v += 5u16;
        assert_eq!(v, VarIndex::new(15));
    }

    #[test]
    fn constant_index_to_le_bytes_when_called_then_little_endian_bytes() {
        assert_eq!(ConstantIndex::new(0x1234).to_le_bytes(), [0x34, 0x12]);
    }

    #[test]
    fn var_index_into_i64_when_cast_then_value() {
        let v: i64 = VarIndex::new(999).into();
        assert_eq!(v, 999i64);
    }

    #[test]
    fn constant_index_display_when_formatted_then_writes_number() {
        assert_eq!(ConstantIndex::new(777).to_string(), "777");
    }

    #[test]
    fn slot_index_display_when_formatted_then_writes_number() {
        assert_eq!(SlotIndex::new(1234).to_string(), "1234");
    }

    #[test]
    fn slot_index_to_le_bytes_when_called_then_little_endian_bytes() {
        assert_eq!(
            SlotIndex::new(0x12345678).to_le_bytes(),
            [0x78, 0x56, 0x34, 0x12]
        );
    }

    #[test]
    fn slot_index_when_add_u32_then_sum() {
        assert_eq!(SlotIndex::new(100) + 50u32, SlotIndex::new(150));
    }

    #[test]
    fn slot_index_when_add_slot_index_then_sum() {
        assert_eq!(
            SlotIndex::new(100) + SlotIndex::new(50),
            SlotIndex::new(150)
        );
    }

    #[test]
    fn slot_index_when_add_assign_u32_then_mutated() {
        let mut s = SlotIndex::new(100);
        s += 50u32;
        assert_eq!(s, SlotIndex::new(150));
    }
}
