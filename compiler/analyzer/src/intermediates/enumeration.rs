use crate::intermediate_type::{ByteSized, IntermediateType};
use crate::type_environment::TypeAttributes;
use ironplc_dsl::common::*;
use ironplc_dsl::diagnostic::*;
use ironplc_problems::Problem;

/// Try to create the intermediate type information for the enumerated
/// values initializer.
///
/// This function determines how many bytes are needed to represent the
/// enumerated values.
pub fn try_from_values(
    enumerated_values: &dyn HasEnumeratedValues,
) -> Result<TypeAttributes, Diagnostic> {
    // Enumeration with values: MY_ENUM : (VAL1, VAL2, VAL3);
    let value_count = enumerated_values.values().len();
    let underlying_type = if value_count <= 256 {
        IntermediateType::Int {
            size: ByteSized::B8,
        }
    } else if value_count <= 65_536 {
        IntermediateType::Int {
            size: ByteSized::B16,
        }
    } else {
        // We could support more than 65k values, but I cannot imagine a reasonable
        // program with that many states. We can change this if we can find such
        // a program, we can enable more states here.
        return Err(Diagnostic::problem(
            Problem::EnumerationTooManyValues,
            Label::span(enumerated_values.values_span(), "Enumeration initializer"),
        ));
    };

    Ok(TypeAttributes {
        span: enumerated_values.values_span(),
        representation: IntermediateType::Enumeration {
            underlying_type: Box::new(underlying_type),
        },
    })
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::{common::TypeName, core::FileId};
    use ironplc_parser::options::ParseOptions;
    use ironplc_problems::Problem;

    use crate::{
        type_environment::TypeEnvironmentBuilder, xform_resolve_type_decl_environment::apply,
    };

    #[test]
    fn apply_when_10_enumeration_values_then_uses_8bit_underlying_type() {
        // Create an enumeration with less than 256 values to test 8-bit underlying type
        let mut values = Vec::new();
        for i in 0..10 {
            values.push(format!("VALUE_{}", i));
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

        let input =
            ironplc_parser::parse_program(&program, &FileId::default(), &ParseOptions::default())
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
            values.push(format!("VALUE_{}", i));
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

        let input =
            ironplc_parser::parse_program(&program, &FileId::default(), &ParseOptions::default())
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
            values.push(format!("VALUE_{}", i));
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

        let input =
            ironplc_parser::parse_program(&program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let errors = apply(input, &mut env).err().unwrap();
        assert_eq!(1, errors.len());
        assert_eq!(
            Problem::EnumerationTooManyValues.code(),
            errors.get(0).unwrap().code
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
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let _library = apply(input, &mut env).unwrap();

        // Check that the enumeration type was created
        let attributes = env.get(&TypeName::from("LEVEL")).unwrap();
        assert!(attributes.representation.is_enumeration());
        assert_eq!(1, attributes.representation.size_in_bytes());
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
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let _library = apply(input, &mut env).unwrap();

        // Check that the enumeration type was created
        let attributes = env.get(&TypeName::from("LEVEL2")).unwrap();
        assert!(attributes.representation.is_enumeration());
        assert_eq!(1, attributes.representation.size_in_bytes());
    }
}
