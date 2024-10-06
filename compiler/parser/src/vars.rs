//! Represents all the variations on variables as a single type so that
//! they can be the return type from the parser.
//!
//! This representation is only relevant in the parser where we need to
//! have lists of all variable types.
//!
use dsl::{
    common::*,
    configuration::AccessDeclaration,
    core::{Id, SourceSpan},
};

/// Defines VarDecl type without the type information (e.g. input, output).
/// Useful only as an intermediate step in the parser where we do not know
/// the specific type.
pub struct UntypedVarDecl {
    pub name: Id,
    pub initializer: InitialValueAssignmentKind,
}

#[derive(Clone)]
pub struct IncomplVarDecl {
    pub name: Id,
    pub qualifier: DeclarationQualifier,
    pub loc: AddressAssignment,
    pub spec: VariableSpecificationKind,
}

impl From<IncomplVarDecl> for VarDecl {
    fn from(val: IncomplVarDecl) -> Self {
        let init = match val.spec {
            VariableSpecificationKind::Simple(node) => {
                InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: node,
                    initial_value: None,
                })
            }
            VariableSpecificationKind::Subrange(node) => InitialValueAssignmentKind::Subrange(node),
            VariableSpecificationKind::Enumerated(node) => match node {
                EnumeratedSpecificationKind::TypeName(ty) => {
                    InitialValueAssignmentKind::EnumeratedType(EnumeratedInitialValueAssignment {
                        type_name: ty,
                        initial_value: None,
                    })
                }
                EnumeratedSpecificationKind::Values(values) => {
                    InitialValueAssignmentKind::EnumeratedValues(EnumeratedValuesInitializer {
                        values: values.values,
                        initial_value: None,
                    })
                }
            },
            VariableSpecificationKind::Array(node) => {
                InitialValueAssignmentKind::Array(ArrayInitialValueAssignment {
                    spec: node,
                    initial_values: vec![],
                })
            }
            // TODO initialize the variables
            VariableSpecificationKind::Struct(node) => {
                InitialValueAssignmentKind::Structure(StructureInitializationDeclaration {
                    type_name: node.type_name,
                    elements_init: vec![],
                })
            }
            VariableSpecificationKind::String(node) => {
                InitialValueAssignmentKind::String(StringInitializer {
                    length: node.length,
                    width: StringType::String,
                    initial_value: None,
                    keyword_span: node.keyword_span,
                })
            }
            VariableSpecificationKind::WString(node) => {
                InitialValueAssignmentKind::String(StringInitializer {
                    length: node.length,
                    width: StringType::WString,
                    initial_value: None,
                    keyword_span: node.keyword_span,
                })
            }
            VariableSpecificationKind::Ambiguous(node) => {
                InitialValueAssignmentKind::LateResolvedType(node)
            }
        };

        Self {
            identifier: VariableIdentifier::Direct(DirectVariableIdentifier {
                name: Some(val.name),
                address_assignment: val.loc,
                span: SourceSpan::default(),
            }),
            var_type: VariableType::Var,
            qualifier: val.qualifier,
            initializer: init,
        }
    }
}

impl UntypedVarDecl {
    pub fn into_var_decl(self, var_type: VariableType) -> VarDecl {
        VarDecl {
            identifier: VariableIdentifier::Symbol(self.name),
            var_type,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: self.initializer,
        }
    }
}

// Container for IO variable declarations.
//
// This is internal for the parser to help with retaining context (input,
// output, etc). In effect, the parser needs a container because we don't
// know where to put the items until much later. It is even more problematic
// because we need to return a common type but that type is not needed
// outside of the parser.
pub enum VarDeclarations {
    // input_declarations
    Inputs(Vec<VarDecl>),
    // output_declarations
    Outputs(Vec<VarDecl>),
    // input_output_declarations
    Inouts(Vec<VarDecl>),
    // located_var_declarations
    Located(Vec<VarDecl>),
    // var_declarations
    Var(Vec<VarDecl>),
    // external_declarations
    External(Vec<VarDecl>),
    // TODO
    // Retentive(Vec<VarDecl>),
    // NonRetentive(Vec<VarDecl>),
    Incomplete(Vec<IncomplVarDecl>),
    ProgramAccess(Vec<ProgramAccessDecl>),
    ConfigAccess(Vec<AccessDeclaration>),
    Edge(Vec<EdgeVarDecl>),
}

impl VarDeclarations {
    pub fn flatten(nested: Vec<Vec<VarDeclarations>>) -> Vec<VarDeclarations> {
        nested.into_iter().flatten().collect()
    }

    // Given multiple sets of declarations, unzip them into types of
    // declarations.
    pub fn drain_var_decl(mut decls: Vec<VarDeclarations>) -> (Vec<VarDecl>, Vec<VarDeclarations>) {
        let mut vars = Vec::new();
        let mut remainder = Vec::new();

        for decl in decls.drain(..) {
            match decl {
                VarDeclarations::Inputs(mut i) => {
                    vars.append(&mut i);
                }
                VarDeclarations::Outputs(mut o) => {
                    vars.append(&mut o);
                }
                VarDeclarations::Inouts(mut inouts) => {
                    vars.append(&mut inouts);
                }
                VarDeclarations::Located(mut l) => {
                    vars.append(&mut l);
                }
                VarDeclarations::Var(mut v) => {
                    vars.append(&mut v);
                }
                VarDeclarations::External(mut v) => {
                    vars.append(&mut v);
                }
                VarDeclarations::Incomplete(v) => {
                    vars.append(&mut v.into_iter().map(|var| var.into()).collect());
                }
                VarDeclarations::ProgramAccess(v) => {
                    remainder.push(VarDeclarations::ProgramAccess(v));
                }
                VarDeclarations::ConfigAccess(v) => {
                    remainder.push(VarDeclarations::ConfigAccess(v));
                }
                VarDeclarations::Edge(v) => {
                    remainder.push(VarDeclarations::Edge(v));
                }
            }
        }

        (vars, remainder)
    }

    pub fn drain_access(
        mut decls: Vec<VarDeclarations>,
    ) -> (Vec<ProgramAccessDecl>, Vec<VarDeclarations>) {
        let mut vars = Vec::new();
        let mut remainder = Vec::new();

        for decl in decls.drain(..) {
            match decl {
                VarDeclarations::Inputs(i) => {
                    remainder.push(VarDeclarations::Inputs(i));
                }
                VarDeclarations::Outputs(o) => {
                    remainder.push(VarDeclarations::Outputs(o));
                }
                VarDeclarations::Inouts(inouts) => {
                    remainder.push(VarDeclarations::Inouts(inouts));
                }
                VarDeclarations::Located(l) => {
                    remainder.push(VarDeclarations::Located(l));
                }
                VarDeclarations::Var(v) => {
                    remainder.push(VarDeclarations::Var(v));
                }
                VarDeclarations::External(v) => {
                    remainder.push(VarDeclarations::External(v));
                }
                VarDeclarations::Incomplete(v) => {
                    remainder.push(VarDeclarations::Incomplete(v));
                }
                VarDeclarations::ProgramAccess(mut v) => {
                    vars.append(&mut v);
                }
                VarDeclarations::ConfigAccess(v) => {
                    remainder.push(VarDeclarations::ConfigAccess(v));
                }
                VarDeclarations::Edge(v) => {
                    remainder.push(VarDeclarations::Edge(v));
                }
            }
        }

        (vars, remainder)
    }

    pub fn drain_edge_decl(
        mut decls: Vec<VarDeclarations>,
    ) -> (Vec<EdgeVarDecl>, Vec<VarDeclarations>) {
        let mut vars = Vec::new();
        let mut remainder = Vec::new();

        for decl in decls.drain(..) {
            match decl {
                VarDeclarations::Inputs(i) => {
                    remainder.push(VarDeclarations::Inputs(i));
                }
                VarDeclarations::Outputs(o) => {
                    remainder.push(VarDeclarations::Outputs(o));
                }
                VarDeclarations::Inouts(inouts) => {
                    remainder.push(VarDeclarations::Inouts(inouts));
                }
                VarDeclarations::Located(l) => {
                    remainder.push(VarDeclarations::Located(l));
                }
                VarDeclarations::Var(v) => {
                    remainder.push(VarDeclarations::Var(v));
                }
                VarDeclarations::External(v) => {
                    remainder.push(VarDeclarations::External(v));
                }
                VarDeclarations::Incomplete(v) => {
                    remainder.push(VarDeclarations::Incomplete(v));
                }
                VarDeclarations::ProgramAccess(v) => {
                    remainder.push(VarDeclarations::ProgramAccess(v));
                }
                VarDeclarations::ConfigAccess(v) => {
                    remainder.push(VarDeclarations::ConfigAccess(v));
                }
                VarDeclarations::Edge(mut v) => {
                    vars.append(&mut v);
                }
            }
        }

        (vars, remainder)
    }

    pub fn with(
        mut decls: Vec<VarDeclarations>,
        qualifier: DeclarationQualifier,
    ) -> Vec<VarDeclarations> {
        let mut updated_decls = vec![];

        for decl in decls.drain(..) {
            match decl {
                VarDeclarations::Inputs(i) => {
                    updated_decls
                        .push(VarDeclarations::Inputs(VarDeclarations::map(i, &qualifier)));
                }
                VarDeclarations::Outputs(o) => {
                    updated_decls.push(VarDeclarations::Outputs(VarDeclarations::map(
                        o, &qualifier,
                    )));
                }
                VarDeclarations::Inouts(inouts) => {
                    updated_decls.push(VarDeclarations::Inouts(VarDeclarations::map(
                        inouts, &qualifier,
                    )));
                }
                VarDeclarations::Located(l) => {
                    updated_decls.push(VarDeclarations::Located(VarDeclarations::map(
                        l, &qualifier,
                    )));
                }
                VarDeclarations::Var(v) => {
                    updated_decls.push(VarDeclarations::Var(VarDeclarations::map(v, &qualifier)));
                }
                VarDeclarations::External(v) => {
                    updated_decls.push(VarDeclarations::External(VarDeclarations::map(
                        v, &qualifier,
                    )));
                }
                VarDeclarations::Incomplete(v) => {
                    updated_decls.push(VarDeclarations::Incomplete(
                        VarDeclarations::map_incomplete(v, &qualifier),
                    ));
                }
                VarDeclarations::ProgramAccess(v) => {
                    // Does not change based on the type
                    updated_decls.push(VarDeclarations::ProgramAccess(v));
                }
                VarDeclarations::ConfigAccess(v) => {
                    // Does not change based on the type
                    updated_decls.push(VarDeclarations::ConfigAccess(v));
                }
                VarDeclarations::Edge(v) => {
                    updated_decls.push(VarDeclarations::Edge(VarDeclarations::map_edge(
                        v, &qualifier,
                    )));
                }
            }
        }

        updated_decls
    }

    pub fn map(declarations: Vec<VarDecl>, qualifier: &DeclarationQualifier) -> Vec<VarDecl> {
        declarations
            .into_iter()
            .map(|declaration| declaration.clone().with_qualifier(qualifier.clone()))
            .collect()
    }

    pub fn map_incomplete(
        declarations: Vec<IncomplVarDecl>,
        qualifier: &DeclarationQualifier,
    ) -> Vec<IncomplVarDecl> {
        declarations
            .into_iter()
            .map(|declaration| {
                let mut decl = declaration.clone();
                decl.qualifier = qualifier.clone();
                decl
            })
            .collect()
    }

    pub fn map_edge(
        declarations: Vec<EdgeVarDecl>,
        qualifier: &DeclarationQualifier,
    ) -> Vec<EdgeVarDecl> {
        declarations
            .into_iter()
            .map(|declaration| {
                let mut decl = declaration.clone();
                decl.qualifier = qualifier.clone();
                decl
            })
            .collect()
    }

    pub fn flat_map(
        declarations: Vec<Vec<UntypedVarDecl>>,
        var_type: VariableType,
        qualifier: Option<DeclarationQualifier>,
    ) -> Vec<VarDecl> {
        let declarations = declarations.into_iter().flatten();

        declarations
            .into_iter()
            .map(|declaration| {
                let qualifier = qualifier
                    .clone()
                    .unwrap_or(DeclarationQualifier::Unspecified);

                VarDecl {
                    identifier: VariableIdentifier::Symbol(declaration.name),
                    var_type: var_type.clone(),
                    qualifier,
                    initializer: declaration.initializer,
                }
            })
            .collect()
    }
}
