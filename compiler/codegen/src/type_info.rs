//! What a type is, in the terms this backend operates on.
//!
//! The analyzer's elementary type table states what each elementary type is --
//! an `INT` is a signed 16-bit integer. This module turns that into the
//! `VarTypeInfo` codegen needs: the storage width, the signedness of the
//! opcodes, and the width the VM operates at. Keeping the projection here,
//! rather than restating the table, is what stops the two from drifting.

use std::collections::HashMap;

use ironplc_analyzer::value_type::operand_type_name;
use ironplc_dsl::common::{ElementaryTypeName, GenericTypeName, TypeName, VarDecl};
use ironplc_dsl::core::Id;
use ironplc_dsl::textual::{Expr, ExprType};
use ironplc_dsl::type_id::TypeId;

use ironplc_analyzer::intermediate_type::IntermediateType;
use ironplc_analyzer::TypeEnvironment;

use super::compile::{CompileContext, OpWidth, Signedness, VarTypeInfo};

/// What every type in the environment is, by id, so codegen can ask what an
/// expression's type is from its `expr_type` alone. Anonymous types are
/// included: they have no name to look up.
pub(crate) fn type_representations(types: &TypeEnvironment) -> HashMap<TypeId, IntermediateType> {
    types
        .iter_ids()
        .map(|(id, attributes)| (id, attributes.representation.clone()))
        .collect()
}

/// The name the analyzer's name-based relations (the arithmetic overloads)
/// know a value of each type by, by id: `value_type::operand_type_name`,
/// computed once so codegen derives it the same way the analyzer does.
pub(crate) fn operand_names(types: &TypeEnvironment) -> HashMap<TypeId, TypeName> {
    types
        .iter_ids()
        .filter_map(|(id, _)| {
            let name = operand_type_name(types, &ExprType::Concrete(id))?;
            Some((id, name))
        })
        .collect()
}

/// The name the arithmetic overloads know an expression's value by, from
/// its `expr_type` (see [`operand_names`]).
pub(crate) fn expr_operand_name(ctx: &CompileContext, expr: &Expr) -> Option<TypeName> {
    match expr.expr_type.as_ref()? {
        ExprType::Concrete(id) => ctx.operand_names.get(id).cloned(),
        ExprType::Literal(generic) => Some(generic.clone().into()),
        ExprType::Null => None,
    }
}

/// The `VarTypeInfo` of an expression's value, from its `expr_type`.
///
/// `None` when the analyzer resolved no type for the expression, or when its
/// type is not one this backend operates on arithmetically (a string, or a
/// composite type).
pub(crate) fn expr_type_info(ctx: &CompileContext, expr: &Expr) -> Option<VarTypeInfo> {
    match expr.expr_type.as_ref()? {
        ExprType::Concrete(id) => operand_type_info(ctx.types.get(id)?),
        ExprType::Literal(generic) => literal_type_info(generic),
        // NULL is compared and stored as the reference it stands in for.
        ExprType::Null => Some(reference_type_info()),
    }
}

/// The `VarTypeInfo` of the type a declaration declares, from its
/// `type_id`. `None` when the analyzer resolved no type for it, or when the
/// type is not one this backend operates on arithmetically.
pub(crate) fn decl_type_info(ctx: &CompileContext, decl: &VarDecl) -> Option<VarTypeInfo> {
    operand_type_info(ctx.types.get(&decl.type_id?)?)
}

/// What an expression's value is, from its `expr_type`, when it has a
/// concrete type.
pub(crate) fn expr_representation<'a>(
    ctx: &'a CompileContext,
    expr: &Expr,
) -> Option<&'a IntermediateType> {
    match expr.expr_type.as_ref()? {
        ExprType::Concrete(id) => ctx.types.get(id),
        ExprType::Literal(_) | ExprType::Null => None,
    }
}

/// The `VarTypeInfo` a value of a type operates with. Every enumeration
/// operates as a `DINT` (REQ-EN-codegen-003); a subrange operates as its base
/// type; a reference is a 64-bit address, as a reference field is (see
/// `compile_struct::var_type_info_for_field`).
fn operand_type_info(representation: &IntermediateType) -> Option<VarTypeInfo> {
    match representation {
        IntermediateType::Enumeration { .. } => Some(crate::compile_enum::enum_var_type_info()),
        IntermediateType::Subrange { base_type, .. } => var_type_info(base_type),
        IntermediateType::Reference { .. } => Some(reference_type_info()),
        IntermediateType::Bool
        | IntermediateType::Int { .. }
        | IntermediateType::UInt { .. }
        | IntermediateType::Real { .. }
        | IntermediateType::Bytes { .. }
        | IntermediateType::Time { .. }
        | IntermediateType::Date { .. }
        | IntermediateType::TimeOfDay { .. }
        | IntermediateType::DateAndTime { .. } => var_type_info(representation),
        // Not operated on as a single value: a string lives in the data
        // region, and an aggregate or a POU is never an operand.
        IntermediateType::String { .. }
        | IntermediateType::Structure { .. }
        | IntermediateType::Array { .. }
        | IntermediateType::FunctionBlock { .. }
        | IntermediateType::Function { .. } => None,
    }
}

/// The `VarTypeInfo` of a reference: a 64-bit address.
fn reference_type_info() -> VarTypeInfo {
    VarTypeInfo {
        op_width: OpWidth::W64,
        signedness: Signedness::Unsigned,
        storage_bits: 64,
    }
}

/// The `VarTypeInfo` an untyped literal of a generic category operates with
/// when nothing narrows it: an integer literal as a `DINT`, a real literal as
/// a `REAL`. Generic types reach codegen for expressions like `5 + 5` where no
/// concrete type context was available during type resolution.
fn literal_type_info(generic: &GenericTypeName) -> Option<VarTypeInfo> {
    let elementary = match generic {
        GenericTypeName::AnyInt | GenericTypeName::AnyNum | GenericTypeName::AnyMagnitude => {
            ElementaryTypeName::DINT
        }
        GenericTypeName::AnyReal => ElementaryTypeName::REAL,
        // No untyped literal has one of these categories, so none has a
        // default to operate at.
        GenericTypeName::Any
        | GenericTypeName::AnyDerived
        | GenericTypeName::AnyElementary
        | GenericTypeName::AnyBit
        | GenericTypeName::AnyString
        | GenericTypeName::AnyDate => return None,
    };
    var_type_info(ironplc_analyzer::elementary_type(&elementary.into())?)
}

/// Maps an IEC 61131-3 type name to its `VarTypeInfo`.
///
/// Returns `None` for unrecognized type names (e.g., user-defined types)
/// and for STRING/WSTRING which are handled separately.
pub(crate) fn resolve_type_name(name: &Id) -> Option<VarTypeInfo> {
    // Try as elementary type first (the common case), then as a generic
    // type naming an untyped literal.
    match ElementaryTypeName::try_from(name) {
        Ok(elementary) => var_type_info(ironplc_analyzer::elementary_type(&elementary.into())?),
        Err(()) => literal_type_info(&GenericTypeName::try_from(name).ok()?),
    }
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
        // BOOL is handled above; the rest are not elementary.
        IntermediateType::Bool
        | IntermediateType::String { .. }
        | IntermediateType::Enumeration { .. }
        | IntermediateType::Structure { .. }
        | IntermediateType::Array { .. }
        | IntermediateType::Subrange { .. }
        | IntermediateType::FunctionBlock { .. }
        | IntermediateType::Function { .. }
        | IntermediateType::Reference { .. } => return None,
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

    /// The operand type of the type `type_name` names in `source`.
    fn operand_type_info_of(source: &str, type_name: &str) -> Option<VarTypeInfo> {
        let options = ironplc_parser::options::CompilerOptions::default();
        let library =
            ironplc_parser::parse_program(source, &ironplc_dsl::core::FileId::default(), &options)
                .unwrap();
        let (_, context) = ironplc_analyzer::stages::resolve_types(&[&library], &options).unwrap();
        let types = context.types();
        let id = types.id_of(&ironplc_dsl::common::TypeName::from(type_name))?;
        operand_type_info(&type_representations(types)[&id])
    }

    const NAMED_TYPES: &str = "
TYPE
  COLOR : (RED, GREEN);
  SHADE : COLOR;
  BIG_RANGE : ULINT (0..10000000000);
  POINT : STRUCT x : DINT; END_STRUCT;
END_TYPE
PROGRAM main
END_PROGRAM
";

    #[rstest]
    #[case::enumeration("COLOR")]
    #[case::enumeration_alias("SHADE")]
    fn operand_type_info_when_enumeration_then_operates_as_dint(#[case] type_name: &str) {
        let info = operand_type_info_of(NAMED_TYPES, type_name).unwrap();

        assert_eq!(info.op_width, OpWidth::W32);
        assert_eq!(info.signedness, Signedness::Signed);
    }

    #[test]
    fn operand_type_info_when_subrange_then_operates_as_base_type() {
        let info = operand_type_info_of(NAMED_TYPES, "BIG_RANGE").unwrap();

        assert_eq!(info.op_width, OpWidth::W64);
        assert_eq!(info.signedness, Signedness::Unsigned);
        assert_eq!(info.storage_bits, 64);
    }

    #[test]
    fn operand_type_info_when_structure_then_none() {
        assert!(operand_type_info_of(NAMED_TYPES, "POINT").is_none());
    }

    #[test]
    fn operand_type_info_when_reference_then_64_bit_address() {
        let info = operand_type_info(&IntermediateType::Reference {
            target_type: Box::new(IntermediateType::Bool),
        })
        .unwrap();

        assert_eq!(info.op_width, OpWidth::W64);
        assert_eq!(info.signedness, Signedness::Unsigned);
        assert_eq!(info.storage_bits, 64);
    }
}
