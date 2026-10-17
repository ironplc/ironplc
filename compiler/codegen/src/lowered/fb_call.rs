//! Emits a lowered function block call.
//!
//! A call lowers to the stores of its inputs to the instance's fields, an
//! `FbCall`, then the stores of its outputs (REQ-LOW-lowering-069), each
//! with the call's span. The stores of the inputs and the call are emitted as
//! the route that reads the AST emits them (`compile_fb_call.rs`): the
//! instance pushed once, each input stored in its field, then the call and
//! the instance popped. So a call compiles to the same instructions by
//! either route, and the VM copies the instance's fields in and out around
//! the body for every statement of it, as "One field model per body"
//! requires until issue 2120 lands. The outputs are ordinary stores.

use ironplc_dsl::core::{Id, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::{
    Expr, ExprKind, Place as LoweredPlace, PouId, Projection, Stmt, StmtKind,
};
use ironplc_lowering::Lowering;

use super::expr::emit_expr;
use super::place::variable_name;
use super::same_span;
use crate::compile::CompileContext;
use crate::compile_call::standard_fb_layout;
use crate::compile_fb_call::FbCallSite;
use crate::emit::Emitter;

/// The stores of a call's inputs and the call, at the head of a list of
/// lowered statements.
pub(super) struct CallSite<'s> {
    /// The name this backend's layout knows the instance by.
    instance: Id,
    /// Each input's field, by name, and the value stored in it, in order.
    inputs: Vec<(Id, &'s Expr)>,
}

impl CallSite<'_> {
    /// How many statements the call site is: its inputs and the call.
    pub(super) fn len(&self) -> usize {
        self.inputs.len() + 1
    }
}

/// The call site at the head of `stmts`: stores to fields of one instance,
/// then an `FbCall` of that instance, all with the call's span. `None` when
/// `stmts` does not start with one, as when a source assignment to a field
/// comes before a separate call.
pub(super) fn call_site<'s>(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    stmts: &'s [Stmt],
) -> Option<CallSite<'s>> {
    let span = stmts.first()?.span();
    let call_at = stmts.iter().position(|stmt| {
        !matches!(stmt.kind(), StmtKind::Assign { .. }) || !same_span(stmt.span(), span)
    })?;
    let call = &stmts[call_at];
    let StmtKind::FbCall { instance, .. } = call.kind() else {
        return None;
    };
    if !same_span(call.span(), span) {
        return None;
    }
    let inputs = stmts[..call_at]
        .iter()
        .map(|stmt| input(lowering, instance, stmt))
        .collect::<Option<Vec<_>>>()?;
    Some(CallSite {
        instance: variable_name(lowering, ctx, instance)?,
        inputs,
    })
}

/// The call site of `instance` with no inputs: a lone `FbCall`.
pub(super) fn bare_call<'s>(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    instance: &LoweredPlace,
) -> Option<CallSite<'s>> {
    Some(CallSite {
        instance: variable_name(lowering, ctx, instance)?,
        inputs: Vec::new(),
    })
}

/// Emits the call site `call`, whose span is `span`.
pub(super) fn emit_call_site(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    call: &CallSite<'_>,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let site = FbCallSite::open(emitter, ctx, &call.instance, span)?;
    for (field, value) in &call.inputs {
        // `store_input` narrows a value to a field it records the type of,
        // as the AST route does, so the narrowing lowering made
        // (REQ-LOW-lowering-046) is left to it rather than emitted twice.
        let narrowed_by_site = ctx.fb_instances.get(&call.instance).is_some_and(|info| {
            info.field_types
                .contains_key(&field.to_string().to_lowercase())
        });
        let value = match untruncated(value) {
            Some(inner) if narrowed_by_site => inner,
            Some(_) | None => value,
        };
        emit_expr(emitter, ctx, lowering, value)?;
        site.store_input(emitter, field)?;
    }
    site.close(emitter, ctx);
    Ok(())
}

/// Whether this backend calls `block` on `instance`: an instance it reaches
/// by name, of the user-defined block whose body it compiled for `block`, or
/// of the VM's block for the standard block `block` is named after.
///
/// Lowering does not generate the standard blocks yet (issue 2288), so a
/// standard block's POU has no body in the lowered program, and `FB_CALL`
/// runs the VM's block on the instance, as the route that reads the AST
/// calls it.
pub(super) fn reaches_block(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    instance: &LoweredPlace,
    block: PouId,
) -> bool {
    let Some(fb_instance) =
        variable_name(lowering, ctx, instance).and_then(|name| ctx.fb_instances.get(&name))
    else {
        return false;
    };
    let Some(name) = lowering.pous().name(block) else {
        return false;
    };
    let name = name.to_string().to_uppercase();
    let type_id = match ctx.user_fb_types.get(&name) {
        Some(user_fb) => Some(user_fb.type_id),
        None => ctx
            .standard_blocks
            .get(&name)
            .map(|standard| standard_fb_layout(*standard).0),
    };
    type_id == Some(fb_instance.type_id)
}

/// The internal error for a call `can_emit` should have left to the AST
/// route.
pub(super) fn no_instance(span: &SourceSpan) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span.clone(),
        "A lowered function block call has no instance",
    ))
}

/// The field and value of `stmt` when it stores a value in a field of
/// `instance`.
fn input<'s>(
    lowering: &Lowering<'_>,
    instance: &LoweredPlace,
    stmt: &'s Stmt,
) -> Option<(Id, &'s Expr)> {
    let StmtKind::Assign { place, value } = stmt.kind() else {
        return None;
    };
    let (Projection::Field(field), prefix) = place.path().split_last()? else {
        return None;
    };
    let of_instance = place.root() == instance.root()
        && prefix.len() == instance.path().len()
        && prefix
            .iter()
            .zip(instance.path())
            .all(|(step, other)| same_projection(step, other));
    if !of_instance {
        return None;
    }
    let declared = lowering
        .types()
        .fields(instance.ty())?
        .get(field.index() as usize)?;
    Some((Id::from(&declared.name.to_string()), value))
}

/// Whether two steps of a path to an instance are the same step. An instance
/// is never an element of an array: a call names its instance by a variable.
fn same_projection(step: &Projection, other: &Projection) -> bool {
    match step {
        Projection::Field(field) => matches!(other, Projection::Field(f) if f == field),
        Projection::Deref => matches!(other, Projection::Deref),
        Projection::Index(_) => false,
    }
}

/// What `expr` narrows, when it is a `Truncate`.
fn untruncated(expr: &Expr) -> Option<&Expr> {
    match expr.kind() {
        ExprKind::Truncate { value, .. } => Some(value),
        ExprKind::Const(_)
        | ExprKind::Read(_)
        | ExprKind::Convert(_)
        | ExprKind::ConvertChecked(_)
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. }
        | ExprKind::Compare { .. }
        | ExprKind::ShortCircuit { .. }
        | ExprKind::Call { .. }
        | ExprKind::InPlaceCall(_)
        | ExprKind::RefTo(_)
        | ExprKind::Null => None,
    }
}
