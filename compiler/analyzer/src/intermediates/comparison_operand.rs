//! The type at which a comparison compares its operands.
//!
//! A comparison (`=`, `<>`, `<`, `<=`, `>`, `>=` and the functions `EQ`,
//! `NE`, `LT`, `LE`, `GT`, `GE`) has a `BOOL` result, so its own resolved
//! type says nothing about how to compare the operands. They are compared at
//! the type one of them widens to, whichever side it is on, by the same
//! relation the numeric arithmetic overload uses for its result.
//!
//! See `specs/design/comparison-operand-type.md`.

use ironplc_dsl::common::TypeName;
use ironplc_parser::options::CompilerOptions;

use super::common_operand::{common_operand, Side};

/// Returns the type at which a comparison of operands of types `left` and
/// `right` compares them, or `None` when an operand has no type or neither
/// widens to the other.
pub fn comparison_operand_type(
    left: Option<&TypeName>,
    right: Option<&TypeName>,
    options: &CompilerOptions,
) -> Option<TypeName> {
    let (left, right) = (left?, right?);
    let common = match common_operand(left, right, options)? {
        Side::Left => left,
        Side::Right => right,
    };
    Some(common.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use spec_test_macro::spec_test;

    fn operand_type(left: &str, right: &str) -> Option<TypeName> {
        comparison_operand_type(
            Some(&TypeName::from(left)),
            Some(&TypeName::from(right)),
            &CompilerOptions::default(),
        )
    }

    #[spec_test(REQ_CMP_analyzer_001)]
    #[rstest]
    #[case::right_wider("DINT", "LINT", "LINT")]
    #[case::left_wider("LINT", "DINT", "LINT")]
    #[case::unsigned("UDINT", "ULINT", "ULINT")]
    #[case::unsigned_into_signed("UDINT", "LINT", "LINT")]
    fn comparison_operand_type_when_integers_then_wider(
        #[case] left: &str,
        #[case] right: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(operand_type(left, right), Some(TypeName::from(expected)));
    }

    #[spec_test(REQ_CMP_analyzer_002)]
    #[rstest]
    #[case::right_wider("DWORD", "LWORD")]
    #[case::left_wider("LWORD", "BYTE")]
    fn comparison_operand_type_when_bit_strings_then_lword(
        #[case] left: &str,
        #[case] right: &str,
    ) {
        assert_eq!(operand_type(left, right), Some(TypeName::from("LWORD")));
    }

    #[spec_test(REQ_CMP_analyzer_003)]
    #[rstest]
    #[case::real_lreal("REAL", "LREAL", "LREAL")]
    #[case::lreal_real("LREAL", "REAL", "LREAL")]
    #[case::int_real("INT", "REAL", "REAL")]
    #[case::real_dint_lossless("DINT", "LREAL", "LREAL")]
    fn comparison_operand_type_when_reals_then_wider_real(
        #[case] left: &str,
        #[case] right: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(operand_type(left, right), Some(TypeName::from(expected)));
    }

    #[spec_test(REQ_CMP_analyzer_004)]
    #[rstest]
    #[case::time("TIME", "LTIME", "LTIME")]
    #[case::date("LDATE", "DATE", "LDATE")]
    #[case::time_of_day("TIME_OF_DAY", "LTIME_OF_DAY", "LTIME_OF_DAY")]
    #[case::date_and_time("DATE_AND_TIME", "LDATE_AND_TIME", "LDATE_AND_TIME")]
    fn comparison_operand_type_when_short_and_long_temporal_then_long(
        #[case] left: &str,
        #[case] right: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(operand_type(left, right), Some(TypeName::from(expected)));
    }

    #[spec_test(REQ_CMP_analyzer_005)]
    #[rstest]
    #[case::literal_left("ANY_INT", "LINT")]
    #[case::literal_right("LINT", "ANY_INT")]
    fn comparison_operand_type_when_literal_then_concrete(#[case] left: &str, #[case] right: &str) {
        assert_eq!(operand_type(left, right), Some(TypeName::from("LINT")));
    }

    #[spec_test(REQ_CMP_analyzer_006)]
    #[rstest]
    #[case::signed_unsigned("DINT", "UDINT")]
    #[case::integer_real_lossy("DINT", "REAL")]
    #[case::different_families("TIME", "DATE")]
    fn comparison_operand_type_when_neither_widens_then_none(
        #[case] left: &str,
        #[case] right: &str,
    ) {
        assert_eq!(operand_type(left, right), None);
    }

    #[test]
    fn comparison_operand_type_when_operand_untyped_then_none() {
        let lint = TypeName::from("LINT");
        let options = CompilerOptions::default();
        assert_eq!(comparison_operand_type(None, Some(&lint), &options), None);
        assert_eq!(comparison_operand_type(Some(&lint), None, &options), None);
    }
}
