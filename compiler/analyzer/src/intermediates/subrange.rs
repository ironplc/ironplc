//! Subrange bounds validation
//!
//! This module handles validating that subrange bounds are within the limits
//! of the base type.

use crate::intermediate_type::IntermediateType;
use crate::type_environment::{TypeAttributes, TypeEnvironment};
use ironplc_dsl::common::*;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::*;
use ironplc_problems::Problem;

/// Result of processing a subrange specification
#[derive(Debug, Clone, PartialEq)]
pub enum IntermediateResult {
    /// Create a new subrange type with the given attributes
    Type(TypeAttributes),
    /// Create an alias to an existing type
    Alias(TypeName),
}

/// Try to create the intermediate type information from the subrange specification.
pub fn try_from(
    node_name: &TypeName,
    spec: &SubrangeSpecificationKind,
    type_environment: &TypeEnvironment,
) -> Result<IntermediateResult, Diagnostic> {
    match spec {
        SubrangeSpecificationKind::Specification(spec) => {
            // Direct subrange specification: MY_RANGE : INT (1..100);
            let base_type_name: TypeName = spec.type_name.clone().into();
            let base_type = type_environment.get(&base_type_name).ok_or_else(|| {
                Diagnostic::problem(
                    Problem::ParentTypeNotDeclared,
                    Label::span(node_name.span(), "Subrange declaration"),
                )
                .with_secondary(Label::span(base_type_name.span(), "Base type"))
            })?;

            // Validate that base type is numeric
            if !base_type.representation.is_numeric() {
                return Err(Diagnostic::problem(
                    Problem::SubrangeBaseTypeNotNumeric,
                    Label::span(node_name.span(), "Subrange declaration"),
                )
                .with_secondary(Label::span(base_type_name.span(), "Non-numeric base type")));
            }

            // Extract min and max values from the subrange
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
                    Label::span(node_name.span(), "Subrange declaration"),
                )
                .with_secondary(Label::span(
                    spec.subrange.start.value.span(),
                    format!("Minimum value: {}", min_value),
                ))
                .with_secondary(Label::span(
                    spec.subrange.end.value.span(),
                    format!("Maximum value: {}", max_value),
                )));
            }

            // Validate range is within base type bounds
            base_type
                .representation
                .validate_bounds(min_value, max_value, node_name)?;

            Ok(IntermediateResult::Type(TypeAttributes::new(
                node_name.span(),
                IntermediateType::Subrange {
                    base_type: Box::new(base_type.representation.clone()),
                    min_value,
                    max_value,
                },
            )))
        }
        SubrangeSpecificationKind::Type(base_type_name) => {
            // Subrange type alias: MY_RANGE : OTHER_RANGE;
            if type_environment.get(base_type_name).is_none() {
                return Err(Diagnostic::problem(
                    Problem::ParentTypeNotDeclared,
                    Label::span(node_name.span(), "Subrange alias"),
                )
                .with_secondary(Label::span(base_type_name.span(), "Base type")));
            }

            Ok(IntermediateResult::Alias(base_type_name.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intermediate_type::{ByteSized, IntermediateType};
    use crate::type_environment::TypeEnvironmentBuilder;
    use crate::xform_resolve_type_decl_environment::apply;
    use ironplc_dsl::common::TypeName;
    use ironplc_dsl::core::{FileId, SourceSpan};
    use ironplc_parser::options::ParseOptions;

    // Tests for validate_subrange_bounds method
    #[test]
    fn validate_subrange_bounds_with_various_types_then_validates_correctly() {
        // Test SINT bounds
        let sint_type = IntermediateType::Int {
            size: ByteSized::B8,
        };
        assert!(sint_type
            .validate_bounds(-128, 127, &TypeName::from("TEST"))
            .is_ok());
        assert!(sint_type
            .validate_bounds(-129, 127, &TypeName::from("TEST"))
            .is_err());
        assert!(sint_type
            .validate_bounds(-128, 128, &TypeName::from("TEST"))
            .is_err());

        // Test INT bounds
        let int_type = IntermediateType::Int {
            size: ByteSized::B16,
        };
        assert!(int_type
            .validate_bounds(-32768, 32767, &TypeName::from("TEST"))
            .is_ok());
        assert!(int_type
            .validate_bounds(-32769, 32767, &TypeName::from("TEST"))
            .is_err());
        assert!(int_type
            .validate_bounds(-32768, 32768, &TypeName::from("TEST"))
            .is_err());

        // Test DINT bounds
        let dint_type = IntermediateType::Int {
            size: ByteSized::B32,
        };
        assert!(dint_type
            .validate_bounds(-2147483648, 2147483647, &TypeName::from("TEST"))
            .is_ok());

        // Test LINT bounds
        let lint_type = IntermediateType::Int {
            size: ByteSized::B64,
        };
        assert!(lint_type
            .validate_bounds(i64::MIN as i128, i64::MAX as i128, &TypeName::from("TEST"))
            .is_ok());

        // Test USINT bounds
        let usint_type = IntermediateType::UInt {
            size: ByteSized::B8,
        };
        assert!(usint_type
            .validate_bounds(0, 255, &TypeName::from("TEST"))
            .is_ok());
        assert!(usint_type
            .validate_bounds(-1, 255, &TypeName::from("TEST"))
            .is_err());
        assert!(usint_type
            .validate_bounds(0, 256, &TypeName::from("TEST"))
            .is_err());

        // Test UINT bounds
        let uint_type = IntermediateType::UInt {
            size: ByteSized::B16,
        };
        assert!(uint_type
            .validate_bounds(0, 65535, &TypeName::from("TEST"))
            .is_ok());
        assert!(uint_type
            .validate_bounds(-1, 65535, &TypeName::from("TEST"))
            .is_err());
        assert!(uint_type
            .validate_bounds(0, 65536, &TypeName::from("TEST"))
            .is_err());

        // Test UDINT bounds
        let udint_type = IntermediateType::UInt {
            size: ByteSized::B32,
        };
        assert!(udint_type
            .validate_bounds(0, 4294967295, &TypeName::from("TEST"))
            .is_ok());

        // Test ULINT bounds
        let ulint_type = IntermediateType::UInt {
            size: ByteSized::B64,
        };
        assert!(ulint_type
            .validate_bounds(0, u64::MAX as i128, &TypeName::from("TEST"))
            .is_ok());

        // Test nested subrange bounds
        let nested_subrange = IntermediateType::Subrange {
            base_type: Box::new(IntermediateType::Int {
                size: ByteSized::B16,
            }),
            min_value: 10,
            max_value: 100,
        };
        assert!(nested_subrange
            .validate_bounds(20, 80, &TypeName::from("TEST"))
            .is_ok());
        assert!(nested_subrange
            .validate_bounds(5, 80, &TypeName::from("TEST"))
            .is_err());
        assert!(nested_subrange
            .validate_bounds(20, 150, &TypeName::from("TEST"))
            .is_err());
    }

    #[test]
    fn validate_subrange_bounds_with_non_numeric_type_then_error() {
        let string_type = IntermediateType::String { max_len: Some(10) };
        let result = string_type.validate_bounds(0, 10, &TypeName::from("TEST"));
        assert!(result.is_err());
    }

    #[test]
    fn validate_subrange_bounds_with_edge_cases_then_validates_correctly() {
        // Test edge case: min equals max
        let int_type = IntermediateType::Int {
            size: ByteSized::B16,
        };
        assert!(int_type
            .validate_bounds(50, 50, &TypeName::from("TEST"))
            .is_ok());

        // Test edge case: full range
        assert!(int_type
            .validate_bounds(-32768, 32767, &TypeName::from("TEST"))
            .is_ok());

        // Test edge case: single value ranges
        assert!(int_type
            .validate_bounds(0, 0, &TypeName::from("TEST"))
            .is_ok());
        assert!(int_type
            .validate_bounds(-1, -1, &TypeName::from("TEST"))
            .is_ok());
    }

    #[test]
    fn try_from_with_direct_subrange_specification_then_creates_type() {
        let env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();

        // Create a subrange specification: INT (1..100)
        let spec = SubrangeSpecificationKind::Specification(SubrangeSpecification {
            type_name: ElementaryTypeName::INT,
            subrange: Subrange {
                start: SignedInteger {
                    value: Integer {
                        span: SourceSpan::default(),
                        value: 1,
                    },
                    is_neg: false,
                },
                end: SignedInteger {
                    value: Integer {
                        span: SourceSpan::default(),
                        value: 100,
                    },
                    is_neg: false,
                },
            },
        });

        let result = try_from(&TypeName::from("MY_RANGE"), &spec, &env);
        assert!(result.is_ok());

        let attrs = match result.unwrap() {
            IntermediateResult::Type(attrs) => attrs,
            _ => unreachable!("Expected Type result"),
        };
        assert!(attrs.representation.is_subrange());
        if let IntermediateType::Subrange {
            min_value,
            max_value,
            ..
        } = attrs.representation
        {
            assert_eq!(min_value, 1);
            assert_eq!(max_value, 100);
        }
    }

    #[test]
    fn try_from_with_subrange_alias_then_creates_alias() {
        let mut env = TypeEnvironment::new();

        // First create a base subrange type
        env.insert_type(
            &TypeName::from("BASE_RANGE"),
            TypeAttributes::new(
                SourceSpan::default(),
                IntermediateType::Subrange {
                    base_type: Box::new(IntermediateType::Int {
                        size: ByteSized::B16,
                    }),
                    min_value: 1,
                    max_value: 100,
                },
            ),
        )
        .unwrap();

        // Create an alias specification: BASE_RANGE
        let spec = SubrangeSpecificationKind::Type(TypeName::from("BASE_RANGE"));

        let result = try_from(&TypeName::from("ALIAS_RANGE"), &spec, &env);
        assert!(result.is_ok());

        let base_name = match result.unwrap() {
            IntermediateResult::Alias(base_name) => base_name,
            _ => unreachable!("Expected Alias result"),
        };
        assert_eq!(base_name, TypeName::from("BASE_RANGE"));
    }

    #[test]
    fn try_from_with_missing_base_type_then_error() {
        let env = TypeEnvironment::new();

        // Create an alias to a non-existent type
        let spec = SubrangeSpecificationKind::Type(TypeName::from("MISSING_TYPE"));

        let result = try_from(&TypeName::from("ALIAS_RANGE"), &spec, &env);
        assert!(result.is_err());
    }

    #[test]
    fn try_from_with_invalid_range_then_p0004_error() {
        let env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();

        // Create an invalid subrange specification: INT (100..1) - min > max
        let spec = SubrangeSpecificationKind::Specification(SubrangeSpecification {
            type_name: ElementaryTypeName::INT,
            subrange: Subrange {
                start: SignedInteger {
                    value: Integer {
                        span: SourceSpan::default(),
                        value: 100,
                    },
                    is_neg: false,
                },
                end: SignedInteger {
                    value: Integer {
                        span: SourceSpan::default(),
                        value: 1,
                    },
                    is_neg: false,
                },
            },
        });

        let result = try_from(&TypeName::from("INVALID_RANGE"), &spec, &env);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(Problem::SubrangeMinStrictlyLessMax.code(), error.code);
    }

    #[test]
    fn try_from_with_out_of_bounds_range_then_p0044_error() {
        let env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();

        // Create an out-of-bounds subrange specification: SINT (-200..200) - exceeds SINT bounds
        let spec = SubrangeSpecificationKind::Specification(SubrangeSpecification {
            type_name: ElementaryTypeName::SINT,
            subrange: Subrange {
                start: SignedInteger {
                    value: Integer {
                        span: SourceSpan::default(),
                        value: 200,
                    },
                    is_neg: true,
                },
                end: SignedInteger {
                    value: Integer {
                        span: SourceSpan::default(),
                        value: 200,
                    },
                    is_neg: false,
                },
            },
        });

        let result = try_from(&TypeName::from("OUT_OF_BOUNDS"), &spec, &env);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(Problem::SubrangeOutOfBounds.code(), error.code);
    }

    #[test]
    fn try_from_with_missing_base_type_then_p0038_error() {
        let env = TypeEnvironment::new(); // Empty environment without elementary types

        // Create a subrange specification with missing base type
        let spec = SubrangeSpecificationKind::Specification(SubrangeSpecification {
            type_name: ElementaryTypeName::INT,
            subrange: Subrange {
                start: SignedInteger {
                    value: Integer {
                        span: SourceSpan::default(),
                        value: 1,
                    },
                    is_neg: false,
                },
                end: SignedInteger {
                    value: Integer {
                        span: SourceSpan::default(),
                        value: 100,
                    },
                    is_neg: false,
                },
            },
        });

        let result = try_from(&TypeName::from("MY_RANGE"), &spec, &env);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(Problem::ParentTypeNotDeclared.code(), error.code);
    }

    #[test]
    fn try_from_with_non_numeric_base_type_then_perror() {
        let mut env = TypeEnvironment::new();

        // Add a non-numeric type
        env.insert_type(
            &TypeName::from("string"),
            TypeAttributes::new(
                SourceSpan::default(),
                IntermediateType::String { max_len: None },
            ),
        )
        .unwrap();

        // Create a subrange specification with non-numeric base type
        let spec = SubrangeSpecificationKind::Specification(SubrangeSpecification {
            type_name: ElementaryTypeName::STRING,
            subrange: Subrange {
                start: SignedInteger {
                    value: Integer {
                        span: SourceSpan::default(),
                        value: 1,
                    },
                    is_neg: false,
                },
                end: SignedInteger {
                    value: Integer {
                        span: SourceSpan::default(),
                        value: 100,
                    },
                    is_neg: false,
                },
            },
        });

        let result = try_from(&TypeName::from("INVALID_BASE"), &spec, &env);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(Problem::SubrangeBaseTypeNotNumeric.code(), error.code);
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
        let result = apply(input, &mut env);
        assert!(result.is_ok());

        // Check that the subrange types were created
        let my_range_type = env.get(&TypeName::from("MY_RANGE")).unwrap();
        assert!(my_range_type.representation.is_subrange());
        if let IntermediateType::Subrange {
            min_value,
            max_value,
            ..
        } = &my_range_type.representation
        {
            assert_eq!(*min_value, 1);
            assert_eq!(*max_value, 100);
        }

        let small_range_type = env.get(&TypeName::from("SMALL_RANGE")).unwrap();
        assert!(small_range_type.representation.is_subrange());
        if let IntermediateType::Subrange {
            min_value,
            max_value,
            ..
        } = &small_range_type.representation
        {
            assert_eq!(*min_value, -10);
            assert_eq!(*max_value, 10);
        }
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
        let result = apply(input, &mut env);
        assert!(result.is_ok());

        // Check that both types were created
        let base_type = env.get(&TypeName::from("BASE_RANGE")).unwrap();
        let alias_type = env.get(&TypeName::from("ALIAS_RANGE")).unwrap();

        // Both should have the same representation
        assert_eq!(base_type.representation, alias_type.representation);
        assert!(alias_type.representation.is_subrange());
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
        assert_eq!(
            Problem::SubrangeMinStrictlyLessMax.code(),
            error.first().unwrap().code
        );
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

        let error = result.unwrap_err();
        assert_eq!(
            Problem::SubrangeOutOfBounds.code(),
            error.first().unwrap().code
        );
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
        let mut env = TypeEnvironment::new();
        let result = apply(input, &mut env);

        let error = result.unwrap_err();
        assert_eq!(
            Problem::ParentTypeNotDeclared.code(),
            error.first().unwrap().code
        );
    }

    #[test]
    fn apply_when_subrange_memory_size_then_inherits_base_type_size() {
        let program = "
TYPE
SINT_RANGE : SINT (1..10) := 5;
INT_RANGE : INT (1..1000) := 500;
DINT_RANGE : DINT (1..1000000) := 500000;
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
        assert!(result.is_ok());

        // Check memory sizes
        let sint_range = env.get(&TypeName::from("SINT_RANGE")).unwrap();
        assert_eq!(sint_range.representation.size_in_bytes(), 1);

        let int_range = env.get(&TypeName::from("INT_RANGE")).unwrap();
        assert_eq!(int_range.representation.size_in_bytes(), 2);

        let dint_range = env.get(&TypeName::from("DINT_RANGE")).unwrap();
        assert_eq!(dint_range.representation.size_in_bytes(), 4);
    }

    #[test]
    fn apply_when_nested_subrange_aliases_then_creates_all_aliases() {
        let program = "
TYPE
BASE_RANGE : INT (1..100) := 50;
ALIAS1 : BASE_RANGE := 25;
ALIAS2 : ALIAS1 := 75;
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
        assert!(result.is_ok());

        // Check that all types were created with the same representation
        let base_type = env.get(&TypeName::from("BASE_RANGE")).unwrap();
        let alias1_type = env.get(&TypeName::from("ALIAS1")).unwrap();
        let alias2_type = env.get(&TypeName::from("ALIAS2")).unwrap();

        assert_eq!(base_type.representation, alias1_type.representation);
        assert_eq!(base_type.representation, alias2_type.representation);
        assert!(alias2_type.representation.is_subrange());
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
        let result = apply(input, &mut env);
        assert!(result.is_ok());

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
}
