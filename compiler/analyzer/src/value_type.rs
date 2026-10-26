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

use crate::semantic_type::{ParameterType, SemanticType};
use crate::type_compat::are_types_compatible;
use crate::type_environment::TypeEnvironment;
use crate::type_id::elementary_debug_tag;

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
/// - a reference is known by the type it references, behind `REF_TO`
///   ([`reference_name`]): `REF_TO INT`, so that it is neither a value of
///   that type nor a reference to another;
/// - a subrange is its base type's name, as it is operated at its base type:
///   `s + l` on a subrange of `INT` and an `LINT` adds as `LINT`;
/// - any other type is its own name, and has none when it is anonymous.
///
/// `None` for `NULL`, which is not of one type.
pub fn operand_type_name(types: &TypeEnvironment, expr_type: &ExprType) -> Option<TypeName> {
    match expr_type {
        ExprType::Literal(generic) => Some(generic.clone().into()),
        ExprType::Null => None,
        ExprType::Concrete(id) | ExprType::Inferred(id) => operand_name_of(types, *id),
    }
}

fn operand_name_of(types: &TypeEnvironment, id: TypeId) -> Option<TypeName> {
    let representation = &types.get_by_id(id)?.representation;
    match representation {
        SemanticType::Bool
        | SemanticType::Int { .. }
        | SemanticType::UInt { .. }
        | SemanticType::Real { .. }
        | SemanticType::Bytes { .. }
        | SemanticType::Time { .. }
        | SemanticType::Date { .. }
        | SemanticType::TimeOfDay { .. }
        | SemanticType::DateAndTime { .. } => types.elementary_type_name_for(representation),
        SemanticType::String { char_width, .. } => Some(TypeName::from(if char_width.is_wide() {
            "wstring"
        } else {
            "string"
        })),
        SemanticType::Reference { .. } => {
            operand_name_of(types, types.referenced_type(id)?).map(|name| reference_name(&name))
        }
        SemanticType::Subrange { .. } => {
            types.elementary_type_name_for(representation.operated_as())
        }
        SemanticType::Enumeration { .. }
        | SemanticType::Structure { .. }
        | SemanticType::Array { .. }
        | SemanticType::FunctionBlock { .. }
        | SemanticType::Interface { .. } => types.name_of(id).cloned(),
    }
}

/// The name a reference to a value known as `referenced` is known by:
/// `REF_TO INT` for a reference to an `INT`.
///
/// A reference used to be known by the referenced type's own name, so a
/// `REF_TO REAL` was taken for a `REAL` by everything that asks for an
/// operand's name: the code generator read the variable index it holds as
/// float bits, and a `REF_TO INT` assigned to an `INT` passed as an `INT`.
pub(crate) fn reference_name(referenced: &TypeName) -> TypeName {
    TypeName::from(format!("REF_TO {referenced}").as_str())
}

/// The name the value of a parameter of type `param_type` is known by, as
/// [`operand_type_name`] knows an argument's: a subrange by its base type, a
/// reference by [`reference_name`], any other type by its own name, a
/// generic category by its own and a type that does not resolve by the name
/// the declaration wrote. `None` for a type with no name: an anonymous type,
/// or a reference to one.
///
/// A subrange parameter so takes a value of its base type, as a subrange
/// variable does when assigned: `f(s + 1)` on a subrange of `INT` passes the
/// `INT` sum.
pub(crate) fn parameter_type_name(
    types: &TypeEnvironment,
    param_type: &ParameterType,
) -> Option<TypeName> {
    match param_type {
        ParameterType::Generic(generic) => Some(generic.clone().into()),
        ParameterType::Unresolved(name) => Some(name.clone()),
        ParameterType::Type(id) => match &types.get_by_id(*id)?.representation {
            SemanticType::Subrange { .. } | SemanticType::Reference { .. } => {
                operand_name_of(types, *id)
            }
            _ => types.name_of(*id).cloned(),
        },
    }
}

/// The type of a parameter of type `param_type`, as a diagnostic shows the
/// type an argument is expected to have: the name the checks know it by
/// ([`parameter_type_name`]), with a type the declaration names spelled as
/// Structured Text writes it ([`spelling`]), and an anonymous type by its
/// shape.
pub(crate) fn describe_parameter(types: &TypeEnvironment, param_type: &ParameterType) -> String {
    let ParameterType::Type(id) = param_type else {
        return parameter_type_name(types, param_type)
            .map(|name| name.to_string())
            .unwrap_or_default();
    };
    let operated_as = types.get_by_id(*id).is_some_and(|attributes| {
        matches!(
            attributes.representation,
            SemanticType::Subrange { .. } | SemanticType::Reference { .. }
        )
    });
    match parameter_type_name(types, param_type) {
        Some(name) if operated_as => name.to_string(),
        _ => spelling(types, *id),
    }
}

/// Checks that the value of `expr` may be passed to a parameter of type
/// `param_type`, as [`check`] does for a type that has a name
/// ([`parameter_type_name`]).
///
/// A type with no name accepts a value of its own type, and a value of the
/// same shape: an array or a structure of the same shape, or a reference to
/// one. `Ok` when the analyzer resolved no type for the value, or the value
/// is `NULL`.
pub(crate) fn check_parameter(
    types: &TypeEnvironment,
    param_type: &ParameterType,
    expr: &Expr,
    options: &CompilerOptions,
) -> Result<(), Mismatch> {
    if let Some(expected) = parameter_type_name(types, param_type) {
        return check(types, &expected, expr, options);
    }
    let ParameterType::Type(expected) = param_type else {
        return Ok(());
    };
    let Some(expr_type) = expr.expr_type.as_ref() else {
        return Ok(());
    };
    let accepted = match expr_type {
        ExprType::Null => true,
        ExprType::Literal(_) => false,
        ExprType::Concrete(own) | ExprType::Inferred(own) => same_shape(types, *expected, *own),
    };
    if accepted {
        return Ok(());
    }
    let actual = match (operand_type_name(types, expr_type), expr_type.type_id()) {
        (Some(name), _) => name.to_string(),
        (None, Some(id)) => describe(types, id),
        (None, None) => "an unknown type".to_string(),
    };
    Err(Mismatch { actual })
}

/// Whether a value of type `actual` is accepted where the type `expected`,
/// which has no name, is required: it is that type, an array or structure
/// of its shape, or, for a reference, a reference to a type so accepted.
fn same_shape(types: &TypeEnvironment, expected: TypeId, actual: TypeId) -> bool {
    if expected == actual {
        return true;
    }
    let (Some(expected_type), Some(actual_type)) =
        (types.get_by_id(expected), types.get_by_id(actual))
    else {
        return false;
    };
    match (&expected_type.representation, &actual_type.representation) {
        (SemanticType::Reference { .. }, SemanticType::Reference { .. }) => {
            match (
                types.referenced_type(expected),
                types.referenced_type(actual),
            ) {
                (Some(expected), Some(actual)) => same_shape(types, expected, actual),
                _ => false,
            }
        }
        (SemanticType::Array { .. }, SemanticType::Array { .. })
        | (SemanticType::Structure { .. }, SemanticType::Structure { .. }) => {
            expected_type.representation == actual_type.representation
        }
        _ => false,
    }
}

/// Classifies the value of `expr`, or `None` when the analyzer resolved no
/// type for it.
pub(crate) fn of(types: &TypeEnvironment, expr: &Expr) -> Option<ValueType> {
    let expr_type = expr.expr_type.as_ref()?;
    let by_name = || operand_type_name(types, expr_type).map(ValueType::Scalar);
    let id = match expr_type {
        ExprType::Concrete(id) | ExprType::Inferred(id) => id,
        // `NULL` is accepted for any reference, so there is nothing to
        // compare it with.
        ExprType::Null => return None,
        ExprType::Literal(_) => return by_name(),
    };
    let Some(attributes) = types.get_by_id(*id) else {
        return by_name();
    };
    match &attributes.representation {
        SemanticType::Array { .. }
        | SemanticType::Structure { .. }
        | SemanticType::Enumeration { .. }
        | SemanticType::FunctionBlock { .. }
        | SemanticType::Interface { .. } => Some(ValueType::Composite(*id)),
        SemanticType::Subrange { base_type, .. } => types
            .elementary_type_name_for(base_type)
            .map(ValueType::Scalar)
            .or_else(by_name),
        // Compared by name (see `operand_type_name`): an elementary type by
        // its own name, a string by `STRING` or `WSTRING`, a reference by
        // the name of the type it references. A function is not the type of
        // any value, so it never gets here with a name that matters.
        SemanticType::Bool
        | SemanticType::Int { .. }
        | SemanticType::UInt { .. }
        | SemanticType::Real { .. }
        | SemanticType::Bytes { .. }
        | SemanticType::Time { .. }
        | SemanticType::Date { .. }
        | SemanticType::TimeOfDay { .. }
        | SemanticType::DateAndTime { .. }
        | SemanticType::String { .. }
        | SemanticType::Reference { .. } => by_name(),
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
    if let Some(id) = expr.expr_type.as_ref().and_then(ExprType::type_id) {
        if types.id_of(expected) == Some(id) {
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
/// type of that shape. A function block instance or an interface value is
/// accepted where an interface it converts to is (see [`crate::supertypes`]).
/// An enumeration is only ever its own type. Every composite is accepted for
/// `ANY`, the category of every data type (`SIZEOF(a)`), and none for a
/// narrower generic category such as `ANY_NUM`.
fn composite_accepted(types: &TypeEnvironment, expected: &TypeName, id: TypeId) -> bool {
    match GenericTypeName::try_from(&expected.id) {
        Ok(GenericTypeName::Any) => return true,
        Ok(_) => return false,
        Err(_) => {}
    }
    let (Some(expected_type), Some(actual)) = (types.get(expected), types.get_by_id(id)) else {
        return false;
    };
    match &actual.representation {
        SemanticType::Array { .. } | SemanticType::Structure { .. } => {
            expected_type.representation == actual.representation
        }
        SemanticType::FunctionBlock { .. } | SemanticType::Interface { .. } => {
            expected_type.representation.is_interface()
                && types
                    .name_of(id)
                    .is_some_and(|actual| types.supertypes().is_subtype_of(actual, expected))
        }
        _ => false,
    }
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

/// The type `id` identifies, as Structured Text writes it: a type named by
/// an elementary type in upper case (`DINT`, and `STRING` for a
/// `STRING[10]`), any other named type by its declared name, and an
/// anonymous type by its shape, as [`describe`] gives it.
///
/// [`describe`] names an elementary type as the environment registered it
/// (`dint`), which is how diagnostics show it. This is for output read as
/// source, such as the annotated rendering of `ironplcc echo --types`.
pub fn spelling(types: &TypeEnvironment, id: TypeId) -> String {
    let described = describe(types, id);
    let named_by_elementary = types
        .name_of(id)
        .and_then(|name| types.id_of(name))
        .is_some_and(|named| elementary_debug_tag(named).is_some());
    if named_by_elementary {
        return described.to_uppercase();
    }
    described
}

fn describe_representation(types: &TypeEnvironment, representation: &SemanticType) -> String {
    if let Some(name) = types.elementary_type_name_for(representation) {
        return name.to_string().to_uppercase();
    }
    match representation {
        SemanticType::Array {
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
        SemanticType::String {
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
        SemanticType::Structure { .. } => "a structure".to_string(),
        SemanticType::Enumeration { .. } => "an enumeration".to_string(),
        SemanticType::FunctionBlock { name, .. } | SemanticType::Interface { name } => name.clone(),
        SemanticType::Reference { target_type } => {
            format!("REF_TO {}", describe_representation(types, target_type))
        }
        SemanticType::Subrange { base_type, .. } => {
            format!(
                "a subrange of {}",
                describe_representation(types, base_type)
            )
        }
        // Every elementary representation is named above; these only reach
        // here in a representation the elementary table does not list.
        SemanticType::Bool
        | SemanticType::Int { .. }
        | SemanticType::UInt { .. }
        | SemanticType::Real { .. }
        | SemanticType::Bytes { .. }
        | SemanticType::Time { .. }
        | SemanticType::Date { .. }
        | SemanticType::TimeOfDay { .. }
        | SemanticType::DateAndTime { .. } => "an elementary type".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_type::{ArrayDimension, ByteSized};
    use crate::type_attributes::TypeAttributes;
    use crate::type_environment::TypeEnvironmentBuilder;
    use ironplc_container::CharWidth;
    use spec_test_macro::spec_test;

    fn anonymous(types: &mut TypeEnvironment, representation: SemanticType) -> TypeId {
        types.insert_anonymous(TypeAttributes::new(representation))
    }

    fn environment() -> TypeEnvironment {
        TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap()
    }

    #[test]
    fn parameter_type_name_when_generic_or_unresolved_then_name() {
        let types = environment();

        assert_eq!(
            parameter_type_name(&types, &ParameterType::Generic(GenericTypeName::AnyNum)),
            Some(TypeName::from("ANY_NUM"))
        );
        assert_eq!(
            parameter_type_name(&types, &ParameterType::Unresolved(TypeName::from("Nope"))),
            Some(TypeName::from("Nope"))
        );
    }

    #[test]
    fn parameter_type_name_when_anonymous_type_then_none() {
        let mut types = environment();
        let array = anonymous(
            &mut types,
            SemanticType::Array {
                element_type: Box::new(SemanticType::Int {
                    size: ByteSized::B32,
                }),
                dimensions: vec![ArrayDimension { lower: 1, upper: 3 }],
            },
        );

        assert_eq!(
            parameter_type_name(&types, &ParameterType::Type(array)),
            None
        );
        assert_eq!(
            describe_parameter(&types, &ParameterType::Type(array)),
            "ARRAY[1..3] OF DINT"
        );
    }

    #[test]
    fn describe_parameter_when_elementary_then_upper_case_name() {
        let types = environment();
        let dint = types.id_of(&TypeName::from("DINT")).unwrap();

        assert_eq!(
            describe_parameter(&types, &ParameterType::Type(dint)),
            "DINT"
        );
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
            SemanticType::Array {
                element_type: Box::new(SemanticType::Int {
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
            SemanticType::Array {
                element_type: Box::new(SemanticType::String {
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
            SemanticType::Enumeration {
                members: crate::enumeration_members::EnumerationMembers::default(),
                underlying_type: None,
            },
        );

        assert_eq!(describe(&types, id), "an enumeration");
    }

    #[spec_test(REQ_ETR_analyzer_001)]
    fn spelling_when_elementary_named_or_anonymous_then_as_written() {
        let mut types = environment();
        let dint = types.id_of(&TypeName::from("DINT")).unwrap();
        let color = TypeName::from("Color");
        types.insert_type(
            &color,
            TypeAttributes::new(SemanticType::Enumeration {
                members: crate::enumeration_members::EnumerationMembers::default(),
                underlying_type: None,
            }),
        );
        let named = types.id_of(&color).unwrap();
        let array = anonymous(
            &mut types,
            SemanticType::Array {
                element_type: Box::new(SemanticType::Int {
                    size: ByteSized::B32,
                }),
                dimensions: vec![ArrayDimension { lower: 1, upper: 2 }],
            },
        );

        assert_eq!(spelling(&types, dint), "DINT");
        assert_eq!(spelling(&types, named), "Color");
        assert_eq!(spelling(&types, array), "ARRAY[1..2] OF DINT");
    }

    #[test]
    fn spelling_when_sized_string_then_upper_case_elementary() {
        let mut types = environment();
        let string = types.id_of(&TypeName::from("STRING")).unwrap();
        let sized = types.insert_sized_string(
            string,
            TypeAttributes::new(SemanticType::String {
                max_len: Some(20),
                char_width: CharWidth::Narrow,
            }),
        );

        assert_eq!(spelling(&types, sized), "STRING");
    }

    #[test]
    fn spelling_when_reference_then_ref_to_upper_case_target() {
        let mut types = environment();
        let int = types.id_of(&TypeName::from("INT")).unwrap();
        let reference = types.reference_to(int).unwrap();

        assert_eq!(spelling(&types, reference), "REF_TO INT");
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
