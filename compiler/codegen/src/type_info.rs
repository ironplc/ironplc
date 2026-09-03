//! What a type is, in the terms this backend operates on.
//!
//! The analyzer's elementary type table states what each elementary type is --
//! an `INT` is a signed 16-bit integer. This module turns that into the
//! `VarTypeInfo` codegen needs: the storage width, the signedness of the
//! opcodes, and the width the VM operates at. Keeping the projection here,
//! rather than restating the table, is what stops the two from drifting.

use ironplc_dsl::common::{ElementaryTypeName, GenericTypeName, TypeName};
use ironplc_dsl::core::Id;

use ironplc_analyzer::intermediate_type::IntermediateType;

use super::compile::{OpWidth, Signedness, VarTypeInfo};

/// Maps an IEC 61131-3 type name to its `VarTypeInfo`.
///
/// Returns `None` for unrecognized type names (e.g., user-defined types)
/// and for STRING/WSTRING which are handled separately.
pub(crate) fn resolve_type_name(name: &Id) -> Option<VarTypeInfo> {
    // Try as elementary type first (the common case), then fall back to
    // generic types mapped to their default concrete representation.
    // Generic types may reach codegen for expressions like `5 + 5` where
    // no concrete type context was available during type resolution.
    let elem = ElementaryTypeName::try_from(name)
        .or_else(|_| match GenericTypeName::try_from(name)? {
            GenericTypeName::AnyInt | GenericTypeName::AnyNum | GenericTypeName::AnyMagnitude => {
                Ok(ElementaryTypeName::DINT)
            }
            GenericTypeName::AnyReal => Ok(ElementaryTypeName::REAL),
            _ => Err(()),
        })
        .ok()?;
    let elem_name: TypeName = elem.into();
    var_type_info(ironplc_analyzer::elementary_type(&elem_name)?)
}

/// Projects what a type *is* onto how this backend operates on it.
///
/// The analyzer's elementary type table says an `INT` is a signed 16-bit
/// integer; this says that a signed 16-bit integer holds 16 bits of value and
/// is operated on 32 bits wide. Deriving one from the other keeps the two
/// statements from drifting: the operation width is computed from the value
/// width rather than written out per type, so it can never be the narrower of
/// the two.
///
/// `VarTypeInfo::storage_bits` is a *value* width -- how many bits of the
/// operand `TRUNC` keeps, and what bounds the values the type can hold. It is
/// not a footprint and nothing addresses bits with it. The analyzer states
/// size as a footprint in bytes, and the two coincide for every elementary
/// type but one, which is why deriving the first from the second is sound.
///
/// Returns `None` for types this backend does not operate on arithmetically
/// (STRING and WSTRING, which are handled through the data region, and the
/// composite types).
fn var_type_info(representation: &IntermediateType) -> Option<VarTypeInfo> {
    // BOOL is the exception the doc comment above refers to: it holds one bit
    // of value in a byte of footprint, so the analyzer's byte-granular size
    // cannot state it. The value 1 also marks BOOL for the conversion path,
    // which tests an operand for zero rather than keeping its low bit.
    if matches!(representation, IntermediateType::Bool) {
        return Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 1,
        });
    }

    // A duration is signed. A date or a time of day counts forward from an
    // epoch, and a bit string is a pattern rather than a magnitude, so
    // neither is.
    let signedness = match representation {
        IntermediateType::Int { .. }
        | IntermediateType::Real { .. }
        | IntermediateType::Time { .. } => Signedness::Signed,
        IntermediateType::UInt { .. }
        | IntermediateType::Bytes { .. }
        | IntermediateType::Date { .. }
        | IntermediateType::TimeOfDay { .. }
        | IntermediateType::DateAndTime { .. } => Signedness::Unsigned,
        _ => return None,
    };

    let storage_bits = u8::try_from(representation.size_in_bytes()? * 8).ok()?;
    let op_width = match representation {
        IntermediateType::Real { .. } if storage_bits <= 32 => OpWidth::F32,
        IntermediateType::Real { .. } => OpWidth::F64,
        _ if storage_bits <= 32 => OpWidth::W32,
        _ => OpWidth::W64,
    };

    Some(VarTypeInfo {
        op_width,
        signedness,
        storage_bits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// The width in bits an operation of this width works on.
    fn op_width_bits(width: OpWidth) -> u8 {
        match width {
            OpWidth::W32 | OpWidth::F32 => 32,
            OpWidth::W64 | OpWidth::F64 => 64,
        }
    }

    /// Pins what every elementary type projects onto, and with it the
    /// invariant the analyzer's range check depends on: a value that fits a
    /// type's storage fits the width the type is operated on, so the
    /// operation width is never the narrower of the two.
    #[rstest]
    #[case::sint("SINT", OpWidth::W32, Signedness::Signed, 8)]
    #[case::int("INT", OpWidth::W32, Signedness::Signed, 16)]
    #[case::dint("DINT", OpWidth::W32, Signedness::Signed, 32)]
    #[case::lint("LINT", OpWidth::W64, Signedness::Signed, 64)]
    #[case::usint("USINT", OpWidth::W32, Signedness::Unsigned, 8)]
    #[case::uint("UINT", OpWidth::W32, Signedness::Unsigned, 16)]
    #[case::udint("UDINT", OpWidth::W32, Signedness::Unsigned, 32)]
    #[case::ulint("ULINT", OpWidth::W64, Signedness::Unsigned, 64)]
    #[case::real("REAL", OpWidth::F32, Signedness::Signed, 32)]
    #[case::lreal("LREAL", OpWidth::F64, Signedness::Signed, 64)]
    #[case::bool("BOOL", OpWidth::W32, Signedness::Signed, 1)]
    #[case::byte("BYTE", OpWidth::W32, Signedness::Unsigned, 8)]
    #[case::word("WORD", OpWidth::W32, Signedness::Unsigned, 16)]
    #[case::dword("DWORD", OpWidth::W32, Signedness::Unsigned, 32)]
    #[case::lword("LWORD", OpWidth::W64, Signedness::Unsigned, 64)]
    #[case::time("TIME", OpWidth::W32, Signedness::Signed, 32)]
    #[case::ltime("LTIME", OpWidth::W64, Signedness::Signed, 64)]
    #[case::date("DATE", OpWidth::W32, Signedness::Unsigned, 32)]
    #[case::ldate("LDATE", OpWidth::W64, Signedness::Unsigned, 64)]
    #[case::tod("TIME_OF_DAY", OpWidth::W32, Signedness::Unsigned, 32)]
    #[case::ltod("LTIME_OF_DAY", OpWidth::W64, Signedness::Unsigned, 64)]
    #[case::dt("DATE_AND_TIME", OpWidth::W32, Signedness::Unsigned, 32)]
    #[case::ldt("LDATE_AND_TIME", OpWidth::W64, Signedness::Unsigned, 64)]
    fn resolve_type_name_when_elementary_type_then_operates_at_least_as_wide_as_storage(
        #[case] type_name: &str,
        #[case] op_width: OpWidth,
        #[case] signedness: Signedness,
        #[case] storage_bits: u8,
    ) {
        let info = resolve_type_name(&Id::from(type_name)).unwrap();

        assert_eq!(info.op_width, op_width);
        assert_eq!(info.signedness, signedness);
        assert_eq!(info.storage_bits, storage_bits);
        assert!(info.storage_bits <= op_width_bits(info.op_width));
    }

    #[rstest]
    #[case::string("STRING")]
    #[case::wstring("WSTRING")]
    #[case::user_defined("MyStruct")]
    fn resolve_type_name_when_not_operated_on_arithmetically_then_none(#[case] type_name: &str) {
        assert!(resolve_type_name(&Id::from(type_name)).is_none());
    }

    /// A generic type reaches codegen for an expression such as `5 + 5`, where
    /// type resolution had no concrete type to give it.
    #[rstest]
    #[case::any_int("ANY_INT", OpWidth::W32, 32)]
    #[case::any_real("ANY_REAL", OpWidth::F32, 32)]
    fn resolve_type_name_when_generic_type_then_default_concrete_type(
        #[case] type_name: &str,
        #[case] op_width: OpWidth,
        #[case] storage_bits: u8,
    ) {
        let info = resolve_type_name(&Id::from(type_name)).unwrap();

        assert_eq!(info.op_width, op_width);
        assert_eq!(info.storage_bits, storage_bits);
    }
}
