//! The operation each standard function stands for.
//!
//! The analyzer owns the table of standard function signatures, so it is the
//! one place that knows what a name such as `ABS` or `INT_TO_REAL` means.
//! Each standard signature names its [`BuiltinFunction`], and code generation
//! dispatches on that rather than on the function's spelling: a match over
//! [`BuiltinFunction`] is exhaustive, so a standard function added here fails to
//! compile until code generation handles it.
//!
//! A built-in function names an operation, not an operand type. `ABS` is one
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
pub enum BuiltinFunction {
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

impl BuiltinFunction {
    /// Returns `true` for an operation that computes at its own type,
    /// whatever the type of its context, and whose result is converted to the
    /// context's type, as an arithmetic operation's is:
    ///
    /// * an operation on one value whose result has that value's type: `NOT`,
    ///   a numeric function of one input (`ABS`, `SQRT`, ...), `MOVE`, and a
    ///   shift or rotate, whose count only says how far. `SHL` of a `DWORD`
    ///   assigned to an `LWORD` shifts 32 bits;
    /// * a function of several inputs of one type
    ///   ([`BuiltinFunction::inputs_of_one_type`]), whose result the analyzer types
    ///   by those inputs and converts them to. `MAX` of two `UDINT`s compares
    ///   them unsigned wherever it is used, and `AND` of two `DWORD`s
    ///   assigned to an `LWORD` is widened as a `DWORD`.
    /// * `INT_TO_BCD`, whose result is the bit string as wide as its input.
    pub fn computes_at_own_type(&self) -> bool {
        match self {
            BuiltinFunction::Operator(FormOf::Not)
            | BuiltinFunction::Move
            | BuiltinFunction::BitShift(_)
            | BuiltinFunction::Mux
            | BuiltinFunction::IntToBcd => true,
            BuiltinFunction::Numeric(function) => {
                function.has_one_input() || function.inputs_of_one_type().is_some()
            }
            BuiltinFunction::Operator(FormOf::Compare(op)) => op.is_bitwise(),
            BuiltinFunction::Operator(FormOf::Arithmetic(_))
            | BuiltinFunction::Trunc
            | BuiltinFunction::BcdToInt
            | BuiltinFunction::Sizeof
            | BuiltinFunction::String(_)
            | BuiltinFunction::Conversion { .. }
            | BuiltinFunction::Time { .. }
            | BuiltinFunction::DtToDate
            | BuiltinFunction::DtToTod => false,
        }
    }

    /// Returns `true` for an operation whose result has a type its signature
    /// fixes, whatever its inputs and its context: a type conversion
    /// (`INT_TO_REAL` is a `REAL`), a typed time or date function
    /// (`SUB_DT_TIME` is a `DATE_AND_TIME`), `DT_TO_DATE` and `DT_TO_TOD`, and
    /// a string function (`LEN` is an `INT`). Its result is converted to the
    /// type of its context, as a user-defined function's is: `lr :=
    /// INT_TO_REAL(i)` on an `LREAL` widens the `REAL`.
    pub fn result_has_fixed_type(&self) -> bool {
        match self {
            BuiltinFunction::String(_)
            | BuiltinFunction::Conversion { .. }
            | BuiltinFunction::Time { .. }
            | BuiltinFunction::DtToDate
            | BuiltinFunction::DtToTod => true,
            BuiltinFunction::Operator(_)
            | BuiltinFunction::Numeric(_)
            | BuiltinFunction::BitShift(_)
            | BuiltinFunction::Mux
            | BuiltinFunction::Move
            | BuiltinFunction::Trunc
            | BuiltinFunction::BcdToInt
            | BuiltinFunction::IntToBcd
            | BuiltinFunction::Sizeof => false,
        }
    }

    /// Returns `true` for an operation whose result is an integer of the type
    /// its context stores it at, as an untyped integer literal is (ADR-0028):
    /// `TRUNC`, `BCD_TO_INT` and `SIZEOF`, whose signatures return `ANY_INT`
    /// and whose inputs say nothing of which integer. `TRUNC` of a `REAL`
    /// assigned to an `LINT` truncates to 64 bits, and one assigned to a
    /// `REAL` truncates to a `DINT` that is converted to the `REAL`.
    pub fn result_is_integer_of_context(&self) -> bool {
        match self {
            BuiltinFunction::Trunc | BuiltinFunction::BcdToInt | BuiltinFunction::Sizeof => true,
            BuiltinFunction::Operator(_)
            | BuiltinFunction::Numeric(_)
            | BuiltinFunction::BitShift(_)
            | BuiltinFunction::Mux
            | BuiltinFunction::Move
            | BuiltinFunction::IntToBcd
            | BuiltinFunction::String(_)
            | BuiltinFunction::Conversion { .. }
            | BuiltinFunction::Time { .. }
            | BuiltinFunction::DtToDate
            | BuiltinFunction::DtToTod => false,
        }
    }

    /// For a function of several inputs of one type -- `MIN`, `MAX`,
    /// `LIMIT`, `SEL`, `MUX`, `EXPT`, `ATAN2`, and the function forms of
    /// `AND`, `OR` and `XOR` -- which of its inputs have that type and which
    /// type of theirs its result has; `None` for any other operation.
    pub fn inputs_of_one_type(&self) -> Option<InputsOfOneType> {
        match self {
            BuiltinFunction::Numeric(function) => function.inputs_of_one_type(),
            // `K` selects between the others.
            BuiltinFunction::Mux => Some(InputsOfOneType {
                first: 1,
                result: OneTypeResult::Common,
            }),
            // Boolean on `BOOL`s and bitwise on bit strings, at the bit
            // string every input widens to: `OR(w, lw)` on a `WORD` and an
            // `LWORD` is an `LWORD`.
            BuiltinFunction::Operator(FormOf::Compare(op)) if op.is_bitwise() => {
                Some(InputsOfOneType {
                    first: 0,
                    result: OneTypeResult::Common,
                })
            }
            BuiltinFunction::Operator(_)
            | BuiltinFunction::BitShift(_)
            | BuiltinFunction::Move
            | BuiltinFunction::Trunc
            | BuiltinFunction::BcdToInt
            | BuiltinFunction::IntToBcd
            | BuiltinFunction::Sizeof
            | BuiltinFunction::String(_)
            | BuiltinFunction::Conversion { .. }
            | BuiltinFunction::Time { .. }
            | BuiltinFunction::DtToDate
            | BuiltinFunction::DtToTod => None,
        }
    }
}

/// Which inputs of a function of several inputs of one type have that type,
/// and which type of theirs its result has. Each of those inputs is converted
/// to the result type, and the function computes at it (ADR-0056).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputsOfOneType {
    /// The position of the first input of the function's type; every input
    /// after it has the type too. `SEL`'s and `MUX`'s first input selects
    /// between the others and has a type of its own.
    pub first: usize,
    /// Which type of those inputs the result has.
    pub result: OneTypeResult,
}

/// Which type of its inputs a function of several inputs of one type
/// returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OneTypeResult {
    /// The type every input widens to: `MAX(i, l)` on an `INT` and an `LINT`
    /// is an `LINT`.
    Common,
    /// The type of the first input: `EXPT(r, i)` raises a `REAL` to an
    /// `INT` power and is a `REAL`, the exponent converted to it.
    First,
}

impl NumericFunction {
    /// Returns `true` for a function of one input.
    fn has_one_input(self) -> bool {
        match self {
            NumericFunction::Abs
            | NumericFunction::Sqrt
            | NumericFunction::Ln
            | NumericFunction::Log
            | NumericFunction::Exp
            | NumericFunction::Sin
            | NumericFunction::Cos
            | NumericFunction::Tan
            | NumericFunction::Asin
            | NumericFunction::Acos
            | NumericFunction::Atan
            | NumericFunction::TruncReal => true,
            NumericFunction::Atan2
            | NumericFunction::Expt
            | NumericFunction::Min
            | NumericFunction::Max
            | NumericFunction::Limit
            | NumericFunction::Sel
            | NumericFunction::ModReal => false,
        }
    }

    /// See [`BuiltinFunction::inputs_of_one_type`].
    fn inputs_of_one_type(self) -> Option<InputsOfOneType> {
        let (first, result) = match self {
            NumericFunction::Min
            | NumericFunction::Max
            | NumericFunction::Limit
            | NumericFunction::Atan2 => (0, OneTypeResult::Common),
            // `G` selects between the others.
            NumericFunction::Sel => (1, OneTypeResult::Common),
            NumericFunction::Expt => (0, OneTypeResult::First),
            NumericFunction::Abs
            | NumericFunction::Sqrt
            | NumericFunction::Ln
            | NumericFunction::Log
            | NumericFunction::Exp
            | NumericFunction::Sin
            | NumericFunction::Cos
            | NumericFunction::Tan
            | NumericFunction::Asin
            | NumericFunction::Acos
            | NumericFunction::Atan
            | NumericFunction::TruncReal
            | NumericFunction::ModReal => return None,
        };
        Some(InputsOfOneType { first, result })
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::textual::{CompareOp, Operator};
    use rstest::rstest;

    #[rstest]
    #[case::not(BuiltinFunction::Operator(FormOf::Not))]
    #[case::abs(BuiltinFunction::Numeric(NumericFunction::Abs))]
    #[case::sqrt(BuiltinFunction::Numeric(NumericFunction::Sqrt))]
    #[case::shift(BuiltinFunction::BitShift(BitShift::Rol))]
    #[case::move_form(BuiltinFunction::Move)]
    #[case::max(BuiltinFunction::Numeric(NumericFunction::Max))]
    #[case::sel(BuiltinFunction::Numeric(NumericFunction::Sel))]
    #[case::expt(BuiltinFunction::Numeric(NumericFunction::Expt))]
    #[case::mux(BuiltinFunction::Mux)]
    #[case::and(BuiltinFunction::Operator(FormOf::Compare(CompareOp::And)))]
    #[case::xor(BuiltinFunction::Operator(FormOf::Compare(CompareOp::Xor)))]
    fn computes_at_own_type_when_operation_on_one_value_or_inputs_of_one_type_then_true(
        #[case] builtin: BuiltinFunction,
    ) {
        assert!(builtin.computes_at_own_type());
    }

    #[rstest]
    #[case::add(BuiltinFunction::Operator(FormOf::Arithmetic(Operator::Add)))]
    #[case::trunc(BuiltinFunction::Trunc)]
    #[case::bcd_to_int(BuiltinFunction::BcdToInt)]
    #[case::mod_real(BuiltinFunction::Numeric(NumericFunction::ModReal))]
    #[case::gt(BuiltinFunction::Operator(FormOf::Compare(CompareOp::Gt)))]
    fn computes_at_own_type_when_result_not_typed_by_its_inputs_then_false(
        #[case] builtin: BuiltinFunction,
    ) {
        assert!(!builtin.computes_at_own_type());
    }

    #[rstest]
    #[case::min(
        BuiltinFunction::Numeric(NumericFunction::Min),
        0,
        OneTypeResult::Common
    )]
    #[case::max(
        BuiltinFunction::Numeric(NumericFunction::Max),
        0,
        OneTypeResult::Common
    )]
    #[case::limit(
        BuiltinFunction::Numeric(NumericFunction::Limit),
        0,
        OneTypeResult::Common
    )]
    #[case::atan2(
        BuiltinFunction::Numeric(NumericFunction::Atan2),
        0,
        OneTypeResult::Common
    )]
    #[case::sel(
        BuiltinFunction::Numeric(NumericFunction::Sel),
        1,
        OneTypeResult::Common
    )]
    #[case::mux(BuiltinFunction::Mux, 1, OneTypeResult::Common)]
    #[case::expt(
        BuiltinFunction::Numeric(NumericFunction::Expt),
        0,
        OneTypeResult::First
    )]
    #[case::and(
        BuiltinFunction::Operator(FormOf::Compare(CompareOp::And)),
        0,
        OneTypeResult::Common
    )]
    #[case::or(
        BuiltinFunction::Operator(FormOf::Compare(CompareOp::Or)),
        0,
        OneTypeResult::Common
    )]
    #[case::xor(
        BuiltinFunction::Operator(FormOf::Compare(CompareOp::Xor)),
        0,
        OneTypeResult::Common
    )]
    fn inputs_of_one_type_when_function_of_several_inputs_of_one_type_then_its_shape(
        #[case] builtin: BuiltinFunction,
        #[case] first: usize,
        #[case] result: OneTypeResult,
    ) {
        assert_eq!(
            builtin.inputs_of_one_type(),
            Some(InputsOfOneType { first, result })
        );
    }

    #[rstest]
    #[case::abs(BuiltinFunction::Numeric(NumericFunction::Abs))]
    #[case::mod_real(BuiltinFunction::Numeric(NumericFunction::ModReal))]
    #[case::eq(BuiltinFunction::Operator(FormOf::Compare(CompareOp::Eq)))]
    #[case::not(BuiltinFunction::Operator(FormOf::Not))]
    #[case::trunc(BuiltinFunction::Trunc)]
    fn inputs_of_one_type_when_other_operation_then_none(#[case] builtin: BuiltinFunction) {
        assert_eq!(builtin.inputs_of_one_type(), None);
    }
}
