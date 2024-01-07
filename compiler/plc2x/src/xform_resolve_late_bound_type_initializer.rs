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
use log::trace;

use crate::stdlib::{is_elementary_type, is_unsupported_standard_type};
use crate::symbol_table::{SymbolTable, Value};

/// Derived data types declared.
///
/// See section 2.3.3.
#[derive(Debug)]
enum TypeDefinitionKind {
    /// Defines a type that can take one of a set number of values.
    Enumeration,
    Subrange,
    Simple,
    Array(ArraySpecificationKind),
    Structure,
    StructureInitialization,
    String(StringKind, Integer),
    FunctionBlock,
}

impl Value for TypeDefinitionKind {}

pub fn apply(lib: Library) -> Result<Library, Vec<Diagnostic>> {
    let mut id_to_type: SymbolTable<Id, TypeDefinitionKind> = SymbolTable::new();

    // Walk the entire library to find the types. We don't need
    // to keep track of contexts because types are global scoped.
    id_to_type.walk(&lib).map_err(|err| vec![err])?;

    // Set the types for each item.
    let mut resolver = TypeResolver {
        types: id_to_type,
        diagnostics: vec![],
    };
    let result = resolver.fold_library(lib).map_err(|e| vec![e]);

    if !resolver.diagnostics.is_empty() {
        return Err(resolver.diagnostics);
    }

    result
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

    fn visit_data_type_declaration_kind(
        &mut self,
        node: &DataTypeDeclarationKind,
    ) -> Result<(), Diagnostic> {
        // We could visit all of the types individually, but that would allow
        // new types to be created without necessarily handling the type. Using
        // the match ensures that doesn't happen.
        match node {
            DataTypeDeclarationKind::Enumeration(node) => {
                self.add_if_new(&node.type_name, TypeDefinitionKind::Enumeration)
            }
            DataTypeDeclarationKind::Subrange(node) => {
                self.add_if_new(&node.type_name, TypeDefinitionKind::Subrange)
            }
            DataTypeDeclarationKind::Simple(node) => {
                self.add_if_new(&node.type_name, TypeDefinitionKind::Simple)
            }
            DataTypeDeclarationKind::Array(node) => self.add_if_new(
                &node.type_name,
                TypeDefinitionKind::Array(node.spec.clone()),
            ),
            DataTypeDeclarationKind::Structure(node) => {
                self.add_if_new(&node.type_name, TypeDefinitionKind::Structure)
            }
            DataTypeDeclarationKind::StructureInitialization(node) => {
                self.add_if_new(&node.type_name, TypeDefinitionKind::StructureInitialization)
            }
            DataTypeDeclarationKind::String(node) => self.add_if_new(
                &node.type_name,
                TypeDefinitionKind::String(node.width.clone(), node.length.clone()),
            ),
            DataTypeDeclarationKind::LateBound(_) => Ok(()),
        }
    }

    fn visit_function_block_declaration(
        &mut self,
        node: &FunctionBlockDeclaration,
    ) -> Result<(), Diagnostic> {
        self.add_if_new(&node.name, TypeDefinitionKind::FunctionBlock)
    }
}

struct TypeResolver<'a> {
    types: SymbolTable<'a, Id, TypeDefinitionKind>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Fold<Diagnostic> for TypeResolver<'a> {
    fn fold_initial_value_assignment_kind(
        &mut self,
        node: InitialValueAssignmentKind,
    ) -> Result<InitialValueAssignmentKind, Diagnostic> {
        match node {
            // TODO this needs to handle struct definitions
            InitialValueAssignmentKind::LateResolvedType(name) => {
                // Element types resolve to the known type.
                if is_elementary_type(&name) {
                    return Ok(InitialValueAssignmentKind::Simple(SimpleInitializer {
                        type_name: name,
                        initial_value: None,
                    }));
                }

                // Unsupported standard types resolve to a known type that we will detect later.
                // This allows passing the transformation stage to show other errors.
                if is_unsupported_standard_type(&name) {
                    return Ok(InitialValueAssignmentKind::FunctionBlock(
                        FunctionBlockInitialValueAssignment { type_name: name },
                    ));
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
                        TypeDefinitionKind::String(width, length) => {
                            Ok(InitialValueAssignmentKind::String(StringInitializer {
                                length: Some(length.clone()),
                                width: width.clone(),
                                initial_value: None,
                            }))
                        }
                        TypeDefinitionKind::Array(spec) => Ok(InitialValueAssignmentKind::Array(
                            ArrayInitialValueAssignment {
                                spec: spec.clone(),
                                initial_values: vec![],
                            },
                        )),
                        _ => Err(Diagnostic::todo_with_id(&name, file!(), line!())),
                    },
                    None => {
                        trace!("{:?}", self.types);
                        self.diagnostics.push(
                            Diagnostic::problem(
                                Problem::UndeclaredUnknownType,
                                Label::source_loc(name.position(), "Variable type"),
                            )
                            .with_context_id("identifier", &name),
                        );
                        Ok(InitialValueAssignmentKind::LateResolvedType(name))
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
    use ironplc_problems::Problem;

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
                LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: Id::from("called"),
                    variables: vec![],
                    body: FunctionBlockBodyKind::empty(),
                    position: SourceLoc::default(),
                }),
                LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: Id::from("caller"),
                    variables: vec![VarDecl::function_block("fb_var", "called")],
                    body: FunctionBlockBodyKind::empty(),
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
                LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(
                    StructureDeclaration {
                        type_name: Id::from("the_struct"),
                        elements: vec![StructureElementDeclaration {
                            name: Id::from("member"),
                            init: InitialValueAssignmentKind::simple_uninitialized("BOOL"),
                        }],
                    },
                )),
                LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: Id::from("caller"),
                    variables: vec![VarDecl::structure("the_var", "the_struct")],
                    body: FunctionBlockBodyKind::empty(),
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
                LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
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
                LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: Id::from("caller"),
                    variables: vec![VarDecl::uninitialized_enumerated("the_var", "values")],
                    body: FunctionBlockBodyKind::empty(),
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
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(1, err.len());
        assert_eq!(Problem::DefinitionNameDuplicated.code(), err[0].code);
    }
}
