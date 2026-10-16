//! Emits a lowered assignment of a call of a function with outputs.
//!
//! `r := F(a, q => x)` lowers to three parts, each with the assignment's
//! span (REQ-LOW-lowering-092): the call, stored in a temporary of the
//! result's type, with a reference to a temporary for each output; the
//! assignment of each output, read from its temporary; and the assignment
//! to `r`, read from the result's temporary.
//!
//! The three parts are emitted together, as the route that reads the AST
//! emits the assignment (`compile_user_call.rs`): the store in `r`, around
//! the call's inputs, its `CALL`, the stores of its outputs and the
//! conversion of its result. Neither kind of temporary has storage of its
//! own. The result is the value the `CALL` leaves on the stack, which stays
//! there while the outputs are stored. An output is the function's own
//! variable, which this backend's layout keeps the output in and which keeps
//! its value until the function is called again (ADR-0046), so a call
//! passes no reference for it.

use ironplc_container::VarIndex;
use ironplc_dsl::core::{Id, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::{
    Arg, Callee, Expr, ExprKind, Place as LoweredPlace, Stmt, StmtKind, StrExprKind, VarId,
};
use ironplc_lowering::Lowering;

use super::expr::{emit_operation, emit_user_call};
use super::place::{resolve, resolve_string};
use super::string::reaches_string;
use super::{
    args_can_emit, op_type, operand, operation_can_emit, passes, reaches, same_span, user_function,
};
use crate::compile::{CompileContext, StringVarInfo, UserFunctionInfo};
use crate::compile_expr::emit_load_var;
use crate::emit::Emitter;

/// The parts of an assignment of a call of a function with outputs, at the
/// head of a list of lowered statements.
pub(super) struct OutputCall<'s> {
    /// The function, as this backend's layout knows it.
    function: UserFunctionInfo,
    /// The value stored in the result's temporary: the call, inside the
    /// narrowing of its result, if any.
    value: &'s Expr,
    /// The call's arguments for the function's inputs and in-outs, in order.
    inputs: Vec<&'s Arg>,
    /// Where the function leaves the output each temporary is passed for.
    temporaries: Vec<(VarId, Output)>,
    /// The assignments of the outputs.
    outputs: &'s [Stmt],
    /// The assignment of the result: its target, and its value, a read of
    /// the result's temporary, inside the conversion the analyzer recorded.
    store: (&'s LoweredPlace, &'s Expr),
}

/// Where a function leaves one of its outputs, in this backend's layout.
#[derive(Clone)]
enum Output {
    /// A value, in the slot of the output's variable.
    Scalar(VarIndex),
    /// A string, in the data region.
    String(StringVarInfo),
}

impl OutputCall<'_> {
    /// How many statements the assignment is.
    pub(super) fn len(&self) -> usize {
        self.outputs.len() + 2
    }

    /// Whether this backend emits every part of the assignment.
    pub(super) fn can_emit(&self, lowering: &Lowering<'_>, ctx: &CompileContext) -> bool {
        let (target, value) = self.store;
        passes(&self.function.params, self.inputs.iter().copied())
            && args_can_emit(lowering, ctx, self.inputs.iter().copied())
            && operations_can_emit(self.value)
            && self
                .outputs
                .iter()
                .all(|output| self.output_can_emit(lowering, ctx, output))
            && operations_can_emit(value)
            && reaches(lowering, ctx, target, value.ty())
    }

    /// Whether this backend emits `stmt`, the assignment of an output read
    /// from its temporary.
    fn output_can_emit(&self, lowering: &Lowering<'_>, ctx: &CompileContext, stmt: &Stmt) -> bool {
        match stmt.kind() {
            StmtKind::Assign { place, value } => {
                matches!(self.read_output(innermost(value)), Some(Output::Scalar(_)))
                    && operations_can_emit(value)
                    && reaches(lowering, ctx, place, value.ty())
            }
            StmtKind::AssignStr { place, value } => {
                let output = match value.kind() {
                    StrExprKind::Read(temporary) => self.output(temporary),
                    StrExprKind::Literal(_) | StrExprKind::Call { .. } => None,
                };
                matches!(output, Some(Output::String(_))) && reaches_string(lowering, ctx, place)
            }
            StmtKind::AssignBits { .. }
            | StmtKind::FbCall { .. }
            | StmtKind::If { .. }
            | StmtKind::Case { .. }
            | StmtKind::For { .. }
            | StmtKind::Loop { .. }
            | StmtKind::Exit(_)
            | StmtKind::Continue(_)
            | StmtKind::Return => false,
        }
    }

    /// Where the function leaves the output whose temporary `expr` reads.
    fn read_output(&self, expr: &Expr) -> Option<&Output> {
        match expr.kind() {
            ExprKind::Read(temporary) => self.output(temporary),
            ExprKind::Const(_)
            | ExprKind::Convert(_)
            | ExprKind::ConvertChecked(_)
            | ExprKind::Truncate { .. }
            | ExprKind::Unary { .. }
            | ExprKind::Binary { .. }
            | ExprKind::Compare { .. }
            | ExprKind::ShortCircuit { .. }
            | ExprKind::Call { .. }
            | ExprKind::RefTo(_)
            | ExprKind::Null => None,
        }
    }

    /// Where the function leaves the output `temporary` is passed for.
    fn output(&self, temporary: &LoweredPlace) -> Option<&Output> {
        let temporary = whole(temporary)?;
        self.temporaries
            .iter()
            .find_map(|(passed, output)| (*passed == temporary).then_some(output))
    }
}

/// The assignment of a call of a function with outputs at the head of
/// `stmts`: the call stored in a temporary, the assignments of the outputs,
/// then the assignment of a read of the temporary, all with one span. `None`
/// when `stmts` does not start with one.
pub(super) fn output_call<'s>(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    stmts: &'s [Stmt],
) -> Option<OutputCall<'s>> {
    let [first, rest @ ..] = stmts else {
        return None;
    };
    let StmtKind::Assign { place, value } = first.kind() else {
        return None;
    };
    let result = whole(place)?;
    let ExprKind::Call {
        callee: Callee::User(pou),
        args,
    } = innermost(value).kind()
    else {
        return None;
    };
    let function = user_function(lowering, ctx, *pou)?;
    let parameters = lowering.pous().function(*pou)?.parameters();
    if parameters.len() != args.len() {
        return None;
    }
    let mut inputs = Vec::with_capacity(args.len());
    let mut temporaries = Vec::new();
    for (parameter, arg) in parameters.iter().zip(args) {
        let name = Id::from(&lowering.variables().get(*parameter)?.name().to_string());
        let Some(slot) = function.outputs.get(&name) else {
            inputs.push(arg);
            continue;
        };
        let Arg::Ref(temporary) = arg else {
            return None;
        };
        let output = match function.string_outputs.get(&name) {
            Some(string) => Output::String(string.clone()),
            None => Output::Scalar(*slot),
        };
        temporaries.push((whole(temporary)?, output));
    }
    if temporaries.is_empty() {
        return None;
    }
    let span = first.span();
    let end = rest.iter().position(|stmt| reads(stmt, result))?;
    let StmtKind::Assign {
        place: target,
        value: stored,
    } = rest[end].kind()
    else {
        return None;
    };
    if !rest[..=end].iter().all(|stmt| same_span(stmt.span(), span)) {
        return None;
    }
    Some(OutputCall {
        function,
        value,
        inputs,
        temporaries,
        outputs: &rest[..end],
        store: (target, stored),
    })
}

/// Emits `call`, whose span is `span`.
pub(super) fn emit_output_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    call: &OutputCall<'_>,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let (target, value) = call.store;
    resolve(lowering, ctx, target, span)?.emit_assign_fitting(emitter, ctx, |emitter, ctx| {
        emit_around(emitter, ctx, call.value, |emitter, ctx| {
            emit_user_call(
                emitter,
                ctx,
                lowering,
                &call.function,
                call.inputs.iter().copied(),
                span,
            )
        })?;
        for output in call.outputs {
            emit_output(emitter, ctx, lowering, call, output)?;
        }
        // The result is the value the call left on the stack.
        emit_around(emitter, ctx, value, |_, _| Ok(()))
    })
}

/// Emits `stmt`, the assignment of an output of `call` read from its
/// temporary, by reading the output where the function left it.
fn emit_output(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    call: &OutputCall<'_>,
    stmt: &Stmt,
) -> Result<(), Diagnostic> {
    let span = stmt.span();
    match stmt.kind() {
        StmtKind::Assign { place, value } => {
            let read = innermost(value);
            let slot = match call.read_output(read) {
                Some(Output::Scalar(slot)) => *slot,
                Some(Output::String(_)) | None => return Err(no_output(span)),
            };
            let at = op_type(read.ty()).ok_or_else(|| no_output(span))?;
            resolve(lowering, ctx, place, span)?.emit_assign_fitting(
                emitter,
                ctx,
                |emitter, ctx| {
                    emit_around(emitter, ctx, value, |emitter, _| {
                        emit_load_var(emitter, slot, at);
                        Ok(())
                    })
                },
            )
        }
        StmtKind::AssignStr { place, value } => {
            let output = match value.kind() {
                StrExprKind::Read(temporary) => call.output(temporary),
                StrExprKind::Literal(_) | StrExprKind::Call { .. } => None,
            };
            let Some(Output::String(string)) = output else {
                return Err(no_output(span));
            };
            emitter.emit_str_load_var(string.data_offset);
            resolve_string(lowering, ctx, place, span)?.emit_store(emitter, ctx)
        }
        StmtKind::AssignBits { .. }
        | StmtKind::FbCall { .. }
        | StmtKind::If { .. }
        | StmtKind::Case { .. }
        | StmtKind::For { .. }
        | StmtKind::Loop { .. }
        | StmtKind::Exit(_)
        | StmtKind::Continue(_)
        | StmtKind::Return => Err(no_output(span)),
    }
}

/// Emits `expr`, operations of one operand applied in turn to its innermost
/// operand, which `inner` emits.
fn emit_around(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    expr: &Expr,
    inner: impl FnOnce(&mut Emitter, &mut CompileContext) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    match operand(expr) {
        Some(value) => {
            emit_around(emitter, ctx, value, inner)?;
            emit_operation(emitter, expr)
        }
        None => inner(emitter, ctx),
    }
}

/// Whether this backend applies each operation of one operand around the
/// innermost operand of `expr`, which has an operation type.
fn operations_can_emit(expr: &Expr) -> bool {
    let mut expr = expr;
    while let Some(value) = operand(expr) {
        if !operation_can_emit(expr) {
            return false;
        }
        expr = value;
    }
    op_type(expr.ty()).is_some()
}

/// What `expr` applies operations of one operand to.
fn innermost(expr: &Expr) -> &Expr {
    let mut expr = expr;
    while let Some(value) = operand(expr) {
        expr = value;
    }
    expr
}

/// Whether `stmt` assigns a read of the variable `result`, inside
/// operations of one operand.
fn reads(stmt: &Stmt, result: VarId) -> bool {
    let StmtKind::Assign { value, .. } = stmt.kind() else {
        return false;
    };
    match innermost(value).kind() {
        ExprKind::Read(place) => whole(place) == Some(result),
        ExprKind::Const(_)
        | ExprKind::Convert(_)
        | ExprKind::ConvertChecked(_)
        | ExprKind::Truncate { .. }
        | ExprKind::Unary { .. }
        | ExprKind::Binary { .. }
        | ExprKind::Compare { .. }
        | ExprKind::ShortCircuit { .. }
        | ExprKind::Call { .. }
        | ExprKind::RefTo(_)
        | ExprKind::Null => false,
    }
}

/// The variable `place` is the whole of.
fn whole(place: &LoweredPlace) -> Option<VarId> {
    place.path().is_empty().then(|| place.root())
}

/// The internal error for an output `can_emit` should have left to the AST
/// route.
fn no_output(span: &SourceSpan) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span.clone(),
        "An output of a lowered function call this backend does not read",
    ))
}
