//! Transformation rule that resolves declarations and builds
//! a type environment for all types in the source. This rule
//! handles types that are:
//!
//! * defined in the language
//! * defined by particular implementations
//! * defined by users
//!
//! This rules also transforms late bound declarations (those
//! that are ambiguous during parsing).
//!
//! The transformation succeeds when all data type declarations
//! resolve to a declared type.
use crate::type_environment::{IntermediateType, TypeAttributes, TypeEnvironment};
use ironplc_dsl::common::*;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_problems::Problem;

pub fn apply(
    lib: Library,
    type_environment: &mut TypeEnvironment,
) -> Result<Library, Vec<Diagnostic>> {
    // Populate environment (this also transforms late bound declarations).
    type_environment.fold_library(lib).map_err(|err| vec![err])
}

impl TypeEnvironment {
    fn transform_late_bound_declaration(
        &mut self,
        node: LateBoundDeclaration,
    ) -> Result<DataTypeDeclarationKind, Diagnostic> {
        // At this point we should have a type for the late bound declaration
        // so we can replace the late bound declaration with the correct type
        let existing = self.get(&node.base_type_name).unwrap();

        if existing.representation.is_primitive() {
            Ok(DataTypeDeclarationKind::Simple(SimpleDeclaration {
                type_name: node.data_type_name,
                spec_and_init: InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: node.base_type_name,
                    initial_value: None,
                }),
            }))
        } else {
            match existing.representation {
                IntermediateType::Enumeration { underlying_type: _ } => Ok(
                    DataTypeDeclarationKind::Enumeration(EnumerationDeclaration {
                        type_name: node.data_type_name,
                        spec_init: EnumeratedSpecificationInit {
                            spec: EnumeratedSpecificationKind::TypeName(node.base_type_name),
                            default: None,
                        },
                    }),
                ),
                IntermediateType::Structure { fields: _ } => {
                    Ok(DataTypeDeclarationKind::StructureInitialization(
                        StructureInitializationDeclaration {
                            type_name: node.data_type_name,
                            elements_init: vec![],
                        },
                    ))
                }
                IntermediateType::Array {
                    element_type: _,
                    size: _,
                } => Ok(DataTypeDeclarationKind::Array(ArrayDeclaration {
                    type_name: node.data_type_name,
                    spec: ArraySpecificationKind::Type(node.base_type_name),
                    init: vec![],
                })),
                _ => todo!(),
            }
        }
    }
}

impl Fold<Diagnostic> for TypeEnvironment {
    fn fold_simple_declaration(
        &mut self,
        node: SimpleDeclaration,
    ) -> Result<SimpleDeclaration, Diagnostic> {
        // A simple declaration cannot refer to another type so we
        // just need to insert this into the type environment.
        // TODO: Implement proper type resolution for simple declarations
        // For now, just return the node without adding to type environment
        match &node.spec_and_init {
            InitialValueAssignmentKind::None(_source_span) => {
                // TODO: Handle simple type declarations without initializers
            }
            InitialValueAssignmentKind::Simple(_simple_initializer) => {
                // TODO: Handle simple type declarations with initializers
            }
            InitialValueAssignmentKind::String(_string_initializer) => {
                // TODO: Handle string type declarations
            }
            InitialValueAssignmentKind::EnumeratedValues(_enumerated_values_initializer) => {
                // TODO: Handle enumerated values initializers
            }
            InitialValueAssignmentKind::EnumeratedType(_enumerated_initial_value_assignment) => {
                // TODO: Handle enumerated type initializers
            }
            InitialValueAssignmentKind::FunctionBlock(_function_block_initial_value_assignment) => {
                // TODO: Handle function block initializers
            }
            InitialValueAssignmentKind::Subrange(_subrange_specification_kind) => {
                // TODO: Handle subrange specifications
            }
            InitialValueAssignmentKind::Structure(_structure_initialization_declaration) => {
                // TODO: Handle structure initializations
            }
            InitialValueAssignmentKind::Array(_array_initial_value_assignment) => {
                // TODO: Handle array initializers
            }
            InitialValueAssignmentKind::LateResolvedType(_type_name) => {
                // TODO: Handle late resolved types
            }
        }

        Ok(node)
    }

    fn fold_enumeration_declaration(
        &mut self,
        node: EnumerationDeclaration,
    ) -> Result<EnumerationDeclaration, Diagnostic> {
        // Enumeration declaration can define a set of values
        // or rename another enumeration.
        match &node.spec_init.spec {
            EnumeratedSpecificationKind::TypeName(base_type_name) => {
                // Alias of another enumeration: base must already exist due to toposort
                if self.get(base_type_name).is_none() {
                    return Err(Diagnostic::problem(
                        Problem::ParentEnumNotDeclared,
                        Label::span(node.type_name.span(), "Enumeration"),
                    )
                    .with_secondary(Label::span(base_type_name.span(), "Base type name")));
                }
                // Use explicit alias insertion to avoid duplicating representation logic
                self.insert_alias(&node.type_name, base_type_name)?;
            }
            EnumeratedSpecificationKind::Values(values) => {
                // Compute underlying type width based on cardinality
                let value_count = values.values.len();
                let underlying_type = if value_count <= 256 {
                    IntermediateType::Int { size: 8 }
                } else if value_count <= 65536 {
                    IntermediateType::Int { size: 16 }
                } else {
                    IntermediateType::Int { size: 32 }
                };

                self.insert_type(
                    &node.type_name,
                    TypeAttributes {
                        span: node.type_name.span(),
                        representation: IntermediateType::Enumeration {
                            underlying_type: Box::new(underlying_type),
                        },
                    },
                )?;
            }
        }

        Ok(node)
    }

    fn fold_string_declaration(
        &mut self,
        node: StringDeclaration,
    ) -> Result<StringDeclaration, Diagnostic> {
        self.insert_type(
            &node.type_name,
            TypeAttributes {
                span: node.type_name.span(),
                representation: IntermediateType::String {
                    max_len: Some(node.length.value),
                },
            },
        )?;
        Ok(node)
    }

    fn fold_structure_declaration(
        &mut self,
        node: StructureDeclaration,
    ) -> Result<StructureDeclaration, Diagnostic> {
        // TODO: Implement proper structure field resolution
        // For now, create an empty structure
        self.insert_type(
            &node.type_name,
            TypeAttributes {
                span: node.type_name.span(),
                representation: IntermediateType::Structure {
                    fields: Vec::new(), // TODO: Resolve structure fields
                },
            },
        )?;
        Ok(node)
    }

    fn fold_array_declaration(
        &mut self,
        node: ArrayDeclaration,
    ) -> Result<ArrayDeclaration, Diagnostic> {
        // Resolve the element type from the array specification
        let element_type = match &node.spec {
            ArraySpecificationKind::Type(type_name) => {
                // Array of a specific type - check if the type exists
                if self.get(type_name).is_none() {
                    return Err(Diagnostic::problem(
                        Problem::ParentTypeNotDeclared,
                        Label::span(node.type_name.span(), "Array element type not found"),
                    )
                    .with_secondary(Label::span(type_name.span(), "Element type name")));
                }

                // Get the element type representation
                let element_attrs = self.get(type_name).unwrap();
                element_attrs.representation.clone()
            }
            ArraySpecificationKind::Subranges(subranges) => {
                // Array with subranges - check if the base type exists
                if self.get(&subranges.type_name).is_none() {
                    return Err(Diagnostic::problem(
                        Problem::ArrayElementTypeNotDeclared,
                        Label::span(node.type_name.span(), "Array declaration"),
                    )
                    .with_secondary(Label::span(
                        subranges.type_name.span(),
                        "Array element type name",
                    )));
                }

                // Get the base type representation
                let base_attrs = self.get(&subranges.type_name).unwrap();
                base_attrs.representation.clone()
            }
        };

        // Calculate array size from subranges if present
        let array_size = match &node.spec {
            ArraySpecificationKind::Subranges(subranges) => {
                // For now, calculate total size as product of all subrange sizes
                // TODO: Handle multi-dimensional arrays properly
                let total_size = subranges
                    .ranges
                    .iter()
                    .map(|range| {
                        // Calculate range size: end - start + 1
                        let start = range.start.value.value as i128;
                        let end = range.end.value.value as i128;
                        (end - start + 1) as u32
                    })
                    .product::<u32>();
                Some(total_size)
            }
            ArraySpecificationKind::Type(_) => {
                // Single-dimensional array without explicit size
                // TODO: Determine size from initialization or make it dynamic
                None
            }
        };

        // Insert the array type into the type environment
        self.insert_type(
            &node.type_name,
            TypeAttributes {
                span: node.type_name.span(),
                representation: IntermediateType::Array {
                    element_type: Box::new(element_type),
                    size: array_size,
                },
            },
        )?;

        Ok(node)
    }

    fn fold_data_type_declaration_kind(
        &mut self,
        node: DataTypeDeclarationKind,
    ) -> Result<DataTypeDeclarationKind, Diagnostic> {
        match node {
            DataTypeDeclarationKind::LateBound(lb) => {
                let result = self.transform_late_bound_declaration(lb)?;

                // If the LateBound was transformed into an Enumeration or Array, we need to add it to the TypeEnvironment
                match &result {
                    DataTypeDeclarationKind::Enumeration(enum_decl) => {
                        // Get the base type from the existing type environment
                        let base_type_name = match &enum_decl.spec_init.spec {
                            EnumeratedSpecificationKind::TypeName(base_type) => base_type,
                            _ => {
                                unreachable!("LateBound should always resolve to TypeName variant")
                            }
                        };

                        if self.get(base_type_name).is_none() {
                            return Err(Diagnostic::problem(
                                Problem::ParentTypeNotDeclared,
                                Label::span(enum_decl.type_name.span(), "Base type not found"),
                            ));
                        }

                        // Insert as alias rather than cloning representation
                        self.insert_alias(&enum_decl.type_name, base_type_name)?;
                    }
                    DataTypeDeclarationKind::Array(array_decl) => {
                        // Get the base type from the array specification
                        let base_type_name = match &array_decl.spec {
                            ArraySpecificationKind::Type(base_type) => base_type,
                            _ => unreachable!(
                                "LateBound array should always resolve to Type variant"
                            ),
                        };

                        if self.get(base_type_name).is_none() {
                            return Err(Diagnostic::problem(
                                Problem::ParentTypeNotDeclared,
                                Label::span(array_decl.type_name.span(), "Base type not found"),
                            ));
                        }

                        // Insert as alias rather than cloning representation
                        self.insert_alias(&array_decl.type_name, base_type_name)?;
                    }
                    _ => {} // Other types don't need special handling
                }

                Ok(result)
            }
            DataTypeDeclarationKind::Enumeration(enum_decl) => {
                let enum_decl = self.fold_enumeration_declaration(enum_decl)?;
                Ok(DataTypeDeclarationKind::Enumeration(enum_decl))
            }
            DataTypeDeclarationKind::Array(array_decl) => {
                let array_decl = self.fold_array_declaration(array_decl)?;
                Ok(DataTypeDeclarationKind::Array(array_decl))
            }
            _ => node.recurse_fold(self),
        }
    }

    fn fold_library_element_kind(
        &mut self,
        node: LibraryElementKind,
    ) -> Result<LibraryElementKind, Diagnostic> {
        match node {
            LibraryElementKind::DataTypeDeclaration(kind) => {
                let kind = self.fold_data_type_declaration_kind(kind)?;
                Ok(LibraryElementKind::DataTypeDeclaration(kind))
            }
            _ => node.recurse_fold(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::type_environment::{IntermediateType, TypeEnvironment, TypeEnvironmentBuilder};

    use super::apply;
    use ironplc_dsl::{common::*, core::FileId};
    use ironplc_parser::options::ParseOptions;

    #[test]
    fn apply_when_ambiguous_enum_then_resolves_type() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
LEVEL_ALIAS : LEVEL;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironment::new();
        let library = apply(input, &mut env).unwrap();

        let expected = Library {
            elements: vec![
                LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
                    EnumerationDeclaration {
                        type_name: TypeName::from("LEVEL"),
                        spec_init: EnumeratedSpecificationInit::values_and_default(
                            vec!["CRITICAL"],
                            "CRITICAL",
                        ),
                    },
                )),
                LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
                    EnumerationDeclaration {
                        type_name: TypeName::from("LEVEL_ALIAS"),
                        spec_init: EnumeratedSpecificationInit {
                            spec: EnumeratedSpecificationKind::TypeName(TypeName::from("LEVEL")),
                            default: None,
                        },
                    },
                )),
            ],
        };

        assert_eq!(library, expected)
    }

    #[test]
    fn apply_when_has_duplicate_items_then_error() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
LEVEL : (CRITICAL) := CRITICAL;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironment::new();
        let result = apply(input, &mut env);
        let result = result.unwrap_err();
        assert_eq!("P0019", result.first().unwrap().code);
    }

    #[test]
    fn apply_when_declares_stdlib_type_then_error() {
        let program = "
TYPE
LREAL : REAL;
END_TYPE
        ";
        let result =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap_err();
        // This doesn't actually fail due to this transform but something should
        // catch this.
        assert_eq!("P0002", result.code);
    }

    #[test]
    fn apply_when_array_declaration_then_creates_array_type() {
        let program = "
TYPE
MY_ARRAY : ARRAY[1..10] OF INT;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let _library = apply(input, &mut env).unwrap();

        // Check that the array type was created
        let array_type = env.get(&TypeName::from("MY_ARRAY")).unwrap();
        match &array_type.representation {
            IntermediateType::Array { element_type, size } => {
                // Check element type
                match element_type.as_ref() {
                    IntermediateType::Int { size: 16 } => {} // INT is 16-bit
                    _ => panic!("Expected INT element type, got: {element_type:?}"),
                }
                // Check array size (1..10 = 10 elements)
                assert_eq!(size, &Some(10));
            }
            _ => panic!("Expected Array type"),
        }
    }

    #[test]
    fn apply_when_array_alias_then_creates_alias() {
        let program = "
TYPE
ORIGINAL_ARRAY : ARRAY[1..5] OF INT;
ARRAY_ALIAS : ORIGINAL_ARRAY;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let _library = apply(input, &mut env).unwrap();

        // Check that the array alias was created
        let alias_type = env.get(&TypeName::from("ARRAY_ALIAS")).unwrap();
        match &alias_type.representation {
            IntermediateType::Array { element_type, size } => {
                // Check element type
                match element_type.as_ref() {
                    IntermediateType::Int { size: 16 } => {} // INT is 16-bit
                    _ => panic!("Expected INT element type"),
                }
                // Check array size (1..5 = 5 elements)
                assert_eq!(size, &Some(5));
            }
            _ => panic!("Expected Array type"),
        }
    }

    #[test]
    fn apply_when_array_element_is_string_type_then_ok() {
        let program = "
TYPE
  oscat_STRING10               : STRING(10);
END_TYPE

TYPE
  oscat_ELEMENT_WEEKDAYS	: ARRAY [1..7] OF oscat_STRING10;
END_TYPE
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();

        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let _library = apply(input, &mut env).unwrap();
    }
}
