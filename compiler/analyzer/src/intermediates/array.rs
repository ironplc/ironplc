//! Array type processing
//!
//! This module handles creating array types from array specifications,
//! including validation of array bounds and element types.

use crate::intermediate_type::IntermediateType;
use crate::type_environment::{TypeAttributes, TypeEnvironment};
use ironplc_dsl::common::*;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::*;
use ironplc_problems::Problem;

/// Result of processing an array specification
#[derive(Debug, Clone, PartialEq)]
pub enum IntermediateResult {
    /// Create a new array type with the given attributes
    Type(TypeAttributes),
    /// Create an alias to an existing type
    Alias(TypeName),
}

/// Try to create the intermediate type information from the array specification.
pub fn try_from(
    node_name: &TypeName,
    spec: &ArraySpecificationKind,
    type_environment: &TypeEnvironment,
) -> Result<IntermediateResult, Diagnostic> {
    match spec {
        SpecificationKind::Inline(array_subranges) => {
            // Array with explicit subranges: MY_ARRAY : ARRAY [1..10, 1..5] OF INT;
            let element_type_name = &array_subranges.type_name;
            let element_type = type_environment.get(element_type_name).ok_or_else(|| {
                Diagnostic::problem(
                    Problem::ArrayElementTypeNotDeclared,
                    Label::span(node_name.span(), "Array declaration"),
                )
                .with_secondary(Label::span(element_type_name.span(), "Element type"))
            })?;

            // Validate array bounds
            validate_array_bounds(&array_subranges.ranges, node_name)?;

            // Calculate total array size
            let total_size = calculate_array_size(&array_subranges.ranges)?;

            Ok(IntermediateResult::Type(TypeAttributes::new(
                node_name.span(),
                IntermediateType::Array {
                    element_type: Box::new(element_type.representation.clone()),
                    size: Some(total_size),
                },
            )))
        }
        SpecificationKind::Named(base_type_name) => {
            // Array type alias: MY_ARRAY : OTHER_ARRAY;
            if type_environment.get(base_type_name).is_none() {
                return Err(Diagnostic::problem(
                    Problem::ParentTypeNotDeclared,
                    Label::span(node_name.span(), "Array alias"),
                )
                .with_secondary(Label::span(base_type_name.span(), "Base type")));
            }

            Ok(IntermediateResult::Alias(base_type_name.clone()))
        }
    }
}

/// Validates that array bounds are valid (min <= max for each dimension)
pub fn validate_array_bounds(ranges: &[Subrange], type_name: &TypeName) -> Result<(), Diagnostic> {
    if ranges.is_empty() {
        return Err(Diagnostic::problem(
            Problem::ArrayDimensionEmpty,
            Label::span(type_name.span(), "Array declaration"),
        ));
    }

    for range in ranges.iter() {
        let min_value = if range.start.is_neg {
            -(range.start.value.value as i128)
        } else {
            range.start.value.value as i128
        };
        let max_value = if range.end.is_neg {
            -(range.end.value.value as i128)
        } else {
            range.end.value.value as i128
        };

        if min_value > max_value {
            return Err(Diagnostic::problem(
                Problem::ArrayDimensionInvalid,
                Label::span(type_name.span(), "Array declaration"),
            )
            .with_secondary(Label::span(
                range.start.value.span(),
                format!("Minimum value: {}", min_value),
            ))
            .with_secondary(Label::span(
                range.end.value.span(),
                format!("Maximum value: {}", max_value),
            )));
        }

        // Check for reasonable array size limits to prevent overflow
        let dimension_size = (max_value - min_value + 1) as u64;
        if dimension_size > u32::MAX as u64 {
            return Err(Diagnostic::problem(
                Problem::ArraySizeOverflow,
                Label::span(type_name.span(), "Array declaration"),
            )
            .with_secondary(Label::span(range.start.value.span(), "Dimension start"))
            .with_secondary(Label::span(range.end.value.span(), "Dimension end")));
        }
    }

    Ok(())
}

/// Calculates the total size of an array from its subranges
fn calculate_array_size(ranges: &[Subrange]) -> Result<u32, Diagnostic> {
    let mut total_size: u64 = 1;

    for range in ranges {
        let min_value = if range.start.is_neg {
            -(range.start.value.value as i128)
        } else {
            range.start.value.value as i128
        };
        let max_value = if range.end.is_neg {
            -(range.end.value.value as i128)
        } else {
            range.end.value.value as i128
        };

        let dimension_size = (max_value - min_value + 1) as u64;
        total_size = total_size.checked_mul(dimension_size).ok_or_else(|| {
            Diagnostic::problem(
                Problem::ArraySizeOverflow,
                Label::span(range.start.value.span(), "Array dimension"),
            )
        })?;

        // Check if total size exceeds u32::MAX
        if total_size > u32::MAX as u64 {
            return Err(Diagnostic::problem(
                Problem::ArraySizeOverflow,
                Label::span(range.start.value.span(), "Array dimension"),
            ));
        }
    }

    Ok(total_size as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intermediate_type::{ByteSized, IntermediateType};
    use crate::type_environment::TypeEnvironmentBuilder;
    use ironplc_dsl::common::TypeName;
    use ironplc_dsl::core::SourceSpan;

    #[test]
    fn validate_array_bounds_with_valid_ranges_then_succeeds() {
        let ranges = vec![
            Subrange {
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
                        value: 10,
                    },
                    is_neg: false,
                },
            },
            Subrange {
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
                        value: 5,
                    },
                    is_neg: false,
                },
            },
        ];

        let result = validate_array_bounds(&ranges, &TypeName::from("TEST_ARRAY"));
        assert!(result.is_ok());
    }

    #[test]
    fn validate_array_bounds_with_empty_ranges_then_error() {
        let ranges = vec![];
        let result = validate_array_bounds(&ranges, &TypeName::from("TEST_ARRAY"));
        assert!(result.is_err());
    }

    #[test]
    fn validate_array_bounds_with_invalid_range_then_error() {
        let ranges = vec![Subrange {
            start: SignedInteger {
                value: Integer {
                    span: SourceSpan::default(),
                    value: 10,
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
        }];

        let result = validate_array_bounds(&ranges, &TypeName::from("TEST_ARRAY"));
        assert!(result.is_err());
    }

    #[test]
    fn validate_array_bounds_with_negative_ranges_then_succeeds() {
        let ranges = vec![Subrange {
            start: SignedInteger {
                value: Integer {
                    span: SourceSpan::default(),
                    value: 5,
                },
                is_neg: true, // -5
            },
            end: SignedInteger {
                value: Integer {
                    span: SourceSpan::default(),
                    value: 5,
                },
                is_neg: false, // 5
            },
        }];

        let result = validate_array_bounds(&ranges, &TypeName::from("TEST_ARRAY"));
        assert!(result.is_ok());
    }

    #[test]
    fn calculate_array_size_with_single_dimension_then_correct_size() {
        let ranges = vec![Subrange {
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
                    value: 10,
                },
                is_neg: false,
            },
        }];

        let result = calculate_array_size(&ranges);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 10);
    }

    #[test]
    fn calculate_array_size_with_multiple_dimensions_then_correct_size() {
        let ranges = vec![
            Subrange {
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
                        value: 10,
                    },
                    is_neg: false,
                },
            },
            Subrange {
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
                        value: 5,
                    },
                    is_neg: false,
                },
            },
        ];

        let result = calculate_array_size(&ranges);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 50); // 10 * 5
    }

    #[test]
    fn calculate_array_size_with_negative_ranges_then_correct_size() {
        let ranges = vec![Subrange {
            start: SignedInteger {
                value: Integer {
                    span: SourceSpan::default(),
                    value: 5,
                },
                is_neg: true, // -5
            },
            end: SignedInteger {
                value: Integer {
                    span: SourceSpan::default(),
                    value: 5,
                },
                is_neg: false, // 5
            },
        }];

        let result = calculate_array_size(&ranges);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 11); // -5 to 5 inclusive = 11 elements
    }

    #[test]
    fn try_from_with_subranges_specification_then_creates_array_type() {
        let env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();

        let array_subranges = ArraySubranges {
            ranges: vec![Subrange {
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
                        value: 10,
                    },
                    is_neg: false,
                },
            }],
            type_name: TypeName::from("int"),
        };

        let spec = SpecificationKind::Inline(array_subranges);
        let result = try_from(&TypeName::from("MY_ARRAY"), &spec, &env);
        assert!(result.is_ok());

        let attrs = match result.unwrap() {
            IntermediateResult::Type(attrs) => attrs,
            _ => unreachable!("Expected Type result"),
        };

        if let IntermediateType::Array { element_type, size } = attrs.representation {
            assert_eq!(
                *element_type,
                IntermediateType::Int {
                    size: ByteSized::B16
                }
            );
            assert_eq!(size, Some(10));
        } else {
            unreachable!("Expected Array type");
        }
    }

    #[test]
    fn try_from_with_type_alias_then_creates_alias() {
        let mut env = TypeEnvironment::new();

        // First create a base array type
        env.insert_type(
            &TypeName::from("BASE_ARRAY"),
            TypeAttributes::new(
                SourceSpan::default(),
                IntermediateType::Array {
                    element_type: Box::new(IntermediateType::Int {
                        size: ByteSized::B16,
                    }),
                    size: Some(10),
                },
            ),
        )
        .unwrap();

        let spec = SpecificationKind::Named(TypeName::from("BASE_ARRAY"));
        let result = try_from(&TypeName::from("ALIAS_ARRAY"), &spec, &env);
        assert!(result.is_ok());

        let base_name = match result.unwrap() {
            IntermediateResult::Alias(base_name) => base_name,
            _ => unreachable!("Expected Alias result"),
        };
        assert_eq!(base_name, TypeName::from("BASE_ARRAY"));
    }

    #[test]
    fn try_from_with_missing_element_type_then_error() {
        let env = TypeEnvironment::new(); // Empty environment

        let array_subranges = ArraySubranges {
            ranges: vec![Subrange {
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
                        value: 10,
                    },
                    is_neg: false,
                },
            }],
            type_name: TypeName::from("MISSING_TYPE"),
        };

        let spec = SpecificationKind::Inline(array_subranges);
        let result = try_from(&TypeName::from("MY_ARRAY"), &spec, &env);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(Problem::ArrayElementTypeNotDeclared.code(), error.code);
    }

    #[test]
    fn try_from_with_missing_base_array_type_then_error() {
        let env = TypeEnvironment::new(); // Empty environment

        let spec = SpecificationKind::Named(TypeName::from("MISSING_ARRAY"));
        let result = try_from(&TypeName::from("ALIAS_ARRAY"), &spec, &env);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(Problem::ParentTypeNotDeclared.code(), error.code);
    }

    #[test]
    fn try_from_with_multidimensional_array_then_creates_correct_size() {
        let env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();

        let array_subranges = ArraySubranges {
            ranges: vec![
                Subrange {
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
                            value: 3,
                        },
                        is_neg: false,
                    },
                },
                Subrange {
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
                            value: 4,
                        },
                        is_neg: false,
                    },
                },
            ],
            type_name: TypeName::from("bool"),
        };

        let spec = SpecificationKind::Inline(array_subranges);
        let result = try_from(&TypeName::from("MATRIX"), &spec, &env);
        assert!(result.is_ok());

        let attrs = match result.unwrap() {
            IntermediateResult::Type(attrs) => attrs,
            _ => unreachable!("Expected Type result"),
        };

        if let IntermediateType::Array { element_type, size } = attrs.representation {
            assert_eq!(*element_type, IntermediateType::Bool);
            assert_eq!(size, Some(12)); // 3 * 4 = 12
        } else {
            unreachable!("Expected Array type");
        }
    }
}
