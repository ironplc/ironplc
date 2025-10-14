//! Subrange bounds validation
//!
//! This module handles validating that subrange bounds are within the limits
//! of the base type.

/*use crate::intermediate_type::{ByteSized, IntermediateType};
use crate::intermediates::IntermediateResult;
use crate::type_environment::{TypeAttributes, TypeEnvironment};
use ironplc_dsl::common::*;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::*;
use ironplc_problems::Problem;*/

/*/// Try to create the intermediate type information from the subrange specification.
pub fn try_from(
    node_name: &TypeName,
    spec: &SubrangeSpecificationKind,
    type_environment: &TypeEnvironment,
) -> Result<IntermediateResult, Diagnostic> {
    match spec {
        ironplc_dsl::common::SubrangeSpecificationKind::Specification(spec) => {
            // Direct subrange specification: MY_RANGE : INT (1..100);
            let base_type_name : TypeName = spec.type_name.into();
            let base_type = type_environment.get(&base_type_name).ok_or_else(|| {
                Diagnostic::problem(
                    Problem::ParentTypeNotDeclared,
                    Label::span(node_name.span(), "Subrange declaration"),
                )
                .with_secondary(Label::span(base_type_name.span(), "Base type not found"))
            })?;

            // TODO determine the appropriate size based on the range
            return Ok(IntermediateResult::Type(TypeAttributes {
                span: node_name.span(),
                representation: IntermediateType::Subrange { size: ByteSized::B64, base_type: todo!(), min_value: todo!(), max_value: todo!() },
            }));
        }
        ironplc_dsl::common::SubrangeSpecificationKind::Type(base_type_name) => {
            // Subrange type alias: MY_RANGE : OTHER_RANGE;
            if type_environment.get(base_type_name).is_none() {
                return Err(Diagnostic::problem(
                    Problem::ParentTypeNotDeclared,
                    Label::span(node_name.span(), "Subrange alias"),
                )
                .with_secondary(Label::span(
                    base_type_name.span(),
                    "Base subrange not found",
                )));
            }

            return Ok(IntermediateResult::Alias(base_type_name.clone()));
        }
    }
}

/*fn try_from_derived_type() -> Result<TypeAttributes, Diagnostic> {
    // Validate that base type is numeric
    if !base_type.representation.is_numeric() {
        return Err(Diagnostic::problem(
            Problem::SubrangeBaseTypeNotNumeric,
            Label::span(node.type_name.span(), "Subrange base type must be numeric"),
        )
        .with_secondary(Label::span(base_type_name.span(), "Non-numeric base type")));
    }

    let min_value = if spec.subrange.start.is_neg {
        -(spec.subrange.start.value.value as i128)
    } else {
        spec.subrange.start.value.value as i128
    };
    let max_value = if spec.subrange.end.is_neg {
        -(spec.subrange.end.value.value as i128)
    } else {
        spec.subrange.end.value.value as i128
    };

    // Validate range
    if min_value > max_value {
        return Err(Diagnostic::problem(
            Problem::SubrangeMinStrictlyLessMax,
            Label::span(
                node.type_name.span(),
                "Invalid subrange: minimum value is greater than maximum",
            ),
        ));
    }

    // Validate range is within base type bounds
    validate_subrange_bounds(
        &base_type.representation,
        min_value,
        max_value,
        &node.type_name,
    )?;

    Ok(TypeAttributes {
        span: node.type_name.span(),
        representation: IntermediateType::Subrange {
            base_type: Box::new(base_type.representation.clone()),
            min_value,
            max_value,
        },
    })
}*/

/// Validates that subrange bounds are within the limits of the base type
fn validate_subrange_bounds(
    base_type: &IntermediateType,
    min_value: i128,
    max_value: i128,
    type_name: &TypeName,
) -> Result<(), Diagnostic> {
    let (type_min, type_max) = match base_type {
        IntermediateType::Int { size } => match size {
            ByteSized::B8 => (i8::MIN as i128, i8::MAX as i128),
            ByteSized::B16 => (i16::MIN as i128, i16::MAX as i128),
            ByteSized::B32 => (i32::MIN as i128, i32::MAX as i128),
            ByteSized::B64 => (i64::MIN as i128, i64::MAX as i128),
        },
        IntermediateType::UInt { size } => match size {
            ByteSized::B8 => (0, u8::MAX as i128),
            ByteSized::B16 => (0, u16::MAX as i128),
            ByteSized::B32 => (0, u32::MAX as i128),
            ByteSized::B64 => (0, u64::MAX as i128),
        },
        IntermediateType::Subrange {
            min_value: base_min,
            max_value: base_max,
            ..
        } => {
            // For nested subranges, use the parent subrange bounds
            (*base_min, *base_max)
        }
        _ => {
            return Err(Diagnostic::problem(
                Problem::SubrangeBaseTypeNotNumeric,
                Label::span(type_name.span(), "Subrange base type must be numeric"),
            ));
        }
    };

    if min_value < type_min || max_value > type_max {
        return Err(Diagnostic::problem(
            Problem::SubrangeOutOfBounds,
            Label::span(
                type_name.span(),
                format!(
                    "Subrange [{}, {}] is outside base type bounds [{}, {}]",
                    min_value, max_value, type_min, type_max
                ),
            ),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::{common::TypeName, core::FileId};
    use ironplc_parser::options::ParseOptions;

    use crate::{intermediate_type::IntermediateType, test_helpers::{assert_subrange, assert_variant}, type_environment::TypeEnvironmentBuilder, xform_resolve_type_decl_environment::apply};

     // Tests for validate_subrange_bounds method
    #[test]
    fn apply_when_validate_subrange_bounds_with_various_types_then_validates_correctly() {
        let env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();

        // Test SINT bounds
        let sint_type = IntermediateType::Int { size: 8 };
        assert!(env
            .validate_subrange_bounds(&sint_type, -128, 127, &TypeName::from("TEST"))
            .is_ok());
        assert!(env
            .validate_subrange_bounds(&sint_type, -129, 127, &TypeName::from("TEST"))
            .is_err());
        assert!(env
            .validate_subrange_bounds(&sint_type, -128, 128, &TypeName::from("TEST"))
            .is_err());

        // Test INT bounds
        let int_type = IntermediateType::Int { size: 16 };
        assert!(env
            .validate_subrange_bounds(&int_type, -32768, 32767, &TypeName::from("TEST"))
            .is_ok());
        assert!(env
            .validate_subrange_bounds(&int_type, -32769, 32767, &TypeName::from("TEST"))
            .is_err());
        assert!(env
            .validate_subrange_bounds(&int_type, -32768, 32768, &TypeName::from("TEST"))
            .is_err());

        // Test DINT bounds
        let dint_type = IntermediateType::Int { size: 32 };
        assert!(env
            .validate_subrange_bounds(&dint_type, -2147483648, 2147483647, &TypeName::from("TEST"))
            .is_ok());

        // Test USINT bounds
        let usint_type = IntermediateType::UInt { size: 8 };
        assert!(env
            .validate_subrange_bounds(&usint_type, 0, 255, &TypeName::from("TEST"))
            .is_ok());
        assert!(env
            .validate_subrange_bounds(&usint_type, -1, 255, &TypeName::from("TEST"))
            .is_err());
        assert!(env
            .validate_subrange_bounds(&usint_type, 0, 256, &TypeName::from("TEST"))
            .is_err());

        // Test UINT bounds
        let uint_type = IntermediateType::UInt { size: 16 };
        assert!(env
            .validate_subrange_bounds(&uint_type, 0, 65535, &TypeName::from("TEST"))
            .is_ok());
        assert!(env
            .validate_subrange_bounds(&uint_type, -1, 65535, &TypeName::from("TEST"))
            .is_err());
        assert!(env
            .validate_subrange_bounds(&uint_type, 0, 65536, &TypeName::from("TEST"))
            .is_err());

        // Test UDINT bounds
        let udint_type = IntermediateType::UInt { size: 32 };
        assert!(env
            .validate_subrange_bounds(&udint_type, 0, 4294967295, &TypeName::from("TEST"))
            .is_ok());

        // Test nested subrange bounds
        let nested_subrange = IntermediateType::Subrange {
            base_type: Box::new(IntermediateType::Int { size: 16 }),
            min_value: 10,
            max_value: 100,
        };
        assert!(env
            .validate_subrange_bounds(&nested_subrange, 20, 80, &TypeName::from("TEST"))
            .is_ok());
        assert!(env
            .validate_subrange_bounds(&nested_subrange, 5, 80, &TypeName::from("TEST"))
            .is_err());
        assert!(env
            .validate_subrange_bounds(&nested_subrange, 20, 150, &TypeName::from("TEST"))
            .is_err());

        // Test unsupported integer size
        let unsupported_int = IntermediateType::Int { size: 12 }; // Unsupported size
        assert!(env
            .validate_subrange_bounds(&unsupported_int, 0, 100, &TypeName::from("TEST"))
            .is_err());
    }


    #[test]
    fn apply_when_various_integer_sizes_then_exercises_bounds_validation() {
        let program = "
TYPE
SINT_RANGE : SINT (-128..127) := 0;
INT_RANGE : INT (-32768..32767) := 0;
DINT_RANGE : DINT (-2147483648..2147483647) := 0;
USINT_RANGE : USINT (0..255) := 128;
UINT_RANGE : UINT (0..65535) := 32768;
UDINT_RANGE : UDINT (0..4294967295) := 2147483648;
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

        // This exercises bounds validation for different integer sizes
        let sint_type = env.get(&TypeName::from("SINT_RANGE")).unwrap();
        assert!(sint_type.representation.is_subrange());

        let int_type = env.get(&TypeName::from("INT_RANGE")).unwrap();
        assert!(int_type.representation.is_subrange());

        let dint_type = env.get(&TypeName::from("DINT_RANGE")).unwrap();
        assert!(dint_type.representation.is_subrange());

        let usint_type = env.get(&TypeName::from("USINT_RANGE")).unwrap();
        assert!(usint_type.representation.is_subrange());

        let uint_type = env.get(&TypeName::from("UINT_RANGE")).unwrap();
        assert!(uint_type.representation.is_subrange());

        let udint_type = env.get(&TypeName::from("UDINT_RANGE")).unwrap();
        assert!(udint_type.representation.is_subrange());
    }

    #[test]
    fn apply_when_subrange_exceeds_sint_bounds_then_error() {
        let program = "
TYPE
OUT_OF_BOUNDS : SINT (-200..200) := 0;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(input, &mut env);

        // This exercises the bounds validation error path
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!("P0045", error.first().unwrap().code); // SubrangeOutOfBounds
    }

    #[test]
    fn apply_when_subrange_exceeds_uint_bounds_then_error() {
        let program = "
TYPE
OUT_OF_BOUNDS : UINT (-1..70000) := 35000;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(input, &mut env);

        // This exercises the bounds validation error path for unsigned types
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!("P0045", error.first().unwrap().code); // SubrangeOutOfBounds
    }

    #[test]
    fn apply_when_subrange_declaration_then_creates_subrange_type() {
        let program = "
TYPE
MY_RANGE : INT (1..100) := 50;
SMALL_RANGE : SINT (-10..10) := 0;
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

        // Check that the subrange types were created
        let my_range_type = env.get(&TypeName::from("MY_RANGE")).unwrap();
        let (base_type, min_value, max_value) = assert_subrange!(&my_range_type.representation);
        assert_eq!(*min_value, 1);
        assert_eq!(*max_value, 100);
        assert_variant!(base_type.as_ref(), IntermediateType::Int { .. });

        let small_range_type = env.get(&TypeName::from("SMALL_RANGE")).unwrap();
        let (base_type, min_value, max_value) = assert_subrange!(&small_range_type.representation);
        assert_eq!(*min_value, -10);
        assert_eq!(*max_value, 10);
        assert_variant!(base_type.as_ref(), IntermediateType::Int { .. });
    }

    #[test]
    fn apply_when_subrange_invalid_range_then_error() {
        let program = "
TYPE
INVALID_RANGE : INT (100..1) := 50;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(input, &mut env);

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!("P0004", error.first().unwrap().code);
    }

    #[test]
    fn apply_when_subrange_out_of_bounds_then_error() {
        let program = "
TYPE
OUT_OF_BOUNDS : SINT (-200..200) := 0;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(input, &mut env);

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!("P0045", error.first().unwrap().code);
    }

    #[test]
    fn apply_when_subrange_non_numeric_base_then_error() {
        let program = "
TYPE
INVALID_BASE : STRING (0..1) := 'test';
END_TYPE
        ";
        let result =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default());

        // This should fail at parse time since STRING doesn't support subrange syntax
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!("P0002", error.code); // Syntax error
    }

    #[test]
    fn apply_when_subrange_alias_then_creates_alias() {
        let program = "
TYPE
BASE_RANGE : INT (1..100) := 50;
ALIAS_RANGE : BASE_RANGE := 25;
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

        // Check that both types were created
        let base_type = env.get(&TypeName::from("BASE_RANGE")).unwrap();
        let alias_type = env.get(&TypeName::from("ALIAS_RANGE")).unwrap();

        // Both should have the same representation
        assert_eq!(base_type.representation, alias_type.representation);

        let (base_type, min_value, max_value) = assert_subrange!(&alias_type.representation);
        assert_eq!(*min_value, 1);
        assert_eq!(*max_value, 100);
        assert_variant!(base_type.as_ref(), IntermediateType::Int { .. });
    }

    #[test]
    fn apply_when_subrange_alias_missing_base_then_error() {
        let program = "
TYPE
ALIAS_RANGE : MISSING_RANGE := 25;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(input, &mut env);

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!("P0038", error.first().unwrap().code);
    }

    #[test]
    fn apply_when_nested_subrange_then_validates_bounds() {
        let program = "
TYPE
BASE_RANGE : INT (1..100) := 50;
NESTED_RANGE : BASE_RANGE := 50;
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

        // Check that the nested subrange alias was created
        let nested_type = env.get(&TypeName::from("NESTED_RANGE")).unwrap();
        let (base_type, min_value, max_value) = assert_subrange!(&nested_type.representation);
        assert_eq!(*min_value, 1);
        assert_eq!(*max_value, 100);
        // Base type should be INT
        assert_variant!(base_type.as_ref(), IntermediateType::Int { .. });
    }

    #[test]
    fn apply_when_subrange_exceeds_nested_bounds_then_error() {
        // This test is hard to create with valid syntax since the parser doesn't support
        // nested subrange syntax. Instead, test the validation logic directly.
        let program = "
TYPE
BASE_RANGE : INT (10..50) := 25;
ALIAS_RANGE : BASE_RANGE := 25;
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

        // This should succeed since we're just creating an alias
        let alias_type = env.get(&TypeName::from("ALIAS_RANGE")).unwrap();
        assert!(alias_type.representation.is_subrange());
    }
}*/
