//! Transformation that resolves declarations and builds
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
use crate::type_environment::{TypeClass, TypeAttributes, TypeEnvironment};
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
    let mut resolver = TypeEnvironmentResolver {
        env: type_environment,
        scope: None,
    };
    resolver.fold_library(lib).map_err(|err| vec![err])
}

struct TypeEnvironmentResolver<'a> {
    env: &'a mut TypeEnvironment,
    scope: Option<Type>,
}

impl<'a> TypeEnvironmentResolver<'a> {
    fn transform_late_bound_declaration(
        &mut self,
        node: LateBoundDeclaration,
    ) -> Result<DataTypeDeclarationKind, Diagnostic> {
        // At this point we should have a type for the late bound declaration
        // so we can replace the late bound declaration with the correct type
        let existing = self.env.get(&node.base_type_name);
        let existing = existing.unwrap();

        match existing.class {
            TypeClass::Simple => Ok(DataTypeDeclarationKind::Simple(SimpleDeclaration {
                type_name: node.data_type_name,
                spec_and_init: InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: node.base_type_name,
                    initial_value: None,
                }),
            })),
            TypeClass::Enumeration => Ok(DataTypeDeclarationKind::Enumeration(
                EnumerationDeclaration {
                    type_name: node.data_type_name,
                    spec_init: EnumeratedSpecificationInit {
                        spec: EnumeratedSpecificationKind::TypeName(node.base_type_name),
                        default: None,
                    },
                },
            )),
            TypeClass::Structure => Ok(DataTypeDeclarationKind::StructureInitialization(
                StructureInitializationDeclaration {
                    type_name: node.data_type_name,
                    elements_init: vec![],
                },
            )),
            TypeClass::FunctionBlock => Err(Diagnostic::todo_with_span(node.span(), file!(), line!())),
            TypeClass::FunctionBlockOutput(_) => Err(Diagnostic::todo_with_span(node.span(), file!(), line!())),
        }
    }
}

impl<'a> Fold<Diagnostic> for TypeEnvironmentResolver<'a> {
    fn fold_simple_declaration(
        &mut self,
        node: SimpleDeclaration,
    ) -> Result<SimpleDeclaration, Diagnostic> {
        // A simple declaration cannot refer to another type so we
        // just need to insert this into the type environment.
        self.env.insert(
            &node.type_name,
            TypeAttributes {
                span: node.type_name.span(),
                class: TypeClass::Simple,
            },
        )?;
        Ok(node)
    }

    fn fold_enumeration_declaration(
        &mut self,
        node: EnumerationDeclaration,
    ) -> Result<EnumerationDeclaration, Diagnostic> {
        // Enumeration declaration can define a set of values
        // or rename another enumeration.
        if let EnumeratedSpecificationKind::TypeName(base_type_name) = &node.spec_init.spec {
            if self.env.get(base_type_name).is_none() {
                return Err(Diagnostic::problem(
                    Problem::ParentEnumNotDeclared,
                    Label::span(node.type_name.span(), "Enumeration"),
                )
                .with_secondary(Label::span(base_type_name.span(), "Base type name")));
            }
        }

        self.env.insert(
            &node.type_name,
            TypeAttributes {
                span: node.type_name.span(),
                class: TypeClass::Enumeration,
            },
        )?;
        Ok(node)
    }

    fn fold_structure_declaration(
        &mut self,
        node: StructureDeclaration,
    ) -> Result<StructureDeclaration, Diagnostic> {
        self.env.insert(
            &node.type_name,
            TypeAttributes {
                span: node.type_name.span(),
                class: TypeClass::Structure,
            },
        )?;
        Ok(node)
    }

    fn fold_function_block_declaration(&mut self, node:FunctionBlockDeclaration) -> Result<FunctionBlockDeclaration,Diagnostic> {
        self.env.insert(&node.name,
            TypeAttributes {
                span: node.span.clone(),
                class: TypeClass::FunctionBlock,
            },
        )?;

        for var in &node.variables {
            if var.var_type == VariableType::InOut || var.var_type == VariableType::Output {
                // TODO it's impossible to have a direct symbol identifier here, but our types allow it
                // TODO find a way to remove this match statement
                if let VariableIdentifier::Symbol(name) = &var.identifier {
                    if let Some(type_name) = var.type_name() {
                        let mut mangled_name = node.name.name.lower_case.clone();
                        mangled_name.push('.');
                        mangled_name.push_str(name.lower_case.as_str());

                        self.env.insert(
                            &Type::from(mangled_name.as_str()), 
                            TypeAttributes { span: name.span(), class: TypeClass::FunctionBlockOutput(type_name) })?;
                    }
                }
            }
        }

        Ok(node)
    }

    fn fold_data_type_declaration_kind(
        &mut self,
        node: DataTypeDeclarationKind,
    ) -> Result<DataTypeDeclarationKind, Diagnostic> {
        // The only type we care about here is late bound. We want to transform that
        // into a different type and need to return a different data type declaration
        // kind to do that. So, check the type and only handle it here if it is
        // a late bound kind.
        if let DataTypeDeclarationKind::LateBound(lb) = node {
            self.transform_late_bound_declaration(lb)
        } else {
            node.recurse_fold(self)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::type_environment::{TypeClass, TypeEnvironmentBuilder};

    use super::apply;
    use ironplc_dsl::{common::*, core::FileId};
    use ironplc_parser::options::ParseOptions;

    #[test]
    fn apply_when_ambiguous_enum_then_resolves_type() {
        let program = "
FUNCTION_BLOCK CALLEE
VAR_INPUT
	IN :	BYTE;
END_VAR
VAR_OUTPUT
	OUT :	BYTE;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK CALLER
VAR
	CALLEE1 :	CALLEE;
END_VAR
END_FUNCTION_BLOCK
        ";
        let input =
            ironplc_parser::parse_program(program, &FileId::default(), &ParseOptions::default())
                .unwrap();
        let mut type_env = TypeEnvironmentBuilder::new().build().unwrap();
        let _ = apply(input, &mut type_env).unwrap();

        // We should have types for each function block declaration
        assert_eq!(TypeClass::FunctionBlock, type_env.get(&Type::from("CALLEE")).unwrap().class);
        assert_eq!(TypeClass::FunctionBlock, type_env.get(&Type::from("CALLER")).unwrap().class);

        // We should also have types for the outputs from the function block declaration
        if let TypeClass::FunctionBlockOutput(type_name) = &type_env.get(&Type::from("CALLEE.OUT")).unwrap().class {
            assert_eq!("byte", type_name.name.lower_case);
        } else {
            panic!("Invalid type");
        }
    }
}
