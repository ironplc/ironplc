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
use crate::intermediate_type::IntermediateType;
use crate::intermediates::*;
use crate::type_environment::{TypeAttributes, TypeEnvironment};
use ironplc_dsl::common::*;
use ironplc_dsl::core::{Id, Located, SourceSpan};
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
        let existing = match self.get(&node.base_type_name) {
            Some(existing) => existing,
            None => {
                return Err(Diagnostic::problem(
                    Problem::ParentTypeNotDeclared,
                    Label::span(node.base_type_name.span(), "Base type not found"),
                ));
            }
        };

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

    fn convert_type_definition_to_data_type_declaration(
        &mut self,
        type_def: TypeDefinition,
    ) -> Result<DataTypeDeclarationKind, Diagnostic> {
        use ironplc_dsl::common::*;
        
        match type_def.base_type {
            DataTypeSpecificationKind::Elementary(elem_type) => {
                match elem_type {
                    ElementaryTypeName::StringWithLength(len) => {
                        // String with length should create a StringDeclaration
                        Ok(DataTypeDeclarationKind::String(StringDeclaration {
                            type_name: type_def.name,
                            length: Integer {
                                span: SourceSpan::default(),
                                value: len as u128,
                            },
                            width: StringType::String,
                            init: match type_def.default_value {
                                Some(ConstantKind::CharacterString(s)) => {
                                    Some(s.value.iter().collect::<String>())
                                }
                                _ => None,
                            },
                        }))
                    }
                    _ => {
                        // Other elementary types create simple declarations
                        Ok(DataTypeDeclarationKind::Simple(SimpleDeclaration {
                            type_name: type_def.name,
                            spec_and_init: InitialValueAssignmentKind::Simple(SimpleInitializer {
                                type_name: TypeName::from(match elem_type {
                                    ElementaryTypeName::BOOL => "BOOL",
                                    ElementaryTypeName::INT => "INT",
                                    ElementaryTypeName::DINT => "DINT",
                                    ElementaryTypeName::REAL => "REAL",
                                    ElementaryTypeName::TIME => "TIME",
                                    ElementaryTypeName::BYTE => "BYTE",
                                    _ => "UNKNOWN", // Handle other elementary types
                                }),
                                initial_value: type_def.default_value,
                            }),
                        }))
                    }
                }
            }
            DataTypeSpecificationKind::UserDefined(type_name) => {
                // Type alias to another user-defined type
                Ok(DataTypeDeclarationKind::LateBound(LateBoundDeclaration {
                    data_type_name: type_def.name,
                    base_type_name: type_name,
                }))
            }
            DataTypeSpecificationKind::Enumeration(enum_spec) => {
                Ok(DataTypeDeclarationKind::Enumeration(EnumerationDeclaration {
                    type_name: type_def.name,
                    spec_init: EnumeratedSpecificationInit {
                        spec: EnumeratedSpecificationKind::Values(EnumeratedSpecificationValues {
                            values: enum_spec.values,
                        }),
                        default: match type_def.default_value {
                            Some(ConstantKind::CharacterString(s)) => {
                                // Convert string to enumerated value
                                Some(EnumeratedValue {
                                    type_name: None,
                                    value: Id::from(&s.value.iter().collect::<String>()),
                                })
                            }
                            _ => None,
                        },
                    },
                }))
            }
            DataTypeSpecificationKind::Subrange(subrange_spec) => {
                Ok(DataTypeDeclarationKind::Subrange(SubrangeDeclaration {
                    type_name: type_def.name,
                    spec: SubrangeSpecificationKind::Specification(SubrangeSpecification {
                        type_name: subrange_spec.base_type,
                        subrange: Subrange {
                            start: subrange_spec.lower_bound,
                            end: subrange_spec.upper_bound,
                        },
                    }),
                    default: match type_def.default_value {
                        Some(ConstantKind::IntegerLiteral(int_lit)) => Some(int_lit.value),
                        _ => None,
                    },
                }))
            }
            DataTypeSpecificationKind::Array(array_spec) => {
                // Convert ArraySpecification to ArraySpecificationKind
                let array_subranges = ArraySubranges {
                    ranges: array_spec.bounds.into_iter().map(|bounds| {
                        Subrange {
                            start: bounds.lower,
                            end: bounds.upper,
                        }
                    }).collect(),
                    type_name: match array_spec.element_type.as_ref() {
                        DataTypeSpecificationKind::Elementary(elem_type) => {
                            TypeName::from(elem_type.as_id().original())
                        }
                        DataTypeSpecificationKind::UserDefined(type_name) => type_name.clone(),
                        _ => TypeName::from("UNKNOWN"), // Fallback for complex types
                    },
                };
                
                Ok(DataTypeDeclarationKind::Array(ArrayDeclaration {
                    type_name: type_def.name,
                    spec: ArraySpecificationKind::Subranges(array_subranges),
                    init: vec![], // TODO: Handle array initialization if needed
                }))
            }
            DataTypeSpecificationKind::String(string_spec) => {
                // Convert string specification to string declaration
                Ok(DataTypeDeclarationKind::String(StringDeclaration {
                    type_name: type_def.name,
                    length: Integer {
                        span: SourceSpan::default(),
                        value: 80, // Default string length
                    },
                    width: StringType::String, // Default to regular string
                    init: match type_def.default_value {
                        Some(ConstantKind::CharacterString(s)) => {
                            Some(s.value.iter().collect::<String>())
                        }
                        _ => None,
                    },
                }))
            }
        }
    }
}

impl Fold<Diagnostic> for TypeEnvironment {
    fn fold_simple_declaration(
        &mut self,
        node: SimpleDeclaration,
    ) -> Result<SimpleDeclaration, Diagnostic> {
        // A simple declaration consists of a type name followed by specification/initialization.
        match &node.spec_and_init {
            InitialValueAssignmentKind::None(_source_span) => {
                // TODO: Handle simple type declarations without initializers
            }
            InitialValueAssignmentKind::Simple(simple_initializer) => {
                match self.get(&simple_initializer.type_name) {
                    Some(_base_type) => {
                        // If the base type is known, then the type is valid this type
                        // will have the same attributes as the base type.
                        self.insert_alias(&node.type_name, &simple_initializer.type_name)?;
                    }
                    None => {
                        // If the base type is not know, then this is not valid
                        return Err(Diagnostic::problem(
                            Problem::ParentTypeNotDeclared,
                            Label::span(node.type_name.span(), "Derived type"),
                        )
                        .with_secondary(Label::span(
                            simple_initializer.type_name.span(),
                            "Base type",
                        )));
                    }
                }
            }
            InitialValueAssignmentKind::String(string_initializer) => {
                self.insert_type(&node.type_name, string::from(string_initializer))?;
            }
            InitialValueAssignmentKind::EnumeratedValues(enumerated_values_initializer) => {
                let attributes = enumeration::try_from_values(enumerated_values_initializer)?;
                self.insert_type(&node.type_name, attributes)?;
            }
            InitialValueAssignmentKind::EnumeratedType(_enumerated_initial_value_assignment) => {
                // I don't think this is needed because this should refer to a declared type, not declare a type.
            }
            InitialValueAssignmentKind::FunctionBlock(_function_block_initial_value_assignment) => {
                // TODO: Handle function block initializers
            }
            InitialValueAssignmentKind::Subrange(_spec) => {
                // TODO: Handle subrange specifications
            }
            InitialValueAssignmentKind::Structure(_structure_initialization_declaration) => {
                // TODO: Handle structure initializations
            }
            InitialValueAssignmentKind::Array(_array_initial_value_assignment) => {
                // TODO: Handle array initializers
            }
            InitialValueAssignmentKind::LateResolvedType(_type_name) => {
                return Err(Diagnostic::internal_error(file!(), line!()));
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
                // Alias of another enumeration: base must already exist because we sort the items
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
            EnumeratedSpecificationKind::Values(spec_values) => {
                let attributes = enumeration::try_from_values(spec_values)?;
                self.insert_type(&node.type_name, attributes)?;
            }
        }

        Ok(node)
    }

    fn fold_string_declaration(
        &mut self,
        node: StringDeclaration,
    ) -> Result<StringDeclaration, Diagnostic> {
        self.insert_type(&node.type_name, string::from_decl(&node))?;
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

    fn fold_subrange_declaration(
        &mut self,
        node: SubrangeDeclaration,
    ) -> Result<SubrangeDeclaration, Diagnostic> {
        let result = subrange::try_from(&node.type_name, &node.spec, self)?;

        match result {
            subrange::IntermediateResult::Type(attributes) => {
                self.insert_type(&node.type_name, attributes)?;
            }
            subrange::IntermediateResult::Alias(base_type_name) => {
                self.insert_alias(&node.type_name, &base_type_name)?;
            }
        }

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
        // Although most of the folding is handled by element-specific methods,
        // we need to handle folding of late bound at declaration kind level
        // because this will change the type of the declaration.
        match node {
            DataTypeDeclarationKind::LateBound(lb) => {
                let result = self.transform_late_bound_declaration(lb)?;
                let result = result.recurse_fold(self)?;
                Ok(result)
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
            LibraryElementKind::TypeDefinitionBlock(block) => {
                // Convert TypeDefinitionBlock to individual DataTypeDeclarations
                // and process each one to populate the type environment
                for type_def in &block.definitions {
                    let data_type_decl = self.convert_type_definition_to_data_type_declaration(type_def.clone())?;
                    // Process the converted declaration to add it to the type environment
                    self.fold_data_type_declaration_kind(data_type_decl)?;
                }
                
                // Return the original TypeDefinitionBlock as a placeholder
                // since we've already processed all the definitions
                Ok(LibraryElementKind::TypeDefinitionBlock(block))
            }
            _ => node.recurse_fold(self),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::intermediate_type::{ByteSized, IntermediateType};
    use crate::type_environment::{TypeEnvironment, TypeEnvironmentBuilder};

    use super::apply;
    use ironplc_dsl::diagnostic::Diagnostic;
    use ironplc_dsl::{common::*, core::FileId};
    use ironplc_parser::options::ParseOptions;
    use ironplc_problems::Problem;

    #[test]
    fn apply_when_ambiguous_enum_then_resolves_type() {
        let program = "
TYPE
LEVEL : (CRITICAL) := CRITICAL;
LEVEL_ALIAS : LEVEL;
END_TYPE
        ";
        let (result, _env) = parse_and_apply_with_elementary_types(program);
        let result = result.unwrap();

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

        assert_eq!(result, expected)
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
    fn apply_when_array_element_is_string_type_then_ok() {
        let program = "
TYPE
  STRING10 : STRING(10);
END_TYPE

TYPE
  ELEMENT_WEEKDAYS	: ARRAY [1..7] OF STRING10;
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

    #[test]
    fn apply_when_simple_type_alias_then_creates_alias() {
        let program = "
TYPE
MY_INT : INT := 0;
MY_BOOL : BOOL := FALSE;
END_TYPE
        ";
        let (_result, env) = parse_and_apply_with_elementary_types(program);

        let my_int_type = env.get(&TypeName::from("MY_INT")).unwrap();
        assert!(matches!(
            &my_int_type.representation,
            IntermediateType::Int {
                size: ByteSized::B16
            }
        ));

        let my_bool_type = env.get(&TypeName::from("MY_BOOL")).unwrap();
        assert!(matches!(
            &my_bool_type.representation,
            IntermediateType::Bool
        ));
    }

    #[test]
    fn apply_when_simple_type_alias_missing_base_then_error() {
        let program = "
TYPE
MY_TYPE : UNKNOWN_TYPE := 0;
END_TYPE

        ";
        let (result, _env) = parse_and_apply_with_elementary_types(program);

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(
            Problem::ParentTypeNotDeclared.code(),
            error.first().unwrap().code
        );
    }

    #[test]
    fn apply_when_int_type_alias_then_creates_alias() {
        let program = "
TYPE
MY_INT : INT := 42;
END_TYPE
        ";
        let (result, env) = parse_and_apply_with_elementary_types(program);
        assert!(result.is_ok());

        // Verify the alias was created
        let my_int_type = env.get(&TypeName::from("MY_INT")).unwrap();
        assert!(matches!(
            &my_int_type.representation,
            IntermediateType::Int {
                size: ByteSized::B16
            }
        ));
    }

    #[test]
    fn apply_when_invalid_base_type_then_error() {
        let program = "
TYPE
MY_TYPE : UNKNOWN_TYPE := 0;
END_TYPE
        ";
        let (result, _env) = parse_and_apply_with_empty_env(program);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(
            Problem::ParentTypeNotDeclared.code(),
            error.first().unwrap().code
        );
    }

    #[test]
    fn apply_when_real_type_alias_then_creates_alias() {
        let program = "
TYPE
MY_REAL : REAL := 3.14;
END_TYPE
        ";
        let (result, env) = parse_and_apply_with_elementary_types(program);
        assert!(result.is_ok());

        // Verify the alias was created
        let my_real_type = env.get(&TypeName::from("MY_REAL")).unwrap();
        assert!(matches!(
            &my_real_type.representation,
            IntermediateType::Real {
                size: ByteSized::B32
            }
        ));
    }

    #[test]
    fn apply_when_bool_type_alias_then_creates_alias() {
        let program = "
TYPE
MY_BOOL : BOOL := TRUE;
END_TYPE
        ";
        let (result, env) = parse_and_apply_with_elementary_types(program);
        assert!(result.is_ok());

        // Verify the alias was created
        let my_bool_type = env.get(&TypeName::from("MY_BOOL")).unwrap();
        assert!(matches!(
            &my_bool_type.representation,
            IntermediateType::Bool
        ));
    }

    #[test]
    fn apply_when_dint_type_alias_then_creates_alias() {
        let program = "
TYPE
MY_DINT : DINT := 1000;
END_TYPE
        ";
        let (result, env) = parse_and_apply_with_elementary_types(program);
        assert!(result.is_ok());

        // Verify the alias was created
        let my_dint_type = env.get(&TypeName::from("MY_DINT")).unwrap();
        assert!(matches!(
            &my_dint_type.representation,
            IntermediateType::Int {
                size: ByteSized::B32
            }
        ));
    }

    #[test]
    fn apply_when_time_type_alias_then_creates_alias() {
        let program = "
TYPE
MY_TIME : TIME := T#5s;
END_TYPE
        ";
        let (result, env) = parse_and_apply_with_elementary_types(program);
        assert!(result.is_ok());

        // Verify the alias was created
        let my_time_type = env.get(&TypeName::from("MY_TIME")).unwrap();
        assert!(matches!(
            &my_time_type.representation,
            IntermediateType::Time
        ));
    }

    #[test]
    fn apply_when_multiple_type_aliases_then_creates_all_aliases() {
        let program = "
TYPE
MY_INT : INT := 42;
MY_BOOL : BOOL := FALSE;
MY_REAL : REAL := 2.71;
END_TYPE
        ";
        let (result, env) = parse_and_apply_with_elementary_types(program);
        assert!(result.is_ok());

        // Verify all aliases were created
        let my_int_type = env.get(&TypeName::from("MY_INT")).unwrap();
        assert!(matches!(
            &my_int_type.representation,
            IntermediateType::Int {
                size: ByteSized::B16
            }
        ));

        let my_bool_type = env.get(&TypeName::from("MY_BOOL")).unwrap();
        assert!(matches!(
            &my_bool_type.representation,
            IntermediateType::Bool
        ));

        let my_real_type = env.get(&TypeName::from("MY_REAL")).unwrap();
        assert!(matches!(
            &my_real_type.representation,
            IntermediateType::Real {
                size: ByteSized::B32
            }
        ));
    }

    #[test]
    fn apply_when_byte_type_alias_then_creates_alias() {
        let program = "
TYPE
MY_BYTE : BYTE := 16#FF;
END_TYPE
        ";
        let (result, env) = parse_and_apply_with_elementary_types(program);
        assert!(result.is_ok());

        // Verify the alias was created
        let my_byte_type = env.get(&TypeName::from("MY_BYTE")).unwrap();
        assert!(matches!(
            &my_byte_type.representation,
            IntermediateType::Bytes {
                size: ByteSized::B8
            }
        ));
    }

    /// Helper function to parse 61131-3 code and apply type resolution with elementary types
    fn parse_and_apply_with_elementary_types(
        program: &str,
    ) -> (Result<Library, Vec<Diagnostic>>, TypeEnvironment) {
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let result = apply(input, &mut env);
        (result, env)
    }

    /// Helper function to parse 61131-3 code and apply type resolution with empty environment
    fn parse_and_apply_with_empty_env(
        program: &str,
    ) -> (Result<Library, Vec<Diagnostic>>, TypeEnvironment) {
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut env = TypeEnvironment::new();
        let result = apply(input, &mut env);
        (result, env)
    }
}
