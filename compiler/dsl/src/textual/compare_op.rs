//! The operators of a compare expression, and the one place each is
//! classified by what it computes.
use std::fmt;

/// Comparison operators.
///
/// See section 3.2.2, especially table 52.
///
/// The enum holds the bitwise operators as well as the comparisons. Code
/// that treats the two differently asks [`CompareOp::kind`] (or
/// [`CompareOp::is_comparison`] and [`CompareOp::is_bitwise`]) rather than
/// listing operators, so an operator added here is classified once.
#[derive(Debug, PartialEq, Clone)]
pub enum CompareOp {
    Or,
    Xor,
    And,
    /// CODESYS/TwinCAT short-circuit `AND` (Beckhoff/CODESYS extension).
    /// Kept as a distinct variant from `And` (not normalized) since the
    /// short-circuit vs. eager evaluation distinction is real and
    /// externally-visible in TwinCAT/CODESYS itself.
    AndThen,
    /// CODESYS/TwinCAT short-circuit `OR`, the dual of [`CompareOp::AndThen`].
    OrElse,
    Eq,
    Ne,
    Lt,
    Gt,
    LtEq,
    GtEq,
}

/// What a [`CompareOp`] computes, which decides how the operator and its
/// operands are typed and checked.
///
/// [`CompareOp::kind`] is the single definition of which operator is of
/// which kind. A site that does something different per kind matches on the
/// kind, never on a list of operators: when each site wrote its own list,
/// `AND_THEN` and `OR_ELSE` were typed as `AND` and `OR` at some sites and
/// as comparisons at others.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CompareOpKind {
    /// `=` and `<>`, whose result is `BOOL` whatever their operands are.
    Equality,
    /// `<`, `<=`, `>` and `>=`, whose result is `BOOL` whatever their
    /// operands are, and whose operands must have an order.
    Ordering,
    /// `AND`, `OR` and `XOR`, and their short-circuit forms `AND_THEN` and
    /// `OR_ELSE`, which compute on operands of one bit type: the result has
    /// the type of the operands, a `BOOL` of `BOOL`s and a bit string of bit
    /// strings.
    Bitwise,
}

impl CompareOp {
    /// The operator's source spelling.
    ///
    /// This is the single definition of how a `CompareOp` is written: both
    /// [`fmt::Display`] and the `plc2plc` renderer read it, so an operator
    /// added to this enum is spelled once.
    pub fn as_str(&self) -> &'static str {
        match self {
            CompareOp::Or => "OR",
            CompareOp::Xor => "XOR",
            CompareOp::And => "AND",
            CompareOp::AndThen => "AND_THEN",
            CompareOp::OrElse => "OR_ELSE",
            CompareOp::Eq => "=",
            CompareOp::Ne => "<>",
            CompareOp::Lt => "<",
            CompareOp::Gt => ">",
            CompareOp::LtEq => "<=",
            CompareOp::GtEq => ">=",
        }
    }

    /// The kind of operation the operator performs.
    ///
    /// This is the single definition of which operator is of which kind (see
    /// [`CompareOpKind`]), so an operator added to this enum is classified
    /// once.
    pub fn kind(&self) -> CompareOpKind {
        match self {
            CompareOp::Eq | CompareOp::Ne => CompareOpKind::Equality,
            CompareOp::Lt | CompareOp::Gt | CompareOp::LtEq | CompareOp::GtEq => {
                CompareOpKind::Ordering
            }
            CompareOp::And
            | CompareOp::Or
            | CompareOp::Xor
            | CompareOp::AndThen
            | CompareOp::OrElse => CompareOpKind::Bitwise,
        }
    }

    /// Returns `true` for the comparisons `=`, `<>`, `<`, `<=`, `>` and
    /// `>=`, whose result is `BOOL` whatever their operands are, and `false`
    /// for the bitwise operators that share this enum.
    pub fn is_comparison(&self) -> bool {
        match self.kind() {
            CompareOpKind::Equality | CompareOpKind::Ordering => true,
            CompareOpKind::Bitwise => false,
        }
    }

    /// Returns `true` for `AND`, `OR` and `XOR`, and for their short-circuit
    /// forms `AND_THEN` and `OR_ELSE`, whose result has the type of their
    /// operands -- a `BOOL` of `BOOL`s, a bit string of bit strings -- and
    /// `false` for the comparisons, whose result is a `BOOL` whatever they
    /// compare.
    pub fn is_bitwise(&self) -> bool {
        match self.kind() {
            CompareOpKind::Bitwise => true,
            CompareOpKind::Equality | CompareOpKind::Ordering => false,
        }
    }
}

impl fmt::Display for CompareOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::eq(CompareOp::Eq, CompareOpKind::Equality)]
    #[case::ne(CompareOp::Ne, CompareOpKind::Equality)]
    #[case::lt(CompareOp::Lt, CompareOpKind::Ordering)]
    #[case::gt(CompareOp::Gt, CompareOpKind::Ordering)]
    #[case::lt_eq(CompareOp::LtEq, CompareOpKind::Ordering)]
    #[case::gt_eq(CompareOp::GtEq, CompareOpKind::Ordering)]
    #[case::and(CompareOp::And, CompareOpKind::Bitwise)]
    #[case::or(CompareOp::Or, CompareOpKind::Bitwise)]
    #[case::xor(CompareOp::Xor, CompareOpKind::Bitwise)]
    #[case::and_then(CompareOp::AndThen, CompareOpKind::Bitwise)]
    #[case::or_else(CompareOp::OrElse, CompareOpKind::Bitwise)]
    fn kind_when_operator_then_its_kind(#[case] op: CompareOp, #[case] expected: CompareOpKind) {
        assert_eq!(op.kind(), expected);
    }

    #[rstest]
    #[case::equality(CompareOp::Ne)]
    #[case::ordering(CompareOp::GtEq)]
    fn is_comparison_when_equality_or_ordering_then_true_and_not_bitwise(#[case] op: CompareOp) {
        assert!(op.is_comparison());
        assert!(!op.is_bitwise());
    }

    #[rstest]
    #[case::eager(CompareOp::Xor)]
    #[case::short_circuit(CompareOp::OrElse)]
    fn is_bitwise_when_bitwise_then_true_and_not_comparison(#[case] op: CompareOp) {
        assert!(op.is_bitwise());
        assert!(!op.is_comparison());
    }
}
