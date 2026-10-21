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
use crate::intermediates::*;
use crate::semantic_type::SemanticType;
use crate::type_environment::TypeEnvironment;
use ironplc_dsl::common::*;
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_problems::Problem;

/// Populates the type environment (this also transforms late bound
/// declarations).
///
/// A repeated type or function block name is returned as a diagnostic
/// alongside the library rather than failing the pass: the environment keeps
/// the first declaration and every other declaration still resolves. `Err`
/// is reserved for a declaration that cannot be resolved at all.
pub fn apply(
    lib: Library,
    type_environment: &mut TypeEnvironment,
) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>> {
    let lib = type_environment
        .fold_library(lib)
        .map_err(|err| vec![err])?;
    Ok((lib, type_environment.take_duplicates()))
}

impl TypeEnvironment {
    fn transform_late_bound_declaration(
        &mut self,
        node: LateBoundDeclaration,
    ) -> Result<DataTypeDeclarationKind, Diagnostic> {
        // At this point we should have a type for the late bound declaration
        // so we can replace the late bound declaration with the correct type
        let existing = self.get(&node.base_type_name).ok_or_else(|| {
            Diagnostic::problem(
                Problem::ParentTypeNotDeclared,
                Label::span(node.data_type_name.span(), "Type alias"),
            )
            .with_secondary(Label::span(node.base_type_name.span(), "Base type"))
        })?;

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
                SemanticType::Enumeration { .. } => Ok(DataTypeDeclarationKind::Enumeration(
                    EnumerationDeclaration {
                        type_name: node.data_type_name,
                        spec_init: EnumeratedSpecificationInit {
                            spec: SpecificationKind::Named(node.base_type_name),
                            default: None,
                            underlying_type: None,
                        },
                    },
                )),
                SemanticType::Structure { fields: _ } => {
                    Ok(DataTypeDeclarationKind::StructureInitialization(
                        StructureInitializationDeclaration {
                            type_name: node.base_type_name,
                            elements_init: vec![],
                        },
                    ))
                }
                SemanticType::Array { .. } => {
                    Ok(DataTypeDeclarationKind::Array(ArrayDeclaration {
                        type_name: node.data_type_name,
                        spec: SpecificationKind::Named(node.base_type_name),
                        init: vec![],
                    }))
                }
                SemanticType::Subrange { .. } => {
                    Ok(DataTypeDeclarationKind::Subrange(SubrangeDeclaration {
                        type_name: node.data_type_name,
                        spec: SpecificationKind::Named(node.base_type_name),
                        default: None,
                    }))
                }
                SemanticType::Reference { .. } => {
                    // Reference type alias: treat as a simple alias
                    Ok(DataTypeDeclarationKind::Reference(
                        ironplc_dsl::common::ReferenceDeclaration {
                            type_name: node.data_type_name,
                            target: ironplc_dsl::common::ReferenceTarget::Named(
                                node.base_type_name,
                            ),
                            // Resolved alias; original surface keyword not preserved.
                            syntax: ironplc_dsl::common::RefSyntax::RefTo,
                        },
                    ))
                }
                // A function block is a POU (Program Organization Unit), not a TYPE
                // declaration, so it should never appear here.
                // If we reach this branch, it indicates a bug in the compiler.
                SemanticType::FunctionBlock { .. } => Err(Diagnostic::internal_error()),
                // An alias of an interface type (`TYPE T : I_X; END_TYPE`)
                // is not supported yet.
                SemanticType::Interface { .. } => {
                    Err(Diagnostic::todo_with_type(&node.base_type_name))
                }
                // Primitive types are handled by the is_primitive() check above,
                // so reaching this branch indicates a bug in the compiler
                SemanticType::Bool
                | SemanticType::Int { .. }
                | SemanticType::UInt { .. }
                | SemanticType::Real { .. }
                | SemanticType::Bytes { .. }
                | SemanticType::Time { .. }
                | SemanticType::Date { .. }
                | SemanticType::TimeOfDay { .. }
                | SemanticType::DateAndTime { .. }
                | SemanticType::String { .. } => Err(Diagnostic::internal_error()),
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
            InitialValueAssignmentKind::None(source_span) => {
                // Simple declarations must have a type specification
                // Example: TYPE MY_TYPE : INT; END_TYPE (valid)
                // Example: TYPE MY_TYPE; END_TYPE (invalid - this case)
                return Err(Diagnostic::problem(
                    Problem::InvalidSimpleTypeDecl,
                    Label::span(node.type_name.span(), "Type declaration"),
                )
                .with_secondary(Label::span(
                    source_span.clone(),
                    "Type specification required (e.g., ': INT')",
                )));
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
                self.insert_type(&node.type_name, string::from(string_initializer));
            }
            InitialValueAssignmentKind::EnumeratedValues(enumerated_values_initializer) => {
                let default = enumerated_values_initializer
                    .initial_value
                    .as_ref()
                    .map(|value| &value.value);
                let attributes =
                    enumeration::try_from_values(enumerated_values_initializer, default, None)?;
                self.insert_type(&node.type_name, attributes);
            }
            InitialValueAssignmentKind::EnumeratedType(_enumerated_initial_value_assignment) => {
                // I don't think this is needed because this should refer to a declared type, not declare a type.
            }
            InitialValueAssignmentKind::FunctionBlock(fb_init) => {
                // Handle function block type aliases like: TYPE MyFBAlias : ExistingFB := (input := 10); END_TYPE
                // This creates an alias to an existing function block type
                if self.get(&fb_init.type_name).is_none() {
                    return Err(Diagnostic::problem(
                        Problem::ParentTypeNotDeclared,
                        Label::span(node.type_name.span(), "Function block type alias"),
                    )
                    .with_secondary(Label::span(fb_init.type_name.span(), "Base type")));
                }
                self.insert_alias(&node.type_name, &fb_init.type_name)?;
            }
            // Only the type resolver produces an interface initializer, and
            // only for variables, so a type declaration never has one.
            InitialValueAssignmentKind::Interface(_) => return Err(Diagnostic::internal_error()),
            InitialValueAssignmentKind::FunctionBlockCall(fb_call) => {
                // The call-style FB initializer (`X : FB(args)`) is only
                // produced by the parser for VAR declarations, not type
                // declarations, so this arm is effectively unreachable here.
                // Handle it like the FunctionBlock alias arm for robustness:
                // treat it as an alias to the referenced FB type, ignoring
                // the (codegen-unsupported) constructor arguments.
                if self.get(&fb_call.type_name).is_none() {
                    return Err(Diagnostic::problem(
                        Problem::ParentTypeNotDeclared,
                        Label::span(node.type_name.span(), "Function block type alias"),
                    )
                    .with_secondary(Label::span(fb_call.type_name.span(), "Base type")));
                }
                self.insert_alias(&node.type_name, &fb_call.type_name)?;
            }
            InitialValueAssignmentKind::Subrange(subrange) => {
                // Handle subrange specifications like: TYPE MY_RANGE : INT (1..100); END_TYPE
                let result = subrange::try_from(&node.type_name, &subrange.spec, self)?;
                match result {
                    TypeResolution::Type(attributes) => {
                        self.insert_type(&node.type_name, attributes);
                    }
                    TypeResolution::Alias(base_type_name) => {
                        self.insert_alias(&node.type_name, &base_type_name)?;
                    }
                }
            }
            InitialValueAssignmentKind::Structure(structure_init) => {
                // Handle structure type aliases like: TYPE MyAlias : ExistingStruct := (field := 10); END_TYPE
                // This creates an alias to an existing structure type
                if self.get(&structure_init.type_name).is_none() {
                    return Err(Diagnostic::problem(
                        Problem::ParentTypeNotDeclared,
                        Label::span(node.type_name.span(), "Structure type alias"),
                    )
                    .with_secondary(Label::span(structure_init.type_name.span(), "Base type")));
                }
                self.insert_alias(&node.type_name, &structure_init.type_name)?;
            }
            InitialValueAssignmentKind::Array(array_init) => {
                // Handle array specifications like: TYPE MY_ARRAY : ARRAY [1..10] OF INT; END_TYPE
                let result = array::try_from(&node.type_name, &array_init.spec, self)?;
                match result {
                    TypeResolution::Type(attributes) => {
                        self.insert_type(&node.type_name, attributes);
                        self.record_declared_array_element(&node.type_name, &array_init.spec);
                    }
                    TypeResolution::Alias(base_type_name) => {
                        self.insert_alias(&node.type_name, &base_type_name)?;
                    }
                }
            }
            InitialValueAssignmentKind::Reference(_) => {
                // Reference types are not resolved through simple declarations
            }
            InitialValueAssignmentKind::LateResolvedType(_type_name) => {
                return Err(Diagnostic::internal_error());
            }
            InitialValueAssignmentKind::SimpleExpr(_) => {
                // Constant-expression initializers are a VAR-declaration
                // extension; they never appear in a TYPE alias's
                // spec_and_init (that grammar path is unchanged).
                return Err(Diagnostic::internal_error());
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
            SpecificationKind::Named(base_type_name) => {
                // Alias of another enumeration: base must already exist because we sort the items
                let Some(base) = self.get(base_type_name) else {
                    return Err(Diagnostic::problem(
                        Problem::ParentEnumNotDeclared,
                        Label::span(node.type_name.span(), "Enumeration"),
                    )
                    .with_secondary(Label::span(base_type_name.span(), "Base type name")));
                };
                let default = node.spec_init.default.as_ref().map(|value| &value.value);
                let attributes = enumeration::alias_of(base, default);
                self.insert_type(&node.type_name, attributes);
            }
            SpecificationKind::Inline(spec_values) => {
                let attributes = enumeration::try_from_values(
                    spec_values,
                    node.spec_init.default.as_ref().map(|value| &value.value),
                    node.spec_init.underlying_type.as_ref(),
                )?;
                self.insert_type(&node.type_name, attributes);
            }
        }

        Ok(node)
    }

    fn fold_string_declaration(
        &mut self,
        node: StringDeclaration,
    ) -> Result<StringDeclaration, Diagnostic> {
        self.insert_type(&node.type_name, string::from_decl(&node));
        Ok(node)
    }

    fn fold_structure_declaration(
        &mut self,
        node: StructureDeclaration,
    ) -> Result<StructureDeclaration, Diagnostic> {
        // Use the structure processing module to create the structure type
        let attrs = crate::intermediates::structure::try_from(&node, self)?;
        self.insert_type(&node.type_name, attrs);
        Ok(node)
    }

    fn fold_subrange_declaration(
        &mut self,
        node: SubrangeDeclaration,
    ) -> Result<SubrangeDeclaration, Diagnostic> {
        let result = subrange::try_from(&node.type_name, &node.spec, self)?;

        match result {
            TypeResolution::Type(attributes) => {
                self.insert_type(&node.type_name, attributes);
            }
            TypeResolution::Alias(base_type_name) => {
                self.insert_alias(&node.type_name, &base_type_name)?;
            }
        }

        Ok(node)
    }

    fn fold_array_declaration(
        &mut self,
        node: ArrayDeclaration,
    ) -> Result<ArrayDeclaration, Diagnostic> {
        // Use the array processing module to create the array type
        let result = array::try_from(&node.type_name, &node.spec, self)?;

        match result {
            TypeResolution::Type(attributes) => {
                self.insert_type(&node.type_name, attributes);
                self.record_declared_array_element(&node.type_name, &node.spec);
            }
            TypeResolution::Alias(base_type_name) => {
                self.insert_alias(&node.type_name, &base_type_name)?;
            }
        }

        Ok(node)
    }

    fn fold_reference_declaration(
        &mut self,
        node: ReferenceDeclaration,
    ) -> Result<ReferenceDeclaration, Diagnostic> {
        let target_type = self.resolve_reference_target(&node.type_name, &node.target)?;

        let attrs = crate::type_attributes::TypeAttributes::new(SemanticType::Reference {
            target_type: Box::new(target_type),
        });
        self.insert_type(&node.type_name, attrs);
        Ok(node)
    }

    fn fold_function_block_declaration(
        &mut self,
        node: FunctionBlockDeclaration,
    ) -> Result<FunctionBlockDeclaration, Diagnostic> {
        // Register the user-defined function block in the type environment
        // so that variable declarations like `myFb : MY_FB` resolve to
        // SemanticType::FunctionBlock, just like stdlib FBs. Its fields are
        // its variables, whose types are not resolved yet:
        // `xform_resolve_decl_types` records them in the environment once
        // they are, never in the block's type.
        let attrs = crate::type_attributes::TypeAttributes::new(SemanticType::FunctionBlock {
            name: node.name.name.to_string(),
        });
        self.insert_type(&node.name, attrs);
        if let Some(oop) = &node.oop {
            let supertypes = oop.base.iter().chain(&oop.implements).cloned().collect();
            self.insert_supertypes(&node.name, supertypes);
        }

        Ok(node)
    }

    fn fold_interface_declaration(
        &mut self,
        node: InterfaceDeclaration,
    ) -> Result<InterfaceDeclaration, Diagnostic> {
        // Register the interface name as a known type so that variables
        // declared with an interface type (e.g. `comm : I_Comm;`) resolve.
        let attrs = crate::type_attributes::TypeAttributes::new(SemanticType::Interface {
            name: node.name.to_string(),
        });
        self.insert_supertypes(&TypeName::from_id(&node.name), node.extends.clone());
        self.insert_type(&TypeName::from_id(&node.name), attrs);
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
            DataTypeDeclarationKind::LateBound(ref lb) => {
                // For structure aliases, we need to insert the alias directly here
                // because StructureInitializationDeclaration doesn't have a field
                // for the new type name being declared.
                let existing = self.get(&lb.base_type_name);
                if let Some(existing) = existing {
                    if matches!(existing.representation, SemanticType::Structure { .. }) {
                        // Insert the alias directly and return the transformed declaration
                        self.insert_alias(&lb.data_type_name, &lb.base_type_name)?;
                        return Ok(DataTypeDeclarationKind::StructureInitialization(
                            StructureInitializationDeclaration {
                                type_name: lb.base_type_name.clone(),
                                elements_init: vec![],
                            },
                        ));
                    }
                }
                // For non-structure types, use the existing transform mechanism
                let result = self.transform_late_bound_declaration(lb.clone())?;
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
            LibraryElementKind::FunctionBlockDeclaration(fb) => {
                let fb = self.fold_function_block_declaration(fb)?;
                Ok(LibraryElementKind::FunctionBlockDeclaration(fb))
            }
            _ => node.recurse_fold(self),
        }
    }
}

#[cfg(test)]
mod tests;
