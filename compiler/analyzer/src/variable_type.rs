//! Resolves a variable reference to the type of the variable it names.
//!
//! A rule that checks something about a variable's type needs two things: the
//! declarations that are in scope, and a way to walk from a reference such as
//! `s.field[i]` to the type of the element it names. Both live here so that
//! rules share one answer rather than each carrying its own copy.
//!
//! ```ignore
//! // In the visitor: open a scope per POU, record each declaration in it.
//! fn enter_scope(&mut self, _: ScopeNode<'_>) { self.declarations.enter() }
//! fn exit_scope(&mut self) { self.declarations.exit() }
//! fn visit_var_decl(&mut self, node: &VarDecl) {
//!     self.declarations
//!         .add_if(node.identifier.symbolic_id(), Declared(node.initializer.clone()));
//! }
//!
//! let element = variable_type::of(&kind, &self.declarations, type_environment);
//! ```

use ironplc_dsl::{common::*, core::Id, textual::*};

use crate::{
    intermediate_type::IntermediateType,
    scoped_table::{ScopedTable, Value},
    type_environment::TypeEnvironment,
};

/// A variable's declared type, as spelled at its declaration site.
#[derive(Debug)]
pub(crate) struct Declared(pub(crate) InitialValueAssignmentKind);
impl Value for Declared {}

/// The declared type of every variable in scope.
///
/// A POU's own declarations shadow outer ones while still resolving the names
/// it does not declare itself. The base scope -- the one
/// [`ScopedTable::new`] opens -- is where declarations made outside any POU
/// land, a `CONFIGURATION`'s `VAR_GLOBAL` block most importantly, so a POU
/// body sees the globals.
pub(crate) type Declarations<'a> = ScopedTable<'a, Id, Declared>;

/// Resolves the [`IntermediateType`] a declaration denotes.
pub(crate) fn resolve_initializer(
    init: &InitialValueAssignmentKind,
    type_env: &TypeEnvironment,
) -> Option<IntermediateType> {
    match init {
        InitialValueAssignmentKind::Simple(si) => {
            Some(type_env.get(&si.type_name)?.representation.clone())
        }
        InitialValueAssignmentKind::LateResolvedType(tn) => {
            Some(type_env.get(tn)?.representation.clone())
        }
        InitialValueAssignmentKind::Structure(si) => {
            Some(type_env.get(&si.type_name)?.representation.clone())
        }
        InitialValueAssignmentKind::Array(ai) => match &ai.spec {
            SpecificationKind::Named(tn) => Some(type_env.get(tn)?.representation.clone()),
            SpecificationKind::Inline(subranges) => {
                let element_type = type_env
                    .get(&subranges.type_name.to_type_name())?
                    .representation
                    .clone();
                Some(IntermediateType::Array {
                    element_type: Box::new(element_type),
                    dimensions: vec![],
                })
            }
        },
        _ => None,
    }
}

/// Resolves the [`IntermediateType`] of the variable a reference names,
/// walking through struct field accesses and array subscripts to the element
/// it selects.
///
/// A bit or partial access answers with the type of the variable it accesses,
/// **not** with what the selection denotes: `x.3` answers `x`'s type rather
/// than `BOOL`, and `w.B1` answers `w`'s type rather than a byte. That is the
/// question an index check asks -- the variable's width is what bounds the
/// index -- and it is the wrong question for a caller asking what type a
/// value read from or written to the reference has.
pub(crate) fn of(
    kind: &SymbolicVariableKind,
    declarations: &Declarations,
    type_env: &TypeEnvironment,
) -> Option<IntermediateType> {
    match kind {
        SymbolicVariableKind::Named(named) => {
            resolve_initializer(&declarations.find(&named.name)?.0, type_env)
        }
        SymbolicVariableKind::Structured(structured) => {
            let record_type = of(&structured.record, declarations, type_env)?;
            struct_field_type(&record_type, &structured.field)
        }
        SymbolicVariableKind::Array(array) => {
            let array_type = of(&array.subscripted_variable, declarations, type_env)?;
            match array_type {
                IntermediateType::Array { element_type, .. } => Some(*element_type),
                _ => None,
            }
        }
        // A selection answers with the variable it selects from; see the
        // note on this function.
        SymbolicVariableKind::BitAccess(bit_access) => {
            of(&bit_access.variable, declarations, type_env)
        }
        SymbolicVariableKind::PartialAccess(partial) => {
            of(&partial.variable, declarations, type_env)
        }
        SymbolicVariableKind::SelfRef(_) => {
            // Typing a member of THIS^/SUPER^ needs function-block member
            // resolution, which does not exist yet. See issue #1406.
            None
        }
        SymbolicVariableKind::Deref(deref) => of(&deref.variable, declarations, type_env),
    }
}

/// Finds the type of a field within a structure or function block type.
pub(crate) fn struct_field_type(
    parent_type: &IntermediateType,
    field_name: &Id,
) -> Option<IntermediateType> {
    let fields = match parent_type {
        IntermediateType::Structure { fields } => fields,
        IntermediateType::FunctionBlock { fields, .. } => fields,
        _ => return None,
    };
    fields
        .iter()
        .find(|f| f.name == *field_name)
        .map(|f| f.field_type.clone())
}
