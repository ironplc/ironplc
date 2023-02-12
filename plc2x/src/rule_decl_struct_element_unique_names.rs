//! Semantic rule that checks that the element names in a structure type
//! declaration are unique.
//!
//! See 2.3.3.2.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//!     CUSTOM_STRUCT : STRUCT
//!         NAME: BOOL;
//!     END_STRUCT;
//! END_TYPE
//! ```
//!
//! ## Fails
//! ```ignore
//! TYPE
//!     CUSTOM_STRUCT : STRUCT
//!         NAME: BOOL;
//!         NAME: BOOL;
//!     END_STRUCT;
//! END_TYPE
//! ```
use crate::error::SemanticDiagnostic;
use ironplc_dsl::{common::*, core::*, visitor::Visitor};
use std::collections::HashSet;

pub fn apply(lib: &Library) -> Result<(), SemanticDiagnostic> {
    let mut visitor = RuleStructElementNamesUnique {};
    visitor.walk(lib)
}

struct RuleStructElementNamesUnique {}

impl Visitor<SemanticDiagnostic> for RuleStructElementNamesUnique {
    type Value = ();

    fn visit_structure_declaration(
        &mut self,
        node: &StructureDeclaration,
    ) -> Result<Self::Value, SemanticDiagnostic> {
        let mut element_names: HashSet<&Id> = HashSet::new();

        for element in &node.elements {
            let seen = element_names.get(&element.name);
            match seen {
                Some(first) => {
                    return Err(SemanticDiagnostic::error(
                        "S0001",
                        format!(
                            "Structure {} has duplicated element name {}",
                            node.type_name, element.name
                        ),
                    )
                    .maybe_with_label(node.type_name.position(), "Structure")
                    .maybe_with_label(first.position(), "First use of name")
                    .maybe_with_label(element.name.position(), "Dupliced use of name"));
                }
                None => {
                    element_names.insert(&element.name);
                }
            }
            if element_names.contains(&element.name) {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::stages::parse;

    #[test]
    fn apply_when_structure_has_unique_names_then_ok() {
        let program = "
TYPE
    CUSTOM_STRUCT : STRUCT 
        NAME: BOOL;
    END_STRUCT;
END_TYPE";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_structure_has_duplicated_names_then_error() {
        let program = "
TYPE
    CUSTOM_STRUCT : STRUCT 
        NAME: BOOL;
        NAME: BOOL;
    END_STRUCT;
END_TYPE";

        let library = parse(program).unwrap();
        let result = apply(&library);

        assert!(result.is_err());
    }
}
