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
use ironplc_dsl::{
    common::*,
    core::*,
    diagnostic::{Diagnostic, Label},
    visitor::Visitor,
};
use ironplc_problems::Problem;
use std::collections::HashSet;

use crate::{
    result::SemanticResult, symbol_environment::SymbolEnvironment,
    type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    _type_environment: &TypeEnvironment,
    _symbol_environment: &SymbolEnvironment,
) -> SemanticResult {
    let mut visitor = RuleStructElementNamesUnique {
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleStructElementNamesUnique {
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<Diagnostic> for RuleStructElementNamesUnique {
    type Value = ();

    fn visit_structure_declaration(
        &mut self,
        node: &StructureDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        let mut element_names: HashSet<&Id> = HashSet::new();

        for element in &node.elements {
            let seen = element_names.get(&element.name);
            match seen {
                Some(first) => {
                    self.diagnostics.push(
                        Diagnostic::problem(
                            Problem::StructureDuplicatedElement,
                            Label::span(node.type_name.span(), "Structure"),
                        )
                        .with_context_type("structure", &node.type_name)
                        .with_context_id("element", &element.name)
                        .with_secondary(Label::span(first.span(), "First use of name"))
                        .with_secondary(Label::span(element.name.span(), "Second use of name")),
                    );
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
    use crate::test_helpers::parse_and_resolve_types;

    use super::*;

    #[test]
    fn apply_when_structure_has_unique_names_then_ok() {
        let program = "
TYPE
    CUSTOM_STRUCT : STRUCT 
        NAME: BOOL;
    END_STRUCT;
END_TYPE";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

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

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        assert!(result.is_err());
    }
}
