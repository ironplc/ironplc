//! Type categorization for the IronPLC compiler.
//!
//! This module provides functionality for categorizing types into elementary,
//! user-defined, and derived types according to IEC 61131-3 standards.

use crate::semantic_type::SemanticType;

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
    /// Determines the category for a given semantic type
    pub fn for_type(semantic_type: &SemanticType) -> Self {
        match semantic_type {
            SemanticType::Bool
            | SemanticType::Int { .. }
            | SemanticType::UInt { .. }
            | SemanticType::Real { .. }
            | SemanticType::Bytes { .. }
            | SemanticType::Time { .. }
            | SemanticType::Date { .. }
            | SemanticType::TimeOfDay { .. }
            | SemanticType::DateAndTime { .. }
            | SemanticType::String { .. } => TypeCategory::Elementary,
            SemanticType::Structure { .. } | SemanticType::Enumeration { .. } => {
                TypeCategory::UserDefined
            }
            SemanticType::Subrange { .. } | SemanticType::Array { .. } => TypeCategory::Derived,
            SemanticType::FunctionBlock { .. }
            | SemanticType::Function { .. }
            | SemanticType::Interface { .. } => TypeCategory::UserDefined,
            SemanticType::Reference { .. } => TypeCategory::Derived,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_type::{ArrayDimension, ByteSized, SemanticType};
    use ironplc_container::CharWidth;

    #[test]
    fn type_category_classification() {
        // Test elementary types
        assert_eq!(
            TypeCategory::for_type(&SemanticType::Bool),
            TypeCategory::Elementary
        );
        assert_eq!(
            TypeCategory::for_type(&SemanticType::Int {
                size: ByteSized::B16
            }),
            TypeCategory::Elementary
        );
        assert_eq!(
            TypeCategory::for_type(&SemanticType::String {
                max_len: None,
                char_width: CharWidth::Narrow,
            }),
            TypeCategory::Elementary
        );

        // Test user-defined types
        assert_eq!(
            TypeCategory::for_type(&SemanticType::Structure { fields: vec![] }),
            TypeCategory::UserDefined
        );
        assert_eq!(
            TypeCategory::for_type(&SemanticType::Enumeration {
                underlying_type: Box::new(SemanticType::Int {
                    size: ByteSized::B8
                }),
                members: crate::enumeration_members::EnumerationMembers::default(),
            }),
            TypeCategory::UserDefined
        );

        // Test derived types
        assert_eq!(
            TypeCategory::for_type(&SemanticType::Subrange {
                base_type: Box::new(SemanticType::Int {
                    size: ByteSized::B16
                }),
                min_value: 1,
                max_value: 100
            }),
            TypeCategory::Derived
        );
        assert_eq!(
            TypeCategory::for_type(&SemanticType::Array {
                element_type: Box::new(SemanticType::Bool),
                dimensions: vec![ArrayDimension { lower: 0, upper: 9 }]
            }),
            TypeCategory::Derived
        );
    }
}
