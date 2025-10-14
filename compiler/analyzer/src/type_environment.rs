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

use crate::intermediate_type::{ByteSized, IntermediateType};

static ELEMENTARY_TYPES_LOWER_CASE: [(&str, IntermediateType); 23] = [
    // signed_integer_type_name
    (
        "sint",
        IntermediateType::Int {
            size: ByteSized::B8,
        },
    ),
    (
        "int",
        IntermediateType::Int {
            size: ByteSized::B16,
        },
    ),
    (
        "dint",
        IntermediateType::Int {
            size: ByteSized::B32,
        },
    ),
    (
        "lint",
        IntermediateType::Int {
            size: ByteSized::B64,
        },
    ),
    // unsigned_integer_type_name
    (
        "usint",
        IntermediateType::UInt {
            size: ByteSized::B8,
        },
    ),
    (
        "uint",
        IntermediateType::UInt {
            size: ByteSized::B16,
        },
    ),
    (
        "udint",
        IntermediateType::UInt {
            size: ByteSized::B32,
        },
    ),
    (
        "ulint",
        IntermediateType::UInt {
            size: ByteSized::B64,
        },
    ),
    // real_type_name
    (
        "real",
        IntermediateType::Real {
            size: ByteSized::B32,
        },
    ),
    (
        "lreal",
        IntermediateType::Real {
            size: ByteSized::B64,
        },
    ),
    // date_type_name
    ("date", IntermediateType::Date),
    ("time_of_day", IntermediateType::Time),
    ("tod", IntermediateType::Time),
    ("date_and_time", IntermediateType::Date),
    ("dt", IntermediateType::Date),
    // bit_string_type_name
    ("bool", IntermediateType::Bool),
    (
        "byte",
        IntermediateType::Bytes {
            size: ByteSized::B8,
        },
    ),
    (
        "word",
        IntermediateType::Bytes {
            size: ByteSized::B16,
        },
    ),
    (
        "dword",
        IntermediateType::Bytes {
            size: ByteSized::B32,
        },
    ),
    (
        "lword",
        IntermediateType::Bytes {
            size: ByteSized::B64,
        },
    ),
    // remaining elementary_type_name
    ("string", IntermediateType::String { max_len: None }),
    ("wstring", IntermediateType::String { max_len: None }),
    ("time", IntermediateType::Time),
];

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

    #[test]
    fn intermediate_type_helper_methods_work_correctly() {
        // Test primitive types
        assert!(IntermediateType::Bool.is_primitive());
        assert!(IntermediateType::Int {
            size: ByteSized::B16
        }
        .is_primitive());
        assert!(IntermediateType::UInt {
            size: ByteSized::B32
        }
        .is_primitive());
        assert!(IntermediateType::Real {
            size: ByteSized::B64
        }
        .is_primitive());
        assert!(IntermediateType::String { max_len: Some(10) }.is_primitive());
        assert!(IntermediateType::Time.is_primitive());
        assert!(IntermediateType::Date.is_primitive());

        // Test non-primitive types
        assert!(!IntermediateType::Enumeration {
            underlying_type: Box::new(IntermediateType::Int {
                size: ByteSized::B8
            })
        }
        .is_primitive());
        assert!(!IntermediateType::Structure { fields: vec![] }.is_primitive());
        assert!(!IntermediateType::Array {
            element_type: Box::new(IntermediateType::Int {
                size: ByteSized::B16
            }),
            size: Some(10)
        }
        .is_primitive());

        // Test numeric types
        assert!(IntermediateType::Int {
            size: ByteSized::B16
        }
        .is_numeric());
        assert!(IntermediateType::UInt {
            size: ByteSized::B32
        }
        .is_numeric());
        assert!(IntermediateType::Real {
            size: ByteSized::B64
        }
        .is_numeric());
        assert!(!IntermediateType::Bool.is_numeric());
        assert!(!IntermediateType::String { max_len: Some(10) }.is_numeric());

        // Test integer types
        assert!(IntermediateType::Int {
            size: ByteSized::B16
        }
        .is_integer());
        assert!(IntermediateType::UInt {
            size: ByteSized::B32
        }
        .is_integer());
        assert!(!IntermediateType::Real {
            size: ByteSized::B64
        }
        .is_integer());
        assert!(!IntermediateType::Bool.is_integer());

        // Test subrange types
        let subrange = IntermediateType::Subrange {
            base_type: Box::new(IntermediateType::Int {
                size: ByteSized::B16,
            }),
            min_value: 1,
            max_value: 100,
        };
        assert!(subrange.is_subrange());
        assert!(!subrange.is_primitive());

        // Test function block types
        let fb_type = IntermediateType::FunctionBlock {
            name: "MyFB".to_string(),
        };
        assert!(fb_type.is_function_block());
        assert!(!fb_type.is_primitive());

        // Test function types
        let func_type = IntermediateType::Function {
            return_type: Some(Box::new(IntermediateType::Int {
                size: ByteSized::B16,
            })),
            parameters: vec![],
        };
        assert!(func_type.is_function());
        assert!(!func_type.is_primitive());
    }

    #[test]
    fn type_environment_builder_with_elementary_types() {
        let env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();

        // Check that elementary types are present
        assert!(env.get(&TypeName::from("bool")).is_some());
        assert!(env.get(&TypeName::from("int")).is_some());
        assert!(env.get(&TypeName::from("real")).is_some());
        assert!(env.get(&TypeName::from("string")).is_some());
        assert!(env.get(&TypeName::from("time")).is_some());
        assert!(env.get(&TypeName::from("date")).is_some());

        // Check specific type representations
        let int_type = env.get(&TypeName::from("int")).unwrap();
        assert!(matches!(
            &int_type.representation,
            IntermediateType::Int {
                size: ByteSized::B16
            }
        ));

        let bool_type = env.get(&TypeName::from("bool")).unwrap();
        assert!(matches!(&bool_type.representation, IntermediateType::Bool));
    }

    #[test]
    fn type_environment_is_enumeration_helper() {
        let mut env = TypeEnvironment::new();

        // Add an enumeration type
        env.insert_type(
            &TypeName::from("MY_ENUM"),
            TypeAttributes {
                span: SourceSpan::default(),
                representation: IntermediateType::Enumeration {
                    underlying_type: Box::new(IntermediateType::Int {
                        size: ByteSized::B8,
                    }),
                },
            },
        )
        .unwrap();

        // Add a non-enumeration type
        env.insert_type(
            &TypeName::from("MY_INT"),
            TypeAttributes {
                span: SourceSpan::default(),
                representation: IntermediateType::Int {
                    size: ByteSized::B16,
                },
            },
        )
        .unwrap();

        // Test the helper method
        assert!(env.is_enumeration(&TypeName::from("MY_ENUM")));
        assert!(!env.is_enumeration(&TypeName::from("MY_INT")));
        assert!(!env.is_enumeration(&TypeName::from("NONEXISTENT")));
    }
}
