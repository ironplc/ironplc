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

use crate::scoped_table::{ScopedTable, Value};
use crate::semantic_type::SemanticType;
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
    Array,
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
    /// Records the kind of `to_add`, keeping the first kind recorded for a
    /// name declared more than once. The repeat is not diagnosed here: the
    /// symbol environment reports it, and this pass resolves initializers
    /// against the declaration that is kept.
    fn add_if_new(
        &mut self,
        to_add: &TypeName,
        kind: TypeDefinitionKind,
    ) -> Result<(), Diagnostic> {
        self.try_add(to_add, kind);
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
            DataTypeDeclarationKind::Array(node) => {
                self.add_if_new(&node.type_name, TypeDefinitionKind::Array)
            }
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
                SemanticType::FunctionBlock { .. } => ResolvedKind::FunctionBlock,
                SemanticType::Structure { .. } => ResolvedKind::Structure,
                SemanticType::Enumeration { .. } => ResolvedKind::Enumeration,
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
                        // Keeps the name, as for a subrange, so the declaration
                        // still says which array type it declares; the name
                        // resolves to the same array as its definition would.
                        TypeDefinitionKind::Array => Ok(InitialValueAssignmentKind::Array(
                            ArrayInitialValueAssignment {
                                spec: SpecificationKind::Named(name),
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
mod tests;
