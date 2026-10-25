//! The type two operands of different types compute at.
//!
//! A binary operator on two operands of different types computes at the type
//! of one of them: the wider one, or the concrete one when the other is an
//! untyped literal. An untyped real literal beside an integer computes at
//! neither: the literal takes `REAL`, the type it defaults to (ADR-0028), and
//! the integer widens to it or does not. [`common_operand`] says which type,
//! by the project's implicit widening (`type_compat::are_types_compatible`),
//! so every operator that needs the answer asks the same relation.
//! [`common_operand_of`] asks it of the several inputs of a function such as
//! `MAX` or `LIMIT`, so `MIN(i, 1.5)` computes at the type `i * 1.5` does.

use ironplc_dsl::common::{GenericTypeName, TypeName};
use ironplc_parser::options::CompilerOptions;

use super::numeric_operation::literal_default_type;
use crate::type_compat::are_types_compatible;

/// Returns the type the operands of types `left` and `right` compute at: the
/// type of the operand the other is acceptable as, or the type an untyped
/// real literal defaults to when the other operand is acceptable as that.
/// `None` when the pair has no such type.
///
/// A literal's category accepts any concrete type in it, so the concrete
/// operand is tried as the expected type first: for `1` and a `DINT` the
/// answer is the `DINT`, not `ANY_INT`. Otherwise the left operand is tried
/// first, so two operands of the same type answer the left one's.
///
/// An untyped integer literal is acceptable as every numeric type
/// (ADR-0028), so it always takes the other operand's type. An untyped real
/// literal is acceptable only as a real type, so beside an integer it takes
/// `REAL`, its default type (ADR-0028), which the integer widens to or does
/// not: `i * 1.5` on an `INT` computes at `REAL`, and `d * 1.5` on a `DINT`
/// has no type, as `d * r` on a `DINT` and a `REAL` has none (ADR-0031).
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
    real_literal_default(first, second, options)
}

/// Returns the type the concrete operand `concrete` and the untyped real
/// literal `literal` compute at: `REAL`, the type the literal defaults to,
/// when `concrete` is acceptable as a `REAL`. `None` for any other pair.
fn real_literal_default(
    concrete: &TypeName,
    literal: &TypeName,
    options: &CompilerOptions,
) -> Option<TypeName> {
    if is_generic(concrete)
        || !matches!(
            GenericTypeName::try_from(&literal.id),
            Ok(GenericTypeName::AnyReal)
        )
    {
        return None;
    }
    let default: TypeName = literal_default_type(&GenericTypeName::AnyReal)?.into();
    are_types_compatible(&default, concrete, options).then_some(default)
}

/// The type several operands compute at, as [`common_operand_of`] answers
/// it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommonOperand {
    /// The operand at this position has the type every operand computes at:
    /// for `MAX(i, l)` on an `INT` and an `LINT`, the `LINT`.
    Input(usize),
    /// No operand has that type; every operand computes at this one, the
    /// type an untyped real literal among them defaults to: `MIN(i, 1.5)` on
    /// an `INT` computes at `REAL`.
    Type(TypeName),
}

/// Returns the type every one of `operands` computes at, by the relation
/// [`common_operand`] answers for a pair: the first operand whose type it is,
/// else the `REAL` an untyped real literal among them defaults to when every
/// operand computes at that.
///
/// `None` when an operand has no type, or when no type holds every operand,
/// as none holds a `DINT` and a `UDINT`, or a `DINT` and a real literal.
pub(crate) fn common_operand_of(
    operands: &[Option<&TypeName>],
    options: &CompilerOptions,
) -> Option<CommonOperand> {
    let operands: Vec<&TypeName> = operands.iter().copied().collect::<Option<_>>()?;
    let computes_at = |candidate: &TypeName| {
        operands
            .iter()
            .all(|other| common_operand(candidate, other, options).as_ref() == Some(candidate))
    };
    if let Some(position) = operands.iter().position(|candidate| computes_at(candidate)) {
        return Some(CommonOperand::Input(position));
    }
    let real_literal = operands.iter().any(|operand| {
        matches!(
            GenericTypeName::try_from(&operand.id),
            Ok(GenericTypeName::AnyReal)
        )
    });
    if !real_literal {
        return None;
    }
    let default: TypeName = literal_default_type(&GenericTypeName::AnyReal)?.into();
    computes_at(&default).then_some(CommonOperand::Type(default))
}

/// Returns true if `type_name` is a generic category, the type of an untyped
/// literal (`ANY_INT`, `ANY_REAL`).
fn is_generic(type_name: &TypeName) -> bool {
    GenericTypeName::try_from(&type_name.id).is_ok()
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
    #[case::integer_literal_with_real("REAL", "ANY_INT", Some("REAL"))]
    #[case::real_literal_with_real("ANY_REAL", "REAL", Some("REAL"))]
    #[case::real_literal_with_lreal("LREAL", "ANY_REAL", Some("LREAL"))]
    #[case::two_literals("ANY_INT", "ANY_REAL", Some("ANY_REAL"))]
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

    /// An untyped real literal beside an integer takes `REAL`, its default
    /// type, which the integers of 16 bits or fewer widen to and the wider
    /// ones do not.
    #[rstest]
    #[case::int_right("INT", "ANY_REAL", Some("REAL"))]
    #[case::int_left("ANY_REAL", "INT", Some("REAL"))]
    #[case::sint("SINT", "ANY_REAL", Some("REAL"))]
    #[case::usint("USINT", "ANY_REAL", Some("REAL"))]
    #[case::uint("ANY_REAL", "UINT", Some("REAL"))]
    #[case::dint("DINT", "ANY_REAL", None)]
    #[case::udint("ANY_REAL", "UDINT", None)]
    #[case::lint("LINT", "ANY_REAL", None)]
    #[case::ulint("ULINT", "ANY_REAL", None)]
    #[case::not_a_number("BOOL", "ANY_REAL", None)]
    #[case::duration("TIME", "ANY_REAL", None)]
    fn common_operand_when_real_literal_beside_integer_then_real_if_it_widens(
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

    fn common_of(operands: &[&str]) -> Option<CommonOperand> {
        let names: Vec<TypeName> = operands.iter().map(|name| TypeName::from(name)).collect();
        let names: Vec<Option<&TypeName>> = names.iter().map(Some).collect();
        common_operand_of(&names, &CompilerOptions::default())
    }

    fn input(position: usize) -> Option<CommonOperand> {
        Some(CommonOperand::Input(position))
    }

    #[rstest]
    #[case::one(&["DINT"], input(0))]
    #[case::same_type(&["DINT", "DINT"], input(0))]
    #[case::last_widest(&["INT", "DINT", "LINT"], input(2))]
    #[case::first_widest(&["LINT", "INT", "DINT"], input(0))]
    #[case::literal_and_concrete(&["ANY_INT", "DINT"], input(1))]
    #[case::literals(&["ANY_INT", "ANY_INT"], input(0))]
    #[case::widest_after_a_pair_that_does_not_widen(&["DINT", "UDINT", "LINT"], input(2))]
    #[case::real_literal_with_a_real(&["ANY_REAL", "REAL"], input(1))]
    #[case::real_literal_and_integer_with_lreal(&["INT", "ANY_REAL", "LREAL"], input(2))]
    #[case::no_widening(&["DINT", "UDINT"], None)]
    fn common_operand_of_when_operands_then_input_every_other_widens_to(
        #[case] operands: &[&str],
        #[case] expected: Option<CommonOperand>,
    ) {
        assert_eq!(common_of(operands), expected);
    }

    /// The type an integer and a real literal compute at is neither input's:
    /// `REAL`, when every integer widens to it.
    #[rstest]
    #[case::int(&["INT", "ANY_REAL"], Some("REAL"))]
    #[case::literal_first(&["ANY_REAL", "UINT"], Some("REAL"))]
    #[case::literal_between(&["ANY_REAL", "SINT", "ANY_REAL"], Some("REAL"))]
    #[case::dint(&["DINT", "ANY_REAL"], None)]
    #[case::int_and_dint(&["INT", "ANY_REAL", "DINT"], None)]
    fn common_operand_of_when_real_literal_among_integers_then_real_if_every_one_widens(
        #[case] operands: &[&str],
        #[case] expected: Option<&str>,
    ) {
        assert_eq!(
            common_of(operands),
            expected.map(|name| CommonOperand::Type(TypeName::from(name)))
        );
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
