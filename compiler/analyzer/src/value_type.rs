//! What the type checks compare about a value: the type an expression's
//! value has, and whether it may be used where a given type is required.
//!
//! [`type_compat::are_types_compatible`] states the relation between two
//! type *names*: exact matches, the generic categories, literal inference and
//! implicit widening. That relation is about scalars. A value whose type is
//! not a scalar -- a whole array, a structure, an enumeration, a function
//! block instance -- is only ever the type it is, and it may have no name at
//! all (`a : ARRAY[1..2] OF DINT`). This module classifies a value by its
//! [`ExprType`] so each check asks one question of it, [`check`], and a
//! composite value reaching a scalar position is a mismatch rather than
//! something to skip.
//!
//! ```ignore
//! VAR a : ARRAY[1..2] OF DINT; n : DINT; END_VAR
//!     n := LEN(a);   (* P4026: ANY_STRING is not ARRAY[1..2] OF DINT *)
//!     n := a;        (* P4035 *)
//! ```

use ironplc_dsl::common::{GenericTypeName, TypeName};
use ironplc_dsl::textual::{Expr, ExprType};
use ironplc_dsl::type_id::TypeId;
use ironplc_parser::options::CompilerOptions;

use crate::intermediate_type::IntermediateType;
use crate::type_compat::are_types_compatible;
use crate::type_environment::TypeEnvironment;

/// The type of a value, as the checks compare it.
#[derive(Debug, PartialEq)]
pub(crate) enum ValueType {
    /// A scalar, named as [`are_types_compatible`] understands it: an
    /// elementary type or a user-defined one it compares by name, a generic
    /// category for an untyped literal, or the base type of a subrange.
    Scalar(TypeName),
    /// A value that is not a scalar: a whole array, a structure, an
    /// enumeration or a function block instance.
    Composite(TypeId),
}

/// A value used where its type is not accepted.
#[derive(Debug, PartialEq)]
pub(crate) struct Mismatch {
    /// The value's type as a diagnostic shows it.
    pub(crate) actual: String,
}

/// The name the name-based type relations -- [`are_types_compatible`] and
/// the arithmetic overloads -- know a value of type `expr_type` by.
///
/// An expression's type is its [`ExprType`]; this name is derived from it
/// each time it is asked for, so the two cannot disagree:
///
/// - an untyped literal is its generic category (`ANY_INT`);
/// - an elementary type, or an alias of one, is the elementary type's name;
/// - a string is `STRING` or `WSTRING`, sized or not;
/// - a reference is known by the type it references, as a `REF_TO`
///   parameter records its type;
/// - a subrange is its own name, or its base type's when it has none;
/// - any other type is its own name, and has none when it is anonymous.
///
/// `None` for `NULL`, which is not of one type.
pub fn operand_type_name(types: &TypeEnvironment, expr_type: &ExprType) -> Option<TypeName> {
    match expr_type {
        ExprType::Literal(generic) => Some(generic.clone().into()),
        ExprType::Null => None,
        ExprType::Concrete(id) => operand_name_of(types, *id),
    }
}

fn operand_name_of(types: &TypeEnvironment, id: TypeId) -> Option<TypeName> {
    let representation = &types.get_by_id(id)?.representation;
    match representation {
        IntermediateType::Bool
        | IntermediateType::Int { .. }
        | IntermediateType::UInt { .. }
        | IntermediateType::Real { .. }
        | IntermediateType::Bytes { .. }
        | IntermediateType::Time { .. }
        | IntermediateType::Date { .. }
        | IntermediateType::TimeOfDay { .. }
        | IntermediateType::DateAndTime { .. } => types.elementary_type_name_for(representation),
        IntermediateType::String { char_width, .. } => {
            Some(TypeName::from(if char_width.is_wide() {
                "wstring"
            } else {
                "string"
            }))
        }
        IntermediateType::Reference { .. } => operand_name_of(types, types.referenced_type(id)?),
        // A named subrange is known by its name; one spelled out in place
        // (`x : INT(-100..100)`) by its base type's.
        IntermediateType::Subrange { base_type, .. } => types
            .name_of(id)
            .cloned()
            .or_else(|| types.elementary_type_name_for(base_type)),
        IntermediateType::Enumeration { .. }
        | IntermediateType::Structure { .. }
        | IntermediateType::Array { .. }
        | IntermediateType::FunctionBlock { .. }
        | IntermediateType::Function { .. } => types.name_of(id).cloned(),
    }
}

/// Classifies the value of `expr`, or `None` when the analyzer resolved no
/// type for it.
pub(crate) fn of(types: &TypeEnvironment, expr: &Expr) -> Option<ValueType> {
    let expr_type = expr.expr_type.as_ref()?;
    let by_name = || operand_type_name(types, expr_type).map(ValueType::Scalar);
    let id = match expr_type {
        ExprType::Concrete(id) => id,
        // `NULL` is accepted for any reference, so there is nothing to
        // compare it with.
        ExprType::Null => return None,
        ExprType::Literal(_) => return by_name(),
    };
    let Some(attributes) = types.get_by_id(*id) else {
        return by_name();
    };
    match &attributes.representation {
        IntermediateType::Array { .. }
        | IntermediateType::Structure { .. }
        | IntermediateType::Enumeration { .. }
        | IntermediateType::FunctionBlock { .. } => Some(ValueType::Composite(*id)),
        IntermediateType::Subrange { base_type, .. } => types
            .elementary_type_name_for(base_type)
            .map(ValueType::Scalar)
            .or_else(by_name),
        // Compared by name (see `operand_type_name`): an elementary type by
        // its own name, a string by `STRING` or `WSTRING`, a reference by
        // the name of the type it references. A function is not the type of
        // any value, so it never gets here with a name that matters.
        IntermediateType::Bool
        | IntermediateType::Int { .. }
        | IntermediateType::UInt { .. }
        | IntermediateType::Real { .. }
        | IntermediateType::Bytes { .. }
        | IntermediateType::Time { .. }
        | IntermediateType::Date { .. }
        | IntermediateType::TimeOfDay { .. }
        | IntermediateType::DateAndTime { .. }
        | IntermediateType::String { .. }
        | IntermediateType::Reference { .. }
        | IntermediateType::Function { .. } => by_name(),
    }
}

/// Checks that the value of `expr` may be used where `expected` is
/// required. `Ok` when it may, and when the analyzer resolved no type for
/// the value, since there is nothing to compare.
pub(crate) fn check(
    types: &TypeEnvironment,
    expected: &TypeName,
    expr: &Expr,
    options: &CompilerOptions,
) -> Result<(), Mismatch> {
    if let Some(ExprType::Concrete(id)) = &expr.expr_type {
        if types.id_of(expected) == Some(*id) {
            return Ok(());
        }
    }
    match of(types, expr) {
        None => Ok(()),
        Some(ValueType::Scalar(actual)) => {
            if are_types_compatible(expected, &actual, options) {
                Ok(())
            } else {
                Err(Mismatch {
                    actual: actual.to_string(),
                })
            }
        }
        Some(ValueType::Composite(id)) => {
            if composite_accepted(types, expected, id) {
                Ok(())
            } else {
                Err(Mismatch {
                    actual: describe(types, id),
                })
            }
        }
    }
}

/// Whether a composite value of type `id` is accepted where `expected` is
/// required, when `expected` is not `id` itself.
///
/// An array or a structure is accepted where an array or structure of the
/// same shape is, as a whole-aggregate assignment is (P2037): an inline
/// `ARRAY[1..2] OF DINT` passes for a parameter declared with a named array
/// type of that shape. An enumeration or a function block instance is only
/// ever its own type. No composite is accepted for a generic category.
fn composite_accepted(types: &TypeEnvironment, expected: &TypeName, id: TypeId) -> bool {
    if GenericTypeName::try_from(&expected.name).is_ok() {
        return false;
    }
    let (Some(expected), Some(actual)) = (types.get(expected), types.get_by_id(id)) else {
        return false;
    };
    matches!(
        actual.representation,
        IntermediateType::Array { .. } | IntermediateType::Structure { .. }
    ) && expected.representation == actual.representation
}

/// The type `id` identifies, as a diagnostic shows it: its name when it
/// has one, else its shape (`ARRAY[1..2] OF DINT`).
pub(crate) fn describe(types: &TypeEnvironment, id: TypeId) -> String {
    if let Some(name) = types.name_of(id) {
        return name.to_string();
    }
    match types.get_by_id(id) {
        Some(attributes) => describe_representation(types, &attributes.representation),
        None => "an unknown type".to_string(),
    }
}

fn describe_representation(types: &TypeEnvironment, representation: &IntermediateType) -> String {
    if let Some(name) = types.elementary_type_name_for(representation) {
        return name.to_string().to_uppercase();
    }
    match representation {
        IntermediateType::Array {
            element_type,
            dimensions,
        } => {
            let element = describe_representation(types, element_type);
            if dimensions.is_empty() {
                return format!("ARRAY OF {element}");
            }
            let bounds: Vec<String> = dimensions
                .iter()
                .map(|dimension| format!("{}..{}", dimension.lower, dimension.upper))
                .collect();
            format!("ARRAY[{}] OF {element}", bounds.join(", "))
        }
        IntermediateType::String {
            max_len,
            char_width,
        } => {
            let keyword = if char_width.is_wide() {
                "WSTRING"
            } else {
                "STRING"
            };
            match max_len {
                Some(len) => format!("{keyword}[{len}]"),
                None => keyword.to_string(),
            }
        }
        IntermediateType::Structure { .. } => "a structure".to_string(),
        IntermediateType::Enumeration { .. } => "an enumeration".to_string(),
        IntermediateType::FunctionBlock { name, .. } => name.clone(),
        IntermediateType::Reference { target_type } => {
            format!("REF_TO {}", describe_representation(types, target_type))
        }
        IntermediateType::Subrange { base_type, .. } => {
            format!(
                "a subrange of {}",
                describe_representation(types, base_type)
            )
        }
        IntermediateType::Function { .. } => "a function".to_string(),
        // Every elementary representation is named above; these only reach
        // here in a representation the elementary table does not list.
        IntermediateType::Bool
        | IntermediateType::Int { .. }
        | IntermediateType::UInt { .. }
        | IntermediateType::Real { .. }
        | IntermediateType::Bytes { .. }
        | IntermediateType::Time { .. }
        | IntermediateType::Date { .. }
        | IntermediateType::TimeOfDay { .. }
        | IntermediateType::DateAndTime { .. } => "an elementary type".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intermediate_type::{ArrayDimension, ByteSized};
    use crate::type_attributes::TypeAttributes;
    use crate::type_environment::TypeEnvironmentBuilder;
    use ironplc_container::CharWidth;
    use ironplc_dsl::core::SourceSpan;

    fn anonymous(types: &mut TypeEnvironment, representation: IntermediateType) -> TypeId {
        types.insert_anonymous(TypeAttributes::new(SourceSpan::default(), representation))
    }

    fn environment() -> TypeEnvironment {
        TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap()
    }

    #[test]
    fn describe_when_named_type_then_name() {
        let types = environment();
        let dint = types.id_of(&TypeName::from("DINT")).unwrap();

        assert_eq!(describe(&types, dint), "dint");
    }

    #[test]
    fn describe_when_anonymous_array_then_bounds_and_element() {
        let mut types = environment();
        let id = anonymous(
            &mut types,
            IntermediateType::Array {
                element_type: Box::new(IntermediateType::Int {
                    size: ByteSized::B32,
                }),
                dimensions: vec![
                    ArrayDimension { lower: 1, upper: 2 },
                    ArrayDimension { lower: 0, upper: 3 },
                ],
            },
        );

        assert_eq!(describe(&types, id), "ARRAY[1..2, 0..3] OF DINT");
    }

    #[test]
    fn describe_when_array_of_sized_strings_then_string_length() {
        let mut types = environment();
        let id = anonymous(
            &mut types,
            IntermediateType::Array {
                element_type: Box::new(IntermediateType::String {
                    max_len: Some(8),
                    char_width: CharWidth::Wide,
                }),
                dimensions: vec![ArrayDimension { lower: 1, upper: 2 }],
            },
        );

        assert_eq!(describe(&types, id), "ARRAY[1..2] OF WSTRING[8]");
    }

    #[test]
    fn describe_when_anonymous_enumeration_then_kind() {
        let mut types = environment();
        let id = anonymous(
            &mut types,
            IntermediateType::Enumeration {
                underlying_type: Box::new(IntermediateType::Int {
                    size: ByteSized::B8,
                }),
            },
        );

        assert_eq!(describe(&types, id), "an enumeration");
    }

    #[test]
    fn describe_when_id_unknown_then_unknown_type() {
        let types = environment();

        assert_eq!(
            describe(&types, TypeId::from_raw(100_000)),
            "an unknown type"
        );
    }
}
