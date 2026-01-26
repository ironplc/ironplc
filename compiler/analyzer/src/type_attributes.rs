//! Type attributes for the IronPLC compiler.
//!
//! This module provides the TypeAttributes struct that combines type representation
//! with categorization information. Memory layout information is accessed directly
//! through IntermediateType methods.

use ironplc_dsl::core::{Located, SourceSpan};

use crate::{intermediate_type::IntermediateType, type_category::TypeCategory};

/// Attributes associated with a type in the type environment.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAttributes {
    /// The location in source code that defined the type.
    /// TODO this should be unnecessary since the TypeName already has a span.
    pub span: SourceSpan,
    /// The intermediate representation of the type
    pub representation: IntermediateType,
    /// Category of the type (elementary, user-defined, or derived)
    pub type_category: TypeCategory,
}

impl TypeAttributes {
    /// Creates new TypeAttributes with calculated type category
    pub fn new(span: SourceSpan, representation: IntermediateType) -> Self {
        let type_category = TypeCategory::for_type(&representation);
        Self {
            span,
            representation,
            type_category,
        }
    }

    /// Creates new TypeAttributes for elementary types
    pub fn elementary(representation: IntermediateType) -> Self {
        Self::new(SourceSpan::default(), representation)
    }

    /// Gets the size in bytes for this type (delegates to IntermediateType)
    pub fn size_bytes(&self) -> u32 {
        self.representation.size_in_bytes() as u32
    }

    /// Gets the alignment requirement in bytes for this type (delegates to IntermediateType)
    pub fn alignment_bytes(&self) -> u32 {
        self.representation.alignment_bytes() as u32
    }

    /// Returns whether this type has an explicitly specified size (delegates to IntermediateType)
    pub fn has_explicit_size(&self) -> bool {
        self.representation.has_explicit_size()
    }
}

impl Located for TypeAttributes {
    fn span(&self) -> SourceSpan {
        self.span.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intermediate_type::{ByteSized, IntermediateType};

    #[test]
    fn type_attributes_constructor_sets_correct_fields() {
        let attrs = TypeAttributes::new(
            SourceSpan::default(),
            IntermediateType::Int {
                size: ByteSized::B32,
            },
        );

        assert_eq!(attrs.size_bytes(), 4);
        assert_eq!(attrs.alignment_bytes(), 4);
        assert!(attrs.has_explicit_size());
        assert_eq!(attrs.type_category, TypeCategory::Elementary);
    }

    #[test]
    fn type_attributes_elementary_constructor() {
        let attrs = TypeAttributes::elementary(IntermediateType::Bool);

        assert_eq!(attrs.size_bytes(), 1);
        assert_eq!(attrs.alignment_bytes(), 1);
        assert!(attrs.has_explicit_size());
        assert_eq!(attrs.type_category, TypeCategory::Elementary);
        assert_eq!(attrs.span, SourceSpan::default());
    }

    #[test]
    fn type_attributes_convenience_methods_delegate_to_intermediate_type() {
        let attrs = TypeAttributes::new(
            SourceSpan::default(),
            IntermediateType::Real {
                size: ByteSized::B64,
            },
        );

        // Test that convenience methods return the same values as IntermediateType methods
        assert_eq!(
            attrs.size_bytes(),
            attrs.representation.size_in_bytes() as u32
        );
        assert_eq!(
            attrs.alignment_bytes(),
            attrs.representation.alignment_bytes() as u32
        );
        assert_eq!(
            attrs.has_explicit_size(),
            attrs.representation.has_explicit_size()
        );
    }
}
