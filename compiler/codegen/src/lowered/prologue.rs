//! Each body's prologue from the lowered program (REQ-LOW-codegen-202): for
//! each declaration that starts again on every call, in declaration order,
//! then a function's or method's result, the setup of the storage this
//! backend gives it on every call, then the statements of the lowered
//! prologue that store its starting value.
//!
//! The setup is the AST's prologue's: a string's header, which empties the
//! string, and a structure result's data-region offset and the headers of
//! the strings it holds. A program's `VAR_TEMP` that is an aggregate keeps
//! the setup `init` gives it.
//!
//! The prologue runs over whatever the last call left, so every store is
//! emitted, whatever its value, but an empty string's into a whole string
//! variable, which its setup has just emptied.
//!
//! A prologue lowering does not complete, or one with a statement this
//! backend does not emit yet, is left to the AST, whole: a function's or
//! method's prologue then comes from its declarations, and a program's
//! `VAR_TEMP` takes its value once, in `init`.

use ironplc_dsl::common::VarDecl;
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_ir::lowered::{Place, Projection, Stmt, StmtKind, StrExprKind, TypeId, TypeKind};
use ironplc_lowering::PartialProgram;

use crate::compile::CompileContext;
use crate::emit::Emitter;

/// Whether any of `variables` starts again on every call: a program or a
/// function block has a prologue only when it declares a `VAR_TEMP`.
pub(crate) fn restarts(variables: &[VarDecl]) -> bool {
    variables.iter().any(|decl| decl.reset_on_call)
}

/// Emits the prologue of the current body, which declares `variables` and,
/// for a function or a method that returns a value, the result `result`,
/// from the lowered program, when it emits every store; returns `false`,
/// having emitted nothing, when the prologue is left to the AST.
pub(crate) fn try_emit_prologue(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    variables: &[VarDecl],
    result: Option<&Id>,
) -> Result<bool, Diagnostic> {
    let Some(route) = ctx.lowered_route.take() else {
        return Ok(false);
    };
    let emitted = emit(emitter, ctx, route.lowering(), variables, result);
    ctx.lowered_route = Some(route);
    emitted
}

/// Whether the prologue of the current body, which declares `variables`,
/// is emitted from the lowered program, as [`try_emit_prologue`] emits it.
pub(crate) fn emits(
    lowering: &PartialProgram,
    ctx: &CompileContext,
    variables: &[VarDecl],
) -> bool {
    planned(lowering, ctx, variables, None).is_some()
}

/// A declaration of the prologue, and the statements that store its
/// starting value.
struct Declared<'l> {
    name: Id,
    span: SourceSpan,
    /// Whether it is a function's or method's result.
    result: bool,
    /// The type of what it declares, as its stores reach it; `None` when
    /// nothing is stored into it.
    ty: Option<TypeId>,
    stores: Vec<&'l Stmt>,
}

fn emit(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &PartialProgram,
    variables: &[VarDecl],
    result: Option<&Id>,
) -> Result<bool, Diagnostic> {
    let Some(planned) = planned(lowering, ctx, variables, result) else {
        return Ok(false);
    };
    for declared in planned {
        set_up(emitter, ctx, lowering, &declared)?;
        let stores: Vec<Stmt> = declared.stores.into_iter().cloned().collect();
        super::stmt::emit_stmts(emitter, ctx, lowering, &stores)?;
    }
    Ok(true)
}

/// Sets up the storage of `declared` as the AST's prologue does: a string's
/// header, and a structure result's offset and string headers.
fn set_up(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &PartialProgram,
    declared: &Declared,
) -> Result<(), Diagnostic> {
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
                    &info,
                    &declared.span,
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

/// The declarations of the prologue of the current body, in the order the
/// AST's prologue visits them, each with its stores; `None` when the
/// prologue does not lower, holds a statement this backend does not emit,
/// or stores into a variable that is not one of them.
fn planned<'l>(
    lowering: &'l PartialProgram,
    ctx: &CompileContext,
    variables: &[VarDecl],
    result: Option<&Id>,
) -> Option<Vec<Declared<'l>>> {
    let statements = lowering.prologue(super::current_pou(ctx)?)?;
    let mut planned: Vec<Declared> = variables
        .iter()
        .filter(|decl| decl.reset_on_call)
        .filter_map(|decl| {
            let name = decl.identifier.symbolic_id()?;
            Some((name.clone(), decl.identifier.span(), false))
        })
        .chain(result.map(|result| (result.clone(), result.span(), true)))
        .map(|(name, span, result)| Declared {
            name,
            span,
            result,
            ty: None,
            stores: Vec::new(),
        })
        .collect();
    for stmt in statements {
        if !super::can_emit(lowering, ctx, stmt) {
            return None;
        }
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
            | StmtKind::Return => return None,
        };
        let (scope, name) = ctx.lowered_ids.variable(place.root())?;
        if *scope != ctx.scope {
            return None;
        }
        let declared = planned.iter_mut().find(|declared| declared.name == *name)?;
        let (ty, whole) = declared_type(lowering, place)?;
        declared.ty = Some(ty);
        // The setup of a whole string variable empties it.
        if !(empty && whole) {
            declared.stores.push(stmt);
        }
    }
    Some(planned)
}

/// The type of the variable `place` stores into, through the reference that
/// holds it for an output or a result passed by reference, and whether
/// `place` is that whole variable.
fn declared_type(lowering: &PartialProgram, place: &Place) -> Option<(TypeId, bool)> {
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
