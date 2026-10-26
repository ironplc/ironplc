//! Each body's prologue from the lowered program (REQ-LOW-codegen-202): for
//! each declaration that starts again on every call, in declaration order,
//! then a function's or method's result, the setup of the storage this
//! backend gives it on every call, then the statements of the lowered
//! prologue that store its starting value.
//!
//! The setup is a string's header, which empties the string, and a
//! structure result's data-region offset and the headers of the strings it
//! holds. A program's `VAR_TEMP` that is an aggregate keeps the setup `init`
//! gives it. A structure, an array or an instance of a function's or
//! method's own has no storage of its own in this layout, so its slot is
//! cleared instead ([`unstored`]).
//!
//! The prologue runs over whatever the last call left, so every store is
//! emitted, whatever its value, but an empty string's into a whole string
//! variable, which its setup has just emptied.
//!
//! A starting value lowering does not express yet, or a store this backend
//! does not emit yet, is not supported yet (P9999).

use ironplc_ir::lowered::{Declaration, Program};
use ironplc_ir::lowered::{
    Place, PouId, Projection, Section, Stmt, StmtKind, StrExprKind, TypeId, TypeKind,
};
use ironplc_ir::{Diagnostic, Label};

use crate::access::{Named, Stored};
use crate::compile::CompileContext;
use crate::emit::Emitter;
use crate::layout::declarations::named;

/// Whether the current body has a prologue: a program or a function block
/// has one only when it declares a `VAR_TEMP`.
pub(crate) fn restarts(ctx: &CompileContext) -> Result<bool, Diagnostic> {
    let pou = super::current_pou(ctx).ok_or_else(Diagnostic::internal_error)?;
    Ok(!restarting(ctx.lowering()?, pou)
        .ok_or_else(Diagnostic::internal_error)?
        .is_empty())
}

/// Emits the prologue of the current body, from the lowered program: its
/// declarations that start again on every call and, for a function or a
/// method that returns a value, its result.
pub(crate) fn emit_prologue(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
) -> Result<(), Diagnostic> {
    let lowering = ctx.lowering()?;
    emit(emitter, ctx, lowering)
}

/// The variables of `pou` that start again on every call, in declaration
/// order: a function's or a method's `VAR`, `VAR_TEMP` and `VAR_OUTPUT`,
/// and a program's or a function block's `VAR_TEMP`.
fn restarting(lowering: &Program, pou: PouId) -> Option<Vec<Named>> {
    let called = lowering.pous().function(pou).is_some();
    let variables = lowering.variables();
    let mut restarting = Vec::new();
    for declaration in lowering.declarations(pou)? {
        let Declaration::Variable(var) = declaration else {
            continue;
        };
        let restarts = match variables.get(*var)?.section() {
            Section::Temporary => true,
            Section::Local | Section::Output => called,
            Section::Global
            | Section::Input
            | Section::InOut
            | Section::Instance
            | Section::Internal => false,
        };
        if restarts {
            restarting.push(named(lowering, *var)?);
        }
    }
    Some(restarting)
}

/// A declaration of the prologue, and the statements that store its
/// starting value.
struct Declared<'l> {
    name: Named,
    /// Whether it is a function's or method's result.
    result: bool,
    /// The type of what it declares, as its stores reach it; `None` when
    /// nothing is stored into it.
    ty: Option<TypeId>,
    /// Whether it is a structure, an array or an instance this backend's
    /// layout gives no storage of its own in the body ([`unstored`]), whose
    /// slot is cleared instead of its fields or elements stored.
    cleared: bool,
    stores: Vec<&'l Stmt>,
}

fn emit(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Program,
) -> Result<(), Diagnostic> {
    for declared in planned(lowering, ctx)? {
        set_up(emitter, ctx, lowering, &declared)?;
        let stores: Vec<Stmt> = declared.stores.into_iter().cloned().collect();
        super::stmt::emit_stmts(emitter, ctx, lowering, &stores)?;
    }
    Ok(())
}

/// Sets up the storage of `declared`: a string's header, a structure
/// result's offset and string headers, and the slot of an aggregate this
/// layout keeps no storage for, cleared.
fn set_up(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Program,
    declared: &Declared,
) -> Result<(), Diagnostic> {
    if declared.cleared {
        let var_index = ctx.var_index(&declared.name)?;
        let zero = ctx.add_i32_constant(0);
        emitter.emit_load_const_i32(zero);
        emitter.emit_store_var_i32(var_index);
        return Ok(());
    }
    let Some(ty) = declared.ty else {
        return Ok(());
    };
    match lowering.types().kind(ty) {
        Some(TypeKind::String(_)) => {
            if let Some(info) = ctx.string_var(&declared.name) {
                emitter.emit_str_init(info.data_offset, info.max_length, info.char_width);
            }
        }
        Some(TypeKind::Structure { .. }) if declared.result => {
            if let Some(info) = ctx.struct_var(&declared.name).cloned() {
                crate::compile_struct_init::set_up_struct_variable(
                    emitter,
                    ctx,
                    lowering.types(),
                    &info,
                    &declared.name.span(),
                )?;
            }
        }
        Some(
            TypeKind::Bool
            | TypeKind::Integer { .. }
            | TypeKind::BitString { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Subrange { .. }
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. }
            | TypeKind::Reference { .. },
        )
        | None => {}
    }
    Ok(())
}

/// The declarations of the prologue of the current body, in declaration
/// order, then the result, each with its stores; P9999 at a store this
/// backend does not emit yet.
fn planned<'l>(
    lowering: &'l Program,
    ctx: &CompileContext,
) -> Result<Vec<Declared<'l>>, Diagnostic> {
    let pou = super::current_pou(ctx).ok_or_else(Diagnostic::internal_error)?;
    let statements = lowering
        .prologue(pou)
        .ok_or_else(Diagnostic::internal_error)?;
    let restarting = restarting(lowering, pou).ok_or_else(Diagnostic::internal_error)?;
    let result = match lowering.result(pou) {
        Some(var) => Some(named(lowering, var).ok_or_else(Diagnostic::internal_error)?),
        None => None,
    };
    let mut planned: Vec<Declared> = restarting
        .into_iter()
        .map(|name| (name, false))
        .chain(result.map(|result| (result, true)))
        .map(|(name, result)| Declared {
            name,
            result,
            ty: None,
            cleared: false,
            stores: Vec::new(),
        })
        .collect();
    for stmt in statements {
        let (place, empty) = match stmt.kind() {
            StmtKind::Assign { place, .. } => (place, false),
            StmtKind::AssignStr { place, value } => (
                place,
                matches!(value.kind(), StrExprKind::Literal(chars) if chars.is_empty()),
            ),
            StmtKind::AssignBits { .. }
            | StmtKind::Copy { .. }
            | StmtKind::FbCall { .. }
            | StmtKind::Call { .. }
            | StmtKind::If { .. }
            | StmtKind::Case { .. }
            | StmtKind::For { .. }
            | StmtKind::Loop { .. }
            | StmtKind::Exit(_)
            | StmtKind::Continue(_)
            | StmtKind::Return => return Err(super::unsupported_store(stmt)),
        };
        let unknown = || {
            Diagnostic::internal_error_at(Label::span(
                stmt.span().clone(),
                "A store of the prologue is into no variable the prologue declares",
            ))
        };
        let declared = planned
            .iter_mut()
            .find(|declared| declared.name.stored == Stored::Var(place.root()))
            .ok_or_else(unknown)?;
        let (ty, whole) = declared_type(lowering, place).ok_or_else(unknown)?;
        declared.ty = Some(ty);
        if unstored(lowering, ctx, place, &declared.name) {
            declared.cleared = true;
            continue;
        }
        if !super::can_emit(lowering, ctx, stmt) {
            return Err(super::unsupported_store(stmt));
        }
        // The setup of a whole string variable empties it.
        if !(empty && whole) {
            declared.stores.push(stmt);
        }
    }
    Ok(planned)
}

/// Whether `place`, a field or an element of the variable `name` of the
/// body, is in a structure, an array or an instance this backend's layout
/// keeps no storage of its own for inside a function, function block or
/// method body (REQ-LOW-codegen-165): the variable's slot holds what another
/// stores in it, so the prologue clears it rather than storing into what it
/// does not have.
fn unstored(lowering: &Program, ctx: &CompileContext, place: &Place, name: &Named) -> bool {
    if !matches!(
        place.path(),
        [Projection::Field(_) | Projection::Index(_), ..]
    ) {
        return false;
    }
    let Some(root) = lowering.variables().get(place.root()) else {
        return false;
    };
    let aggregate = matches!(
        lowering.types().kind(root.ty()),
        Some(TypeKind::Array { .. } | TypeKind::Structure { .. } | TypeKind::FunctionBlock { .. })
    );
    aggregate
        && ctx.struct_var(name).is_none()
        && ctx.array_var(name).is_none()
        && ctx.struct_array_var(name).is_none()
}

/// The type of the variable `place` stores into, through the reference that
/// holds it for an output or a result passed by reference, and whether
/// `place` is that whole variable.
fn declared_type(lowering: &Program, place: &Place) -> Option<(TypeId, bool)> {
    let ty = lowering.variables().get(place.root())?.ty();
    match place.path() {
        [] => Some((ty, true)),
        [Projection::Deref, rest @ ..] => {
            let (target, _) = lowering.types().reference(ty)?;
            Some((target, rest.is_empty()))
        }
        [Projection::Field(_) | Projection::Index(_), ..] => Some((ty, false)),
    }
}
