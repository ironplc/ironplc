//! Behaviour policies (ADR-0049): the alternatives IronPLC offers where
//! IEC 61131-3 leaves an operation's meaning to the implementer. Lowering
//! selects one from the compiler's options and names it in the `Intrinsic`
//! it chooses, so a backend implements the alternative it is given and reads
//! no options of its own (REQ-LOW-lowering-073).

/// How a `STRING_TO_*` conversion reads its string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParsePolicy {
    pub non_numeric: NonNumeric,
    pub failure: ParseFailure,
}

/// What a `STRING_TO_*` conversion reads as a number when its string holds
/// characters that are not part of a literal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonNumeric {
    /// The whole string, less surrounding whitespace, is a literal of the
    /// target type.
    Reject,
    /// The longest literal at the start of the string, after leading
    /// whitespace; what follows it is ignored.
    IgnoreTrailing,
    /// The first literal in the string, wherever it starts; what surrounds
    /// it is ignored.
    IgnoreSurrounding,
}

/// What a `STRING_TO_*` conversion does with a string that does not
/// convert: no literal as its [`NonNumeric`] reads it, or a literal whose
/// value the target type does not hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseFailure {
    /// Trap (V4006).
    Trap,
    /// Yield the target type's zero.
    Zero,
}
