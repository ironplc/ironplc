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
use ironplc_dsl::textual::{Expr, ExprKind, NamedVariable, SymbolicVariableKind, Variable};
use ironplc_dsl::visitor::Visitor;
use ironplc_problems::Problem;
use log::trace;

use crate::intermediate_type::IntermediateType;
use crate::scoped_table::{ScopedTable, Value};
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

/// What a user type name turned out to be, as far as an initializer's shape
/// is concerned.
enum ResolvedKind {
    FunctionBlock,
    Structure,
    Enumeration,
    /// Any other declared type: an alias, a subrange, a string, an array.
    Other,
}

impl TypeResolver<'_> {
    /// Classifies `name` from the type environment, or from this pass's own
    /// table of declared types when the environment does not hold it, in
    /// the same order the bare-declaration arm consults them.
    fn classify(&self, name: &TypeName) -> Option<ResolvedKind> {
        if let Some(attrs) = self.type_environment.get(name) {
            return Some(match &attrs.representation {
                IntermediateType::FunctionBlock { .. } => ResolvedKind::FunctionBlock,
                IntermediateType::Structure { .. } => ResolvedKind::Structure,
                IntermediateType::Enumeration { .. } => ResolvedKind::Enumeration,
                _ => ResolvedKind::Other,
            });
        }
        self.types.find(name).map(|kind| match kind {
            TypeDefinitionKind::FunctionBlock => ResolvedKind::FunctionBlock,
            TypeDefinitionKind::Structure | TypeDefinitionKind::StructureInitialization => {
                ResolvedKind::Structure
            }
            TypeDefinitionKind::Enumeration => ResolvedKind::Enumeration,
            _ => ResolvedKind::Other,
        })
    }

    /// Gives a declaration written against a user type name, with an
    /// initializer, the kind its type implies (ADR-0050). The parser could
    /// not tell a structure from a function block, or an enumeration value
    /// from a named constant; here the types are known.
    fn resolve_initialized(
        &mut self,
        name: TypeName,
        initial_value: LateResolvedInitialValue,
    ) -> Result<InitialValueAssignmentKind, Diagnostic> {
        let Some(kind) = self.classify(&name) else {
            // Undeclared, as for a bare declaration: say so and keep the
            // placeholder so the rest of the library still resolves.
            self.diagnostics.push(
                Diagnostic::problem(
                    Problem::UndeclaredUnknownType,
                    Label::span(name.span(), "Variable type"),
                )
                .with_context_type("identifier", &name),
            );
            return Ok(InitialValueAssignmentKind::LateResolvedType(
                LateResolvedInitializer {
                    type_name: name,
                    initial_value: Some(initial_value),
                },
            ));
        };
        Ok(match (initial_value, kind) {
            (LateResolvedInitialValue::Members(elements), ResolvedKind::FunctionBlock) => {
                InitialValueAssignmentKind::FunctionBlock(FunctionBlockInitialValueAssignment {
                    type_name: name,
                    init: elements,
                })
            }
            // A structure, or a type that takes no members at all; the
            // initializer checks report the latter against the declared type.
            (LateResolvedInitialValue::Members(elements), _) => {
                InitialValueAssignmentKind::Structure(StructureInitializationDeclaration {
                    type_name: name,
                    elements_init: elements,
                })
            }
            (LateResolvedInitialValue::Value(value), ResolvedKind::Enumeration) => {
                InitialValueAssignmentKind::EnumeratedType(EnumeratedInitialValueAssignment {
                    type_name: name,
                    initial_value: Some(EnumeratedValue {
                        type_name: None,
                        value,
                        explicit_value: None,
                    }),
                })
            }
            // A named constant for any other type: a constant expression,
            // which `xform_fold_initializer_expressions` evaluates or
            // diagnoses like every other.
            (LateResolvedInitialValue::Value(value), _) => {
                InitialValueAssignmentKind::SimpleExpr(SimpleExprInitializer {
                    type_name: name,
                    initial_value: Expr::new(ExprKind::Variable(Variable::Symbolic(
                        SymbolicVariableKind::Named(NamedVariable { name: value }),
                    ))),
                })
            }
        })
    }
}

impl Fold<Diagnostic> for TypeResolver<'_> {
    fn fold_initial_value_assignment_kind(
        &mut self,
        node: InitialValueAssignmentKind,
    ) -> Result<InitialValueAssignmentKind, Diagnostic> {
        match node {
            // TODO this needs to handle struct definitions
            InitialValueAssignmentKind::LateResolvedType(LateResolvedInitializer {
                type_name: name,
                initial_value: Some(initial_value),
            }) => self.resolve_initialized(name, initial_value),
            InitialValueAssignmentKind::LateResolvedType(LateResolvedInitializer {
                type_name: name,
                initial_value: None,
            }) => {
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
                        Ok(InitialValueAssignmentKind::LateResolvedType(
                            LateResolvedInitializer::bare(name),
                        ))
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

    #[rstest::rstest]
    #[case::declared_block("FB_Counter", "(limit := 20)")]
    #[case::stdlib_block("TON", "(PT := T#1s)")]
    fn apply_when_member_initializer_on_function_block_then_function_block_initializer(
        #[case] type_name: &str,
        #[case] initializer: &str,
    ) {
        let program = format!(
            "
FUNCTION_BLOCK FB_Counter
VAR
    limit : INT := 10;
END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
    inst : {type_name} := {initializer};
END_VAR
END_PROGRAM"
        );
        let library = crate::test_helpers::parse_and_resolve_types(&program);

        let decl = library
            .elements
            .iter()
            .find_map(|element| match element {
                LibraryElementKind::ProgramDeclaration(program) => program.variables.first(),
                _ => None,
            })
            .unwrap();
        assert!(matches!(
            &decl.initializer,
            InitialValueAssignmentKind::FunctionBlock(init)
                if init.type_name == TypeName::from(type_name) && init.init.len() == 1
        ));
    }

    // -----------------------------------------------------------------
    // A user-typed initializer is classified by the type (ADR-0050).
    // -----------------------------------------------------------------

    /// The first variable of the first program organization unit after the
    /// pass, with the diagnostics it raised.
    fn resolve_first_var(program: &str) -> (VarDecl, Vec<ironplc_dsl::diagnostic::Diagnostic>) {
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &CompilerOptions::default())
                .unwrap();
        let mut type_environment = TypeEnvironment::new();
        let (result, diagnostics) = apply(input, &mut type_environment).unwrap();
        let decl = result
            .elements
            .iter()
            .find_map(|element| match element {
                LibraryElementKind::ProgramDeclaration(program) => program.variables.first(),
                _ => None,
            })
            .unwrap()
            .clone();
        (decl, diagnostics)
    }

    #[test]
    fn apply_when_members_on_structure_type_then_structure_initializer() {
        let (decl, diagnostics) = resolve_first_var(
            "
TYPE
    Point : STRUCT a : INT; b : INT; END_STRUCT;
END_TYPE
PROGRAM main
VAR
    p : Point := (a := 1);
END_VAR
END_PROGRAM",
        );
        assert!(diagnostics.is_empty());
        assert!(matches!(
            &decl.initializer,
            InitialValueAssignmentKind::Structure(init)
                if init.type_name == TypeName::from("Point") && init.elements_init.len() == 1
        ));
    }

    #[test]
    fn apply_when_value_on_enumeration_type_then_enumerated_initializer() {
        let (decl, diagnostics) = resolve_first_var(
            "
TYPE
    Color : (Red, Green);
END_TYPE
PROGRAM main
VAR
    c : Color := Green;
END_VAR
END_PROGRAM",
        );
        assert!(diagnostics.is_empty());
        assert!(matches!(
            &decl.initializer,
            InitialValueAssignmentKind::EnumeratedType(init)
                if init.type_name == TypeName::from("Color")
                    && init.initial_value.as_ref().map(|v| v.value.clone()) == Some(Id::from("Green"))
        ));
    }

    #[test]
    fn apply_when_value_on_alias_type_then_constant_expression_initializer() {
        // A bare identifier on a non-enumeration type is a named constant,
        // for the initializer-expression fold to evaluate.
        let (decl, diagnostics) = resolve_first_var(
            "
TYPE
    Count : INT := 0;
END_TYPE
PROGRAM main
VAR
    n : Count := LIMIT;
END_VAR
END_PROGRAM",
        );
        assert!(diagnostics.is_empty());
        assert!(matches!(
            &decl.initializer,
            InitialValueAssignmentKind::SimpleExpr(init)
                if init.type_name == TypeName::from("Count")
                    && matches!(&init.initial_value.kind, ironplc_dsl::textual::ExprKind::Variable(_))
        ));
    }

    #[test]
    fn apply_when_members_on_undeclared_type_then_diagnosed_and_placeholder_kept() {
        let (decl, diagnostics) = resolve_first_var(
            "
PROGRAM main
VAR
    p : Missing := (a := 1);
END_VAR
END_PROGRAM",
        );
        assert_eq!(1, diagnostics.len());
        assert_eq!(Problem::UndeclaredUnknownType.code(), diagnostics[0].code);
        assert!(matches!(
            &decl.initializer,
            InitialValueAssignmentKind::LateResolvedType(LateResolvedInitializer {
                initial_value: Some(LateResolvedInitialValue::Members(_)),
                ..
            })
        ));
    }
}
