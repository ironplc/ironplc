//! The arguments of a call of a user-defined function or method, for its
//! inputs and in-outs.
//!
//! This backend passes each argument as the route that reads the AST passes
//! it (`compile_user_call.rs`, `compile_method.rs`), which the callee's
//! layout records for each parameter as a [`ParamPassing`]: a value on the
//! stack, or a place by reference. A function and a method pass their
//! arguments alike, so both check and push them here.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::Arg;
use ironplc_lowering::Lowering;

use super::expr::{emit_expr, emit_ref};
use crate::compile::{CompileContext, ParamPassing};
use crate::emit::Emitter;

/// Whether `args` are one for each of `params`, each passed as its
/// parameter takes it: a value by value, and a place by reference.
pub(super) fn passes<'s>(
    params: &[ParamPassing],
    args: impl ExactSizeIterator<Item = &'s Arg>,
) -> bool {
    params.len() == args.len()
        && params
            .iter()
            .zip(args)
            .all(|(param, arg)| passes_as(param, arg))
}

/// Pushes each of `args`, the arguments of the call at `span`, as its
/// parameter of `params` takes it, in order.
pub(super) fn emit_passed<'s>(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    params: &[ParamPassing],
    args: impl IntoIterator<Item = &'s Arg>,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    // `can_emit` leaves to the AST a call whose arguments do not pass.
    let mut args = args.into_iter();
    for param in params {
        let Some(arg) = args.next() else {
            return Err(not_passed(span));
        };
        match param {
            ParamPassing::Value => {
                let Arg::Value(value) = arg else {
                    return Err(not_passed(span));
                };
                emit_expr(emitter, ctx, lowering, value)?;
            }
            ParamPassing::Reference => {
                let Arg::Ref(place) = arg else {
                    return Err(not_passed(span));
                };
                emit_ref(emitter, ctx, lowering, place, span)?;
            }
            ParamPassing::String(_) => return Err(not_passed(span)),
        }
    }
    match args.next() {
        Some(_) => Err(not_passed(span)),
        None => Ok(()),
    }
}

/// Whether `arg` is passed as `param` takes it.
fn passes_as(param: &ParamPassing, arg: &Arg) -> bool {
    matches!(
        (param, arg),
        (ParamPassing::Value, Arg::Value(_)) | (ParamPassing::Reference, Arg::Ref(_))
    )
}

/// The internal error for an argument `can_emit` should have left to the
/// AST route.
fn not_passed(span: &SourceSpan) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span.clone(),
        "An argument of a lowered call this backend does not pass",
    ))
}
