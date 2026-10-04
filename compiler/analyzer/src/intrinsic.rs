//! The operation each standard function stands for.
//!
//! The analyzer owns the table of standard function signatures, so it is the
//! one place that knows what a name such as `ABS` or `INT_TO_REAL` means.
//! Each standard signature names its [`Intrinsic`], and code generation
//! dispatches on that rather than on the function's spelling: a match over
//! [`Intrinsic`] is exhaustive, so a standard function added here fails to
//! compile until code generation handles it.
//!
//! An intrinsic names an operation, not an operand type. `ABS` is one
//! [`NumericFunction::Abs`] whatever it is applied to; code generation picks
//! the width and signedness from the operands, as it does for an operator.
//! What the name itself fixes is part of the variant: the source and target
//! of a conversion, and whether a typed time function is the long form.

use ironplc_dsl::common::ElementaryTypeName;

use crate::intermediates::operator_function_form::FormOf;

/// The operation a standard function stands for: one per operation the
/// compiler implements itself (ADR-0042), which are the IEC 61131-3 standard
/// functions, the `__`-prefixed compiler intrinsics and the `SIZEOF`
/// extension.
#[derive(Debug, Clone, PartialEq)]
pub enum Intrinsic {
    /// The function form of an operator: `ADD`, `GT`, `AND`, `NOT`, ...
    Operator(FormOf),
    /// A numeric function the VM computes in one builtin: `ABS`, `EXPT`,
    /// `SIN`, `MIN`, `SEL`, ...
    Numeric(NumericFunction),
    /// A bit shift or rotate: `SHL`, `SHR`, `ROL`, `ROR`.
    BitShift(BitShift),
    /// `MUX`: selects one of its inputs by an integer.
    Mux,
    /// `MOVE`: the assignment function.
    Move,
    /// `TRUNC`: a real truncated toward zero to an integer.
    Trunc,
    /// `BCD_TO_INT`: a BCD-encoded bit string decoded to an integer.
    BcdToInt,
    /// `INT_TO_BCD`: an integer encoded as a BCD bit string.
    IntToBcd,
    /// `SIZEOF`: the size in bytes of its argument (extension).
    Sizeof,
    /// A string function: `LEN`, `CONCAT`, `LEFT`, ...
    String(StringFunction),
    /// A type conversion `<SOURCE>_TO_<TARGET>`, including to and from
    /// `STRING`.
    Conversion {
        source: ElementaryTypeName,
        target: ElementaryTypeName,
    },
    /// A two-operand time or date function: a typed overload of `ADD`,
    /// `SUB`, `MUL` or `DIV` (IEC 61131-3 Table 30), or `CONCAT_DATE_TOD`.
    /// `long` is true for the form over the long-width types (`ADD_LTIME`).
    Time { function: TimeFunction, long: bool },
    /// `DT_TO_DATE`, also spelled `DATE_AND_TIME_TO_DATE`: the date of a
    /// date and time.
    DtToDate,
    /// `DT_TO_TOD`, also spelled `DATE_AND_TIME_TO_TIME_OF_DAY`: the time of
    /// day of a date and time.
    DtToTod,
}

/// A numeric function the VM computes in one builtin, chosen by the width of
/// the operation and, for `MIN`, `MAX` and `LIMIT`, its signedness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericFunction {
    Abs,
    Sqrt,
    Ln,
    Log,
    Exp,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Expt,
    Min,
    Max,
    Limit,
    Sel,
    /// `__TRUNC`: truncation toward zero that stays in the input's real type.
    TruncReal,
    /// `__MOD`: the IEEE-754 floating remainder.
    ModReal,
}

/// A bit shift or rotate function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitShift {
    Shl,
    Shr,
    Rol,
    Ror,
}

/// A standard string function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringFunction {
    Len,
    Find,
    Replace,
    Insert,
    Delete,
    Left,
    Right,
    Mid,
    Concat,
}

/// A two-operand time or date function, named by its short form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeFunction {
    /// `ADD_TIME`: a duration plus a duration.
    AddTime,
    /// `SUB_TIME`: a duration minus a duration.
    SubTime,
    /// `MUL_TIME`: a duration times a number.
    MulTime,
    /// `DIV_TIME`: a duration divided by a number.
    DivTime,
    /// `ADD_TOD_TIME`: a time of day plus a duration.
    AddTodTime,
    /// `SUB_TOD_TIME`: a time of day minus a duration.
    SubTodTime,
    /// `ADD_DT_TIME`: a date and time plus a duration.
    AddDtTime,
    /// `SUB_DT_TIME`: a date and time minus a duration.
    SubDtTime,
    /// `SUB_DT_DT`: the duration between two dates and times.
    SubDtDt,
    /// `SUB_DATE_DATE`: the duration between two dates.
    SubDateDate,
    /// `SUB_TOD_TOD`: the duration between two times of day.
    SubTodTod,
    /// `CONCAT_DATE_TOD`: a date and a time of day joined into a date and
    /// time.
    ConcatDateTod,
}
