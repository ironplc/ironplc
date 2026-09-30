//! Which of two operand types the other one is acceptable as.
//!
//! A binary operator on two operands of different types computes at the type
//! of one of them: the wider one, or the concrete one when the other is an
//! untyped literal. [`common_operand`] says which, by the project's implicit
//! widening (`type_compat::are_types_compatible`), so every operator that
//! needs the answer asks the same relation.

use ironplc_dsl::common::{GenericTypeName, TypeName};
use ironplc_parser::options::CompilerOptions;

use crate::type_compat::are_types_compatible;

/// One operand of a binary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Side {
    Left,
    Right,
}

/// Returns the operand whose type the other operand is acceptable as, or
/// `None` when neither is acceptable as the other.
///
/// A literal's category accepts any concrete type in it, so the concrete
/// operand is tried as the expected type first: for `1` and a `DINT` the
/// answer is the `DINT`, not `ANY_INT`. Otherwise the left operand is tried
/// first, so two operands of the same type answer [`Side::Left`].
pub(crate) fn common_operand(
    left: &TypeName,
    right: &TypeName,
    options: &CompilerOptions,
) -> Option<Side> {
    let (first, second) = if is_generic(left) && !is_generic(right) {
        (Side::Right, Side::Left)
    } else {
        (Side::Left, Side::Right)
    };
    let type_of = |side: Side| match side {
        Side::Left => left,
        Side::Right => right,
    };
    if are_types_compatible(type_of(first), type_of(second), options) {
        Some(first)
    } else if are_types_compatible(type_of(second), type_of(first), options) {
        Some(second)
    } else {
        None
    }
}

/// Returns true if `type_name` is a generic category, the type of an untyped
/// literal (`ANY_INT`, `ANY_REAL`).
fn is_generic(type_name: &TypeName) -> bool {
    GenericTypeName::try_from(&type_name.name).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::same_type("DINT", "DINT", Some(Side::Left))]
    #[case::right_wider("DINT", "LINT", Some(Side::Right))]
    #[case::left_wider("LINT", "DINT", Some(Side::Left))]
    #[case::literal_left("ANY_INT", "DINT", Some(Side::Right))]
    #[case::literal_right("DINT", "ANY_INT", Some(Side::Left))]
    #[case::no_widening("DINT", "UDINT", None)]
    fn common_operand_when_pair_then_side_the_other_widens_to(
        #[case] left: &str,
        #[case] right: &str,
        #[case] expected: Option<Side>,
    ) {
        let side = common_operand(
            &TypeName::from(left),
            &TypeName::from(right),
            &CompilerOptions::default(),
        );
        assert_eq!(side, expected);
    }
}
