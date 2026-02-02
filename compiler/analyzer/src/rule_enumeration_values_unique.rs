//! Semantic rule that enumeration values in declarations must be unique.
//!
//! ## Passes
//!
//! ```ignore
//! TYPE
//! LOGLEVEL : (CRITICAL) := CRITICAL;
//! END_TYPE
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! TYPE
//! LOGLEVEL : (CRITICAL, CRITICAL) := CRITICAL;
//! END_TYPE
//! ```
use ironplc_dsl::{
    common::*,
    core::{Id, Located},
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
    let mut visitor = RuleEnumerationValuesUnique {
        diagnostics: Vec::new(),
    };
    visitor.walk(lib).map_err(|e| vec![e])?;

    if !visitor.diagnostics.is_empty() {
        return Err(visitor.diagnostics);
    }
    Ok(())
}

struct RuleEnumerationValuesUnique {
    diagnostics: Vec<Diagnostic>,
}

impl Visitor<Diagnostic> for RuleEnumerationValuesUnique {
    type Value = ();

    fn visit_enumeration_declaration(
        &mut self,
        node: &EnumerationDeclaration,
    ) -> Result<(), Diagnostic> {
        match &node.spec_init.spec {
            SpecificationKind::Named(_) => Ok(()),
            SpecificationKind::Inline(spec) => {
                let mut seen_values: HashSet<&Id> = HashSet::new();
                for current in &spec.values {
                    // TODO this needs to be updated - this doesn't do
                    // a comparison that includes the type of the enumeration
                    let seen = seen_values.get(&current.value);
                    match seen {
                        Some(first) => {
                            self.diagnostics.push(
                                Diagnostic::problem(
                                    Problem::EnumTypeDeclDuplicateItem,
                                    Label::span(first.span(), "First instance"),
                                )
                                .with_context_type("declaration", &node.type_name)
                                .with_context_id("duplicate value", first)
                                .with_secondary(Label::span(current.span(), "Duplicate value")),
                            );
                        }
                        None => {
                            seen_values.insert(&current.value);
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::parse_and_resolve_types;

    use super::*;

    #[test]
    fn apply_when_values_unique_then_ok() {
        let program = "
TYPE
LOGLEVEL : (CRITICAL, ERROR);
END_TYPE";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_typename_values_unique_then_ok() {
        let program = "
TYPE
LOGLEVEL : (CRITICAL, ERROR);
LOGLEVEL2 : LOGLEVEL;
END_TYPE";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        assert!(result.is_ok());
    }

    #[test]
    fn apply_when_value_duplicated_then_error() {
        let program = "
TYPE
LOGLEVEL : (CRITICAL, CRITICAL);
END_TYPE";

        let library = parse_and_resolve_types(program);
        let type_env = TypeEnvironment::new();
        let symbol_env = SymbolEnvironment::new();
        let result = apply(&library, &type_env, &symbol_env);

        assert!(result.is_err());
    }
}
