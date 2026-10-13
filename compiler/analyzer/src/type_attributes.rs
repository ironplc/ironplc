//! Type attributes for the IronPLC compiler.
//!
//! This module provides the TypeAttributes struct that combines type representation
//! with categorization information. Memory layout information is accessed directly
//! through SemanticType methods.

use ironplc_dsl::core::{Located, SourceSpan};

use crate::{
    semantic_type::SemanticType, standard_function_block::StandardFunctionBlock,
    type_category::TypeCategory,
};

/// Attributes associated with a type in the type environment.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAttributes {
    /// The location in source code that defined the type.
    /// TODO this should be unnecessary since the TypeName already has a span.
    pub span: SourceSpan,
    /// The semantic type that describes the type
    pub representation: SemanticType,
    /// Category of the type (elementary, user-defined, or derived)
    pub type_category: TypeCategory,
    /// Which standard function block the type is, for the types the type
    /// environment provides for them; `None` for every other type, including
    /// a user-defined block of the same name.
    pub standard_block: Option<StandardFunctionBlock>,
}

impl TypeAttributes {
    /// Creates new TypeAttributes with calculated type category
    pub fn new(span: SourceSpan, representation: SemanticType) -> Self {
        let type_category = TypeCategory::for_type(&representation);
        Self {
            span,
            representation,
            type_category,
            standard_block: None,
        }
    }

    /// Records that the type is the standard function block `block`.
    pub fn with_standard_block(mut self, block: StandardFunctionBlock) -> Self {
        self.standard_block = Some(block);
        self
    }

    /// Creates new TypeAttributes for elementary types
    pub fn elementary(representation: SemanticType) -> Self {
        Self::new(SourceSpan::default(), representation)
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
    use crate::semantic_type::{ByteSized, SemanticType};

    #[test]
    fn type_attributes_constructor_sets_correct_fields() {
        let attrs = TypeAttributes::new(
            SourceSpan::default(),
            SemanticType::Int {
                size: ByteSized::B32,
            },
        );

        assert_eq!(attrs.type_category, TypeCategory::Elementary);
    }

    #[test]
    fn type_attributes_elementary_constructor() {
        let attrs = TypeAttributes::elementary(SemanticType::Bool);

        assert_eq!(attrs.type_category, TypeCategory::Elementary);
        assert_eq!(attrs.span, SourceSpan::default());
    }
}
