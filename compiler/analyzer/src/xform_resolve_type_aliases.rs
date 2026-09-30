//! Transformation rule that resolves enumeration type aliases by
//! duplicating the enumeration values of the base type to the alias type.
//!
//! This phase runs after both type and symbol environments are built.

use crate::symbol_environment::SymbolEnvironment;
use crate::type_environment::TypeEnvironment;
use ironplc_dsl::common::*;
use ironplc_dsl::diagnostic::Diagnostic;

pub fn apply(
    _lib: Library,
    type_environment: &TypeEnvironment,
    symbol_environment: &mut SymbolEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    let mut errors = Vec::new();

    // Find all enumeration aliases and duplicate their values
    for (type_name, _) in type_environment.iter() {
        if let Some(base_type) = find_base_type_for_alias(type_name, type_environment) {
            if !type_environment.is_enumeration(type_name) {
                continue;
            }
            if let Err(diagnostic) =
                symbol_environment.duplicate_enumeration_values_for_alias(&base_type, type_name)
            {
                errors.push(diagnostic);
            }
        }
    }

    if errors.is_empty() {
        Ok(_lib)
    } else {
        Err(errors)
    }
}

/// Find the base type for an alias by looking for types with the same representation
fn find_base_type_for_alias(
    alias_type: &TypeName,
    type_environment: &TypeEnvironment,
) -> Option<TypeName> {
    if let Some(alias_attrs) = type_environment.get(alias_type) {
        // Find the first type that has the same representation (the base type)
        for (other_name, other_attrs) in type_environment.iter() {
            if other_name != alias_type && other_attrs.representation == alias_attrs.representation
            {
                return Some(other_name.clone());
            }
        }
    }
    None
}
