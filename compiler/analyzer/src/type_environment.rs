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
pub(crate) struct TypeEnvironment {
    table: HashMap<Type, TypeAttributes>,
}

#[derive(Debug)]
pub struct TypeAttributes {
    pub span: SourceSpan,
    pub class: TypeClass,
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
                    },
                )?;
            }
        }
        Ok(env)
    }
}
