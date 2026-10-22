//! The type two operands of different types compute at.
//!
//! A binary operator on two operands of different types computes at the type
//! of one of them: the wider one, or the concrete one when the other is an
//! untyped literal. [`common_operand`] says which type, by the project's
//! implicit widening (`type_compat::are_types_compatible`), so every operator
//! that needs the answer asks the same relation. [`common_operand_of`] asks it
//! of the several inputs of a function such as `MAX` or `LIMIT`.

use ironplc_dsl::common::{GenericTypeName, TypeName};
use ironplc_parser::options::CompilerOptions;

use crate::type_compat::are_types_compatible;

/// Returns the type the operands of types `left` and `right` compute at: the
/// type of the operand the other is acceptable as. `None` when neither is
/// acceptable as the other.
///
/// A literal's category accepts any concrete type in it, so the concrete
/// operand is tried as the expected type first: for `1` and a `DINT` the
/// answer is the `DINT`, not `ANY_INT`. Otherwise the left operand is tried
/// first, so two operands of the same type answer the left one's.
pub(crate) fn common_operand(
    left: &TypeName,
    right: &TypeName,
    options: &CompilerOptions,
) -> Option<TypeName> {
    let (first, second) = if is_generic(left) && !is_generic(right) {
        (right, left)
    } else {
        (left, right)
    };
    if are_types_compatible(first, second, options) {
        return Some(first.clone());
    }
    if are_types_compatible(second, first, options) {
        return Some(second.clone());
    }
    None
}

/// Returns the position of the first of `operands` whose type every operand
/// computes at, by the relation [`common_operand`] answers for a pair: for
/// `MAX(i, l)` on an `INT` and an `LINT`, the `LINT`.
///
/// `None` when an operand has no type, or when no operand's type accepts
/// every other one, as a `DINT` and a `UDINT` do not.
pub(crate) fn common_operand_of(
    operands: &[Option<&TypeName>],
    options: &CompilerOptions,
) -> Option<usize> {
    let operands: Vec<&TypeName> = operands.iter().copied().collect::<Option<_>>()?;
    operands.iter().position(|candidate| {
        operands
            .iter()
            .all(|other| common_operand(candidate, other, options).as_ref() == Some(*candidate))
    })
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
    #[case::same_type("DINT", "DINT", Some("DINT"))]
    #[case::right_wider("DINT", "LINT", Some("LINT"))]
    #[case::left_wider("LINT", "DINT", Some("LINT"))]
    #[case::literal_left("ANY_INT", "DINT", Some("DINT"))]
    #[case::literal_right("DINT", "ANY_INT", Some("DINT"))]
    #[case::no_widening("DINT", "UDINT", None)]
    fn common_operand_when_pair_then_type_the_other_widens_to(
        #[case] left: &str,
        #[case] right: &str,
        #[case] expected: Option<&str>,
    ) {
        let common = common_operand(
            &TypeName::from(left),
            &TypeName::from(right),
            &CompilerOptions::default(),
        );
        assert_eq!(common, expected.map(TypeName::from));
    }

    fn common_of(operands: &[&str]) -> Option<usize> {
        let names: Vec<TypeName> = operands.iter().map(|name| TypeName::from(name)).collect();
        let names: Vec<Option<&TypeName>> = names.iter().map(Some).collect();
        common_operand_of(&names, &CompilerOptions::default())
    }

    #[rstest]
    #[case::one(&["DINT"], Some(0))]
    #[case::same_type(&["DINT", "DINT"], Some(0))]
    #[case::last_widest(&["INT", "DINT", "LINT"], Some(2))]
    #[case::first_widest(&["LINT", "INT", "DINT"], Some(0))]
    #[case::literal_and_concrete(&["ANY_INT", "DINT"], Some(1))]
    #[case::literals(&["ANY_INT", "ANY_INT"], Some(0))]
    #[case::widest_after_a_pair_that_does_not_widen(&["DINT", "UDINT", "LINT"], Some(2))]
    #[case::no_widening(&["DINT", "UDINT"], None)]
    fn common_operand_of_when_operands_then_position_every_other_widens_to(
        #[case] operands: &[&str],
        #[case] expected: Option<usize>,
    ) {
        assert_eq!(common_of(operands), expected);
    }

    #[test]
    fn common_operand_of_when_operand_untyped_then_none() {
        let lint = TypeName::from("LINT");
        let operands = [Some(&lint), None];
        assert_eq!(
            common_operand_of(&operands, &CompilerOptions::default()),
            None
        );
    }
}
