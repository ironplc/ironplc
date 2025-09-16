//! Type environment describes "what is needed to implement the type
//! as machine code". The type environment contains both types defined
//! by the language and user-defined types.
use std::collections::HashMap;

use ironplc_dsl::{
    common::TypeName,
    core::{Located, SourceSpan},
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;

static ELEMENTARY_TYPES_LOWER_CASE: [(&str, IntermediateType); 23] = [
    // signed_integer_type_name
    ("sint", IntermediateType::Int { size: 8 }),
    ("int", IntermediateType::Int { size: 16 }),
    ("dint", IntermediateType::Int { size: 32 }),
    ("lint", IntermediateType::Int { size: 64 }),
    // unsigned_integer_type_name
    ("usint", IntermediateType::UInt { size: 8 }),
    ("uint", IntermediateType::UInt { size: 16 }),
    ("udint", IntermediateType::UInt { size: 32 }),
    ("ulint", IntermediateType::UInt { size: 64 }),
    // real_type_name
    ("real", IntermediateType::Real { size: 32 }),
    ("lreal", IntermediateType::Real { size: 64 }),
    // date_type_name
    ("date", IntermediateType::Date),
    ("time_of_day", IntermediateType::Time),
    ("tod", IntermediateType::Time),
    ("date_and_time", IntermediateType::Date),
    ("dt", IntermediateType::Date),
    // bit_string_type_name
    ("bool", IntermediateType::Bool),
    ("byte", IntermediateType::Bytes { size: 8 }),
    ("word", IntermediateType::Bytes { size: 16 }),
    ("dword", IntermediateType::Bytes { size: 32 }),
    ("lword", IntermediateType::Bytes { size: 64 }),
    // remaining elementary_type_name
    ("string", IntermediateType::String { max_len: None }),
    ("wstring", IntermediateType::String { max_len: None }),
    ("time", IntermediateType::Time),
];

#[derive(Debug, Clone, PartialEq)]
pub enum IntermediateType {
    // Elementary types
    Bool,
    Int {
        size: u8,
    }, // 8, 16, 32, 64 bits
    UInt {
        size: u8,
    },
    Real {
        size: u8,
    }, // 32, 64 bits
    Bytes {
        size: u8,
    },
    Time,
    Date,

    String {
        max_len: Option<u128>,
    },

    // User-defined types
    Enumeration {
        underlying_type: Box<IntermediateType>, // Usually Int { size: 8 }
    },
    Structure {
        fields: Vec<IntermediateStructField>,
    },
    Array {
        element_type: Box<IntermediateType>,
        size: Option<u32>, // Fixed size or dynamic
    },
}

impl IntermediateType {
    /// Returns if the type is a primitive type.
    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            IntermediateType::Bool
                | IntermediateType::Int { .. }
                | IntermediateType::Real { .. }
                | IntermediateType::String { .. }
                | IntermediateType::Time
                | IntermediateType::Date
        )
    }

    /// Returns if the type is an enumeration.
    pub fn is_enumeration(&self) -> bool {
        matches!(self, IntermediateType::Enumeration { .. })
    }

    /// Returns if the type is a structure.
    pub fn is_structure(&self) -> bool {
        matches!(self, IntermediateType::Structure { .. })
    }

    /// Returns if the type is an array.
    pub fn is_array(&self) -> bool {
        matches!(self, IntermediateType::Array { .. })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntermediateStructField {
    pub name: TypeName,
    pub field_type: IntermediateType,
    pub offset: u32, // For memory layout
}

#[derive(Debug, Clone)]
pub struct TypeAttributes {
    /// The location in source code that defined the type.
    /// TODO this should be unnecessary since the TypeName already has a span.
    pub span: SourceSpan,
    pub representation: IntermediateType,
}

#[derive(Debug)]
pub struct TypeEnvironment {
    table: HashMap<TypeName, TypeAttributes>,
}

impl Located for TypeAttributes {
    fn span(&self) -> ironplc_dsl::core::SourceSpan {
        self.span.clone()
    }
}

impl TypeEnvironment {
    /// Initializes a new instance of the type environment.
    pub fn new() -> Self {
        Self {
            table: HashMap::new(),
        }
    }

    /// Adds the type into the environment.
    ///
    /// Returns an error if a type already exists with the name
    /// and does not insert the type.
    pub fn insert_type(
        &mut self,
        type_name: &TypeName,
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

    /// Adds an alias type into the environment.
    ///
    /// Returns an error if a type already exists with the name
    /// and does not insert the type.
    ///
    /// Returns an error if the base type is not already in the type
    /// environment.
    pub fn insert_alias(
        &mut self,
        type_name: &TypeName,
        base_type_name: &TypeName,
    ) -> Result<(), Diagnostic> {
        let base_intermediate_type = self.table.get(base_type_name).ok_or_else(|| {
            Diagnostic::problem(
                Problem::AliasParentTypeNotDeclared,
                Label::span(type_name.span(), "Type name"),
            )
            .with_secondary(Label::span(base_type_name.span(), "Missing declaration"))
        })?;

        self.insert_type(type_name, base_intermediate_type.clone())
    }

    /// Gets the type from the environment.
    pub fn get(&self, type_name: &TypeName) -> Option<&TypeAttributes> {
        self.table.get(type_name)
    }

    /// Returns if the type is an enumeration.
    pub fn is_enumeration(&self, name: &TypeName) -> bool {
        self.table
            .get(name)
            .map(|ty| ty.representation.is_enumeration())
            .unwrap_or(false)
    }

    // Note: removed is_structure(name) to avoid duplicate API with representation helpers

    /// An iterator for all types in the environment
    pub fn iter(&self) -> impl Iterator<Item = (&TypeName, &TypeAttributes)> {
        self.table.iter()
    }
}

pub struct TypeEnvironmentBuilder {
    has_elementary_types: bool,
}

impl TypeEnvironmentBuilder {
    /// Initializes a new instance of the type environment builder.
    pub fn new() -> Self {
        Self {
            has_elementary_types: false,
        }
    }

    /// Adds the elementary types to the type environment.
    /// The elementary types are the types that are built into the language.
    pub fn with_elementary_types(mut self) -> Self {
        self.has_elementary_types = true;
        self
    }

    /// Builds the type environment.
    pub fn build(self) -> Result<TypeEnvironment, Diagnostic> {
        let mut env = TypeEnvironment::new();
        if self.has_elementary_types {
            for (name, representation) in ELEMENTARY_TYPES_LOWER_CASE.iter() {
                env.insert_type(
                    &TypeName::from(name),
                    TypeAttributes {
                        span: SourceSpan::default(),
                        representation: representation.clone(),
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
    fn insert_type_when_type_already_exists_then_error() {
        let mut env = TypeEnvironment::new();
        assert!(env
            .insert_type(
                &TypeName::from("TYPE"),
                TypeAttributes {
                    span: SourceSpan::default(),
                    representation: IntermediateType::Bool,
                }
            )
            .is_ok());

        assert!(env
            .insert_type(
                &TypeName::from("TYPE"),
                TypeAttributes {
                    span: SourceSpan::default(),
                    representation: IntermediateType::Bool,
                }
            )
            .is_err());
    }

    #[test]
    fn insert_alias_when_type_already_exists_then_ok() {
        let mut env = TypeEnvironment::new();
        env.insert_type(
            &TypeName::from("TYPE"),
            TypeAttributes {
                span: SourceSpan::default(),
                representation: IntermediateType::Bool,
            },
        )
        .unwrap();
        assert!(env
            .insert_alias(&TypeName::from("TYPE_ALIAS"), &TypeName::from("TYPE"))
            .is_ok());
    }

    #[test]
    fn insert_alias_when_type_doesnt_exist_then_error() {
        let mut env = TypeEnvironment::new();
        assert!(env
            .insert_alias(&TypeName::from("TYPE_ALIAS"), &TypeName::from("TYPE"))
            .is_err());
    }
}
