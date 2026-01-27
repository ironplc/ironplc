//! Type environment describes "what is needed to implement the type
//! as machine code". The type environment contains both types defined
//! by the language and user-defined types.
use std::collections::HashMap;

use ironplc_dsl::{
    common::TypeName,
    core::Located,
    diagnostic::{Diagnostic, Label},
};
use ironplc_problems::Problem;

use crate::intermediate_type::{ByteSized, IntermediateType};

/// Context for type usage validation
#[derive(Debug, Clone, PartialEq)]
pub enum UsageContext {
    /// Type used in variable declaration
    VariableDeclaration,
    /// Type used in function parameter
    FunctionParameter,
    /// Type used in function return type
    FunctionReturn,
    /// Type used in array element type
    ArrayElement,
    /// Type used in structure field
    StructureField,
    /// Type used in subrange base type
    SubrangeBase,
    /// Type used in enumeration underlying type
    EnumerationUnderlying,
    /// General type usage
    General,
}

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

#[derive(Debug)]
pub struct TypeEnvironment {
    table: HashMap<TypeName, crate::type_attributes::TypeAttributes>,
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
        symbol: crate::type_attributes::TypeAttributes,
    ) -> Result<(), Diagnostic> {
        self.table.insert(type_name.clone(), symbol).map_or_else(
            || Ok(()),
            |existing| {
                Err(Diagnostic::problem(
                    Problem::TypeDeclNameDuplicated,
                    Label::span(type_name.span(), "Type declaration"),
                )
                .with_secondary(Label::span(existing.span(), "Previous declaration")))
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
                Label::span(type_name.span(), "Type alias"),
            )
            .with_secondary(Label::span(base_type_name.span(), "Base type"))
        })?;

        self.insert_type(type_name, base_intermediate_type.clone())
    }

    /// Gets the type from the environment.
    pub fn get(&self, type_name: &TypeName) -> Option<&crate::type_attributes::TypeAttributes> {
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
    pub fn iter(
        &self,
    ) -> impl Iterator<Item = (&TypeName, &crate::type_attributes::TypeAttributes)> {
        self.table.iter()
    }

    /// Gets the memory size of a type by name
    ///
    /// Returns `Ok(Some(size))` if the type exists and has a known size,
    /// `Ok(None)` if the type exists but has unknown size (e.g., dynamic arrays),
    /// or `Err` if the type is not declared.
    #[allow(dead_code)]
    pub fn get_memory_size(&self, type_name: &TypeName) -> Result<Option<u32>, Diagnostic> {
        self.table
            .get(type_name)
            .map(|attrs| attrs.size_bytes())
            .ok_or_else(|| {
                Diagnostic::problem(
                    Problem::UndeclaredUnknownType,
                    Label::span(type_name.span(), "Type reference"),
                )
            })
    }

    /// Validates type usage in a specific context
    #[allow(dead_code)]
    pub fn validate_type_usage(
        &self,
        type_name: &TypeName,
        context: &UsageContext,
    ) -> Result<(), Diagnostic> {
        let type_attrs = self.table.get(type_name).ok_or_else(|| {
            Diagnostic::problem(
                Problem::UndeclaredUnknownType,
                Label::span(type_name.span(), "Type reference"),
            )
        })?;

        // Context-specific validation rules
        match context {
            UsageContext::SubrangeBase => {
                // Subrange base types must be numeric
                if !type_attrs.representation.is_numeric() {
                    return Err(Diagnostic::problem(
                        Problem::SubrangeBaseTypeNotNumeric,
                        Label::span(type_name.span(), "Subrange base type"),
                    ));
                }
            }
            UsageContext::EnumerationUnderlying => {
                // Enumeration underlying types must be integer types
                if !type_attrs.representation.is_integer() {
                    return Err(Diagnostic::problem(
                        Problem::UndeclaredUnknownType, // Using existing code for now
                        Label::span(type_name.span(), "Enumeration underlying type"),
                    ));
                }
            }
            UsageContext::ArrayElement => {
                // Array elements cannot be function blocks or functions
                if type_attrs.representation.is_function_block()
                    || type_attrs.representation.is_function()
                {
                    return Err(Diagnostic::problem(
                        Problem::UndeclaredUnknownType, // Using existing code for now
                        Label::span(type_name.span(), "Array element type"),
                    ));
                }
            }
            UsageContext::FunctionReturn => {
                // Function return types cannot be function blocks
                if type_attrs.representation.is_function_block() {
                    return Err(Diagnostic::problem(
                        Problem::UndeclaredUnknownType, // Using existing code for now
                        Label::span(type_name.span(), "Function return type"),
                    ));
                }
            }
            // Other contexts have no specific restrictions for now
            UsageContext::VariableDeclaration
            | UsageContext::FunctionParameter
            | UsageContext::StructureField
            | UsageContext::General => {}
        }

        Ok(())
    }

    /// Gets all types organized by category
    #[allow(dead_code)]
    pub fn get_all_types_by_category(
        &self,
    ) -> std::collections::HashMap<crate::type_category::TypeCategory, Vec<&TypeName>> {
        let mut result = std::collections::HashMap::new();
        for (name, attrs) in &self.table {
            result
                .entry(attrs.type_category.clone())
                .or_insert_with(Vec::new)
                .push(name);
        }
        result
    }
}

impl Default for TypeEnvironment {
    fn default() -> Self {
        Self::new()
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
                    crate::type_attributes::TypeAttributes::elementary(representation.clone()),
                )?;
            }
        }
        Ok(env)
    }
}

impl Default for TypeEnvironmentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export TypeAttributes for use by other modules
pub use crate::type_attributes::TypeAttributes;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        intermediate_type::{ByteSized, IntermediateType},
        type_attributes::TypeAttributes,
        type_category::TypeCategory,
    };
    use ironplc_dsl::core::SourceSpan;

    #[test]
    fn insert_type_when_type_already_exists_then_error() {
        let mut env = TypeEnvironment::new();
        assert!(env
            .insert_type(
                &TypeName::from("TYPE"),
                TypeAttributes::new(SourceSpan::default(), IntermediateType::Bool)
            )
            .is_ok());

        assert!(env
            .insert_type(
                &TypeName::from("TYPE"),
                TypeAttributes::new(SourceSpan::default(), IntermediateType::Bool)
            )
            .is_err());
    }

    #[test]
    fn insert_alias_when_type_already_exists_then_ok() {
        let mut env = TypeEnvironment::new();
        env.insert_type(
            &TypeName::from("TYPE"),
            TypeAttributes::new(SourceSpan::default(), IntermediateType::Bool),
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
            fields: vec![],
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
            TypeAttributes::new(
                SourceSpan::default(),
                IntermediateType::Enumeration {
                    underlying_type: Box::new(IntermediateType::Int {
                        size: ByteSized::B8,
                    }),
                },
            ),
        )
        .unwrap();

        // Add a non-enumeration type
        env.insert_type(
            &TypeName::from("MY_INT"),
            TypeAttributes::new(
                SourceSpan::default(),
                IntermediateType::Int {
                    size: ByteSized::B16,
                },
            ),
        )
        .unwrap();

        // Test the helper method
        assert!(env.is_enumeration(&TypeName::from("MY_ENUM")));
        assert!(!env.is_enumeration(&TypeName::from("MY_INT")));
        assert!(!env.is_enumeration(&TypeName::from("NONEXISTENT")));
    }

    #[test]
    fn type_environment_get_memory_size() {
        let mut env = TypeEnvironment::new();
        env.insert_type(
            &TypeName::from("MY_INT"),
            TypeAttributes::new(
                SourceSpan::default(),
                IntermediateType::Int {
                    size: ByteSized::B32,
                },
            ),
        )
        .unwrap();

        // Test successful memory size retrieval
        assert_eq!(
            env.get_memory_size(&TypeName::from("MY_INT")).unwrap(),
            Some(4)
        );

        // Test error for non-existent type
        assert!(env.get_memory_size(&TypeName::from("NONEXISTENT")).is_err());
    }

    #[test]
    fn type_environment_get_all_types_by_category() {
        let mut env = TypeEnvironment::new();

        // Add types of different categories
        env.insert_type(
            &TypeName::from("MY_BOOL"),
            TypeAttributes::new(SourceSpan::default(), IntermediateType::Bool),
        )
        .unwrap();

        env.insert_type(
            &TypeName::from("MY_ENUM"),
            TypeAttributes::new(
                SourceSpan::default(),
                IntermediateType::Enumeration {
                    underlying_type: Box::new(IntermediateType::Int {
                        size: ByteSized::B8,
                    }),
                },
            ),
        )
        .unwrap();

        env.insert_type(
            &TypeName::from("MY_SUBRANGE"),
            TypeAttributes::new(
                SourceSpan::default(),
                IntermediateType::Subrange {
                    base_type: Box::new(IntermediateType::Int {
                        size: ByteSized::B16,
                    }),
                    min_value: 1,
                    max_value: 100,
                },
            ),
        )
        .unwrap();

        let categories = env.get_all_types_by_category();

        // Check that types are correctly categorized
        assert!(categories
            .get(&TypeCategory::Elementary)
            .unwrap()
            .contains(&&TypeName::from("MY_BOOL")));
        assert!(categories
            .get(&TypeCategory::UserDefined)
            .unwrap()
            .contains(&&TypeName::from("MY_ENUM")));
        assert!(categories
            .get(&TypeCategory::Derived)
            .unwrap()
            .contains(&&TypeName::from("MY_SUBRANGE")));
    }

    #[test]
    fn validate_type_usage_when_type_exists_and_context_valid_then_ok() {
        let mut env = TypeEnvironment::new();
        env.insert_type(
            &TypeName::from("MY_INT"),
            TypeAttributes::new(
                SourceSpan::default(),
                IntermediateType::Int {
                    size: ByteSized::B32,
                },
            ),
        )
        .unwrap();

        // Test valid usage contexts
        assert!(env
            .validate_type_usage(
                &TypeName::from("MY_INT"),
                &UsageContext::VariableDeclaration
            )
            .is_ok());
        assert!(env
            .validate_type_usage(&TypeName::from("MY_INT"), &UsageContext::SubrangeBase)
            .is_ok());
        assert!(env
            .validate_type_usage(
                &TypeName::from("MY_INT"),
                &UsageContext::EnumerationUnderlying
            )
            .is_ok());
    }

    #[test]
    fn validate_type_usage_when_type_not_found_then_error() {
        let env = TypeEnvironment::new();

        assert!(env
            .validate_type_usage(&TypeName::from("NONEXISTENT"), &UsageContext::General)
            .is_err());
    }

    #[test]
    fn validate_type_usage_when_non_numeric_subrange_base_then_error() {
        let mut env = TypeEnvironment::new();
        env.insert_type(
            &TypeName::from("MY_BOOL"),
            TypeAttributes::new(SourceSpan::default(), IntermediateType::Bool),
        )
        .unwrap();

        // Boolean is not numeric, should fail for subrange base
        assert!(env
            .validate_type_usage(&TypeName::from("MY_BOOL"), &UsageContext::SubrangeBase)
            .is_err());
    }

    #[test]
    fn validate_type_usage_when_non_integer_enumeration_underlying_then_error() {
        let mut env = TypeEnvironment::new();
        env.insert_type(
            &TypeName::from("MY_REAL"),
            TypeAttributes::new(
                SourceSpan::default(),
                IntermediateType::Real {
                    size: ByteSized::B32,
                },
            ),
        )
        .unwrap();

        // Real is numeric but not integer, should fail for enumeration underlying
        assert!(env
            .validate_type_usage(
                &TypeName::from("MY_REAL"),
                &UsageContext::EnumerationUnderlying
            )
            .is_err());
    }

    #[test]
    fn validate_type_usage_when_function_block_array_element_then_error() {
        let mut env = TypeEnvironment::new();
        env.insert_type(
            &TypeName::from("MY_FB"),
            TypeAttributes::new(
                SourceSpan::default(),
                IntermediateType::FunctionBlock {
                    name: "MyFB".to_string(),
                    fields: vec![],
                },
            ),
        )
        .unwrap();

        // Function blocks cannot be array elements
        assert!(env
            .validate_type_usage(&TypeName::from("MY_FB"), &UsageContext::ArrayElement)
            .is_err());
    }

    #[test]
    fn validate_type_usage_when_function_block_return_type_then_error() {
        let mut env = TypeEnvironment::new();
        env.insert_type(
            &TypeName::from("MY_FB"),
            TypeAttributes::new(
                SourceSpan::default(),
                IntermediateType::FunctionBlock {
                    name: "MyFB".to_string(),
                    fields: vec![],
                },
            ),
        )
        .unwrap();

        // Function blocks cannot be return types
        assert!(env
            .validate_type_usage(&TypeName::from("MY_FB"), &UsageContext::FunctionReturn)
            .is_err());
    }
}
