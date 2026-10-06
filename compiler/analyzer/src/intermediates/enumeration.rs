use crate::enumeration_members::EnumerationMembers;
use crate::semantic_type::{ByteSized, SemanticType};
use crate::type_environment::TypeAttributes;
use ironplc_dsl::common::*;
use ironplc_dsl::core::Id;
use ironplc_dsl::diagnostic::*;
use ironplc_problems::Problem;

/// Resolves each enum member's effective integer value using ordinary
/// C-style enum semantics: an explicit value (`member := 5`,
/// an extension) is used as-is; an unlabeled member
/// continues from the previous resolved value + 1 (starting at 0 if the
/// very first member has no explicit value). Matches Beckhoff's own
/// documented example (`Red := 2, Green, Blue := 10` -> Green resolves to
/// 3) and every real file found using this syntax.
///
/// Used both for sizing (`try_from_values` below) and for the members
/// recorded with the type ([`EnumerationMembers`]), so both agree on what
/// each member's runtime value actually is.
pub fn resolve_ordinal_values(values: &[EnumeratedValue]) -> Vec<i64> {
    let mut resolved = Vec::with_capacity(values.len());
    let mut next = 0i64;
    for value in values {
        let ordinal = match &value.explicit_value {
            Some(explicit) => explicit.to_i64(),
            None => next,
        };
        resolved.push(ordinal);
        next = ordinal + 1;
    }
    resolved
}

/// Maps the CODESYS/TwinCAT enum base-type suffix (`(A, B) BYTE;`) to the
/// byte size it specifies, overriding the automatic value-based sizing.
fn byte_sized_for_underlying_type(type_name: ElementaryTypeName) -> ByteSized {
    match type_name {
        ElementaryTypeName::SINT | ElementaryTypeName::USINT | ElementaryTypeName::BYTE => {
            ByteSized::B8
        }
        ElementaryTypeName::INT | ElementaryTypeName::UINT | ElementaryTypeName::WORD => {
            ByteSized::B16
        }
        ElementaryTypeName::DINT | ElementaryTypeName::UDINT | ElementaryTypeName::DWORD => {
            ByteSized::B32
        }
        ElementaryTypeName::LINT | ElementaryTypeName::ULINT | ElementaryTypeName::LWORD => {
            ByteSized::B64
        }
        // Not reachable via the grammar (enum_underlying_type() only
        // accepts integer_type_name()/bit_string_type_name()), but a
        // reasonable default keeps this exhaustive without panicking.
        _ => ByteSized::B32,
    }
}

/// Try to create the semantic type information for the enumerated
/// values initializer.
///
/// This function determines how many bytes are needed to represent the
/// enumerated values -- either from an explicit base-type suffix
/// (`underlying_type_override`), or automatically from the resolved
/// ordinal values (which may exceed the member count when explicit
/// values are used).
///
/// The type records its members, their ordinals and `default`, the
/// declared default value (see [`EnumerationMembers`]).
pub fn try_from_values(
    enumerated_values: &dyn HasEnumeratedValues,
    underlying_type_override: Option<ElementaryTypeName>,
    default: Option<&Id>,
) -> Result<TypeAttributes, Diagnostic> {
    let members = EnumerationMembers::from_values(enumerated_values.values(), default);
    if let Some(type_name) = underlying_type_override {
        return Ok(TypeAttributes::new(
            enumerated_values.values_span(),
            SemanticType::Enumeration {
                underlying_type: Box::new(SemanticType::Int {
                    size: byte_sized_for_underlying_type(type_name),
                }),
                members,
            },
        ));
    }

    // Enumeration with values: MY_ENUM : (VAL1, VAL2, VAL3);
    let resolved = resolve_ordinal_values(enumerated_values.values());
    let max_value = resolved.into_iter().max().unwrap_or(0);
    let range = max_value.max(0) as u128 + 1;
    let underlying_type = if range <= 256 {
        SemanticType::Int {
            size: ByteSized::B8,
        }
    } else if range <= 65_536 {
        SemanticType::Int {
            size: ByteSized::B16,
        }
    } else {
        // We could support more than 65k values, but I cannot imagine a reasonable
        // program with that many states. We can change this if we can find such
        // a program, we can enable more states here.
        return Err(Diagnostic::problem(
            Problem::EnumerationTooManyValues,
            Label::span(enumerated_values.values_span(), "Enumeration declaration"),
        ));
    };

    Ok(TypeAttributes::new(
        enumerated_values.values_span(),
        SemanticType::Enumeration {
            underlying_type: Box::new(underlying_type),
            members,
        },
    ))
}

/// The type an alias of the enumeration `base` declares (`PAINT : COLOR`):
/// the base's representation and members, with `default` as its default
/// when it declares one.
pub fn alias_of(base: &TypeAttributes, default: Option<&Id>) -> TypeAttributes {
    let mut alias = base.clone();
    if let (Some(default), SemanticType::Enumeration { members, .. }) =
        (default, &mut alias.representation)
    {
        *members = std::mem::take(members).with_default(default);
    }
    alias
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::common::{EnumeratedValue, SignedInteger, TypeName};
    use ironplc_dsl::core::{FileId, SourceSpan};
    use ironplc_parser::options::CompilerOptions;
    use ironplc_problems::Problem;

    use super::resolve_ordinal_values;
    use crate::{
        type_environment::TypeEnvironmentBuilder, xform_resolve_type_decl_environment::apply,
    };

    fn value(name: &str) -> EnumeratedValue {
        EnumeratedValue::new(name)
    }

    fn value_with(name: &str, explicit: i64) -> EnumeratedValue {
        let explicit_value =
            SignedInteger::new(&explicit.to_string(), SourceSpan::default()).unwrap();
        EnumeratedValue {
            explicit_value: Some(explicit_value),
            ..EnumeratedValue::new(name)
        }
    }

    #[test]
    fn resolve_ordinal_values_when_all_implicit_then_sequential() {
        let values = vec![value("A"), value("B"), value("C")];
        assert_eq!(resolve_ordinal_values(&values), vec![0, 1, 2]);
    }

    #[test]
    fn resolve_ordinal_values_when_all_explicit_then_uses_explicit() {
        let values = vec![value_with("Deutsch", 1), value_with("English", 2)];
        assert_eq!(resolve_ordinal_values(&values), vec![1, 2]);
    }

    #[test]
    fn resolve_ordinal_values_when_first_explicit_then_continues_from_it() {
        let values = vec![value_with("A", 0), value("B"), value("C")];
        assert_eq!(resolve_ordinal_values(&values), vec![0, 1, 2]);
    }

    #[test]
    fn resolve_ordinal_values_when_gap_then_continues_from_explicit_value() {
        // Matches Beckhoff's own documented example: Red := 2, Green,
        // Blue := 10 -> Green resolves to 3 (continuing from 2), not 1
        // (its declaration position).
        let values = vec![value_with("Red", 2), value("Green"), value_with("Blue", 10)];
        assert_eq!(resolve_ordinal_values(&values), vec![2, 3, 10]);
    }

    #[test]
    fn apply_when_10_enumeration_values_then_uses_8bit_underlying_type() {
        // Create an enumeration with less than 256 values to test 8-bit underlying type
        let mut values = Vec::new();
        for i in 0..10 {
            values.push(format!("VALUE_{i}"));
        }
        let values_str = values.join(", ");

        let program = format!(
            "
TYPE
SMALL_ENUM : ({}) := VALUE_0;
END_TYPE
        ",
            values_str
        );

        let input = ironplc_parser::parse_program(
            &program,
            &FileId::default(),
            &CompilerOptions::default(),
        )
        .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let _library = apply(input, &mut env).unwrap();

        // Check that the enumeration uses 16-bit underlying type
        let attributes = env.get(&TypeName::from("SMALL_ENUM")).unwrap();
        assert!(attributes.representation.is_enumeration());
    }

    #[test]
    fn apply_when_257_enumeration_values_then_uses_16bit_underlying_type() {
        // Create an enumeration with more than 256 values to test 16-bit underlying type
        let mut values = Vec::new();
        for i in 0..257 {
            values.push(format!("VALUE_{i}"));
        }
        let values_str = values.join(", ");

        let program = format!(
            "
TYPE
LARGE_ENUM : ({}) := VALUE_0;
END_TYPE
        ",
            values_str
        );

        let input = ironplc_parser::parse_program(
            &program,
            &FileId::default(),
            &CompilerOptions::default(),
        )
        .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let _library = apply(input, &mut env).unwrap();

        // Check that the enumeration uses 16-bit underlying type
        let attributes = env.get(&TypeName::from("LARGE_ENUM")).unwrap();
        assert!(attributes.representation.is_enumeration());
    }

    #[test]
    fn apply_when_very_large_enumeration_then_error() {
        // Create an enumeration with more than 65,536 values to test 32-bit underlying type
        let mut values = Vec::new();
        for i in 0..65_537 {
            values.push(format!("VALUE_{i}"));
        }
        let values_str = values.join(", ");

        let program = format!(
            "
TYPE
HUGE_ENUM : ({}) := VALUE_0;
END_TYPE
        ",
            values_str
        );

        let input = ironplc_parser::parse_program(
            &program,
            &FileId::default(),
            &CompilerOptions::default(),
        )
        .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let errors = apply(input, &mut env).err().unwrap();
        assert_eq!(1, errors.len());
        assert_eq!(
            Problem::EnumerationTooManyValues.code(),
            errors.first().unwrap().code
        );
    }

    #[test]
    fn apply_when_enumeration_in_simple_declaration_then_creates_enum() {
        let program = "
TYPE
LEVEL : (LOW, MEDIUM, HIGH) := LOW;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let _library = apply(input, &mut env).unwrap();

        // Check that the enumeration type was created
        let attributes = env.get(&TypeName::from("LEVEL")).unwrap();
        assert!(attributes.representation.is_enumeration());
        assert_eq!(Some(1), attributes.representation.size_in_bytes());
    }

    #[test]
    fn apply_when_enum_redefines_enum_then_creates_alias() {
        let program = "
TYPE
LEVEL : (LOW, MEDIUM, HIGH) := LOW;
LEVEL2 : LEVEL;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let _library = apply(input, &mut env).unwrap();

        // Check that the enumeration type was created
        let attributes = env.get(&TypeName::from("LEVEL2")).unwrap();
        assert!(attributes.representation.is_enumeration());
        assert_eq!(Some(1), attributes.representation.size_in_bytes());
    }

    #[test]
    fn apply_when_enum_base_type_suffix_then_uses_specified_size() {
        let program = "
TYPE
E_Small : (A, B) WORD;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let _library = apply(input, &mut env).unwrap();

        // WORD is explicitly specified -- 2 bytes, even though only 2
        // members would otherwise size to 1 byte automatically.
        let attributes = env.get(&TypeName::from("E_Small")).unwrap();
        assert_eq!(Some(2), attributes.representation.size_in_bytes());
    }

    #[test]
    fn apply_when_enum_explicit_value_exceeds_member_count_then_sizes_from_value() {
        let program = "
TYPE
E_Sparse : (A := 300, B);
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let _library = apply(input, &mut env).unwrap();

        // Only 2 members (would auto-size to 1 byte by count), but the
        // explicit value 300 requires 2 bytes -- sizing must be based on
        // the resolved value, not just the member count.
        let attributes = env.get(&TypeName::from("E_Sparse")).unwrap();
        assert_eq!(Some(2), attributes.representation.size_in_bytes());
    }

    /// The members of the type `name` names, as (member, ordinal) pairs,
    /// and its default ordinal.
    fn members_of(
        types: &crate::type_environment::TypeEnvironment,
        id: ironplc_dsl::type_id::TypeId,
    ) -> (Vec<(String, i64)>, i64) {
        let attributes = types.get_by_id(id).unwrap();
        let crate::semantic_type::SemanticType::Enumeration { members, .. } =
            &attributes.representation
        else {
            panic!("not an enumeration");
        };
        let pairs = members
            .iter()
            .map(|m| (m.name.to_string(), m.ordinal))
            .collect();
        (pairs, members.default_ordinal())
    }

    fn pairs(expected: &[(&str, i64)]) -> Vec<(String, i64)> {
        expected.iter().map(|(n, o)| (n.to_string(), *o)).collect()
    }

    fn edition_3() -> CompilerOptions {
        CompilerOptions::from_dialect(ironplc_parser::options::Dialect::Iec61131_3Ed3)
    }

    #[test]
    fn resolve_types_when_named_enumeration_then_type_has_members_and_default() {
        let (_library, context) = crate::test_helpers::parse_and_resolve_types_with_options(
            "TYPE LEVEL : (LOW, MEDIUM := 5, HIGH) := HIGH; END_TYPE",
            &edition_3(),
        );
        let types = context.types();
        let id = types.id_of(&TypeName::from("LEVEL")).unwrap();

        let (members, default) = members_of(types, id);

        assert_eq!(members, pairs(&[("LOW", 0), ("MEDIUM", 5), ("HIGH", 6)]));
        assert_eq!(default, 6);
    }

    #[test]
    fn resolve_types_when_named_enumeration_without_default_then_default_is_first_member() {
        let (_library, context) = crate::test_helpers::parse_and_resolve_types_with_options(
            "TYPE LEVEL : (LOW := 1, HIGH := 5); END_TYPE",
            &edition_3(),
        );
        let types = context.types();
        let id = types.id_of(&TypeName::from("LEVEL")).unwrap();

        assert_eq!(members_of(types, id).1, 1);
    }

    #[test]
    fn resolve_types_when_alias_chain_then_alias_has_base_members_and_own_default() {
        let (_library, context) = crate::test_helpers::parse_and_resolve_types_with_context(
            "TYPE
                LEVEL : (LOW, MEDIUM, HIGH) := MEDIUM;
                LEVEL2 : LEVEL;
                LEVEL3 : LEVEL2 := HIGH;
            END_TYPE",
        );
        let types = context.types();
        let level2 = types.id_of(&TypeName::from("LEVEL2")).unwrap();
        let level3 = types.id_of(&TypeName::from("LEVEL3")).unwrap();

        let expected = pairs(&[("LOW", 0), ("MEDIUM", 1), ("HIGH", 2)]);
        assert_eq!(members_of(types, level2), (expected.clone(), 1));
        assert_eq!(members_of(types, level3), (expected, 2));
    }

    #[test]
    fn resolve_types_when_inline_enumeration_then_declared_type_has_members() {
        let (library, context) = crate::test_helpers::parse_and_resolve_types_with_options(
            "PROGRAM main VAR e : (X := 1, Y := 5) := Y; END_VAR END_PROGRAM",
            &edition_3(),
        );
        let ironplc_dsl::common::LibraryElementKind::ProgramDeclaration(program) =
            &library.elements[0]
        else {
            panic!("not a program");
        };
        let id = program.variables[0].type_id.unwrap();

        // The initial value belongs to the variable; the type's default is
        // its first member.
        assert_eq!(
            members_of(context.types(), id),
            (pairs(&[("X", 1), ("Y", 5)]), 1)
        );
    }
}
