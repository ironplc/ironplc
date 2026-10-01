//! Resolves a variable reference to the type of the variable it names.
//!
//! A rule that checks something about a variable's type needs two things: the
//! variable's declared type, and a way to walk from a reference such as
//! `s.field[i]` to the type of the element it names. The declared type comes
//! from the symbol environment, looked up from the scope the rule is in; the
//! walk lives here so that rules share one answer rather than each carrying
//! its own copy.
//!
//! ```ignore
//! // In the visitor: track the scope the traversal is in.
//! fn enter_scope(&mut self, node: ScopeNode<'_>) { self.scope.enter(&node) }
//! fn exit_scope(&mut self) { self.scope.exit() }
//!
//! let element = variable_type::of(&kind, context, &self.scope.current());
//! ```
//!
//! [`Declarations`] is the older scoped table a pass builds itself from the
//! declarations it visits, still used by `xform_resolve_expr_types`.

use ironplc_dsl::{common::*, core::Id, textual::*};

use crate::{
    intermediate_type::IntermediateType,
    scoped_table::{ScopedTable, Value},
    semantic_context::SemanticContext,
    symbol_environment::ScopeKind,
    type_environment::TypeEnvironment,
};
use ironplc_dsl::type_id::TypeId;

/// A variable's declared type, as spelled where the name is bound.
#[derive(Debug)]
pub(crate) enum Declared {
    /// A variable declaration: its initializer as written, and the id of
    /// the type it declares once the analyzer has resolved it.
    Variable {
        init: Box<InitialValueAssignmentKind>,
        type_id: Option<TypeId>,
    },
    /// A name bound to a type without a declaration of its own: a
    /// function's or method's result variable, or an implicit system
    /// global.
    Typed(TypeName),
}
impl Value for Declared {}

impl Declared {
    /// The id of the declared type, when the analyzer has resolved one.
    pub(crate) fn type_id(&self, type_env: &TypeEnvironment) -> Option<TypeId> {
        match self {
            Declared::Variable { type_id, .. } => *type_id,
            Declared::Typed(type_name) => type_env.id_of(type_name),
        }
    }

    /// The declaration of `node`.
    pub(crate) fn of(node: &VarDecl) -> Self {
        Declared::Variable {
            init: Box::new(node.initializer.clone()),
            type_id: node.type_id,
        }
    }
}

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
        InitialValueAssignmentKind::LateResolvedType(LateResolvedInitializer {
            type_name: tn,
            ..
        }) => Some(type_env.get(tn)?.representation.clone()),
        InitialValueAssignmentKind::Structure(si) => {
            Some(type_env.get(&si.type_name)?.representation.clone())
        }
        InitialValueAssignmentKind::FunctionBlock(fbi) => {
            Some(type_env.get(&fbi.type_name)?.representation.clone())
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
    context: &SemanticContext,
    scope: &ScopeKind,
) -> Option<IntermediateType> {
    match kind {
        SymbolicVariableKind::Named(named) => declared(&named.name, context, scope).cloned(),
        SymbolicVariableKind::Structured(structured) => {
            let record_type = of(&structured.record, context, scope)?;
            struct_field_type(&record_type, &structured.field)
        }
        SymbolicVariableKind::Array(array) => {
            let array_type = of(&array.subscripted_variable, context, scope)?;
            match array_type {
                IntermediateType::Array { element_type, .. } => Some(*element_type),
                _ => None,
            }
        }
        // A selection answers with the variable it selects from; see the
        // note on this function.
        SymbolicVariableKind::BitAccess(bit_access) => of(&bit_access.variable, context, scope),
        SymbolicVariableKind::PartialAccess(partial) => of(&partial.variable, context, scope),
        SymbolicVariableKind::SelfRef(_) => {
            // Typing a member of THIS^/SUPER^ needs function-block member
            // resolution, which does not exist yet. See issue #1406.
            None
        }
        SymbolicVariableKind::Deref(deref) => of(&deref.variable, context, scope),
    }
}

/// The declared type of the variable `name` names from `scope`: a variable,
/// parameter or result variable, looked up in the symbol environment.
pub(crate) fn declared<'a>(
    name: &Id,
    context: &'a SemanticContext,
    scope: &ScopeKind,
) -> Option<&'a IntermediateType> {
    let type_id = context.symbols().find(name, scope)?.type_id?;
    Some(&context.types().get_by_id(type_id)?.representation)
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
