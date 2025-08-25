//! Type environment stores the type definitions. The type
//! environment contains both types defined by the language
//! and user-defined types.
use std::collections::HashMap;

use ironplc_dsl::{
    common::Type,
    core::{Located, SourceSpan},
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;
use phf::{phf_set, Set};

static ELEMENTARY_TYPES_LOWER_CASE: Set<&'static str> = phf_set! {
    // signed_integer_type_name
    "sint",
    "int",
    "dint",
    "lint",
    // unsigned_integer_type_name
    "usint",
    "uint",
    "udint",
    "ulint",
    // real_type_name
    "real",
    "lreal",
    // date_type_name
    "date",
    "time_of_day",
    "tod",
    "date_and_time",
    "dt",
    // bit_string_type_name
    "bool",
    "byte",
    "word",
    "dword",
    "lword",
    // remaining elementary_type_name
    "string",
    "wstring",
    "time",
};

#[derive(Debug, PartialEq)]
pub enum TypeClass {
    Simple,
    Enumeration,
    Structure,
}

#[derive(Debug)]
pub struct TypeAttributes {
    pub span: SourceSpan,
    pub class: TypeClass,
    /// For alias types, stores the base type name
    pub base_type: Option<Type>,
}

#[derive(Debug)]
pub struct TypeEnvironment {
    table: HashMap<Type, TypeAttributes>,
}

impl Located for TypeAttributes {
    fn span(&self) -> ironplc_dsl::core::SourceSpan {
        self.span.clone()
    }
}

impl TypeEnvironment {
    /// Initializes a new instance of the type environment.
    pub(crate) fn new() -> Self {
        Self {
            table: HashMap::new(),
        }
    }

    /// Adds the type into the environment.
    ///
    /// Returns a diagnostics if a type already exists with the name
    /// and does not insert the type.
    pub(crate) fn insert(
        &mut self,
        type_name: &Type,
        symbol: TypeAttributes,
    ) -> Result<(), Diagnostic> {
        self.table.insert(type_name.clone(), symbol).map_or_else(
            || Ok(()),
            |existing| {
                Err(Diagnostic::problem(
                    Problem::TypeDeclNameDuplicated,
                    Label::span(type_name.span(), "Duplicate declaration"),
                )
                .with_secondary(Label::span(existing.span(), "First declaration")))
            },
        )
    }

    /// Gets the type from the environment.
    pub(crate) fn get(&self, type_name: &Type) -> Option<&TypeAttributes> {
        self.table.get(type_name)
    }

    /// Checks if a type is an enumeration.
    pub(crate) fn is_enumeration(&self, type_name: &Type) -> bool {
        self.table
            .get(type_name)
            .map(|attrs| matches!(attrs.class, TypeClass::Enumeration))
            .unwrap_or(false)
    }

    /// Gets the base type for an alias type, following the alias chain.
    /// Returns the final non-alias type in the chain.
    ///
    /// # Arguments
    /// * `type_name` - The type to resolve
    ///
    /// # Returns
    /// * `Ok(base_type)` - The resolved type (could be the original type if no alias)
    ///
    /// # Errors
    /// * Returns an error if there's a circular reference in the alias chain
    /// * Returns an error if the type is not found
    pub(crate) fn resolve_alias(&self, type_name: &Type) -> Result<Type, Diagnostic> {
        let mut visited = std::collections::HashSet::new();
        let mut current = type_name.clone();

        while let Some(attrs) = self.table.get(&current) {
            if let Some(ref base_type) = attrs.base_type {
                // Check for circular references
                if !visited.insert(current.clone()) {
                    return Err(Diagnostic::problem(
                        Problem::RecursiveTypeCycle,
                        Label::span(current.span(), "Circular type reference detected"),
                    ));
                }
                current = base_type.clone();
            } else {
                // This is not an alias, return it as the base type
                return Ok(current);
            }
        }

        // Type not found
        Err(Diagnostic::problem(
            Problem::UndeclaredUnknownType,
            Label::span(type_name.span(), "Type not found"),
        ))
    }

    /// Gets the base type for an enumeration, resolving any aliases.
    /// This is a convenience method that combines alias resolution with enumeration checking.
    ///
    /// # Arguments
    /// * `type_name` - The type to resolve
    ///
    /// # Returns
    /// * `Ok(base_type)` if the type resolves to an enumeration
    ///
    /// # Errors
    /// * Returns an error if there's a circular reference in the alias chain
    /// * Returns an error if the type is not found
    /// * Returns an error if the type is not an enumeration
    pub(crate) fn resolve_enumeration_alias(&self, type_name: &Type) -> Result<Type, Diagnostic> {
        let base_type = self.resolve_alias(type_name)?;
        if self.is_enumeration(&base_type) {
            Ok(base_type)
        } else {
            Err(Diagnostic::problem(
                Problem::EnumNotDeclared,
                Label::span(base_type.span(), "Type is not an enumeration"),
            )
            .with_context_type("name", &base_type))
        }
    }
}

pub(crate) struct TypeEnvironmentBuilder {
    has_elementary_types: bool,
}

impl TypeEnvironmentBuilder {
    pub fn new() -> Self {
        Self {
            has_elementary_types: false,
        }
    }

    pub fn with_elementary_types(mut self) -> Self {
        self.has_elementary_types = true;
        self
    }

    pub fn build(self) -> Result<TypeEnvironment, Diagnostic> {
        let mut env = TypeEnvironment::new();
        if self.has_elementary_types {
            for name in ELEMENTARY_TYPES_LOWER_CASE.iter() {
                env.insert(
                    &Type::from(name),
                    TypeAttributes {
                        span: SourceSpan::default(),
                        class: TypeClass::Simple,
                        base_type: None,
                    },
                )?;
            }
        }
        Ok(env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_alias_when_type_is_alias_then_resolves_to_base_type() {
        let mut env = TypeEnvironment::new();

        // Add a base type
        let base_type = Type::from("BASE");
        env.insert(
            &base_type,
            TypeAttributes {
                span: SourceSpan::default(),
                class: TypeClass::Enumeration,
                base_type: None,
            },
        )
        .unwrap();

        // Add an alias type
        let alias_type = Type::from("ALIAS");
        let base_type_clone = base_type.clone();
        env.insert(
            &alias_type,
            TypeAttributes {
                span: SourceSpan::default(),
                class: TypeClass::Enumeration,
                base_type: Some(base_type_clone),
            },
        )
        .unwrap();

        // Test alias resolution
        let resolved = env.resolve_alias(&alias_type).unwrap();
        assert_eq!(resolved, base_type.clone());

        // Test base type resolution (should return itself)
        let resolved = env.resolve_alias(&base_type).unwrap();
        assert_eq!(resolved, base_type);
    }

    #[test]
    fn resolve_enumeration_alias_when_type_is_enumeration_alias_then_resolves_to_base_enumeration()
    {
        let mut env = TypeEnvironment::new();

        // Add a base enumeration type
        let base_type = Type::from("BASE");
        env.insert(
            &base_type,
            TypeAttributes {
                span: SourceSpan::default(),
                class: TypeClass::Enumeration,
                base_type: None,
            },
        )
        .unwrap();

        // Add an alias type
        let alias_type = Type::from("ALIAS");
        let base_type_clone = base_type.clone();
        env.insert(
            &alias_type,
            TypeAttributes {
                span: SourceSpan::default(),
                class: TypeClass::Enumeration,
                base_type: Some(base_type_clone),
            },
        )
        .unwrap();

        // Test enumeration alias resolution
        let resolved = env.resolve_enumeration_alias(&alias_type).unwrap();
        assert_eq!(resolved, base_type.clone());

        // Test base type resolution (should return itself)
        let resolved = env.resolve_enumeration_alias(&base_type).unwrap();
        assert_eq!(resolved, base_type);
    }

    #[test]
    fn resolve_alias_when_circular_reference_two_levels_then_returns_error() {
        let mut env = TypeEnvironment::new();

        // Create a circular reference: A -> B -> A
        let type_a = Type::from("A");
        let type_b = Type::from("B");

        env.insert(
            &type_a,
            TypeAttributes {
                span: SourceSpan::default(),
                class: TypeClass::Enumeration,
                base_type: Some(type_b.clone()),
            },
        )
        .unwrap();

        env.insert(
            &type_b,
            TypeAttributes {
                span: SourceSpan::default(),
                class: TypeClass::Enumeration,
                base_type: Some(type_a.clone()),
            },
        )
        .unwrap();

        // This should detect the circular reference
        let result = env.resolve_alias(&type_a);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, Problem::RecursiveTypeCycle.code());
    }

    #[test]
    fn resolve_alias_when_circular_reference_three_levels_then_returns_error() {
        let mut env = TypeEnvironment::new();

        // Create a deeper circular reference: A -> B -> C -> A
        let type_a = Type::from("A");
        let type_b = Type::from("B");
        let type_c = Type::from("C");

        env.insert(
            &type_a,
            TypeAttributes {
                span: SourceSpan::default(),
                class: TypeClass::Enumeration,
                base_type: Some(type_b.clone()),
            },
        )
        .unwrap();

        env.insert(
            &type_b,
            TypeAttributes {
                span: SourceSpan::default(),
                class: TypeClass::Enumeration,
                base_type: Some(type_c.clone()),
            },
        )
        .unwrap();

        env.insert(
            &type_c,
            TypeAttributes {
                span: SourceSpan::default(),
                class: TypeClass::Enumeration,
                base_type: Some(type_a.clone()),
            },
        )
        .unwrap();

        // This should detect the circular reference
        let result = env.resolve_alias(&type_a);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, Problem::RecursiveTypeCycle.code());
    }

    #[test]
    fn resolve_enumeration_alias_when_type_is_not_enumeration_then_returns_error() {
        let mut env = TypeEnvironment::new();

        // Add a simple type (not enumeration)
        let simple_type = Type::from("SIMPLE");
        env.insert(
            &simple_type,
            TypeAttributes {
                span: SourceSpan::default(),
                class: TypeClass::Simple,
                base_type: None,
            },
        )
        .unwrap();

        // Test that non-enumeration types return an error
        let result = env.resolve_enumeration_alias(&simple_type);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, Problem::EnumNotDeclared.code());
    }
}
