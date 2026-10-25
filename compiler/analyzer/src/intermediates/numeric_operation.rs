//! The width and signedness at which a numeric type is operated on.
//!
//! An arithmetic operation computes at its result type, and an operand of
//! another operation type -- another width or, for an integer, another
//! signedness -- is converted to it. Whether two numeric types share an
//! operation type is a fact about the types, so the analyzer states it, where
//! the conversion is recorded (ADR-0056).
//!
//! The width is the one the code generator operates on: a value of 32 bits or
//! fewer in a 32-bit word, a wider one in a 64-bit word, and a real in a
//! float of its own size. A value operated on or stored at a type of its
//! width and another signedness is converted too, although its bits do not
//! change.

use ironplc_dsl::common::{ElementaryTypeName, GenericTypeName, TypeName};

use crate::semantic_type::SemanticType;
use crate::type_environment::elementary_type;

/// The width of the word or float a numeric type is operated on in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OperationWidth {
    W32,
    W64,
    F32,
    F64,
}

/// Returns the width `type_name` is operated on at, as
/// [`numeric_operation_type`] gives its operation type.
pub(crate) fn numeric_operation_width(type_name: &TypeName) -> Option<OperationWidth> {
    numeric_operation_type(type_name).map(|at| at.width)
}

/// Returns the type `type_name` is operated on at when it is an elementary
/// numeric or bit-string type, the types the numeric overload computes at,
/// and `None` for anything else: an untyped literal's category, a subrange,
/// an enumeration, a temporal type, or `BOOL`.
pub(crate) fn numeric_operation_type(type_name: &TypeName) -> Option<OperationType> {
    let elementary = ElementaryTypeName::try_from(&type_name.id).ok()?;
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
    operation_type_of(elementary_type(type_name)?)
}

/// Returns the width a value of the type `representation` is operated on
/// at: an enumeration as a 32-bit ordinal, a subrange as its base type, a
/// reference as a 64-bit address, and an elementary type by its size.
/// `None` for a type that is not operated on as a single value: a string,
/// an aggregate or a POU.
pub(crate) fn operation_width_of(representation: &SemanticType) -> Option<OperationWidth> {
    match representation {
        SemanticType::Enumeration { .. } | SemanticType::Bool => Some(OperationWidth::W32),
        SemanticType::Subrange { base_type, .. } => operation_width_of(base_type),
        SemanticType::Reference { .. } => Some(OperationWidth::W64),
        SemanticType::Int { .. }
        | SemanticType::UInt { .. }
        | SemanticType::Real { .. }
        | SemanticType::Bytes { .. }
        | SemanticType::Time { .. }
        | SemanticType::Date { .. }
        | SemanticType::TimeOfDay { .. }
        | SemanticType::DateAndTime { .. } => {
            let bits = representation.bit_width()?;
            Some(match representation {
                SemanticType::Real { .. } if bits <= 32 => OperationWidth::F32,
                SemanticType::Real { .. } => OperationWidth::F64,
                _ if bits <= 32 => OperationWidth::W32,
                _ => OperationWidth::W64,
            })
        }
        SemanticType::String { .. }
        | SemanticType::Structure { .. }
        | SemanticType::Array { .. }
        | SemanticType::FunctionBlock { .. }
        | SemanticType::Interface { .. } => None,
    }
}

/// Whether an integer is operated on as a signed or an unsigned number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Signedness {
    Signed,
    Unsigned,
}

/// The type a value is operated on at: its width and, for an integer, its
/// signedness. A value stored at a type of another operation type is
/// converted to it, as the lowered program requires of every value it
/// stores (REQ-LOW-lowering-094), even where the bits do not change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OperationType {
    pub(crate) width: OperationWidth,
    pub(crate) signedness: Option<Signedness>,
}

/// Returns the type a value of the type `representation` is operated on
/// at: its width, as [`operation_width_of`] gives it, and its signedness,
/// as [`operation_signedness_of`] gives it.
pub(crate) fn operation_type_of(representation: &SemanticType) -> Option<OperationType> {
    Some(OperationType {
        width: operation_width_of(representation)?,
        signedness: operation_signedness_of(representation),
    })
}

/// Returns the signedness a value of the type `representation` is operated
/// on at as an integer: a signed integer, a duration and an enumeration's
/// ordinal are signed, an unsigned integer, a bit string, a date, a time of
/// day and a date and time unsigned, and a subrange is its base type's.
/// `None` for a type that is not operated on as an integer: a `BOOL`, a
/// real, a reference, a string, an aggregate or a POU.
fn operation_signedness_of(representation: &SemanticType) -> Option<Signedness> {
    match representation {
        SemanticType::Subrange { base_type, .. } => operation_signedness_of(base_type),
        SemanticType::Int { .. } | SemanticType::Time { .. } | SemanticType::Enumeration { .. } => {
            Some(Signedness::Signed)
        }
        SemanticType::UInt { .. }
        | SemanticType::Bytes { .. }
        | SemanticType::Date { .. }
        | SemanticType::TimeOfDay { .. }
        | SemanticType::DateAndTime { .. } => Some(Signedness::Unsigned),
        SemanticType::Bool
        | SemanticType::Real { .. }
        | SemanticType::Reference { .. }
        | SemanticType::String { .. }
        | SemanticType::Structure { .. }
        | SemanticType::Array { .. }
        | SemanticType::FunctionBlock { .. }
        | SemanticType::Interface { .. } => None,
    }
}

/// The elementary type an untyped literal of the category `generic` is
/// operated at when nothing gives it a type: `DINT` for an integer and
/// `REAL` for a real (ADR-0028). `None` for a category no untyped literal
/// has.
pub fn literal_default_type(generic: &GenericTypeName) -> Option<ElementaryTypeName> {
    match generic {
        GenericTypeName::AnyInt | GenericTypeName::AnyNum | GenericTypeName::AnyMagnitude => {
            Some(ElementaryTypeName::DINT)
        }
        GenericTypeName::AnyReal => Some(ElementaryTypeName::REAL),
        GenericTypeName::Any
        | GenericTypeName::AnyDerived
        | GenericTypeName::AnyElementary
        | GenericTypeName::AnyBit
        | GenericTypeName::AnyString
        | GenericTypeName::AnyDate => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_type::ByteSized;
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
    #[case::usint("USINT", OperationWidth::W32, Signedness::Unsigned)]
    #[case::dint("DINT", OperationWidth::W32, Signedness::Signed)]
    #[case::lword("LWORD", OperationWidth::W64, Signedness::Unsigned)]
    fn numeric_operation_type_when_integer_then_its_width_and_signedness(
        #[case] name: &str,
        #[case] width: OperationWidth,
        #[case] signedness: Signedness,
    ) {
        assert_eq!(
            numeric_operation_type(&TypeName::from(name)),
            Some(OperationType {
                width,
                signedness: Some(signedness),
            })
        );
    }

    #[rstest]
    #[case::boolean(SemanticType::Bool, OperationWidth::W32)]
    #[case::reference(
        SemanticType::Reference { target_type: Box::new(SemanticType::Bool) },
        OperationWidth::W64
    )]
    fn operation_width_of_when_not_numeric_scalar_then_its_slot_width(
        #[case] representation: SemanticType,
        #[case] expected: OperationWidth,
    ) {
        assert_eq!(operation_width_of(&representation), Some(expected));
    }

    #[rstest]
    #[case::int(SemanticType::Int { size: ByteSized::B8 }, OperationWidth::W32, Some(Signedness::Signed))]
    #[case::uint(SemanticType::UInt { size: ByteSized::B8 }, OperationWidth::W32, Some(Signedness::Unsigned))]
    #[case::bit_string(SemanticType::Bytes { size: ByteSized::B64 }, OperationWidth::W64, Some(Signedness::Unsigned))]
    #[case::time(SemanticType::Time { size: ByteSized::B32 }, OperationWidth::W32, Some(Signedness::Signed))]
    #[case::date(SemanticType::Date { size: ByteSized::B32 }, OperationWidth::W32, Some(Signedness::Unsigned))]
    #[case::real(SemanticType::Real { size: ByteSized::B32 }, OperationWidth::F32, None)]
    #[case::boolean(SemanticType::Bool, OperationWidth::W32, None)]
    fn operation_type_of_when_scalar_then_its_width_and_signedness(
        #[case] representation: SemanticType,
        #[case] width: OperationWidth,
        #[case] signedness: Option<Signedness>,
    ) {
        assert_eq!(
            operation_type_of(&representation),
            Some(OperationType { width, signedness })
        );
    }

    #[test]
    fn operation_type_of_when_subrange_then_its_base_types() {
        let representation = SemanticType::Subrange {
            base_type: Box::new(SemanticType::UInt {
                size: ByteSized::B16,
            }),
            min_value: 0,
            max_value: 10,
        };
        assert_eq!(
            operation_type_of(&representation),
            Some(OperationType {
                width: OperationWidth::W32,
                signedness: Some(Signedness::Unsigned),
            })
        );
    }

    #[rstest]
    #[case::int(GenericTypeName::AnyInt, Some(ElementaryTypeName::DINT))]
    #[case::real(GenericTypeName::AnyReal, Some(ElementaryTypeName::REAL))]
    #[case::string(GenericTypeName::AnyString, None)]
    fn literal_default_type_when_category_then_its_default(
        #[case] generic: GenericTypeName,
        #[case] expected: Option<ElementaryTypeName>,
    ) {
        assert_eq!(literal_default_type(&generic), expected);
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
