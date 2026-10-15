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

use ironplc_dsl::{common::*, core::Id, textual::*, type_id::TypeId};

use crate::{
    semantic_context::SemanticContext, semantic_type::SemanticType, symbol_environment::ScopeKind,
    type_environment::TypeEnvironment,
};

/// Resolves the [`SemanticType`] a declaration denotes.
pub(crate) fn resolve_initializer(
    init: &InitialValueAssignmentKind,
    type_env: &TypeEnvironment,
) -> Option<SemanticType> {
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
                Some(SemanticType::Array {
                    element_type: Box::new(element_type),
                    dimensions: vec![],
                })
            }
        },
        _ => None,
    }
}

/// Resolves the [`SemanticType`] of the variable a reference names,
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
) -> Option<SemanticType> {
    match kind {
        SymbolicVariableKind::Named(named) => declared(&named.name, context, scope).cloned(),
        SymbolicVariableKind::Structured(structured) => {
            let record_type = of(&structured.record, context, scope)?;
            struct_field_type(&record_type, &structured.field)
                .or_else(|| recorded_field_type(structured, context, scope))
        }
        SymbolicVariableKind::Array(array) => {
            let array_type = of(&array.subscripted_variable, context, scope)?;
            match array_type {
                SemanticType::Array { element_type, .. } => Some(*element_type),
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
        // `p^` is the variable `p` references, so it has the referenced
        // type, not `REF_TO`.
        SymbolicVariableKind::Deref(deref) => match of(&deref.variable, context, scope)? {
            SemanticType::Reference { target_type } => Some(*target_type),
            _ => None,
        },
    }
}

/// The declared type of the variable `name` names from `scope`: a variable,
/// parameter or result variable, looked up in the symbol environment.
pub(crate) fn declared<'a>(
    name: &Id,
    context: &'a SemanticContext,
    scope: &ScopeKind,
) -> Option<&'a SemanticType> {
    let type_id = context
        .symbols()
        .resolve_variable(name, scope)
        .declared()?
        .type_id?;
    Some(&context.types().get_by_id(type_id)?.representation)
}

/// The type of the field `structured` names, from the field types the type
/// environment records for each declaration (`TypeEnvironment::field_type`).
///
/// A function block's representation lists only the fields whose types were
/// known when the block was entered in the type environment, before a
/// late-bound type resolved: a field declared as a structure, a string or an
/// array is missing from it. The recorded field types hold every field, so
/// `f.p` and `f.s` have the types the block declares them with.
fn recorded_field_type(
    structured: &StructuredVariable,
    context: &SemanticContext,
    scope: &ScopeKind,
) -> Option<SemanticType> {
    let types = context.types();
    let record = type_id_of(&structured.record, context, scope)?;
    let field = types.field_type(record, &structured.field)?;
    Some(types.get_by_id(field)?.representation.clone())
}

/// The id of the type of the variable a reference names, walking through
/// field accesses, array subscripts and dereferences by the type ids the
/// type environment records. `None` for a bit or partial access, and for
/// `THIS^` and `SUPER^`.
fn type_id_of(
    kind: &SymbolicVariableKind,
    context: &SemanticContext,
    scope: &ScopeKind,
) -> Option<TypeId> {
    let types = context.types();
    match kind {
        SymbolicVariableKind::Named(named) => {
            context
                .symbols()
                .resolve_variable(&named.name, scope)
                .declared()?
                .type_id
        }
        SymbolicVariableKind::Structured(structured) => types.field_type(
            type_id_of(&structured.record, context, scope)?,
            &structured.field,
        ),
        SymbolicVariableKind::Array(array) => {
            types.element_type(type_id_of(&array.subscripted_variable, context, scope)?)
        }
        SymbolicVariableKind::Deref(deref) => {
            types.referenced_type(type_id_of(&deref.variable, context, scope)?)
        }
        SymbolicVariableKind::BitAccess(_)
        | SymbolicVariableKind::PartialAccess(_)
        | SymbolicVariableKind::SelfRef(_) => None,
    }
}

/// Finds the type of a field within a structure or function block type.
pub(crate) fn struct_field_type(
    parent_type: &SemanticType,
    field_name: &Id,
) -> Option<SemanticType> {
    let fields = match parent_type {
        SemanticType::Structure { fields } => fields,
        SemanticType::FunctionBlock { fields, .. } => fields,
        _ => return None,
    };
    fields
        .iter()
        .find(|f| f.name == *field_name)
        .map(|f| f.field_type.clone())
}
