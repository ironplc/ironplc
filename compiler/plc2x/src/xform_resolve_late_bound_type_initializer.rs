//! Transform that resolves late bound type initializers into specific types
//! in an initializer.
//!
//! The IEC 61131-3 syntax has some ambiguous types that are initially
//! parsed into a placeholder. This transform replaces the placeholders
//! with well-known types.
use ironplc_dsl::core::SourcePosition;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::visitor::Visitor;
use ironplc_dsl::{common::*, core::Id};
use ironplc_problems::Problem;
use phf::{phf_set, Set};

use crate::symbol_table::SymbolTable;

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
    "time"
};

/// Derived data types declared.
///
/// See section 2.3.3.
enum TypeDefinitionKind {
    /// Defines a type that can take one of a set number of values.
    Enumeration,
    FunctionBlock,
    /// Defines a type composed of sub-elements.
    Structure,
}

pub fn apply(lib: Library) -> Result<Library, Diagnostic> {
    let mut id_to_type: SymbolTable<Id, TypeDefinitionKind> = SymbolTable::new();

    // Walk the entire library to find the types. We don't need
    // to keep track of contexts because types are global scoped.
    id_to_type.walk(&lib)?;

    // Set the types for each item.
    let mut resolver = TypeResolver { types: id_to_type };
    resolver.fold(lib)
}

impl SymbolTable<'_, Id, TypeDefinitionKind> {
    fn add_if_new(&mut self, to_add: &Id, kind: TypeDefinitionKind) -> Result<(), Diagnostic> {
        if let Some(existing) = self.try_add(to_add, kind) {
            return Err(Diagnostic::problem(
                Problem::DefinitionNameDuplicated,
                Label::source_loc(
                    to_add.position(),
                    format!("Duplicated definition {}", to_add),
                ),
            )
            .with_secondary(Label::source_loc(existing.0.position(), "First definition")));
        }

        Ok(())
    }
}

impl<'a> Visitor<Diagnostic> for SymbolTable<'a, Id, TypeDefinitionKind> {
    type Value = ();

    fn visit_enum_declaration(&mut self, node: &EnumerationDeclaration) -> Result<(), Diagnostic> {
        self.add_if_new(&node.type_name, TypeDefinitionKind::Enumeration)
    }
    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Diagnostic> {
        self.add_if_new(&node.name, TypeDefinitionKind::FunctionBlock)
    }
    fn visit_structure_declaration(
        &mut self,
        node: &StructureDeclaration,
    ) -> Result<Self::Value, Diagnostic> {
        self.add_if_new(&node.type_name, TypeDefinitionKind::Structure)
    }
}

struct TypeResolver<'a> {
    types: SymbolTable<'a, Id, TypeDefinitionKind>,
}

impl<'a> TypeResolver<'a> {
    fn is_elementary_type(id: &Id) -> bool {
        ELEMENTARY_TYPES_LOWER_CASE.contains(&id.lower_case().to_string())
    }
}

impl<'a> Fold<Diagnostic> for TypeResolver<'a> {
    fn fold_initial_value_assignment(
        &mut self,
        node: InitialValueAssignmentKind,
    ) -> Result<InitialValueAssignmentKind, Diagnostic> {
        match node {
            // TODO this needs to handle struct definitions
            InitialValueAssignmentKind::LateResolvedType(name) => {
                // Try to find the type for the specified name.
                if TypeResolver::is_elementary_type(&name) {
                    return Ok(InitialValueAssignmentKind::Simple(SimpleInitializer {
                        type_name: name,
                        initial_value: None,
                    }));
                }

                // TODO error handling
                let maybe_type_kind = self.types.find(&name);
                match maybe_type_kind {
                    Some(type_kind) => match type_kind {
                        TypeDefinitionKind::Enumeration => {
                            Ok(InitialValueAssignmentKind::EnumeratedType(
                                EnumeratedInitialValueAssignment {
                                    type_name: name,
                                    initial_value: None,
                                },
                            ))
                        }
                        TypeDefinitionKind::FunctionBlock => {
                            Ok(InitialValueAssignmentKind::FunctionBlock(
                                FunctionBlockInitialValueAssignment { type_name: name },
                            ))
                        }
                        TypeDefinitionKind::Structure => Ok(InitialValueAssignmentKind::Structure(
                            StructureInitializationDeclaration {
                                type_name: name,
                                elements_init: vec![],
                            },
                        )),
                    },
                    None => {
                        return Err(Diagnostic::problem(
                            Problem::UndeclaredUnknownType,
                            Label::source_loc(name.position(), "Variable type"),
                        )
                        .with_context_id("identifier", &name));
                    }
                }
            }
            _ => Ok(node),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::apply;
    use ironplc_dsl::{
        common::*,
        core::{FileId, Id, SourceLoc},
    };

    #[test]
    fn apply_when_has_function_block_type_then_resolves_type() {
        let program = "
FUNCTION_BLOCK called
        
END_FUNCTION_BLOCK

FUNCTION_BLOCK caller
    VAR
    fb_var : called;
    END_VAR
    
END_FUNCTION_BLOCK
        ";
        let input = ironplc_parser::parse_program(program, &FileId::default()).unwrap();
        let result = apply(input).unwrap();

        let expected = Library {
            elements: vec![
                LibraryElement::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: Id::from("called"),
                    variables: vec![],
                    body: FunctionBlockBody::empty(),
                    position: SourceLoc::default(),
                }),
                LibraryElement::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: Id::from("caller"),
                    variables: vec![VarDecl::function_block("fb_var", "called")],
                    body: FunctionBlockBody::empty(),
                    position: SourceLoc::default(),
                }),
            ],
        };

        assert_eq!(result, expected)
    }

    #[test]
    fn apply_when_has_struct_type_then_resolves_type() {
        let program = "
TYPE
    the_struct : STRUCT
        member: BOOL;
    END_STRUCT;  
END_TYPE

FUNCTION_BLOCK caller
    VAR
        the_var : the_struct;
    END_VAR
    
END_FUNCTION_BLOCK
        ";
        let input = ironplc_parser::parse_program(program, &FileId::default()).unwrap();
        let result = apply(input).unwrap();

        let expected = Library {
            elements: vec![
                LibraryElement::DataTypeDeclaration(DataTypeDeclarationKind::Structure(
                    StructureDeclaration {
                        type_name: Id::from("the_struct"),
                        elements: vec![StructureElementDeclaration {
                            name: Id::from("member"),
                            init: InitialValueAssignmentKind::simple_uninitialized("BOOL"),
                        }],
                    },
                )),
                LibraryElement::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: Id::from("caller"),
                    variables: vec![VarDecl::structure("the_var", "the_struct")],
                    body: FunctionBlockBody::empty(),
                    position: SourceLoc::default(),
                }),
            ],
        };

        assert_eq!(result, expected)
    }

    #[test]
    fn apply_when_has_enum_type_then_resolves_type() {
        let program = "
TYPE
    values : (val1, val2, val3);  
END_TYPE

FUNCTION_BLOCK caller
    VAR
        the_var : values;
    END_VAR
    
END_FUNCTION_BLOCK
        ";
        let input = ironplc_parser::parse_program(program, &FileId::default()).unwrap();
        let result = apply(input).unwrap();

        let expected = Library {
            elements: vec![
                LibraryElement::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
                    EnumerationDeclaration {
                        type_name: Id::from("values"),
                        spec_init: EnumeratedSpecificationInit {
                            spec: EnumeratedSpecificationKind::from_values(vec![
                                "val1", "val2", "val3",
                            ]),
                            default: None,
                        },
                    },
                )),
                LibraryElement::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: Id::from("caller"),
                    variables: vec![VarDecl::uninitialized_enumerated("the_var", "values")],
                    body: FunctionBlockBody::empty(),
                    position: SourceLoc::default(),
                }),
            ],
        };

        assert_eq!(result, expected)
    }

    #[test]
    fn apply_when_duplicated_type_then_error() {
        let program = "
TYPE
    the_struct : STRUCT
        member: BOOL;
    END_STRUCT;  
    the_struct : STRUCT
        member: BOOL;
    END_STRUCT; 
END_TYPE

FUNCTION_BLOCK caller
    VAR
        the_var : the_struct;
    END_VAR
    
END_FUNCTION_BLOCK
        ";
        let input = ironplc_parser::parse_program(program, &FileId::default()).unwrap();
        let result = apply(input);
        assert!(result.is_err())
    }
}
