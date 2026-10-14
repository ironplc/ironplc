//! Calls: what a call calls, and the arguments it passes.

use super::expr::Expr;
use super::ids::{PouId, Stamp};
use super::scalar::{ScalarType, Signedness};
use super::str_expr::StrExpr;
use super::types::Encoding;

/// What a call calls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Callee {
    /// A function of the program, whose parameters and result its POU
    /// declares.
    User(PouId),
    /// An operation the compiler implements itself.
    Intrinsic(Intrinsic),
}

/// One argument of a call, for one parameter of its callee
/// (REQ-LOW-lowering-071).
#[derive(Clone, Debug)]
pub enum Arg {
    /// A scalar value, passed by value.
    Value(Expr),
    /// A string, passed by value.
    Str(StrExpr),
}

impl Arg {
    /// What the argument passes: a value of its type, or a string of its
    /// encoding.
    pub fn param(&self) -> Param {
        match self {
            Arg::Value(value) => Param::Value(value.ty()),
            Arg::Str(value) => Param::Str(value.shape().encoding),
        }
    }

    /// The program whose places the argument reads.
    pub(crate) fn program(&self) -> Option<Stamp> {
        match self {
            Arg::Value(value) => value.program(),
            Arg::Str(value) => value.program(),
        }
    }
}

/// What one parameter of an intrinsic takes (REQ-LOW-lowering-072).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Param {
    /// A value at this operation type.
    Value(ScalarType),
    /// A string of this encoding, of any capacity.
    Str(Encoding),
}

/// An operation the compiler implements itself, at one set of operand types:
/// a call's arguments have exactly the types its variant names
/// (REQ-LOW-ir-145).
///
/// A shift or rotate takes a bit string at its operation type and a count of
/// the same width; `SHR` fills with zeros, and a count is taken modulo the
/// width (REQ-LOW-codegen-089). A rotate of a `BYTE` or a `WORD` keeps its bits
/// within that width, so it has a variant of its own.
///
/// A numeric function takes and returns values of one type. `ABS` and `EXPT`
/// take a signed integer or a real; the other numeric functions take a real.
/// The selection functions `MIN`, `MAX`, `LIMIT` and `SEL` take any integer
/// or real, and `SEL` also its selector, a `BOOL`. `MUX` takes a `DINT`
/// selector, then two or more inputs of its type: its last parameter repeats.
///
/// `BCD_TO_INT` decodes a bit string to a signed integer of its operation
/// width, and `INT_TO_BCD` encodes an integer as the bit string as wide as it.
/// Each has a variant for each width, which fixes its number of digits.
///
/// A string function takes strings of one encoding, so it has a variant for
/// each. `LEN` counts the code units of its string, and `FIND` gives the
/// 1-based position of the first code unit of its second string's first
/// occurrence in its first; both yield an `INT`.
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
    /// `ABS` of a 32-bit signed integer.
    AbsI32,
    /// `ABS` of a 64-bit signed integer.
    AbsI64,
    /// `ABS` of a `REAL`.
    AbsF32,
    /// `ABS` of an `LREAL`.
    AbsF64,
    /// `EXPT` of a 32-bit signed integer, by an exponent of the same type.
    ExptI32,
    /// `EXPT` of a 64-bit signed integer, by an exponent of the same type.
    ExptI64,
    /// `EXPT` of a `REAL`, by a `REAL` exponent.
    ExptF32,
    /// `EXPT` of an `LREAL`, by an `LREAL` exponent.
    ExptF64,
    /// `SQRT` of a `REAL`.
    SqrtF32,
    /// `SQRT` of an `LREAL`.
    SqrtF64,
    /// `LN` of a `REAL`.
    LnF32,
    /// `LN` of an `LREAL`.
    LnF64,
    /// `LOG` of a `REAL`.
    LogF32,
    /// `LOG` of an `LREAL`.
    LogF64,
    /// `EXP` of a `REAL`.
    ExpF32,
    /// `EXP` of an `LREAL`.
    ExpF64,
    /// `SIN` of a `REAL`.
    SinF32,
    /// `SIN` of an `LREAL`.
    SinF64,
    /// `COS` of a `REAL`.
    CosF32,
    /// `COS` of an `LREAL`.
    CosF64,
    /// `TAN` of a `REAL`.
    TanF32,
    /// `TAN` of an `LREAL`.
    TanF64,
    /// `ASIN` of a `REAL`.
    AsinF32,
    /// `ASIN` of an `LREAL`.
    AsinF64,
    /// `ACOS` of a `REAL`.
    AcosF32,
    /// `ACOS` of an `LREAL`.
    AcosF64,
    /// `ATAN` of a `REAL`.
    AtanF32,
    /// `ATAN` of an `LREAL`.
    AtanF64,
    /// `ATAN2` of two `REAL`s.
    Atan2F32,
    /// `ATAN2` of two `LREAL`s.
    Atan2F64,
    /// `__TRUNC` of a `REAL`: truncated toward zero, still a `REAL`.
    TruncF32,
    /// `__TRUNC` of an `LREAL`: truncated toward zero, still an `LREAL`.
    TruncF64,
    /// `__MOD` of two `REAL`s: the remainder with the sign of the dividend.
    ModF32,
    /// `__MOD` of two `LREAL`s: the remainder with the sign of the dividend.
    ModF64,
    /// `MIN` of two 32-bit signed integers.
    MinI32,
    /// `MIN` of two 32-bit unsigned integers.
    MinU32,
    /// `MIN` of two 64-bit signed integers.
    MinI64,
    /// `MIN` of two 64-bit unsigned integers.
    MinU64,
    /// `MIN` of two `REAL`s.
    MinF32,
    /// `MIN` of two `LREAL`s.
    MinF64,
    /// `MAX` of two 32-bit signed integers.
    MaxI32,
    /// `MAX` of two 32-bit unsigned integers.
    MaxU32,
    /// `MAX` of two 64-bit signed integers.
    MaxI64,
    /// `MAX` of two 64-bit unsigned integers.
    MaxU64,
    /// `MAX` of two `REAL`s.
    MaxF32,
    /// `MAX` of two `LREAL`s.
    MaxF64,
    /// `LIMIT` of three 32-bit signed integers: the middle one clamped to the first and the last.
    LimitI32,
    /// `LIMIT` of three 32-bit unsigned integers: the middle one clamped to the first and the last.
    LimitU32,
    /// `LIMIT` of three 64-bit signed integers: the middle one clamped to the first and the last.
    LimitI64,
    /// `LIMIT` of three 64-bit unsigned integers: the middle one clamped to the first and the last.
    LimitU64,
    /// `LIMIT` of three `REAL`s: the middle one clamped to the first and the last.
    LimitF32,
    /// `LIMIT` of three `LREAL`s: the middle one clamped to the first and the last.
    LimitF64,
    /// `SEL` between two 32-bit signed integers: the first when the selector is `FALSE`.
    SelI32,
    /// `SEL` between two 32-bit unsigned integers: the first when the selector is `FALSE`.
    SelU32,
    /// `SEL` between two 64-bit signed integers: the first when the selector is `FALSE`.
    SelI64,
    /// `SEL` between two 64-bit unsigned integers: the first when the selector is `FALSE`.
    SelU64,
    /// `SEL` between two `REAL`s: the first when the selector is `FALSE`.
    SelF32,
    /// `SEL` between two `LREAL`s: the first when the selector is `FALSE`.
    SelF64,
    /// `MUX` among two or more 32-bit signed integers.
    MuxI32,
    /// `MUX` among two or more 32-bit unsigned integers.
    MuxU32,
    /// `MUX` among two or more 64-bit signed integers.
    MuxI64,
    /// `MUX` among two or more 64-bit unsigned integers.
    MuxU64,
    /// `MUX` among two or more `REAL`s.
    MuxF32,
    /// `MUX` among two or more `LREAL`s.
    MuxF64,
    /// `BCD_TO_INT` of a `BYTE`: two digits.
    BcdToIntU8,
    /// `BCD_TO_INT` of a `WORD`: four digits.
    BcdToIntU16,
    /// `BCD_TO_INT` of a `DWORD`: eight digits.
    BcdToIntU32,
    /// `BCD_TO_INT` of an `LWORD`: sixteen digits.
    BcdToIntU64,
    /// `INT_TO_BCD` of a `SINT`, to a `BYTE`.
    IntToBcdI8,
    /// `INT_TO_BCD` of a `USINT`, to a `BYTE`.
    IntToBcdU8,
    /// `INT_TO_BCD` of an `INT`, to a `WORD`.
    IntToBcdI16,
    /// `INT_TO_BCD` of a `UINT`, to a `WORD`.
    IntToBcdU16,
    /// `INT_TO_BCD` of a `DINT`, to a `DWORD`.
    IntToBcdI32,
    /// `INT_TO_BCD` of a `UDINT`, to a `DWORD`.
    IntToBcdU32,
    /// `INT_TO_BCD` of an `LINT`, to an `LWORD`.
    IntToBcdI64,
    /// `INT_TO_BCD` of a `ULINT`, to an `LWORD`.
    IntToBcdU64,
    /// `LEN` of a `STRING`.
    LenString,
    /// `LEN` of a `WSTRING`.
    LenWString,
    /// `FIND` of a `STRING` in a `STRING`.
    FindString,
    /// `FIND` of a `WSTRING` in a `WSTRING`.
    FindWString,
}

const U32: ScalarType = ScalarType::I32(Signedness::Unsigned);
const U64: ScalarType = ScalarType::I64(Signedness::Unsigned);
const I32: ScalarType = ScalarType::I32(Signedness::Signed);
const I64: ScalarType = ScalarType::I64(Signedness::Signed);
const F32: ScalarType = ScalarType::F32;
const F64: ScalarType = ScalarType::F64;

/// The parameters intrinsics take.
mod param {
    use super::{Encoding, Param, ScalarType};

    pub(super) const U32: Param = Param::Value(super::U32);
    pub(super) const U64: Param = Param::Value(super::U64);
    pub(super) const I32: Param = Param::Value(super::I32);
    pub(super) const I64: Param = Param::Value(super::I64);
    pub(super) const F32: Param = Param::Value(super::F32);
    pub(super) const F64: Param = Param::Value(super::F64);
    pub(super) const BOOL: Param = Param::Value(ScalarType::Bool);
    pub(super) const STRING: Param = Param::Str(Encoding::Narrow);
    pub(super) const WSTRING: Param = Param::Str(Encoding::Wide);
}

/// The types of an intrinsic's parameters and of its result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Signature {
    params: &'static [Param],
    repeats_last: bool,
    result: ScalarType,
}

impl Signature {
    /// A call passes one argument of each of `params`, in order.
    const fn fixed(params: &'static [Param], result: ScalarType) -> Self {
        Signature {
            params,
            repeats_last: false,
            result,
        }
    }

    /// A call passes one argument of each of `params`, in order, and then
    /// any number of further arguments of the last one's type.
    const fn repeating(params: &'static [Param], result: ScalarType) -> Self {
        Signature {
            params,
            repeats_last: true,
            result,
        }
    }

    /// The type of the result.
    pub fn result(&self) -> ScalarType {
        self.result
    }

    /// What the parameter at `index` takes; `None` past the last parameter
    /// unless it repeats.
    pub fn param(&self, index: usize) -> Option<Param> {
        match self.params.get(index) {
            Some(param) => Some(*param),
            None if self.repeats_last => self.params.last().copied(),
            None => None,
        }
    }

    /// Whether a call whose arguments pass `args`, in order, passes what the
    /// parameters take.
    pub fn takes(&self, args: &[Param]) -> bool {
        if !self.repeats_last {
            return args == self.params;
        }
        let Some((last, first)) = self.params.split_last() else {
            return args.is_empty();
        };
        args.len() >= self.params.len()
            && args.starts_with(first)
            && args[first.len()..].iter().all(|arg| arg == last)
    }
}

impl std::fmt::Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.params)?;
        if self.repeats_last {
            write!(f, " and more of the last")?;
        }
        Ok(())
    }
}

impl Intrinsic {
    /// The types of the operation's parameters, in order, and of its result.
    pub fn signature(self) -> Signature {
        match self {
            Intrinsic::ShlU32
            | Intrinsic::ShrU32
            | Intrinsic::RolU8
            | Intrinsic::RolU16
            | Intrinsic::RolU32
            | Intrinsic::RorU8
            | Intrinsic::RorU16
            | Intrinsic::RorU32 => Signature::fixed(&[param::U32, param::I32], U32),
            Intrinsic::ShlU64 | Intrinsic::ShrU64 | Intrinsic::RolU64 | Intrinsic::RorU64 => {
                Signature::fixed(&[param::U64, param::I64], U64)
            }
            Intrinsic::AbsI32 => Signature::fixed(&[param::I32], I32),
            Intrinsic::AbsI64 => Signature::fixed(&[param::I64], I64),
            Intrinsic::ExptI32 => Signature::fixed(&[param::I32, param::I32], I32),
            Intrinsic::ExptI64 => Signature::fixed(&[param::I64, param::I64], I64),
            Intrinsic::AbsF32
            | Intrinsic::SqrtF32
            | Intrinsic::LnF32
            | Intrinsic::LogF32
            | Intrinsic::ExpF32
            | Intrinsic::SinF32
            | Intrinsic::CosF32
            | Intrinsic::TanF32
            | Intrinsic::AsinF32
            | Intrinsic::AcosF32
            | Intrinsic::AtanF32
            | Intrinsic::TruncF32 => Signature::fixed(&[param::F32], F32),
            Intrinsic::AbsF64
            | Intrinsic::SqrtF64
            | Intrinsic::LnF64
            | Intrinsic::LogF64
            | Intrinsic::ExpF64
            | Intrinsic::SinF64
            | Intrinsic::CosF64
            | Intrinsic::TanF64
            | Intrinsic::AsinF64
            | Intrinsic::AcosF64
            | Intrinsic::AtanF64
            | Intrinsic::TruncF64 => Signature::fixed(&[param::F64], F64),
            Intrinsic::ExptF32 | Intrinsic::Atan2F32 | Intrinsic::ModF32 => {
                Signature::fixed(&[param::F32, param::F32], F32)
            }
            Intrinsic::ExptF64 | Intrinsic::Atan2F64 | Intrinsic::ModF64 => {
                Signature::fixed(&[param::F64, param::F64], F64)
            }
            Intrinsic::MinI32 | Intrinsic::MaxI32 => {
                Signature::fixed(&[param::I32, param::I32], I32)
            }
            Intrinsic::MinU32 | Intrinsic::MaxU32 => {
                Signature::fixed(&[param::U32, param::U32], U32)
            }
            Intrinsic::MinI64 | Intrinsic::MaxI64 => {
                Signature::fixed(&[param::I64, param::I64], I64)
            }
            Intrinsic::MinU64 | Intrinsic::MaxU64 => {
                Signature::fixed(&[param::U64, param::U64], U64)
            }
            Intrinsic::MinF32 | Intrinsic::MaxF32 => {
                Signature::fixed(&[param::F32, param::F32], F32)
            }
            Intrinsic::MinF64 | Intrinsic::MaxF64 => {
                Signature::fixed(&[param::F64, param::F64], F64)
            }
            Intrinsic::LimitI32 => Signature::fixed(&[param::I32, param::I32, param::I32], I32),
            Intrinsic::LimitU32 => Signature::fixed(&[param::U32, param::U32, param::U32], U32),
            Intrinsic::LimitI64 => Signature::fixed(&[param::I64, param::I64, param::I64], I64),
            Intrinsic::LimitU64 => Signature::fixed(&[param::U64, param::U64, param::U64], U64),
            Intrinsic::LimitF32 => Signature::fixed(&[param::F32, param::F32, param::F32], F32),
            Intrinsic::LimitF64 => Signature::fixed(&[param::F64, param::F64, param::F64], F64),
            Intrinsic::SelI32 => Signature::fixed(&[param::BOOL, param::I32, param::I32], I32),
            Intrinsic::SelU32 => Signature::fixed(&[param::BOOL, param::U32, param::U32], U32),
            Intrinsic::SelI64 => Signature::fixed(&[param::BOOL, param::I64, param::I64], I64),
            Intrinsic::SelU64 => Signature::fixed(&[param::BOOL, param::U64, param::U64], U64),
            Intrinsic::SelF32 => Signature::fixed(&[param::BOOL, param::F32, param::F32], F32),
            Intrinsic::SelF64 => Signature::fixed(&[param::BOOL, param::F64, param::F64], F64),
            Intrinsic::MuxI32 => Signature::repeating(&[param::I32, param::I32, param::I32], I32),
            Intrinsic::MuxU32 => Signature::repeating(&[param::I32, param::U32, param::U32], U32),
            Intrinsic::MuxI64 => Signature::repeating(&[param::I32, param::I64, param::I64], I64),
            Intrinsic::MuxU64 => Signature::repeating(&[param::I32, param::U64, param::U64], U64),
            Intrinsic::MuxF32 => Signature::repeating(&[param::I32, param::F32, param::F32], F32),
            Intrinsic::MuxF64 => Signature::repeating(&[param::I32, param::F64, param::F64], F64),
            Intrinsic::BcdToIntU8 | Intrinsic::BcdToIntU16 | Intrinsic::BcdToIntU32 => {
                Signature::fixed(&[param::U32], I32)
            }
            Intrinsic::BcdToIntU64 => Signature::fixed(&[param::U64], I64),
            Intrinsic::IntToBcdI8 | Intrinsic::IntToBcdI16 | Intrinsic::IntToBcdI32 => {
                Signature::fixed(&[param::I32], U32)
            }
            Intrinsic::IntToBcdU8 | Intrinsic::IntToBcdU16 | Intrinsic::IntToBcdU32 => {
                Signature::fixed(&[param::U32], U32)
            }
            Intrinsic::IntToBcdI64 => Signature::fixed(&[param::I64], U64),
            Intrinsic::IntToBcdU64 => Signature::fixed(&[param::U64], U64),
            Intrinsic::LenString => Signature::fixed(&[param::STRING], I32),
            Intrinsic::LenWString => Signature::fixed(&[param::WSTRING], I32),
            Intrinsic::FindString => Signature::fixed(&[param::STRING, param::STRING], I32),
            Intrinsic::FindWString => Signature::fixed(&[param::WSTRING, param::WSTRING], I32),
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
            Intrinsic::AbsI32 => "abs_i32",
            Intrinsic::AbsI64 => "abs_i64",
            Intrinsic::AbsF32 => "abs_f32",
            Intrinsic::AbsF64 => "abs_f64",
            Intrinsic::ExptI32 => "expt_i32",
            Intrinsic::ExptI64 => "expt_i64",
            Intrinsic::ExptF32 => "expt_f32",
            Intrinsic::ExptF64 => "expt_f64",
            Intrinsic::SqrtF32 => "sqrt_f32",
            Intrinsic::SqrtF64 => "sqrt_f64",
            Intrinsic::LnF32 => "ln_f32",
            Intrinsic::LnF64 => "ln_f64",
            Intrinsic::LogF32 => "log_f32",
            Intrinsic::LogF64 => "log_f64",
            Intrinsic::ExpF32 => "exp_f32",
            Intrinsic::ExpF64 => "exp_f64",
            Intrinsic::SinF32 => "sin_f32",
            Intrinsic::SinF64 => "sin_f64",
            Intrinsic::CosF32 => "cos_f32",
            Intrinsic::CosF64 => "cos_f64",
            Intrinsic::TanF32 => "tan_f32",
            Intrinsic::TanF64 => "tan_f64",
            Intrinsic::AsinF32 => "asin_f32",
            Intrinsic::AsinF64 => "asin_f64",
            Intrinsic::AcosF32 => "acos_f32",
            Intrinsic::AcosF64 => "acos_f64",
            Intrinsic::AtanF32 => "atan_f32",
            Intrinsic::AtanF64 => "atan_f64",
            Intrinsic::Atan2F32 => "atan2_f32",
            Intrinsic::Atan2F64 => "atan2_f64",
            Intrinsic::TruncF32 => "trunc_f32",
            Intrinsic::TruncF64 => "trunc_f64",
            Intrinsic::ModF32 => "mod_f32",
            Intrinsic::ModF64 => "mod_f64",
            Intrinsic::MinI32 => "min_i32",
            Intrinsic::MinU32 => "min_u32",
            Intrinsic::MinI64 => "min_i64",
            Intrinsic::MinU64 => "min_u64",
            Intrinsic::MinF32 => "min_f32",
            Intrinsic::MinF64 => "min_f64",
            Intrinsic::MaxI32 => "max_i32",
            Intrinsic::MaxU32 => "max_u32",
            Intrinsic::MaxI64 => "max_i64",
            Intrinsic::MaxU64 => "max_u64",
            Intrinsic::MaxF32 => "max_f32",
            Intrinsic::MaxF64 => "max_f64",
            Intrinsic::LimitI32 => "limit_i32",
            Intrinsic::LimitU32 => "limit_u32",
            Intrinsic::LimitI64 => "limit_i64",
            Intrinsic::LimitU64 => "limit_u64",
            Intrinsic::LimitF32 => "limit_f32",
            Intrinsic::LimitF64 => "limit_f64",
            Intrinsic::SelI32 => "sel_i32",
            Intrinsic::SelU32 => "sel_u32",
            Intrinsic::SelI64 => "sel_i64",
            Intrinsic::SelU64 => "sel_u64",
            Intrinsic::SelF32 => "sel_f32",
            Intrinsic::SelF64 => "sel_f64",
            Intrinsic::MuxI32 => "mux_i32",
            Intrinsic::MuxU32 => "mux_u32",
            Intrinsic::MuxI64 => "mux_i64",
            Intrinsic::MuxU64 => "mux_u64",
            Intrinsic::MuxF32 => "mux_f32",
            Intrinsic::MuxF64 => "mux_f64",
            Intrinsic::BcdToIntU8 => "bcd_to_int_u8",
            Intrinsic::BcdToIntU16 => "bcd_to_int_u16",
            Intrinsic::BcdToIntU32 => "bcd_to_int_u32",
            Intrinsic::BcdToIntU64 => "bcd_to_int_u64",
            Intrinsic::IntToBcdI8 => "int_to_bcd_i8",
            Intrinsic::IntToBcdU8 => "int_to_bcd_u8",
            Intrinsic::IntToBcdI16 => "int_to_bcd_i16",
            Intrinsic::IntToBcdU16 => "int_to_bcd_u16",
            Intrinsic::IntToBcdI32 => "int_to_bcd_i32",
            Intrinsic::IntToBcdU32 => "int_to_bcd_u32",
            Intrinsic::IntToBcdI64 => "int_to_bcd_i64",
            Intrinsic::IntToBcdU64 => "int_to_bcd_u64",
            Intrinsic::LenString => "len_string",
            Intrinsic::LenWString => "len_wstring",
            Intrinsic::FindString => "find_string",
            Intrinsic::FindWString => "find_wstring",
        }
    }
}
