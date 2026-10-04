//! The width at which a numeric type is operated on.
//!
//! An arithmetic operation computes at the width of its result type, and an
//! operand of another width is converted to it. Whether two numeric types
//! share a width is a fact about the types, so the analyzer states it, where
//! the conversion is recorded (ADR-0056).
//!
//! This is the width the code generator operates on: a value of 32 bits or
//! fewer in a 32-bit word, a wider one in a 64-bit word, and a real in a
//! float of its own size.

use ironplc_dsl::common::{ElementaryTypeName, GenericTypeName, TypeName};

use crate::intermediate_type::IntermediateType;
use crate::type_environment::elementary_type;

/// The width of the word or float a numeric type is operated on in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationWidth {
    W32,
    W64,
    F32,
    F64,
}

/// Returns the width `type_name` is operated on at when it is an elementary
/// numeric or bit-string type, the types the numeric overload computes at,
/// and `None` for anything else: an untyped literal's category, a subrange,
/// an enumeration, a temporal type, or `BOOL`.
pub(crate) fn numeric_operation_width(type_name: &TypeName) -> Option<OperationWidth> {
    let elementary = ElementaryTypeName::try_from(&type_name.name).ok()?;
    let numeric = GenericTypeName::AnyNum.is_compatible_with(&elementary)
        || matches!(
            elementary,
            ElementaryTypeName::BYTE
                | ElementaryTypeName::WORD
                | ElementaryTypeName::DWORD
                | ElementaryTypeName::LWORD
        );
    if !numeric {
        return None;
    }
    let representation = elementary_type(type_name)?;
    let bits = representation.size_in_bytes()? * 8;
    Some(match representation {
        IntermediateType::Real { .. } if bits <= 32 => OperationWidth::F32,
        IntermediateType::Real { .. } => OperationWidth::F64,
        _ if bits <= 32 => OperationWidth::W32,
        _ => OperationWidth::W64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::sint("SINT", OperationWidth::W32)]
    #[case::int("INT", OperationWidth::W32)]
    #[case::udint("UDINT", OperationWidth::W32)]
    #[case::lint("LINT", OperationWidth::W64)]
    #[case::byte("BYTE", OperationWidth::W32)]
    #[case::lword("LWORD", OperationWidth::W64)]
    #[case::real("REAL", OperationWidth::F32)]
    #[case::lreal("LREAL", OperationWidth::F64)]
    fn numeric_operation_width_when_numeric_then_width_of_its_size(
        #[case] name: &str,
        #[case] expected: OperationWidth,
    ) {
        assert_eq!(
            numeric_operation_width(&TypeName::from(name)),
            Some(expected)
        );
    }

    #[rstest]
    #[case::boolean("BOOL")]
    #[case::time("TIME")]
    #[case::string("STRING")]
    #[case::literal("ANY_INT")]
    #[case::user_defined("MyType")]
    fn numeric_operation_width_when_not_numeric_then_none(#[case] name: &str) {
        assert_eq!(numeric_operation_width(&TypeName::from(name)), None);
    }
}
