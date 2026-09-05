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
use crate::type_environment::TypeEnvironment;

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
    String(StringType, IntegerRef),
    FunctionBlock,
    Reference(ReferenceTarget),
}

impl Value for TypeDefinitionKind {}

pub fn apply(
    lib: Library,
    type_environment: &mut TypeEnvironment,
) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>> {
    let mut type_to_type_kind: ScopedTable<TypeName, TypeDefinitionKind> = ScopedTable::new();

    // Walk the entire library to find the types. We don't need
    // to keep track of contexts because types are global scoped.
    type_to_type_kind.walk(&lib).map_err(|err| vec![err])?;

    // Set the types for each item.
    let mut resolver = TypeResolver {
        types: type_to_type_kind,
        type_environment,
        diagnostics: vec![],
    };
    // An unresolvable type on one declaration (e.g. a reference to a type
    // that isn't declared anywhere in the compilation unit) is diagnosed
    // but does not stop the fold: every other, unrelated declaration is
    // still resolved. Only a genuine fold failure (a compiler bug, not a
    // user error) should discard the result.
    let result = resolver.fold_library(lib).map_err(|e| vec![e])?;

    Ok((result, resolver.diagnostics))
}

impl ScopedTable<'_, TypeName, TypeDefinitionKind> {
    fn add_if_new(
        &mut self,
        to_add: &TypeName,
        kind: TypeDefinitionKind,
    ) -> Result<(), Diagnostic> {
        if let Some(existing) = self.try_add(to_add, kind) {
            return Err(Diagnostic::problem(
                Problem::DefinitionNameDuplicated,
                Label::span(to_add.span(), format!("Duplicated definition {to_add}")),
            )
            .with_secondary(Label::span(existing.0.span(), "First definition")));
        }

        Ok(())
    }
}

impl Visitor<Diagnostic> for ScopedTable<'_, TypeName, TypeDefinitionKind> {
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
            DataTypeDeclarationKind::Reference(node) => self.add_if_new(
                &node.type_name,
                TypeDefinitionKind::Reference(node.target.clone()),
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
        self.add_if_new(&node.name, TypeDefinitionKind::FunctionBlock)
    }
}

struct TypeResolver<'a> {
    types: ScopedTable<'a, TypeName, TypeDefinitionKind>,
    type_environment: &'a TypeEnvironment,
    diagnostics: Vec<Diagnostic>,
}

impl TypeResolver<'_> {
    /// Returns whether `name` names a function block type.
    ///
    /// Covers all three places a function block type can come from: the
    /// type environment (standard library blocks such as `TON`), the set of
    /// standard types recognized but not yet supported, and the `FUNCTION_BLOCK`
    /// declarations collected from this compilation unit.
    fn is_function_block_type(&self, name: &TypeName) -> bool {
        if let Some(ty) = self.type_environment.get(name) {
            if ty.representation.is_function_block() {
                return true;
            }
        }
        if is_unsupported_standard_type(name) {
            return true;
        }
        matches!(
            self.types.find(name),
            Some(TypeDefinitionKind::FunctionBlock)
        )
    }
}

impl Fold<Diagnostic> for TypeResolver<'_> {
    fn fold_initial_value_assignment_kind(
        &mut self,
        node: InitialValueAssignmentKind,
    ) -> Result<InitialValueAssignmentKind, Diagnostic> {
        match node {
            // `x : T := (a := 1)` always parses as a structure initializer:
            // the parser cannot know whether `T` names a STRUCT or a
            // function block, because no type declaration is in scope yet.
            // A function-block type makes it an instance with member
            // initial values -- `StructureInitializationDeclaration` and
            // `FunctionBlockInitialValueAssignment` carry the same
            // `Vec<StructureElementInit>` precisely because the two are
            // the same construct. Without this the variable stays a
            // structure, so it can be neither invoked (P4012) nor laid out
            // (there is no such structure type).
            InitialValueAssignmentKind::Structure(decl)
                if self.is_function_block_type(&decl.type_name) =>
            {
                Ok(InitialValueAssignmentKind::FunctionBlock(
                    FunctionBlockInitialValueAssignment {
                        type_name: decl.type_name,
                        init: decl.elements_init,
                    },
                ))
            }
            // TODO this needs to handle struct definitions
            InitialValueAssignmentKind::LateResolvedType(name) => {
                // Check the type environment for known types (elementary types and stdlib FBs)
                if let Some(ty) = self.type_environment.get(&name) {
                    if ty.representation.is_primitive() {
                        return Ok(InitialValueAssignmentKind::Simple(SimpleInitializer {
                            type_name: name,
                            initial_value: None,
                        }));
                    }
                    // Stdlib function blocks (TON, TOF, TP, CTU, etc.) are in the type environment
                    if ty.representation.is_function_block() {
                        return Ok(InitialValueAssignmentKind::FunctionBlock(
                            FunctionBlockInitialValueAssignment {
                                type_name: name,
                                init: vec![],
                            },
                        ));
                    }
                    // Subrange types (e.g., MY_RANGE : INT (1..100))
                    if ty.representation.is_subrange() {
                        return Ok(InitialValueAssignmentKind::Subrange(
                            SpecificationKind::Named(name),
                        ));
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
                        TypeDefinitionKind::Reference(ref_target) => Ok(
                            InitialValueAssignmentKind::Reference(ReferenceInitializer {
                                target: ref_target.clone(),
                                initial_value: None,
                                // Resolved from a named reference-type alias; the
                                // original surface keyword is not preserved through
                                // the alias and is not rendered for a named target.
                                syntax: RefSyntax::RefTo,
                            }),
                        ),
                        TypeDefinitionKind::Subrange => Ok(InitialValueAssignmentKind::Subrange(
                            SpecificationKind::Named(name),
                        )),
                        _ => Err(Diagnostic::todo_with_type(&name)),
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
    use crate::type_environment::{TypeEnvironment, TypeEnvironmentBuilder};

    use super::apply;
    use ironplc_dsl::{
        common::*,
        core::{FileId, Id, SourceSpan},
    };
    use ironplc_parser::options::CompilerOptions;
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
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironment::new();
        let result = apply(input, &mut type_environment).unwrap().0;

        let expected = Library {
            elements: vec![
                LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: TypeName::from("called"),
                    variables: vec![],
                    edge_variables: vec![],
                    body: FunctionBlockBodyKind::empty(),
                    span: SourceSpan::default(),
                    oop: None,
                    methods: vec![],
                }),
                LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: TypeName::from("caller"),
                    variables: vec![VarDecl::function_block("fb_var", "called")],
                    edge_variables: vec![],
                    body: FunctionBlockBodyKind::empty(),
                    span: SourceSpan::default(),
                    oop: None,
                    methods: vec![],
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
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironment::new();
        let result = apply(input, &mut type_environment).unwrap().0;

        let expected = Library {
            elements: vec![
                LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Structure(
                    StructureDeclaration {
                        type_name: TypeName::from("the_struct"),
                        elements: vec![StructureElementDeclaration {
                            name: Id::from("member"),
                            init: InitialValueAssignmentKind::simple_uninitialized(TypeName::from(
                                "BOOL",
                            )),
                        }],
                    },
                )),
                LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: TypeName::from("caller"),
                    variables: vec![VarDecl::structure("the_var", "the_struct")],
                    edge_variables: vec![],
                    body: FunctionBlockBodyKind::empty(),
                    span: SourceSpan::default(),
                    oop: None,
                    methods: vec![],
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
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironment::new();
        let result = apply(input, &mut type_environment).unwrap().0;

        let expected = Library {
            elements: vec![
                LibraryElementKind::DataTypeDeclaration(DataTypeDeclarationKind::Enumeration(
                    EnumerationDeclaration {
                        type_name: TypeName::from("values"),
                        spec_init: EnumeratedSpecificationInit {
                            spec: EnumeratedSpecificationKind::from_values(vec![
                                "val1", "val2", "val3",
                            ]),
                            default: None,
                            underlying_type: None,
                        },
                    },
                )),
                LibraryElementKind::FunctionBlockDeclaration(FunctionBlockDeclaration {
                    name: TypeName::from("caller"),
                    variables: vec![VarDecl::uninitialized_enumerated("the_var", "values")],
                    edge_variables: vec![],
                    body: FunctionBlockBodyKind::empty(),
                    span: SourceSpan::default(),
                    oop: None,
                    methods: vec![],
                }),
            ],
        };

        assert_eq!(result, expected)
    }

    #[test]
    fn apply_when_has_subrange_type_then_resolves_type() {
        let program = "
TYPE
    my_range : INT (1..100);
END_TYPE

FUNCTION_BLOCK caller
    VAR
        the_var : my_range;
    END_VAR

END_FUNCTION_BLOCK
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironment::new();
        let result = apply(input, &mut type_environment).unwrap().0;

        // Find the caller function block and check the variable initializer
        let caller_fb = result.elements.iter().find(|e| {
            matches!(e, LibraryElementKind::FunctionBlockDeclaration(fb) if fb.name == TypeName::from("caller"))
        });
        assert!(caller_fb.is_some());

        if let LibraryElementKind::FunctionBlockDeclaration(fb) = caller_fb.unwrap() {
            assert_eq!(fb.variables.len(), 1);
            assert!(matches!(
                &fb.variables[0].initializer,
                InitialValueAssignmentKind::Subrange(SpecificationKind::Named(tn))
                if *tn == TypeName::from("my_range")
            ));
        }
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
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironment::new();
        let result = apply(input, &mut type_environment);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(1, err.len());
        assert_eq!(Problem::DefinitionNameDuplicated.code(), err[0].code);
    }

    #[test]
    fn apply_when_unrelated_pou_has_undeclared_type_then_other_pou_still_resolves() {
        // FB_A has a genuinely broken reference to an undeclared type.
        // FB_B is entirely unrelated and valid. Resolving FB_A's error
        // must not discard the successful resolution of FB_B's variable.
        let program = "
FUNCTION_BLOCK FB_A
VAR
    x : Undeclared_Type;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Callee
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_B
VAR
    inst : FB_Callee;
END_VAR
END_FUNCTION_BLOCK
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironment::new();
        let (result, diagnostics) = apply(input, &mut type_environment).unwrap();

        assert_eq!(1, diagnostics.len());
        assert_eq!(Problem::UndeclaredUnknownType.code(), diagnostics[0].code);

        let fb_b = result
            .elements
            .iter()
            .find_map(|e| match e {
                LibraryElementKind::FunctionBlockDeclaration(fb)
                    if fb.name == TypeName::from("FB_B") =>
                {
                    Some(fb)
                }
                _ => None,
            })
            .unwrap();

        assert!(matches!(
            &fb_b.variables[0].initializer,
            InitialValueAssignmentKind::FunctionBlock(fb_init)
            if fb_init.type_name == TypeName::from("FB_Callee")
        ));
    }

    #[test]
    fn apply_when_same_pou_has_undeclared_type_then_other_variable_still_resolves() {
        // Both the broken and the valid variable declaration live in the
        // same POU, matching the shape found in a real corpus.
        let program = "
FUNCTION_BLOCK FB_Callee
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_A
VAR
    x : Undeclared_Type;
    inst : FB_Callee;
END_VAR
END_FUNCTION_BLOCK
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironment::new();
        let (result, diagnostics) = apply(input, &mut type_environment).unwrap();

        assert_eq!(1, diagnostics.len());
        assert_eq!(Problem::UndeclaredUnknownType.code(), diagnostics[0].code);

        let fb_a = result
            .elements
            .iter()
            .find_map(|e| match e {
                LibraryElementKind::FunctionBlockDeclaration(fb)
                    if fb.name == TypeName::from("FB_A") =>
                {
                    Some(fb)
                }
                _ => None,
            })
            .unwrap();

        assert!(matches!(
            &fb_a.variables[1].initializer,
            InitialValueAssignmentKind::FunctionBlock(fb_init)
            if fb_init.type_name == TypeName::from("FB_Callee")
        ));
    }

    /// A standard library function block declared with a parenthesized member
    /// initializer is an *instance* of that block, not a structure -- the
    /// remedy `docs/reference/compiler/problems/P4043.rst` offers for P4043.
    #[test]
    fn apply_when_stdlib_fb_type_has_member_initializer_then_resolves_to_instance() {
        let program = "
FUNCTION_BLOCK FB_Example
VAR
    tonDelta : TON := (PT := T#100MS);
END_VAR
END_FUNCTION_BLOCK
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .with_stdlib_function_blocks()
            .build()
            .unwrap();
        let (result, diagnostics) = apply(input, &mut type_environment).unwrap();
        assert!(diagnostics.is_empty());

        let fb = first_function_block(&result);
        let initializer = &fb.variables[0].initializer;
        let InitialValueAssignmentKind::FunctionBlock(fb_init) = initializer else {
            panic!("expected a function block instance, got {initializer:?}");
        };
        assert_eq!(TypeName::from("TON"), fb_init.type_name);
        assert_eq!(1, fb_init.init.len());
        assert_eq!(Id::from("PT"), fb_init.init[0].name);
    }

    /// The same holds for a user-declared function block.
    #[test]
    fn apply_when_user_fb_type_has_member_initializer_then_resolves_to_instance() {
        let program = "
FUNCTION_BLOCK SCALER
VAR_INPUT
    factor : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK caller
VAR
    scaler : SCALER := (factor := 3);
END_VAR
END_FUNCTION_BLOCK
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let (result, diagnostics) = apply(input, &mut type_environment).unwrap();
        assert!(diagnostics.is_empty());

        let caller = result
            .elements
            .iter()
            .find_map(|e| match e {
                LibraryElementKind::FunctionBlockDeclaration(fb)
                    if fb.name == TypeName::from("caller") =>
                {
                    Some(fb)
                }
                _ => None,
            })
            .unwrap();
        assert!(matches!(
            &caller.variables[0].initializer,
            InitialValueAssignmentKind::FunctionBlock(fb_init)
            if fb_init.type_name == TypeName::from("SCALER") && fb_init.init.len() == 1
        ));
    }

    /// A genuine STRUCT type with the same initializer shape stays a structure.
    #[test]
    fn apply_when_struct_type_has_member_initializer_then_stays_a_structure() {
        let program = "
TYPE the_struct : STRUCT x : INT; END_STRUCT; END_TYPE

FUNCTION_BLOCK caller
VAR
    the_var : the_struct := (x := 3);
END_VAR
END_FUNCTION_BLOCK
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        let (result, diagnostics) = apply(input, &mut type_environment).unwrap();
        assert!(diagnostics.is_empty());

        let caller = first_function_block(&result);
        assert!(matches!(
            &caller.variables[0].initializer,
            InitialValueAssignmentKind::Structure(decl)
            if decl.type_name == TypeName::from("the_struct")
        ));
    }

    /// Returns the first function block declaration in the library.
    fn first_function_block(library: &Library) -> &FunctionBlockDeclaration {
        library
            .elements
            .iter()
            .find_map(|e| match e {
                LibraryElementKind::FunctionBlockDeclaration(fb) => Some(fb),
                _ => None,
            })
            .unwrap()
    }
}
