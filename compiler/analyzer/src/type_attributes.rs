//! Type attributes for the IronPLC compiler.
//!
//! This module provides the TypeAttributes struct that the type environment
//! holds for each type. Memory layout information is accessed directly
//! through SemanticType methods.

use crate::{semantic_type::SemanticType, standard_function_block::StandardFunctionBlock};

/// Attributes associated with a type in the type environment.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeAttributes {
    /// The semantic type that describes the type
    pub representation: SemanticType,
    /// Which standard function block the type is, for the types the type
    /// environment provides for them; `None` for every other type, including
    /// a user-defined block of the same name.
    pub standard_block: Option<StandardFunctionBlock>,
}

impl TypeAttributes {
    /// Creates new TypeAttributes for a type that is not a standard function
    /// block.
    pub fn new(representation: SemanticType) -> Self {
        Self {
            representation,
            standard_block: None,
        }
    }

    /// Records that the type is the standard function block `block`.
    pub fn with_standard_block(mut self, block: StandardFunctionBlock) -> Self {
        self.standard_block = Some(block);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_type::{ByteSized, SemanticType};

    #[test]
    fn new_when_called_then_not_standard_block() {
        let attrs = TypeAttributes::new(SemanticType::Int {
            size: ByteSized::B32,
        });

        assert_eq!(attrs.standard_block, None);
    }
}
