//! Semantic types in the IronPLC compiler.
//!
//! This module defines the SemanticType enum and related structures that represent
//! the type system during the compilation process. A semantic type is the resolved
//! description of a type (as opposed to the type as written in the source). These types
//! are used after parsing but before code generation to perform type checking and
//! semantic analysis.
//!
//! The semantic type system is designed to support both primitive types (like integers,
//! booleans) and complex types (like structures, arrays, and function blocks) found in
//! IEC 61131-3 standard and similar PLC programming languages.

use crate::enumeration_members::EnumerationMembers;
use ironplc_container::{string_region_size, CharWidth, DEFAULT_STRING_MAX_LENGTH};
use ironplc_dsl::core::Id;

/// Bounds for a single dimension of an array type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArrayDimension {
    /// Lower bound (inclusive), e.g., 1 in ARRAY[1..10]
    pub lower: i32,
    /// Upper bound (inclusive), e.g., 10 in ARRAY[1..10]
    pub upper: i32,
}

/// Represents the size of a data type in bits, aligned to common byte boundaries.
/// Used for memory layout and type checking of numeric and bit-based types.
#[derive(Debug, Clone, PartialEq)]
pub enum ByteSized {
    /// 8-bit (1 byte) size
    B8,
    /// 16-bit (2 bytes) size
    B16,
    /// 32-bit (4 bytes) size
    B32,
    /// 64-bit (8 bytes) size
    B64,
}

impl ByteSized {
    /// The width in bits.
    pub fn bits(&self) -> u8 {
        match self {
            ByteSized::B8 => 8,
            ByteSized::B16 => 16,
            ByteSized::B32 => 32,
            ByteSized::B64 => 64,
        }
    }
}

/// Represents the semantic type of a type in the PLC program.
///
/// This enum captures all possible types that can appear in a PLC program,
/// from primitive types to complex user-defined types. The semantic type
/// is used during semantic analysis and code generation.
#[derive(Debug, Clone, PartialEq)]
pub enum SemanticType {
    /// Boolean type (true/false)
    Bool,
    /// Signed integer with specified bit width
    Int { size: ByteSized },
    /// Unsigned integer with specified bit width
    UInt { size: ByteSized },
    /// Floating-point number with specified precision
    Real {
        size: ByteSized, // Typically B32 (single precision) or B64 (double precision)
    },
    /// Fixed-size byte array
    Bytes { size: ByteSized },
    /// Time duration type with specified bit width.
    /// TIME is 32-bit (milliseconds), LTIME is 64-bit (milliseconds).
    Time { size: ByteSized },
    /// Calendar date type (DATE is 32-bit, LDATE is 64-bit).
    /// Stored as unsigned seconds since 1970-01-01.
    Date { size: ByteSized },
    /// Time-of-day type (TIME_OF_DAY/TOD is 32-bit, LTOD is 64-bit).
    /// Both store unsigned milliseconds since midnight; LTOD is the wider
    /// variant, not a higher-resolution one (ADR-0025 amendment).
    TimeOfDay { size: ByteSized },
    /// Combined date-and-time type (DATE_AND_TIME/DT is 32-bit, LDT is 64-bit).
    /// Both store unsigned seconds since 1970-01-01; LDT is the wider
    /// variant, not a higher-resolution one (ADR-0025 amendment).
    DateAndTime { size: ByteSized },

    /// Variable-length string with an optional maximum length and a
    /// per-code-unit `char_width`: [`CharWidth::Narrow`] for STRING (Latin-1
    /// per ADR-0016), [`CharWidth::Wide`] for WSTRING (UTF-16LE per ADR-0016).
    /// The two encodings are not implicitly convertible (ADR-0034); the
    /// type checker rejects `STRING := WSTRING` and the reverse.
    String {
        max_len: Option<u128>,
        char_width: CharWidth,
    },

    /// User-defined enumeration type. Every enumeration is stored and
    /// operated on as a `DINT` (REQ-EN-codegen-003).
    Enumeration {
        /// The members, their ordinals and the default. Not compared by
        /// `PartialEq` (see [`EnumerationMembers`]).
        members: EnumerationMembers,
    },
    /// Structure type containing named fields
    Structure {
        /// Ordered list of fields in the structure
        fields: Vec<SemanticStructField>,
    },
    /// Array type with element type and per-dimension bounds.
    Array {
        /// Type of elements in the array
        element_type: Box<SemanticType>,
        /// Per-dimension bounds. Empty for dynamic/unknown-size arrays.
        dimensions: Vec<ArrayDimension>,
    },
    /// Subrange type with base type and bounds
    Subrange {
        /// The base type this subrange is derived from (must be integer type)
        base_type: Box<SemanticType>,
        /// Minimum value (inclusive)
        min_value: i128,
        /// Maximum value (inclusive)
        max_value: i128,
    },
    /// Function block type.
    ///
    /// It names the block and lists none of its fields: those are recorded
    /// in the type environment (`TypeEnvironment::block_fields`), by the
    /// block's id. A block's fields are known only once its variables'
    /// types are, after its type is entered and copied into the types that
    /// use it (an array of the block, a reference to it), so a field list
    /// here would be a copy that could go stale.
    FunctionBlock {
        /// Name of the function block type
        name: String,
    },
    /// Reference type (REF_TO) pointing to a target type.
    /// References are stored as variable-table indices (u64) at runtime.
    Reference {
        /// The type that this reference points to
        target_type: Box<SemanticType>,
    },
}

impl SemanticType {
    /// Returns if the type is a primitive type.
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            SemanticType::Bool
                | SemanticType::Int { .. }
                | SemanticType::UInt { .. }
                | SemanticType::Real { .. }
                | SemanticType::Bytes { .. }
                | SemanticType::String { .. }
                | SemanticType::Time { .. }
                | SemanticType::Date { .. }
                | SemanticType::TimeOfDay { .. }
                | SemanticType::DateAndTime { .. }
        )
    }

    /// Returns if the type is an enumeration.
    pub fn is_enumeration(&self) -> bool {
        matches!(self, SemanticType::Enumeration { .. })
    }

    /// Returns if the type is a structure.
    pub fn is_structure(&self) -> bool {
        matches!(self, SemanticType::Structure { .. })
    }

    /// Returns if the type is an array.
    pub fn is_array(&self) -> bool {
        matches!(self, SemanticType::Array { .. })
    }

    /// Returns if the type is a subrange.
    pub fn is_subrange(&self) -> bool {
        matches!(self, SemanticType::Subrange { .. })
    }

    /// The type a value of this type is operated at: a subrange's base type,
    /// and any other type itself. `s + 1` on a subrange of `INT` adds as
    /// `INT`.
    pub fn operated_as(&self) -> &SemanticType {
        match self {
            SemanticType::Subrange { base_type, .. } => base_type,
            other => other,
        }
    }

    /// Returns if the type is a function block.
    pub fn is_function_block(&self) -> bool {
        matches!(self, SemanticType::FunctionBlock { .. })
    }

    /// Returns if the type exposes named members reachable with `.` access.
    ///
    /// Structures and function block instances both expose named fields, so
    /// both answer `true`.
    pub fn has_members(&self) -> bool {
        matches!(
            self,
            SemanticType::Structure { .. } | SemanticType::FunctionBlock { .. }
        )
    }

    /// The fields a structure lists, `None` for any other type. A function
    /// block's fields are in the type environment
    /// (`TypeEnvironment::block_fields`), not in its type.
    pub fn structure_fields(&self) -> Option<&Vec<SemanticStructField>> {
        match self {
            SemanticType::Structure { fields } => Some(fields),
            _ => None,
        }
    }

    /// Returns if the type is a reference (REF_TO).
    pub fn is_reference(&self) -> bool {
        matches!(self, SemanticType::Reference { .. })
    }

    /// Returns the target type if this is a reference type, None otherwise.
    pub fn referenced_type(&self) -> Option<&SemanticType> {
        match self {
            SemanticType::Reference { target_type } => Some(target_type),
            _ => None,
        }
    }

    /// Returns if the type is numeric (integer, unsigned integer, or real).
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            SemanticType::Int { .. } | SemanticType::UInt { .. } | SemanticType::Real { .. }
        )
    }

    /// Returns the total number of elements across all dimensions,
    /// or None if dimensions is empty.
    pub fn array_total_elements(&self) -> Option<u32> {
        match self {
            SemanticType::Array { dimensions, .. } if !dimensions.is_empty() => {
                let mut total: u32 = 1;
                for dim in dimensions {
                    // Guard: if lower > upper, this dimension is invalid.
                    // The analyzer validates this, but defense-in-depth.
                    let span = dim.upper as i64 - dim.lower as i64 + 1;
                    if span <= 0 {
                        return None;
                    }
                    let dim_size = span as u32;
                    total = total.checked_mul(dim_size)?;
                }
                Some(total)
            }
            _ => None,
        }
    }

    /// The width in bits of a type that states one: an integer, a real, a
    /// bit string, or a time or date. `None` for any other type, including a
    /// subrange or an enumeration, whose width is that of the type it is
    /// based on.
    pub fn bit_width(&self) -> Option<u8> {
        match self {
            SemanticType::Int { size }
            | SemanticType::UInt { size }
            | SemanticType::Real { size }
            | SemanticType::Bytes { size }
            | SemanticType::Time { size }
            | SemanticType::Date { size }
            | SemanticType::TimeOfDay { size }
            | SemanticType::DateAndTime { size } => Some(size.bits()),
            SemanticType::Bool
            | SemanticType::String { .. }
            | SemanticType::Enumeration { .. }
            | SemanticType::Structure { .. }
            | SemanticType::Array { .. }
            | SemanticType::Subrange { .. }
            | SemanticType::FunctionBlock { .. }
            | SemanticType::Reference { .. } => None,
        }
    }

    /// Validates that subrange bounds are within the limits of this type.
    ///
    /// This method checks if the given minimum and maximum values are valid
    /// for this type when used as a subrange base type. It ensures that:
    /// 1. The type is numeric (integer, unsigned integer, or real)
    /// 2. The bounds are within the representable range of the type
    /// 3. For nested subranges, the bounds are within the parent subrange
    ///
    /// # Parameters
    /// - `min_value`: The minimum value of the subrange (inclusive)
    /// - `max_value`: The maximum value of the subrange (inclusive)
    /// - `type_name`: The name of the type being validated (for error reporting)
    ///
    /// # Returns
    /// - `Ok(())`: If the bounds are valid for this type
    /// - `Err(Diagnostic)`: If the bounds are invalid, with specific error details
    ///
    /// # Errors
    /// - `SubrangeBaseTypeNotNumeric`: If this type is not numeric
    /// - `SubrangeOutOfBounds`: If the bounds exceed the type's representable range
    ///
    /// # Examples
    /// ```ignore
    /// let int_type = SemanticType::Int { size: ByteSized::B16 };
    /// let type_name = TypeName::from("MY_RANGE");
    ///
    /// // Valid bounds for INT (-32768 to 32767)
    /// assert!(int_type.validate_bounds(-1000, 1000, &type_name).is_ok());
    ///
    /// // Invalid bounds (exceeds INT range)
    /// assert!(int_type.validate_bounds(-50000, 50000, &type_name).is_err());
    /// ```
    pub fn validate_bounds(
        &self,
        min_value: i128,
        max_value: i128,
        type_name: &ironplc_dsl::common::TypeName,
    ) -> Result<(), ironplc_dsl::diagnostic::Diagnostic> {
        use ironplc_dsl::core::Located;
        use ironplc_dsl::diagnostic::{Diagnostic, Label};
        use ironplc_problems::Problem;

        // A nested subrange is bounded by its parent's bounds. A bit string
        // has a range of values too, but is not numeric, so it is no
        // subrange's base.
        let range = match self {
            SemanticType::Int { .. }
            | SemanticType::UInt { .. }
            | SemanticType::Subrange { .. } => crate::value_range::of(self),
            _ => None,
        };
        let Some((type_min, type_max)) = range else {
            return Err(Diagnostic::problem(
                Problem::SubrangeBaseTypeNotNumeric,
                Label::span(type_name.span(), "Subrange base type"),
            ));
        };

        if min_value < type_min || max_value > type_max {
            return Err(Diagnostic::problem(
                Problem::SubrangeOutOfBounds,
                Label::span(type_name.span(), "Subrange declaration"),
            ));
        }

        Ok(())
    }
}

/// Represents the variable type for function block fields.
/// This distinguishes between input, output, in-out, and internal variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionBlockVarType {
    /// Input variable (VAR_INPUT)
    Input,
    /// Output variable (VAR_OUTPUT)
    Output,
    /// Input-output variable (VAR_IN_OUT)
    InOut,
    /// Internal variable (VAR)
    Internal,
}

/// Reason why `slot_count` could not compute a slot count.
#[derive(Debug, Clone, PartialEq)]
pub enum SlotCountError {
    /// The type contains a field type not yet supported in the data region
    /// (STRING, WSTRING, or FunctionBlock).
    UnsupportedFieldType,
    /// The type nesting depth exceeds the maximum allowed depth (defense-in-depth
    /// guard against recursive type cycles that the analyzer should have rejected).
    MaxDepthExceeded,
    /// The total slot count overflows u32 (structure or array is too large).
    Overflow,
}

impl SemanticType {
    /// Returns the number of 8-byte slots this type occupies in the data region.
    ///
    /// Returns `Ok(n)` for types with known slot counts.
    ///
    /// Returns `Err(SlotCountError::UnsupportedFieldType)` for types that cannot
    /// yet be stored in the data region (STRING, WSTRING, FunctionBlock).
    ///
    /// Returns `Err(SlotCountError::MaxDepthExceeded)` if the nesting depth
    /// exceeds 32 levels (defense-in-depth against recursive type cycles that
    /// the analyzer's toposort pass should have rejected).
    ///
    /// Returns `Err(SlotCountError::Overflow)` if the total slot count exceeds u32.
    pub fn slot_count(&self) -> Result<u32, SlotCountError> {
        self.slot_count_inner(0)
    }

    fn slot_count_inner(&self, depth: u32) -> Result<u32, SlotCountError> {
        // Guard against runaway recursion (defense-in-depth).
        // The analyzer rejects recursive types via toposort, but if a bug
        // allows one through, this prevents a stack overflow.
        const MAX_NESTING_DEPTH: u32 = 32;
        if depth > MAX_NESTING_DEPTH {
            return Err(SlotCountError::MaxDepthExceeded);
        }

        match self {
            // Primitives: 1 slot each
            SemanticType::Bool
            | SemanticType::Int { .. }
            | SemanticType::UInt { .. }
            | SemanticType::Real { .. }
            | SemanticType::Bytes { .. }
            | SemanticType::Time { .. }
            | SemanticType::Date { .. }
            | SemanticType::TimeOfDay { .. }
            | SemanticType::DateAndTime { .. }
            | SemanticType::Enumeration { .. }
            | SemanticType::Subrange { .. }
            | SemanticType::Reference { .. } => Ok(1),

            // Structures: sum of field slot counts
            SemanticType::Structure { fields } => {
                let mut total = 0u32;
                for field in fields {
                    let field_slots = field.field_type.slot_count_inner(depth + 1)?;
                    total = total
                        .checked_add(field_slots)
                        .ok_or(SlotCountError::Overflow)?;
                }
                Ok(total)
            }

            // Arrays: total_elements * element_slots
            SemanticType::Array {
                element_type,
                dimensions,
            } => {
                let elem_slots = element_type.slot_count_inner(depth + 1)?;
                let total_elements = dimensions.iter().try_fold(1u32, |acc, dim| {
                    let size = u32::try_from(dim.upper - dim.lower + 1)
                        .map_err(|_| SlotCountError::Overflow)?;
                    acc.checked_mul(size).ok_or(SlotCountError::Overflow)
                })?;
                total_elements
                    .checked_mul(elem_slots)
                    .ok_or(SlotCountError::Overflow)
            }

            // STRING/WSTRING: the data-region layout (header plus
            // `max_len * char_width` payload bytes, per ADR-0035), rounded up
            // to whole 8-byte slots. A WSTRING needs twice the payload of a
            // STRING of the same declared length, so `char_width` is part of
            // the sizing and not discarded.
            SemanticType::String {
                max_len,
                char_width,
            } => {
                let len = max_len.unwrap_or(DEFAULT_STRING_MAX_LENGTH as u128);
                let len = u16::try_from(len).map_err(|_| SlotCountError::Overflow)?;
                let total_bytes = string_region_size(len, *char_width);
                Ok(total_bytes.div_ceil(8))
            }

            // Not yet supported in data region
            SemanticType::FunctionBlock { .. } => Err(SlotCountError::UnsupportedFieldType),
        }
    }
}

/// Represents a field within a structure semantic type.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticStructField {
    /// Name of the field
    pub name: Id,
    /// Type of the field
    pub field_type: SemanticType,
    /// Whether this field has a default value in the type declaration.
    /// Used for const initialization validation - a const variable of structure type
    /// is fully initialized if all fields without defaults are explicitly initialized.
    pub has_default: bool,
}

/// Represents a parameter in a function or function block declaration.
///
/// Uses `TypeName` instead of resolved `SemanticType` to allow building
/// complete function signatures even when type resolution fails. Types can
/// be resolved on-demand via `TypeEnvironment` when needed for validation.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticFunctionParameter {
    /// Name of the parameter
    pub name: ironplc_dsl::core::Id,
    /// Type name of the parameter (resolved on-demand via TypeEnvironment)
    pub param_type: ironplc_dsl::common::TypeName,
    /// Whether this is an input parameter
    pub is_input: bool,
    /// Whether this is an output parameter
    pub is_output: bool,
    /// Whether this is an input-output parameter
    pub is_inout: bool,
    /// Whether this parameter is a REF_TO reference type.
    /// When true, `param_type` holds the referenced type name (e.g., BYTE for REF_TO BYTE).
    pub is_reference: bool,
}

impl SemanticFunctionParameter {
    /// Returns true if this parameter receives a value from the caller
    /// (either VAR_INPUT or VAR_IN_OUT).
    pub fn is_input_compatible(&self) -> bool {
        self.is_input || self.is_inout
    }
}

#[cfg(test)]
mod tests {
    use crate::semantic_type::{
        ArrayDimension, ByteSized, SemanticStructField, SemanticType, SlotCountError,
    };
    use ironplc_container::{CharWidth, STRING_HEADER_BYTES};
    use ironplc_dsl::core::Id;

    #[test]
    fn bit_width_when_type_states_width_then_returns_bits() {
        assert_eq!(
            SemanticType::Int {
                size: ByteSized::B16
            }
            .bit_width(),
            Some(16)
        );
        assert_eq!(
            SemanticType::UInt {
                size: ByteSized::B8
            }
            .bit_width(),
            Some(8)
        );
        assert_eq!(
            SemanticType::Real {
                size: ByteSized::B64
            }
            .bit_width(),
            Some(64)
        );
        assert_eq!(
            SemanticType::Bytes {
                size: ByteSized::B32
            }
            .bit_width(),
            Some(32)
        );
        assert_eq!(
            SemanticType::DateAndTime {
                size: ByteSized::B64
            }
            .bit_width(),
            Some(64)
        );
    }

    #[test]
    fn bit_width_when_type_states_no_width_then_returns_none() {
        assert_eq!(SemanticType::Bool.bit_width(), None);
        assert_eq!(
            SemanticType::Subrange {
                base_type: Box::new(SemanticType::Int {
                    size: ByteSized::B16,
                }),
                min_value: 1,
                max_value: 100,
            }
            .bit_width(),
            None
        );
        assert_eq!(
            SemanticType::Reference {
                target_type: Box::new(SemanticType::Bool),
            }
            .bit_width(),
            None
        );
    }

    #[test]
    fn validate_bounds_with_all_integer_types_then_validates_correctly() {
        use ironplc_dsl::common::TypeName;

        // Test all signed integer types
        let sint_type = SemanticType::Int {
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

        let int_type = SemanticType::Int {
            size: ByteSized::B16,
        };
        assert!(int_type
            .validate_bounds(-32768, 32767, &TypeName::from("TEST"))
            .is_ok());
        assert!(int_type
            .validate_bounds(-32769, 32767, &TypeName::from("TEST"))
            .is_err());

        let dint_type = SemanticType::Int {
            size: ByteSized::B32,
        };
        assert!(dint_type
            .validate_bounds(-2147483648, 2147483647, &TypeName::from("TEST"))
            .is_ok());
        assert!(dint_type
            .validate_bounds(-2147483649, 2147483647, &TypeName::from("TEST"))
            .is_err());

        let lint_type = SemanticType::Int {
            size: ByteSized::B64,
        };
        assert!(lint_type
            .validate_bounds(i64::MIN as i128, i64::MAX as i128, &TypeName::from("TEST"))
            .is_ok());

        // Test all unsigned integer types
        let usint_type = SemanticType::UInt {
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

        let uint_type = SemanticType::UInt {
            size: ByteSized::B16,
        };
        assert!(uint_type
            .validate_bounds(0, 65535, &TypeName::from("TEST"))
            .is_ok());
        assert!(uint_type
            .validate_bounds(-1, 65535, &TypeName::from("TEST"))
            .is_err());

        let udint_type = SemanticType::UInt {
            size: ByteSized::B32,
        };
        assert!(udint_type
            .validate_bounds(0, 4294967295, &TypeName::from("TEST"))
            .is_ok());
        assert!(udint_type
            .validate_bounds(-1, 4294967295, &TypeName::from("TEST"))
            .is_err());

        let ulint_type = SemanticType::UInt {
            size: ByteSized::B64,
        };
        assert!(ulint_type
            .validate_bounds(0, u64::MAX as i128, &TypeName::from("TEST"))
            .is_ok());
        assert!(ulint_type
            .validate_bounds(-1, u64::MAX as i128, &TypeName::from("TEST"))
            .is_err());
    }

    #[test]
    fn validate_bounds_with_nested_subrange_then_validates_correctly() {
        use ironplc_dsl::common::TypeName;

        // Create a nested subrange: INT (10..100) -> SUBRANGE (20..80)
        let nested_subrange = SemanticType::Subrange {
            base_type: Box::new(SemanticType::Int {
                size: ByteSized::B16,
            }),
            min_value: 10,
            max_value: 100,
        };

        // Valid bounds within the nested subrange
        assert!(nested_subrange
            .validate_bounds(20, 80, &TypeName::from("TEST"))
            .is_ok());
        assert!(nested_subrange
            .validate_bounds(10, 100, &TypeName::from("TEST"))
            .is_ok());

        // Invalid bounds outside the nested subrange
        assert!(nested_subrange
            .validate_bounds(5, 80, &TypeName::from("TEST"))
            .is_err());
        assert!(nested_subrange
            .validate_bounds(20, 150, &TypeName::from("TEST"))
            .is_err());
        assert!(nested_subrange
            .validate_bounds(5, 150, &TypeName::from("TEST"))
            .is_err());
    }

    #[test]
    fn validate_bounds_with_non_numeric_type_then_error() {
        use ironplc_dsl::common::TypeName;
        use ironplc_problems::Problem;

        let string_type = SemanticType::String {
            max_len: Some(10),
            char_width: CharWidth::Narrow,
        };
        let result = string_type.validate_bounds(0, 10, &TypeName::from("TEST"));
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, Problem::SubrangeBaseTypeNotNumeric.code());

        let bool_type = SemanticType::Bool;
        let result = bool_type.validate_bounds(0, 1, &TypeName::from("TEST"));
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, Problem::SubrangeBaseTypeNotNumeric.code());

        let array_type = SemanticType::Array {
            element_type: Box::new(SemanticType::Int {
                size: ByteSized::B16,
            }),
            dimensions: vec![ArrayDimension {
                lower: 1,
                upper: 10,
            }],
        };
        let result = array_type.validate_bounds(0, 9, &TypeName::from("TEST"));
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.code, Problem::SubrangeBaseTypeNotNumeric.code());
    }

    #[test]
    fn structure_fields_when_structure_then_returns_fields() {
        use super::SemanticStructField;
        use ironplc_dsl::core::Id;

        let fields = vec![SemanticStructField {
            name: Id::from("field1"),
            field_type: SemanticType::Bool,
            has_default: false,
        }];
        let struct_type = SemanticType::Structure {
            fields: fields.clone(),
        };

        assert!(struct_type.has_members());
        assert_eq!(struct_type.structure_fields(), Some(&fields));
    }

    #[test]
    fn structure_fields_when_function_block_then_none_but_has_members() {
        let fb_type = SemanticType::FunctionBlock {
            name: "MyFB".to_string(),
        };

        assert!(fb_type.has_members());
        assert_eq!(fb_type.structure_fields(), None);
    }

    #[test]
    fn structure_fields_when_not_composite_then_returns_none() {
        assert!(!SemanticType::Bool.has_members());
        assert_eq!(SemanticType::Bool.structure_fields(), None);

        let array_type = SemanticType::Array {
            element_type: Box::new(SemanticType::Bool),
            dimensions: vec![],
        };
        assert!(!array_type.has_members());
        assert_eq!(array_type.structure_fields(), None);
    }

    #[test]
    fn slot_count_when_primitive_then_returns_1() {
        assert_eq!(SemanticType::Bool.slot_count(), Ok(1));
        assert_eq!(
            SemanticType::Int {
                size: ByteSized::B16
            }
            .slot_count(),
            Ok(1)
        );
        assert_eq!(
            SemanticType::UInt {
                size: ByteSized::B32
            }
            .slot_count(),
            Ok(1)
        );
        assert_eq!(
            SemanticType::Real {
                size: ByteSized::B64
            }
            .slot_count(),
            Ok(1)
        );
        assert_eq!(
            SemanticType::Bytes {
                size: ByteSized::B8
            }
            .slot_count(),
            Ok(1)
        );
        assert_eq!(
            SemanticType::Time {
                size: ByteSized::B32
            }
            .slot_count(),
            Ok(1)
        );
        assert_eq!(
            SemanticType::Date {
                size: ByteSized::B32
            }
            .slot_count(),
            Ok(1)
        );
        assert_eq!(
            SemanticType::TimeOfDay {
                size: ByteSized::B32
            }
            .slot_count(),
            Ok(1)
        );
        assert_eq!(
            SemanticType::DateAndTime {
                size: ByteSized::B64
            }
            .slot_count(),
            Ok(1)
        );
        assert_eq!(
            SemanticType::Enumeration {
                members: crate::enumeration_members::EnumerationMembers::default(),
            }
            .slot_count(),
            Ok(1)
        );
        assert_eq!(
            SemanticType::Subrange {
                base_type: Box::new(SemanticType::Int {
                    size: ByteSized::B16
                }),
                min_value: 1,
                max_value: 100,
            }
            .slot_count(),
            Ok(1)
        );
    }

    #[test]
    fn slot_count_when_structure_with_two_fields_then_returns_2() {
        let s = SemanticType::Structure {
            fields: vec![
                SemanticStructField {
                    name: Id::from("a"),
                    field_type: SemanticType::Int {
                        size: ByteSized::B16,
                    },
                    has_default: false,
                },
                SemanticStructField {
                    name: Id::from("b"),
                    field_type: SemanticType::Bool,
                    has_default: false,
                },
            ],
        };
        assert_eq!(s.slot_count(), Ok(2));
    }

    #[test]
    fn slot_count_when_nested_structure_then_returns_sum() {
        let inner = SemanticType::Structure {
            fields: vec![
                SemanticStructField {
                    name: Id::from("x"),
                    field_type: SemanticType::Int {
                        size: ByteSized::B32,
                    },
                    has_default: false,
                },
                SemanticStructField {
                    name: Id::from("y"),
                    field_type: SemanticType::Int {
                        size: ByteSized::B32,
                    },
                    has_default: false,
                },
            ],
        };
        let outer = SemanticType::Structure {
            fields: vec![
                SemanticStructField {
                    name: Id::from("inner"),
                    field_type: inner,
                    has_default: false,
                },
                SemanticStructField {
                    name: Id::from("z"),
                    field_type: SemanticType::Bool,
                    has_default: false,
                },
            ],
        };
        // inner occupies 2 slots + z occupies 1 slot = 3
        assert_eq!(outer.slot_count(), Ok(3));
    }

    #[test]
    fn slot_count_when_array_of_primitives_then_returns_total_elements() {
        let arr = SemanticType::Array {
            element_type: Box::new(SemanticType::Int {
                size: ByteSized::B32,
            }),
            dimensions: vec![ArrayDimension {
                lower: 1,
                upper: 10,
            }],
        };
        assert_eq!(arr.slot_count(), Ok(10));
    }

    #[test]
    fn slot_count_when_array_of_structures_then_returns_elements_times_struct_slots() {
        let s = SemanticType::Structure {
            fields: vec![
                SemanticStructField {
                    name: Id::from("a"),
                    field_type: SemanticType::Int {
                        size: ByteSized::B32,
                    },
                    has_default: false,
                },
                SemanticStructField {
                    name: Id::from("b"),
                    field_type: SemanticType::Int {
                        size: ByteSized::B32,
                    },
                    has_default: false,
                },
            ],
        };
        let arr = SemanticType::Array {
            element_type: Box::new(s),
            dimensions: vec![ArrayDimension { lower: 1, upper: 5 }],
        };
        // 5 elements * 2 slots each = 10
        assert_eq!(arr.slot_count(), Ok(10));
    }

    #[test]
    fn slot_count_when_reference_field_then_returns_1() {
        let r = SemanticType::Reference {
            target_type: Box::new(SemanticType::Int {
                size: ByteSized::B32,
            }),
        };
        assert_eq!(r.slot_count(), Ok(1));
    }

    #[test]
    fn slot_count_when_string_with_explicit_len_then_returns_ceil_slots() {
        // STRING[255]: ceil((6 + 255 * 1) / 8) = ceil(261/8) = 33 slots
        let s = SemanticType::String {
            max_len: Some(255),
            char_width: CharWidth::Narrow,
        };
        assert_eq!(s.slot_count(), Ok(33));
    }

    #[test]
    fn slot_count_when_string_with_default_len_then_returns_ceil_slots() {
        // STRING (default 254): ceil((6 + 254 * 1) / 8) = ceil(260/8) = 33 slots
        let s = SemanticType::String {
            max_len: None,
            char_width: CharWidth::Narrow,
        };
        assert_eq!(s.slot_count(), Ok(33));
    }

    #[test]
    fn slot_count_when_string_small_then_returns_2() {
        // STRING[4]: ceil((6 + 4 * 1) / 8) = ceil(10/8) = 2 slots.
        // The 6-byte header does not leave a 4-character string inside one slot.
        let s = SemanticType::String {
            max_len: Some(4),
            char_width: CharWidth::Narrow,
        };
        assert_eq!(s.slot_count(), Ok(2));
    }

    #[test]
    fn slot_count_when_wstring_with_explicit_len_then_scales_by_char_width() {
        // WSTRING[255]: ceil((6 + 255 * 2) / 8) = ceil(516/8) = 65 slots
        let s = SemanticType::String {
            max_len: Some(255),
            char_width: CharWidth::Wide,
        };
        assert_eq!(s.slot_count(), Ok(65));
    }

    #[test]
    fn slot_count_when_wstring_with_default_len_then_scales_by_char_width() {
        // WSTRING (default 254): ceil((6 + 254 * 2) / 8) = ceil(514/8) = 65 slots
        let s = SemanticType::String {
            max_len: None,
            char_width: CharWidth::Wide,
        };
        assert_eq!(s.slot_count(), Ok(65));
    }

    #[test]
    fn slot_count_when_wstring_then_returns_more_slots_than_same_length_string() {
        let narrow = SemanticType::String {
            max_len: Some(64),
            char_width: CharWidth::Narrow,
        };
        let wide = SemanticType::String {
            max_len: Some(64),
            char_width: CharWidth::Wide,
        };
        // ceil(70/8) = 9 vs ceil(134/8) = 17 — the wide payload is twice as big.
        assert_eq!(narrow.slot_count(), Ok(9));
        assert_eq!(wide.slot_count(), Ok(17));
    }

    #[test]
    fn slot_count_when_string_then_holds_header_and_payload() {
        // A string takes its header and `max_len` code units of `char_width`
        // bytes each, rounded up to whole slots.
        for char_width in [CharWidth::Narrow, CharWidth::Wide] {
            for max_len in [0u128, 1, 4, 80, 254, 255, 4096] {
                let s = SemanticType::String {
                    max_len: Some(max_len),
                    char_width,
                };
                let payload = max_len as u32 * char_width.byte_width() as u32;
                let expected = (payload + STRING_HEADER_BYTES as u32).div_ceil(8);
                assert_eq!(
                    s.slot_count(),
                    Ok(expected),
                    "{:?} [{}]",
                    char_width,
                    max_len
                );
            }
        }
    }

    #[test]
    fn slot_count_when_string_len_exceeds_u16_then_returns_overflow() {
        // max_length is a u16 field in the data-region header, so a longer
        // declared length has no representable layout.
        let s = SemanticType::String {
            max_len: Some(u16::MAX as u128 + 1),
            char_width: CharWidth::Narrow,
        };
        assert_eq!(s.slot_count(), Err(SlotCountError::Overflow));
    }

    #[test]
    fn slot_count_when_nesting_exceeds_max_depth_then_returns_max_depth_exceeded() {
        // Build 34 levels of nesting (exceeds MAX_NESTING_DEPTH of 32)
        let mut t = SemanticType::Bool;
        for i in 0..34 {
            t = SemanticType::Structure {
                fields: vec![SemanticStructField {
                    name: Id::from(&format!("f{}", i)),
                    field_type: t,
                    has_default: false,
                }],
            };
        }
        assert_eq!(t.slot_count(), Err(SlotCountError::MaxDepthExceeded));
    }

    #[test]
    fn slot_count_when_total_overflows_u32_then_returns_overflow() {
        // Create a multi-dimensional array that overflows u32
        let arr = SemanticType::Array {
            element_type: Box::new(SemanticType::Int {
                size: ByteSized::B32,
            }),
            dimensions: vec![
                ArrayDimension {
                    lower: 1,
                    upper: 100_000,
                },
                ArrayDimension {
                    lower: 1,
                    upper: 100_000,
                },
            ],
        };
        assert_eq!(arr.slot_count(), Err(SlotCountError::Overflow));
    }
}
