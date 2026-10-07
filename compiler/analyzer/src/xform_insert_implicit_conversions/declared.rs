//! The declared types of function block fields and method parameters,
//! collected from the library before the pass folds it.
//!
//! A value stored in a field of a function block, or passed to a parameter of
//! a method, is converted to the type the field or parameter is declared
//! with, and an untyped literal takes that type. The analyzer cannot read it
//! from the type of the block: that type lists only the fields declared with
//! a simple type, and a method's parameters not at all. So the declarations
//! are read here, once, before the library is folded.
//!
//! A declared type is the elementary type the value is operated as: its own,
//! or its base type for a subrange. A field or parameter that is not a single
//! numeric value (an enumeration, a string, an aggregate, a reference) has
//! none, and nothing is converted to it.

use std::collections::HashMap;

use ironplc_dsl::common::{Library, LibraryElementKind, VarDecl, VariableType};
use ironplc_dsl::core::Id;
use ironplc_dsl::type_id::TypeId;

use crate::semantic_type::SemanticType;
use crate::type_environment::TypeEnvironment;

/// The input parameters of a method, in declaration order: each parameter's
/// name and its declared type.
pub(super) type Parameters = Vec<(Id, Option<TypeId>)>;

/// What the declarations of the user-defined function blocks in a library
/// say about the types their fields and method parameters are declared with.
#[derive(Default)]
pub(super) struct Declarations {
    /// The declared type of each field, by block and field name.
    fields: HashMap<(Id, Id), Option<TypeId>>,
    /// The names of each block's inputs, in declaration order.
    inputs: HashMap<Id, Vec<Id>>,
    /// The input parameters of each method, by block and method name.
    methods: HashMap<(Id, Id), Parameters>,
}

impl Declarations {
    /// Collects the declarations of every user-defined function block in
    /// `lib`.
    pub(super) fn collect(lib: &Library, types: &TypeEnvironment) -> Self {
        let mut declarations = Declarations::default();
        for element in &lib.elements {
            let LibraryElementKind::FunctionBlockDeclaration(block) = element else {
                continue;
            };
            let name = &block.name.name;
            for decl in &block.variables {
                if let Some(field) = decl.identifier.symbolic_id() {
                    let at = declared_type(types, decl);
                    declarations
                        .fields
                        .insert((name.clone(), field.clone()), at);
                }
            }
            let inputs = block
                .variables
                .iter()
                .filter(|decl| decl.var_type == VariableType::Input)
                .filter_map(|decl| decl.identifier.symbolic_id().cloned())
                .collect();
            declarations.inputs.insert(name.clone(), inputs);
            for method in &block.methods {
                let parameters = method
                    .variables
                    .iter()
                    .filter(|decl| decl.var_type.is_input_compatible())
                    .filter_map(|decl| {
                        let parameter = decl.identifier.symbolic_id()?.clone();
                        Some((parameter, declared_type(types, decl)))
                    })
                    .collect();
                declarations
                    .methods
                    .insert((name.clone(), method.name.clone()), parameters);
            }
        }
        declarations
    }

    /// The declared type of the field `field` of the user-defined function
    /// block `block`: `None` when `block` is not one or has no such field,
    /// `Some(None)` when the field is not a single numeric value.
    pub(super) fn field(&self, block: &Id, field: &Id) -> Option<Option<TypeId>> {
        self.fields.get(&(block.clone(), field.clone())).copied()
    }

    /// The names of the inputs of the user-defined function block `block`,
    /// in declaration order.
    pub(super) fn inputs(&self, block: &Id) -> Option<&[Id]> {
        self.inputs.get(block).map(Vec::as_slice)
    }

    /// The input parameters of the method `method` of the function block
    /// `block`.
    pub(super) fn method(&self, block: &Id, method: &Id) -> Option<&Parameters> {
        self.methods.get(&(block.clone(), method.clone()))
    }
}

/// The type `decl` declares, as the elementary type a value of it is
/// operated as.
fn declared_type(types: &TypeEnvironment, decl: &VarDecl) -> Option<TypeId> {
    let attributes = types.get_by_id(decl.type_id?)?;
    elementary_of(types, &attributes.representation)
}

/// The elementary type a value of `representation` is operated as: its own,
/// or its base type for a subrange.
pub(super) fn elementary_of(
    types: &TypeEnvironment,
    representation: &SemanticType,
) -> Option<TypeId> {
    let representation = match representation {
        SemanticType::Subrange { base_type, .. } => base_type,
        other => other,
    };
    types.id_of(&types.elementary_type_name_for(representation)?)
}
