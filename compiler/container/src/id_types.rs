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
