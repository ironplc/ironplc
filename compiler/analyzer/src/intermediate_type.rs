//! Intermediate representation of types in the IronPLC compiler.
//!
//! This module defines the IntermediateType enum and related structures that represent
//! the type system during the compilation process. These types are used after parsing
//! but before code generation to perform type checking and semantic analysis.
//!
//! The intermediate type system is designed to support both primitive types (like integers,
//! booleans) and complex types (like structures, arrays, and function blocks) found in
//! IEC 61131-3 standard and similar PLC programming languages.

use ironplc_dsl::common::TypeName;

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
    /// Converts the ByteSized variant to its corresponding bit width
    #[allow(dead_code)]
    pub fn into(&self) -> u8 {
        match self {
            ByteSized::B8 => 8,
            ByteSized::B16 => 16,
            ByteSized::B32 => 32,
            ByteSized::B64 => 64,
        }
    }

    #[allow(dead_code)]
    pub fn as_bytes(&self) -> u8 {
        match self {
            ByteSized::B8 => 1,
            ByteSized::B16 => 2,
            ByteSized::B32 => 4,
            ByteSized::B64 => 8,
        }
    }
}

/// Represents a type in the intermediate representation of the PLC program.
///
/// This enum captures all possible types that can appear in a PLC program,
/// from primitive types to complex user-defined types. The intermediate
/// representation is used during semantic analysis and code generation.
#[derive(Debug, Clone, PartialEq)]
pub enum IntermediateType {
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
    /// Time duration type
    Time,
    /// Calendar date type
    Date,

    /// Variable-length string with optional maximum length
    String { max_len: Option<u128> },

    /// User-defined enumeration type
    Enumeration {
        /// The underlying primitive type (usually Int { size: 8 })
        underlying_type: Box<IntermediateType>,
    },
    /// Structure type containing named fields
    Structure {
        /// Ordered list of fields in the structure
        fields: Vec<IntermediateStructField>,
    },
    /// Array type with element type and optional fixed size
    Array {
        /// Type of elements in the array
        element_type: Box<IntermediateType>,
        /// Fixed size if known at compile-time, None for dynamic arrays
        size: Option<u32>,
    },
    /// Subrange type with base type and bounds
    Subrange {
        /// The base type this subrange is derived from (must be integer type)
        base_type: Box<IntermediateType>,
        /// Minimum value (inclusive)
        min_value: i128,
        /// Maximum value (inclusive)
        max_value: i128,
    },
    /// Function block type
    #[allow(unused)]
    FunctionBlock {
        /// Name of the function block type
        name: String,
    },
    /// Function type with return type and parameters
    #[allow(unused)]
    Function {
        /// Return type of the function, None for procedures
        return_type: Option<Box<IntermediateType>>,
        /// List of function parameters
        parameters: Vec<IntermediateFunctionParameter>,
    },
}

impl IntermediateType {
    /// Returns if the type is a primitive type.
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            IntermediateType::Bool
                | IntermediateType::Int { .. }
                | IntermediateType::UInt { .. }
                | IntermediateType::Real { .. }
                | IntermediateType::Bytes { .. }
                | IntermediateType::String { .. }
                | IntermediateType::Time
                | IntermediateType::Date
        )
    }

    /// Returns if the type is an enumeration.
    pub fn is_enumeration(&self) -> bool {
        matches!(self, IntermediateType::Enumeration { .. })
    }

    /// Returns if the type is a structure.
    pub fn is_structure(&self) -> bool {
        matches!(self, IntermediateType::Structure { .. })
    }

    /// Returns if the type is an array.
    pub fn is_array(&self) -> bool {
        matches!(self, IntermediateType::Array { .. })
    }

    /// Returns if the type is a subrange.
    #[allow(dead_code)]
    pub fn is_subrange(&self) -> bool {
        matches!(self, IntermediateType::Subrange { .. })
    }

    /// Returns if the type is a function block.
    #[allow(dead_code)]
    pub fn is_function_block(&self) -> bool {
        matches!(self, IntermediateType::FunctionBlock { .. })
    }

    /// Returns if the type is a function.
    #[allow(dead_code)]
    pub fn is_function(&self) -> bool {
        matches!(self, IntermediateType::Function { .. })
    }

    /// Returns if the type is numeric (integer, unsigned integer, or real).
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            IntermediateType::Int { .. }
                | IntermediateType::UInt { .. }
                | IntermediateType::Real { .. }
        )
    }

    /// Returns if the type is an integer type (signed or unsigned).
    #[allow(dead_code)]
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            IntermediateType::Int { .. } | IntermediateType::UInt { .. }
        )
    }

    /// Gets the size in bytes for this type.
    ///
    /// Returns the exact size in bytes for fixed-size types. For types that require
    /// complex calculations or are dynamically sized, returns 0.
    ///
    /// Note: IEC 61131-3 doesn't support truly dynamically sized objects at runtime,
    /// but some types require compile-time analysis to determine their final size.
    #[allow(dead_code)]
    pub fn size_in_bytes(&self) -> u8 {
        match self {
            IntermediateType::Bool => 1,
            IntermediateType::Int { size } | IntermediateType::UInt { size } => size.as_bytes(),
            IntermediateType::Real { size } => size.as_bytes(),
            IntermediateType::Bytes { size } => size.as_bytes(),
            IntermediateType::Time => 8, // 64-bit time representation
            IntermediateType::Date => 8, // 64-bit date representation
            IntermediateType::String { max_len } => {
                // Variable-length strings return 0, fixed-length strings return max_len
                max_len.map(|len| len as u8).unwrap_or(0)
            }
            IntermediateType::Subrange { base_type, .. } => base_type.size_in_bytes(),
            IntermediateType::Enumeration { underlying_type } => underlying_type.size_in_bytes(),
            IntermediateType::Structure { .. } => {
                // TODO: Implement proper structure size calculation with field alignment
                // This requires calculating field offsets, padding, and total structure size
                // For now, return 0 to indicate size calculation is not yet implemented
                0
            }
            IntermediateType::Array { element_type, size } => {
                if let Some(array_size) = size {
                    // Fixed-size array: element_size * array_size
                    element_type
                        .size_in_bytes()
                        .saturating_mul(*array_size as u8)
                } else {
                    // Dynamic array (not supported in IEC 61131-3, but used during analysis)
                    0
                }
            }
            IntermediateType::FunctionBlock { .. } => {
                // TODO: Implement proper function block instance size calculation
                // This requires analyzing the function block's variable declarations
                // For now, return 0 to indicate size calculation is not yet implemented
                0
            }
            IntermediateType::Function { .. } => {
                // Functions don't have memory layout in the traditional sense
                0
            }
        }
    }

    /// Gets the alignment requirement in bytes for this type.
    ///
    /// Returns the memory alignment requirement following typical C-style alignment rules.
    /// For types that require complex calculations, returns a conservative default.
    ///
    /// # Alignment Rules
    /// - 8-bit types (BOOL, SINT, USINT, BYTE): 1-byte alignment
    /// - 16-bit types (INT, UINT, WORD): 2-byte alignment  
    /// - 32-bit types (DINT, UDINT, REAL, DWORD): 4-byte alignment
    /// - 64-bit types (LINT, ULINT, LREAL, LWORD, TIME, DATE): 8-byte alignment
    #[allow(dead_code)]
    pub fn alignment_bytes(&self) -> u8 {
        match self {
            IntermediateType::Bool => 1,
            IntermediateType::Int { size } | IntermediateType::UInt { size } => size.as_bytes(),
            IntermediateType::Real { size } => size.as_bytes(),
            IntermediateType::Bytes { size } => size.as_bytes(),
            IntermediateType::Time => 8,          // 64-bit alignment
            IntermediateType::Date => 8,          // 64-bit alignment
            IntermediateType::String { .. } => 1, // Strings are byte-aligned
            IntermediateType::Subrange { base_type, .. } => base_type.alignment_bytes(),
            IntermediateType::Enumeration { underlying_type } => underlying_type.alignment_bytes(),
            IntermediateType::Structure { .. } => {
                // TODO: Implement proper structure alignment calculation (should be max field alignment)
                1
            }
            IntermediateType::Array { element_type, .. } => element_type.alignment_bytes(),
            IntermediateType::FunctionBlock { .. } => 1, // Default alignment for function block instances
            IntermediateType::Function { .. } => 1, // Default alignment (functions don't have memory layout)
        }
    }

    /// Returns whether this type has an explicitly specified size.
    ///
    /// This method determines whether the type's size is explicitly specified
    /// in the type declaration or needs to be inferred from context or defaults.
    /// All IEC 61131-3 types have known, fixed sizes, but some require size
    /// inference during semantic analysis.
    ///
    /// # Return Values
    /// - `true`: Type size is explicitly specified in the declaration
    /// - `false`: Type size needs to be inferred from context or defaults
    #[allow(dead_code)]
    pub fn has_explicit_size(&self) -> bool {
        match self {
            IntermediateType::Bool
            | IntermediateType::Int { .. }
            | IntermediateType::UInt { .. }
            | IntermediateType::Real { .. }
            | IntermediateType::Bytes { .. }
            | IntermediateType::Time
            | IntermediateType::Date => true,
            IntermediateType::String { max_len } => max_len.is_some(),
            IntermediateType::Subrange { base_type, .. } => base_type.has_explicit_size(),
            IntermediateType::Enumeration { underlying_type } => {
                underlying_type.has_explicit_size()
            }
            IntermediateType::Structure { .. } => true, // Structures always have explicit size in IEC 61131-3
            IntermediateType::Array { element_type, size } => {
                // Array has explicit size if it has known dimensions and elements have explicit size
                size.is_some() && element_type.has_explicit_size()
            }
            IntermediateType::FunctionBlock { .. } => true, // Function block instances have explicit size
            IntermediateType::Function { .. } => true, // Functions have explicit size (no variable size)
        }
    }

    /// Gets the memory offset of a field within a structure type.
    ///
    /// This method is used to calculate the byte offset of a specific field
    /// within a structure, taking into account field alignment and padding
    /// requirements. It's essential for generating correct memory access code.
    ///
    /// # Parameters
    /// - `field_name`: The Id of the field to find (case-insensitive comparison)
    ///
    /// # Returns
    /// - `Some(offset)`: The byte offset of the field from the start of the structure
    /// - `None`: If the field is not found or this is not a structure type
    ///
    /// # Examples
    /// ```ignore
    /// // For a structure with fields: x: INT (offset 0), y: DINT (offset 4)
    /// let fields = vec![
    ///     IntermediateStructField {
    ///         name: TypeName::from("x"),
    ///         field_type: IntermediateType::Bool,
    ///         offset: 0,
    ///     }
    /// ];
    /// let struct_type = IntermediateType::Structure { fields };
    /// let field_id = Id::from("x");
    /// assert_eq!(struct_type.get_field_offset(&field_id), Some(0));
    /// let unknown_id = Id::from("unknown");
    /// assert_eq!(struct_type.get_field_offset(&unknown_id), None);
    /// ```
    ///
    /// # Note
    /// Uses case-insensitive comparison following IEC 61131-3 identifier rules.
    /// Currently returns the pre-calculated offset stored in the field definition.
    /// Future enhancements may include dynamic offset calculation with proper
    /// alignment and padding rules.
    #[allow(dead_code)]
    pub fn get_field_offset(&self, field_name: &ironplc_dsl::core::Id) -> Option<u32> {
        match self {
            IntermediateType::Structure { fields } => {
                // Find the field by name using case-insensitive Id comparison
                fields
                    .iter()
                    .find(|field| field.name.name == *field_name)
                    .map(|field| field.offset / 8) // Convert bit offset to byte offset
            }
            _ => None, // Not a structure type
        }
    }
}

/// Represents a field within a structure type in the intermediate representation.
#[derive(Debug, Clone, PartialEq)]
pub struct IntermediateStructField {
    /// Name of the field
    pub name: TypeName,
    /// Type of the field
    pub field_type: IntermediateType,
    /// Memory offset of the field from the start of the structure (in bits)
    pub offset: u32,
}

/// Represents a parameter in a function or function block declaration.
#[derive(Debug, Clone, PartialEq)]
pub struct IntermediateFunctionParameter {
    /// Name of the parameter
    pub name: ironplc_dsl::core::Id,
    /// Type of the parameter
    pub param_type: IntermediateType,
    /// Whether this is an input parameter
    pub is_input: bool,
    /// Whether this is an output parameter
    pub is_output: bool,
    /// Whether this is an input-output parameter
    pub is_inout: bool,
}

#[cfg(test)]
mod tests {
    use crate::intermediate_type::{ByteSized, IntermediateType};

    #[test]
    fn intermediate_type_size_in_bytes_returns_bytes() {
        // Test sized types
        assert_eq!(IntermediateType::Bool.size_in_bytes(), 1);
        assert_eq!(
            IntermediateType::Int {
                size: ByteSized::B16
            }
            .size_in_bytes(),
            2
        );
        assert_eq!(
            IntermediateType::UInt {
                size: ByteSized::B32
            }
            .size_in_bytes(),
            4
        );
        assert_eq!(
            IntermediateType::Real {
                size: ByteSized::B64
            }
            .size_in_bytes(),
            8
        );
        assert_eq!(
            IntermediateType::Bytes {
                size: ByteSized::B8
            }
            .size_in_bytes(),
            1
        );

        // Test time and date types
        assert_eq!(IntermediateType::Time.size_in_bytes(), 8);
        assert_eq!(IntermediateType::Date.size_in_bytes(), 8);

        // Test string types
        assert_eq!(
            IntermediateType::String { max_len: Some(10) }.size_in_bytes(),
            10
        );
        assert_eq!(
            IntermediateType::String { max_len: None }.size_in_bytes(),
            0
        );

        // Test subrange inherits base type size
        let subrange = IntermediateType::Subrange {
            base_type: Box::new(IntermediateType::Int {
                size: ByteSized::B16,
            }),
            min_value: 1,
            max_value: 100,
        };
        assert_eq!(subrange.size_in_bytes(), 2);

        // Test enumeration inherits underlying type size
        let enumeration = IntermediateType::Enumeration {
            underlying_type: Box::new(IntermediateType::Int {
                size: ByteSized::B8,
            }),
        };
        assert_eq!(enumeration.size_in_bytes(), 1);

        // Test fixed-size array
        let array = IntermediateType::Array {
            element_type: Box::new(IntermediateType::Int {
                size: ByteSized::B32,
            }),
            size: Some(5),
        };
        assert_eq!(array.size_in_bytes(), 20); // 4 bytes * 5 elements

        // Test dynamic array
        let dynamic_array = IntermediateType::Array {
            element_type: Box::new(IntermediateType::Int {
                size: ByteSized::B32,
            }),
            size: None,
        };
        assert_eq!(dynamic_array.size_in_bytes(), 0);
    }

    #[test]
    fn intermediate_type_alignment_bytes_returns_alignment() {
        // Test primitive types
        assert_eq!(IntermediateType::Bool.alignment_bytes(), 1);
        assert_eq!(
            IntermediateType::Int {
                size: ByteSized::B16
            }
            .alignment_bytes(),
            2
        );
        assert_eq!(
            IntermediateType::UInt {
                size: ByteSized::B32
            }
            .alignment_bytes(),
            4
        );
        assert_eq!(
            IntermediateType::Real {
                size: ByteSized::B64
            }
            .alignment_bytes(),
            8
        );

        // Test time and date types
        assert_eq!(IntermediateType::Time.alignment_bytes(), 8);
        assert_eq!(IntermediateType::Date.alignment_bytes(), 8);

        // Test string types (byte-aligned)
        assert_eq!(
            IntermediateType::String { max_len: Some(10) }.alignment_bytes(),
            1
        );

        // Test derived types inherit alignment
        let subrange = IntermediateType::Subrange {
            base_type: Box::new(IntermediateType::Int {
                size: ByteSized::B32,
            }),
            min_value: 1,
            max_value: 100,
        };
        assert_eq!(subrange.alignment_bytes(), 4);

        // Test array inherits element alignment
        let array = IntermediateType::Array {
            element_type: Box::new(IntermediateType::Real {
                size: ByteSized::B64,
            }),
            size: Some(3),
        };
        assert_eq!(array.alignment_bytes(), 8);
    }

    #[test]
    fn intermediate_type_has_explicit_size_returns_correct_value() {
        // Test types with explicit size
        assert!(IntermediateType::Bool.has_explicit_size());
        assert!(IntermediateType::Int {
            size: ByteSized::B16
        }
        .has_explicit_size());
        assert!(IntermediateType::Time.has_explicit_size());
        assert!(IntermediateType::Date.has_explicit_size());

        // Test string types
        assert!(IntermediateType::String { max_len: Some(10) }.has_explicit_size());
        assert!(!IntermediateType::String { max_len: None }.has_explicit_size());

        // Test derived types
        let subrange = IntermediateType::Subrange {
            base_type: Box::new(IntermediateType::Int {
                size: ByteSized::B16,
            }),
            min_value: 1,
            max_value: 100,
        };
        assert!(subrange.has_explicit_size());

        // Test arrays
        let fixed_array = IntermediateType::Array {
            element_type: Box::new(IntermediateType::Bool),
            size: Some(10),
        };
        assert!(fixed_array.has_explicit_size());

        let dynamic_array = IntermediateType::Array {
            element_type: Box::new(IntermediateType::Bool),
            size: None,
        };
        assert!(!dynamic_array.has_explicit_size());
    }

    #[test]
    fn get_field_offset_with_structure_then_returns_correct_offset() {
        use super::IntermediateStructField;
        use ironplc_dsl::common::TypeName;
        use ironplc_dsl::core::Id;

        let fields = vec![
            IntermediateStructField {
                name: TypeName::from("field1"),
                field_type: IntermediateType::Int {
                    size: ByteSized::B16,
                },
                offset: 0, // 0 bits = 0 bytes
            },
            IntermediateStructField {
                name: TypeName::from("field2"),
                field_type: IntermediateType::Int {
                    size: ByteSized::B32,
                },
                offset: 32, // 32 bits = 4 bytes
            },
        ];

        let struct_type = IntermediateType::Structure { fields };

        let field1_id = Id::from("field1");
        let field2_id = Id::from("field2");
        let nonexistent_id = Id::from("nonexistent");

        assert_eq!(struct_type.get_field_offset(&field1_id), Some(0));
        assert_eq!(struct_type.get_field_offset(&field2_id), Some(4));
        assert_eq!(struct_type.get_field_offset(&nonexistent_id), None);
    }

    #[test]
    fn get_field_offset_with_case_insensitive_field_name_then_returns_correct_offset() {
        use super::IntermediateStructField;
        use ironplc_dsl::common::TypeName;
        use ironplc_dsl::core::Id;

        let fields = vec![IntermediateStructField {
            name: TypeName::from("MyField"),
            field_type: IntermediateType::Int {
                size: ByteSized::B16,
            },
            offset: 16, // 16 bits = 2 bytes
        }];

        let struct_type = IntermediateType::Structure { fields };

        // Test case-insensitive matching following IEC 61131-3 identifier rules
        let lowercase_id = Id::from("myfield");
        let uppercase_id = Id::from("MYFIELD");
        let mixed_case_id = Id::from("MyField");

        assert_eq!(struct_type.get_field_offset(&lowercase_id), Some(2));
        assert_eq!(struct_type.get_field_offset(&uppercase_id), Some(2));
        assert_eq!(struct_type.get_field_offset(&mixed_case_id), Some(2));
    }

    #[test]
    fn get_field_offset_with_non_structure_then_returns_none() {
        use ironplc_dsl::core::Id;

        let int_type = IntermediateType::Int {
            size: ByteSized::B16,
        };
        let field_id = Id::from("any_field");
        assert_eq!(int_type.get_field_offset(&field_id), None);

        let array_type = IntermediateType::Array {
            element_type: Box::new(IntermediateType::Bool),
            size: Some(10),
        };
        assert_eq!(array_type.get_field_offset(&field_id), None);
    }
}
