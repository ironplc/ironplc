//! Transformation rule that resolves type aliases by duplicating
//! relevant symbols from the base type to the alias type.
//!
//! This phase runs after both type and symbol environments are built,
//! and handles the duplication of symbols for all type aliases:
//! - Enumerations: duplicate enumeration values
//! - Structures: duplicate structure field symbols
//! - Arrays: duplicate array element type information
//! - Other types: handle as needed

use crate::symbol_environment::SymbolEnvironment;
use crate::type_environment::{IntermediateType, TypeEnvironment};
use ironplc_dsl::common::*;
use ironplc_dsl::diagnostic::Diagnostic;

pub fn apply(
    _lib: Library,
    type_environment: &TypeEnvironment,
    symbol_environment: &mut SymbolEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    let mut errors = Vec::new();

    // Find all type aliases and duplicate their relevant symbols
    for (type_name, type_attrs) in type_environment.iter() {
        if let Some(base_type) = find_base_type_for_alias(type_name, type_environment) {
            // Determine kind using helpers to exercise TypeEnvironment/IntermediateType helpers
            let is_enum = type_environment.is_enumeration(type_name);
            let is_struct = type_attrs.representation.is_structure();
            let rep = &type_attrs.representation;

            // This is an alias - duplicate relevant symbols based on type kind
            if let Err(diagnostic) = duplicate_alias_symbols(
                &base_type,
                type_name,
                rep,
                is_enum,
                is_struct,
                symbol_environment,
            ) {
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

/// Duplicate relevant symbols for a type alias based on the type kind
fn duplicate_alias_symbols(
    base_type: &TypeName,
    alias_type: &TypeName,
    type_representation: &IntermediateType,
    is_enum: bool,
    is_struct: bool,
    symbol_environment: &mut SymbolEnvironment,
) -> Result<(), Diagnostic> {
    if is_enum || type_representation.is_enumeration() {
        // For enumerations, duplicate enumeration values
        return symbol_environment.duplicate_enumeration_values_for_alias(base_type, alias_type);
    }

    if is_struct || type_representation.is_structure() {
        // For structures, duplicate structure field symbols
        return symbol_environment.duplicate_structure_fields_for_alias(base_type, alias_type);
    }

    if type_representation.is_array() {
        // For arrays, duplicate array element type information
        return symbol_environment.duplicate_array_elements_for_alias(base_type, alias_type);
    }

    // For simple types (INT, REAL, etc.), no duplication needed
    Ok(())
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
