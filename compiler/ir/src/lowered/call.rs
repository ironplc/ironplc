//! Calls: what a call calls, and the arguments it passes.

use super::expr::Expr;
use super::scalar::{ScalarType, Signedness};

/// What a call calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Callee {
    /// An operation the compiler implements itself.
    Intrinsic(Intrinsic),
}

/// One argument of a call, for one parameter of its callee
/// (REQ-LOW-lowering-071).
#[derive(Clone, Debug)]
pub enum Arg {
    /// A scalar value, passed by value.
    Value(Expr),
}

/// An operation the compiler implements itself, at one set of operand types:
/// a call's arguments have exactly the types its variant names
/// (REQ-LOW-ir-145).
///
/// A shift or rotate takes a bit string at its operation type and a count of
/// the same width; `SHR` fills with zeros, and a count is taken modulo the
/// width (REQ-LOW-codegen-089). A rotate of a `BYTE` or a `WORD` keeps its bits
/// within that width, so it has a variant of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intrinsic {
    /// `SHL` of a 32-bit bit string: a `BYTE`, `WORD` or `DWORD`.
    ShlU32,
    /// `SHL` of an `LWORD`.
    ShlU64,
    /// `SHR` of a 32-bit bit string: a `BYTE`, `WORD` or `DWORD`.
    ShrU32,
    /// `SHR` of an `LWORD`.
    ShrU64,
    /// `ROL` of a `BYTE`.
    RolU8,
    /// `ROL` of a `WORD`.
    RolU16,
    /// `ROL` of a `DWORD`.
    RolU32,
    /// `ROL` of an `LWORD`.
    RolU64,
    /// `ROR` of a `BYTE`.
    RorU8,
    /// `ROR` of a `WORD`.
    RorU16,
    /// `ROR` of a `DWORD`.
    RorU32,
    /// `ROR` of an `LWORD`.
    RorU64,
}

const U32: ScalarType = ScalarType::I32(Signedness::Unsigned);
const U64: ScalarType = ScalarType::I64(Signedness::Unsigned);
const I32: ScalarType = ScalarType::I32(Signedness::Signed);
const I64: ScalarType = ScalarType::I64(Signedness::Signed);

impl Intrinsic {
    /// The types of the operation's parameters, in order, and of its result.
    pub fn signature(self) -> (&'static [ScalarType], ScalarType) {
        match self {
            Intrinsic::ShlU32
            | Intrinsic::ShrU32
            | Intrinsic::RolU8
            | Intrinsic::RolU16
            | Intrinsic::RolU32
            | Intrinsic::RorU8
            | Intrinsic::RorU16
            | Intrinsic::RorU32 => (&[U32, I32], U32),
            Intrinsic::ShlU64 | Intrinsic::ShrU64 | Intrinsic::RolU64 | Intrinsic::RorU64 => {
                (&[U64, I64], U64)
            }
        }
    }

    /// The name the text form prints the operation by.
    pub fn name(self) -> &'static str {
        match self {
            Intrinsic::ShlU32 => "shl_u32",
            Intrinsic::ShlU64 => "shl_u64",
            Intrinsic::ShrU32 => "shr_u32",
            Intrinsic::ShrU64 => "shr_u64",
            Intrinsic::RolU8 => "rol_u8",
            Intrinsic::RolU16 => "rol_u16",
            Intrinsic::RolU32 => "rol_u32",
            Intrinsic::RolU64 => "rol_u64",
            Intrinsic::RorU8 => "ror_u8",
            Intrinsic::RorU16 => "ror_u16",
            Intrinsic::RorU32 => "ror_u32",
            Intrinsic::RorU64 => "ror_u64",
        }
    }
}
