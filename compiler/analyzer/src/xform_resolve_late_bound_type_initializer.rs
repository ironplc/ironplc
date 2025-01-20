//! Transform that resolves late bound type initializers into specific types
//! in an initializer.
//!
//! The IEC 61131-3 syntax has some ambiguous types that are initially
//! parsed into a placeholder. This transform replaces the placeholders
//! with well-known types.
use ironplc_dsl::common::*;
use ironplc_dsl::core::{Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::visitor::Visitor;
use ironplc_problems::Problem;
use log::trace;

use crate::scoped_table::{ScopedTable, Value};
use crate::stdlib::is_unsupported_standard_type;
use crate::type_environment::{TypeClass, TypeEnvironment};

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
    String(StringType, Integer),
    FunctionBlock,
}

impl Value for TypeDefinitionKind {}

pub fn apply(
    lib: Library,
    type_environment: &mut TypeEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    let mut type_to_type_kind: ScopedTable<Type, TypeDefinitionKind> = ScopedTable::new();

    // Walk the entire library to find the types. We don't need
    // to keep track of contexts because types are global scoped.
    type_to_type_kind.walk(&lib).map_err(|err| vec![err])?;

    // Set the types for each item.
    let mut resolver = TypeResolver {
        types: type_to_type_kind,
        type_environment,
        diagnostics: vec![],
    };
    let result = resolver.fold_library(lib).map_err(|e| vec![e]);

    if !resolver.diagnostics.is_empty() {
        return Err(resolver.diagnostics);
    }

    result
}

impl ScopedTable<'_, Type, TypeDefinitionKind> {
    fn add_if_new(&mut self, to_add: &Type, kind: TypeDefinitionKind) -> Result<(), Diagnostic> {
        if let Some(existing) = self.try_add(to_add, kind) {
            return Err(Diagnostic::problem(
                Problem::DefinitionNameDuplicated,
                Label::span(to_add.span(), format!("Duplicated definition {}", to_add)),
            )
            .with_secondary(Label::span(existing.0.span(), "First definition")));
        }

        Ok(())
    }
}

impl Visitor<Diagnostic> for ScopedTable<'_, Type, TypeDefinitionKind> {
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
        // Other items are types, but in the case of a function block declaration, this is
        // actually an identifier, so treat identifier and type as equivalent in this context.
        self.add_if_new(
            &Type::from_id(&node.name),
            TypeDefinitionKind::FunctionBlock,
        )
    }
}

struct TypeResolver<'a> {
    types: ScopedTable<'a, Type, TypeDefinitionKind>,
    type_environment: &'a TypeEnvironment,
    diagnostics: Vec<Diagnostic>,
}

impl Fold<Diagnostic> for TypeResolver<'_> {
    fn fold_initial_value_assignment_kind(
        &mut self,
        node: InitialValueAssignmentKind,
    ) -> Result<InitialValueAssignmentKind, Diagnostic> {
        match node {
            // TODO this needs to handle struct definitions
            InitialValueAssignmentKind::LateResolvedType(name) => {
                // Element types resolve to the known type.
                if let Some(ty) = self.type_environment.get(&name) {
                    if ty.class == TypeClass::Simple {
                        return Ok(InitialValueAssignmentKind::Simple(SimpleInitializer {
                            type_name: name,
                            initial_value: None,
                        }));
                    }
                }

                // Unsupported standard types resolve to a known type that we will detect later.
                // This allows passing the transformation stage to show other errors.
                if is_unsupported_standard_type(&name) {
                    return Ok(InitialValueAssignmentKind::FunctionBlock(
                        FunctionBlockInitialValueAssignment {
                            type_name: name,
                            init: vec![],
                        },
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
                                FunctionBlockInitialValueAssignment {
                                    type_name: name,
                                    init: vec![],
                                },
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
                                keyword_span: SourceSpan::default(),
                            }))
                        }
                        TypeDefinitionKind::Array(spec) => Ok(InitialValueAssignmentKind::Array(
                            ArrayInitialValueAssignment {
                                spec: spec.clone(),
                                initial_values: vec![],
                            },
                        )),
                        _ => Err(Diagnostic::todo_with_type(&name, file!(), line!())),
                    },
                    None => {
                        trace!("{:?}", self.types);
                        self.diagnostics.push(
                            Diagnostic::problem(
                                Problem::UndeclaredUnknownType,
                                Label::span(name.span(), "Variable type"),
                            )
                            .with_context_type("identifier", &name),
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
    use crate::type_environment::TypeEnvironment;

    use super::apply;
    use ironplc_dsl::{
        common::*,
        core::{FileId, Id, SourceSpan},
    };
    use ironplc_parser::options::ParseOptions;
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
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironment::new();
        let result = apply(input, &mut type_environment).unwrap();

        let expected = Library {
            elements: vec![
                LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: Id::from("called"),
                    variables: vec![],
                    edge_variables: vec![],
                    body: FunctionBlockBodyKind::empty(),
                    span: SourceSpan::default(),
                }),
                LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: Id::from("caller"),
                    variables: vec![VarDecl::function_block("fb_var", "called")],
                    edge_variables: vec![],
                    body: FunctionBlockBodyKind::empty(),
                    span: SourceSpan::default(),
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
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironment::new();
        let result = apply(input, &mut type_environment).unwrap();

        let expected = Library {
            elements: vec![
                LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(
                    StructureDeclaration {
                        type_name: Type::from("the_struct"),
                        elements: vec![StructureElementDeclaration {
                            name: Id::from("member"),
                            init: InitialValueAssignmentKind::simple_uninitialized(Type::from(
                                "BOOL",
                            )),
                        }],
                    },
                )),
                LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: Id::from("caller"),
                    variables: vec![VarDecl::structure("the_var", "the_struct")],
                    edge_variables: vec![],
                    body: FunctionBlockBodyKind::empty(),
                    span: SourceSpan::default(),
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
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironment::new();
        let result = apply(input, &mut type_environment).unwrap();

        let expected = Library {
            elements: vec![
                LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
                    EnumerationDeclaration {
                        type_name: Type::from("values"),
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
                    edge_variables: vec![],
                    body: FunctionBlockBodyKind::empty(),
                    span: SourceSpan::default(),
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
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironment::new();
        let result = apply(input, &mut type_environment);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(1, err.len());
        assert_eq!(Problem::DefinitionNameDuplicated.code(), err[0].code);
    }
}
