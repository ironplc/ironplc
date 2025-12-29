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

    /// Gets the size in bytes. 61131-3 doesn't support dynamically sized
    /// objects so we know the size of every item.
    #[allow(dead_code)]
    pub fn size_in_bytes(&self) -> u8 {
        match self {
            IntermediateType::Bool => 1,
            IntermediateType::Int { size } | IntermediateType::UInt { size } => size.as_bytes(),
            IntermediateType::Real { size } => size.as_bytes(),
            IntermediateType::Bytes { size } => size.as_bytes(),
            IntermediateType::Subrange { base_type, .. } => base_type.size_in_bytes(),
            IntermediateType::Enumeration { underlying_type } => underlying_type.size_in_bytes(),
            _ => todo!(), // Complex types don't have a simple bit size
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
    pub name: String,
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
        // Test subrange inherits base type size
        let subrange = IntermediateType::Subrange {
            base_type: Box::new(IntermediateType::Int {
                size: ByteSized::B16,
            }),
            min_value: 1,
            max_value: 100,
        };
        assert_eq!(subrange.size_in_bytes(), 2);
    }
}
