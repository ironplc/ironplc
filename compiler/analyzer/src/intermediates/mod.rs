pub mod arithmetic_overload;
pub mod array;
pub mod common_operand;
pub mod comparison_operand;
pub mod conversion_target;
pub mod enumeration;
pub mod numeric_operation;
pub mod operator_function_form;
pub mod stdlib_conversion_function;
pub mod stdlib_function;
pub mod stdlib_function_block;
pub mod stdlib_time_function;
pub mod string;
pub mod structure;
pub mod subrange;

use ironplc_dsl::common::TypeName;

use crate::type_attributes::TypeAttributes;

/// What an array or subrange specification resolves to: a new type, or
/// another name for a type that already exists.
#[derive(Debug, Clone, PartialEq)]
pub enum TypeResolution {
    /// Create a new type with the given attributes
    Type(TypeAttributes),
    /// Create an alias to an existing type
    Alias(TypeName),
}
