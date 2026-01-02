//! Type categorization for the IronPLC compiler.
//!
//! This module provides functionality for categorizing types into elementary,
//! user-defined, and derived types according to IEC 61131-3 standards.

use crate::intermediate_type::IntermediateType;

/// Categorizes types into elementary, user-defined, or derived types.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeCategory {
    /// Built-in elementary types defined by IEC 61131-3
    Elementary,
    /// User-defined types (structures, enumerations)
    UserDefined,
    /// Derived types (subranges, arrays, aliases)
    Derived,
}

impl TypeCategory {
    /// Determines the category for a given intermediate type
    pub fn for_type(intermediate_type: &IntermediateType) -> Self {
        match intermediate_type {
            IntermediateType::Bool
            | IntermediateType::Int { .. }
            | IntermediateType::UInt { .. }
            | IntermediateType::Real { .. }
            | IntermediateType::Bytes { .. }
            | IntermediateType::Time
            | IntermediateType::Date
            | IntermediateType::String { .. } => TypeCategory::Elementary,
            IntermediateType::Structure { .. } | IntermediateType::Enumeration { .. } => {
                TypeCategory::UserDefined
            }
            IntermediateType::Subrange { .. } | IntermediateType::Array { .. } => {
                TypeCategory::Derived
            }
            IntermediateType::FunctionBlock { .. } | IntermediateType::Function { .. } => {
                TypeCategory::UserDefined
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intermediate_type::{ByteSized, IntermediateType};

    #[test]
    fn type_category_classification() {
        // Test elementary types
        assert_eq!(
            TypeCategory::for_type(&IntermediateType::Bool),
            TypeCategory::Elementary
        );
        assert_eq!(
            TypeCategory::for_type(&IntermediateType::Int {
                size: ByteSized::B16
            }),
            TypeCategory::Elementary
        );
        assert_eq!(
            TypeCategory::for_type(&IntermediateType::String { max_len: None }),
            TypeCategory::Elementary
        );

        // Test user-defined types
        assert_eq!(
            TypeCategory::for_type(&IntermediateType::Structure { fields: vec![] }),
            TypeCategory::UserDefined
        );
        assert_eq!(
            TypeCategory::for_type(&IntermediateType::Enumeration {
                underlying_type: Box::new(IntermediateType::Int {
                    size: ByteSized::B8
                })
            }),
            TypeCategory::UserDefined
        );

        // Test derived types
        assert_eq!(
            TypeCategory::for_type(&IntermediateType::Subrange {
                base_type: Box::new(IntermediateType::Int {
                    size: ByteSized::B16
                }),
                min_value: 1,
                max_value: 100
            }),
            TypeCategory::Derived
        );
        assert_eq!(
            TypeCategory::for_type(&IntermediateType::Array {
                element_type: Box::new(IntermediateType::Bool),
                size: Some(10)
            }),
            TypeCategory::Derived
        );
    }
}
