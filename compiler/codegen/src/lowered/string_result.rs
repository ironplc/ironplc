//! Emits a lowered call of a function whose result is a string.
//!
//! Lowering moves such a call out of the statement it is in: a `Call`
//! statement before it passes a reference to a temporary for the result,
//! and the statement reads the temporary (REQ-LOW-lowering-179). The route
//! that reads the AST compiles the call where its value is used: the
//! arguments, `STR_INIT` of the function's result, then `CALL`, which leaves
//! the result in a temporary buffer (ADR-0052) for the string operation that
//! uses it. This backend emits the call there too, so both routes compile
//! it to the same instructions, with the same temporary buffers and
//! temporary slots:
//!
//! * the moved `Call` statement emits nothing where it stands;
//! * the temporary has no storage: its read is the call, whose buffer the
//!   operation reading it consumes, as it does on the AST route.
//!
//! Emitting the call at the read rather than where the statement stands
//! means the same: lowering moves a call only where nothing evaluated in
//! between could see the difference (REQ-LOW-lowering-180).
//!
//! The moved calls of the statement being compiled are kept in a table on
//! the compile context, by the temporary each writes its result to, which a
//! read of the temporary takes its call from.

use std::collections::HashMap;

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::{Arg, Callee, Place as LoweredPlace, Stmt, StmtKind, VarId};
use ironplc_lowering::Lowering;

use super::call_args::passes;
use super::expr::emit_user_call;
use super::{args_can_emit, user_function};
use crate::compile::{CompileContext, UserFunctionInfo};
use crate::emit::Emitter;

/// The calls of functions whose result is a string, moved out of the
/// statement being compiled, by the temporary each writes its result to.
#[derive(Default)]
pub(crate) struct StringResults {
    calls: HashMap<VarId, MovedCall>,
}

/// A moved call, as this backend's layout knows its function.
struct MovedCall {
    function: UserFunctionInfo,
    /// The call's arguments for the function's inputs and in-outs, after
    /// the one for its result parameter.
    args: Vec<Arg>,
    span: SourceSpan,
}

impl StringResults {
    /// The moved calls among `stmts`, the statements one statement lowered
    /// to, and every statement inside them.
    pub(super) fn of(lowering: &Lowering<'_>, ctx: &CompileContext, stmts: &[Stmt]) -> Self {
        let mut results = Self::default();
        results.collect(lowering, ctx, stmts);
        results
    }

    fn collect(&mut self, lowering: &Lowering<'_>, ctx: &CompileContext, stmts: &[Stmt]) {
        for stmt in stmts {
            match stmt.kind() {
                StmtKind::Call { .. } => {
                    if let Some((temporary, call)) = moved_call(lowering, ctx, stmt) {
                        self.calls.insert(temporary, call);
                    }
                }
                StmtKind::If {
                    then, otherwise, ..
                } => {
                    self.collect(lowering, ctx, then);
                    self.collect(lowering, ctx, otherwise);
                }
                StmtKind::Case {
                    arms, otherwise, ..
                } => {
                    for arm in arms {
                        self.collect(lowering, ctx, arm.body());
                    }
                    self.collect(lowering, ctx, otherwise);
                }
                StmtKind::For { body, .. } => self.collect(lowering, ctx, body),
                StmtKind::Loop {
                    body, continuing, ..
                } => {
                    self.collect(lowering, ctx, body);
                    self.collect(lowering, ctx, continuing);
                }
                StmtKind::Assign { .. }
                | StmtKind::AssignStr { .. }
                | StmtKind::AssignBits { .. }
                | StmtKind::Copy { .. }
                | StmtKind::FbCall { .. }
                | StmtKind::Exit(_)
                | StmtKind::Continue(_)
                | StmtKind::Return => {}
            }
        }
    }

    /// Whether every call is emitted.
    pub(super) fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }
}

/// Whether `stmt` is a moved call of the statement being compiled.
pub(super) fn is_moved(ctx: &CompileContext, stmt: &Stmt) -> bool {
    let StmtKind::Call { args, .. } = stmt.kind() else {
        return false;
    };
    result_temporary(args)
        .is_some_and(|temporary| ctx.string_results.calls.contains_key(&temporary))
}

/// Whether this backend emits the moved call `stmt`: it passes each of the
/// call's arguments as the function takes it.
pub(super) fn call_can_emit(lowering: &Lowering<'_>, ctx: &CompileContext, stmt: &Stmt) -> bool {
    let StmtKind::Call { args, .. } = stmt.kind() else {
        return false;
    };
    let Some(call) =
        result_temporary(args).and_then(|temporary| ctx.string_results.calls.get(&temporary))
    else {
        return false;
    };
    passes(&call.function.params, call.args.iter()) && args_can_emit(lowering, ctx, &call.args)
}

/// Whether `place` is the temporary of a moved call of the statement being
/// compiled.
pub(super) fn is_result(ctx: &CompileContext, place: &LoweredPlace) -> bool {
    whole(place).is_some_and(|temporary| ctx.string_results.calls.contains_key(&temporary))
}

/// Emits the moved call whose temporary `place` is, where the temporary is
/// read: its arguments, `STR_INIT` of the function's result and `CALL`,
/// which leaves the result's buffer on the stack. `false` when `place` is no
/// such temporary.
pub(super) fn emit_at_read(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    place: &LoweredPlace,
) -> Result<bool, Diagnostic> {
    let Some(call) = whole(place).and_then(|temporary| ctx.string_results.calls.remove(&temporary))
    else {
        return Ok(false);
    };
    emit_user_call(
        emitter,
        ctx,
        lowering,
        &call.function,
        call.args.iter(),
        &call.span,
    )?;
    Ok(true)
}

/// The internal error for a moved call that no read of its temporary
/// emitted, which lowering rules out.
pub(super) fn not_emitted(span: &SourceSpan) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span.clone(),
        "A call with a string result that nothing reads",
    ))
}

/// The moved call `stmt` is, and the temporary it writes its result to:
/// a call of a function whose result is a string, whose first argument is a
/// reference to the whole of a temporary. `None` for any other statement.
fn moved_call(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    stmt: &Stmt,
) -> Option<(VarId, MovedCall)> {
    let StmtKind::Call {
        callee: Callee::User(pou),
        args,
    } = stmt.kind()
    else {
        return None;
    };
    lowering.pous().function(*pou)?.result_parameter()?;
    let function = user_function(lowering, ctx, *pou)?;
    function.return_string_info.as_ref()?;
    let temporary = result_temporary(args)?;
    let call = MovedCall {
        function,
        args: args[1..].to_vec(),
        span: stmt.span().clone(),
    };
    Some((temporary, call))
}

/// The temporary the first of `args`, a reference to its whole, passes.
fn result_temporary(args: &[Arg]) -> Option<VarId> {
    match args.first()? {
        Arg::Ref(place) => whole(place),
        Arg::Value(_) | Arg::Str(_) => None,
    }
}

/// The variable `place` is the whole of.
fn whole(place: &LoweredPlace) -> Option<VarId> {
    place.path().is_empty().then(|| place.root())
}
