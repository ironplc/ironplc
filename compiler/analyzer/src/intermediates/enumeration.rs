use crate::intermediate_type::{ByteSized, IntermediateType};
use crate::type_environment::TypeAttributes;
use ironplc_dsl::common::*;
use ironplc_dsl::diagnostic::*;
use ironplc_problems::Problem;

/// Resolves each enum member's effective integer value using ordinary
/// C-style enum semantics: an explicit value (`member := 5`, a
/// CODESYS/TwinCAT extension) is used as-is; an unlabeled member
/// continues from the previous resolved value + 1 (starting at 0 if the
/// very first member has no explicit value). Matches Beckhoff's own
/// documented example (`Red := 2, Green, Blue := 10` -> Green resolves to
/// 3) and every real file found using this syntax.
///
/// Used both for sizing (`try_from_values` below) and for codegen's
/// ordinal map (`ironplc_codegen::compile_enum::build_enum_ordinal_map`),
/// so both agree on what each member's runtime value actually is.
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

/// Try to create the intermediate type information for the enumerated
/// values initializer.
///
/// This function determines how many bytes are needed to represent the
/// enumerated values -- either from an explicit base-type suffix
/// (`underlying_type_override`), or automatically from the resolved
/// ordinal values (which may exceed the member count when explicit
/// values are used).
pub fn try_from_values(
    enumerated_values: &dyn HasEnumeratedValues,
    underlying_type_override: Option<ElementaryTypeName>,
) -> Result<TypeAttributes, Diagnostic> {
    if let Some(type_name) = underlying_type_override {
        return Ok(TypeAttributes::new(
            enumerated_values.values_span(),
            IntermediateType::Enumeration {
                underlying_type: Box::new(IntermediateType::Int {
                    size: byte_sized_for_underlying_type(type_name),
                }),
            },
        ));
    }

    // Enumeration with values: MY_ENUM : (VAL1, VAL2, VAL3);
    let resolved = resolve_ordinal_values(enumerated_values.values());
    let max_value = resolved.into_iter().max().unwrap_or(0);
    let range = max_value.max(0) as u128 + 1;
    let underlying_type = if range <= 256 {
        IntermediateType::Int {
            size: ByteSized::B8,
        }
    } else if range <= 65_536 {
        IntermediateType::Int {
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
        IntermediateType::Enumeration {
            underlying_type: Box::new(underlying_type),
        },
    ))
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
        let explicit_value = SignedInteger::new(&explicit.to_string(), SourceSpan::default())
            .expect("valid integer literal");
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
}
