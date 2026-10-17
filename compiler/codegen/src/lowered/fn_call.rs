//! Emits a lowered call of a function or method with outputs, and a call
//! statement of a method.
//!
//! An assignment whose value has calls with outputs, such as
//! `r := F(a, q => x) + F(b, q => y)`, lowers to the part of each call, then
//! the assignment, each with the assignment's span (REQ-LOW-lowering-172). A
//! part is the call, stored in a temporary of the result's type, with a
//! reference to a temporary for each output, then the assignment of each
//! output, read from its temporary. The assignment reads each result from
//! its temporary. A method call whose result is not used, `c.M(a, q => x);`,
//! lowers to a `Call` statement, then the assignment of each output
//! (REQ-LOW-lowering-092).
//!
//! The statements are emitted together, as the route that reads the AST
//! emits the assignment (`compile_user_call.rs`, `compile_method.rs`): each
//! call's inputs, its `CALL` or `METHOD_CALL` and the stores of its outputs,
//! then the rest of the value and the store of the result, or the result
//! dropped (REQ-LOW-codegen-175). Neither kind of temporary has storage of
//! its own. A result is the value its call leaves on the stack, which stays
//! there while the outputs are stored, and the read of the result's
//! temporary takes it from there (`CompileContext::results_on_stack`). The
//! order lowering checks (REQ-LOW-lowering-174), which this backend checks
//! again before it emits the statements, makes each such read the next value
//! the expression would have pushed. An output is the callee's own variable,
//! which this backend's layout keeps the output in and which keeps its value
//! until the callee is called again (ADR-0046), so a call passes no
//! reference for it.

use std::collections::HashMap;

use ironplc_container::VarIndex;
use ironplc_dsl::core::{Id, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::{
    reads_results_in_order, Arg, Callee, Expr, ExprKind, Place as LoweredPlace, PouId, Stmt,
    StmtKind, StrExprKind, VarId,
};
use ironplc_lowering::Lowering;

use super::call_args::passes;
use super::expr::{emit_expr, emit_operation, emit_user_call};
use super::method::{emit_method_call, method_site, MethodSite};
use super::place::{resolve, resolve_string};
use super::string::reaches_string;
use super::{
    args_can_emit_reading, expr_can_emit_reading, op_type, operand, operation_can_emit, reaches,
    same_span, user_function,
};
use crate::compile::{CompileContext, StringVarInfo, UserFunctionInfo};
use crate::compile_expr::emit_load_var;
use crate::emit::Emitter;

/// The calls of functions or methods with outputs at the head of a list of
/// lowered statements, and what their results go to.
pub(super) enum OutputCall<'s> {
    /// An assignment whose value has calls with outputs: the part of each
    /// call, in the order they run, then `store`, the assignment's target
    /// and value, which reads each call's result from its temporary.
    Assignment {
        parts: Vec<Part<'s>>,
        store: (&'s LoweredPlace, &'s Expr),
    },
    /// A call statement of a method, whose result, if any, is dropped.
    Statement(Call<'s>),
}

/// The part of a call with outputs in the value of an assignment: the call,
/// stored in the temporary `result`, and the assignments of its outputs.
pub(super) struct Part<'s> {
    call: Call<'s>,
    /// The call inside the narrowing of its result, if any, which is stored
    /// in `result`.
    value: &'s Expr,
    result: VarId,
}

/// A call of a function or method with outputs, and the assignments of its
/// outputs.
pub(super) struct Call<'s> {
    /// The callee, as this backend's layout knows it.
    target: Target,
    /// The call's arguments for the callee's inputs and in-outs, in order.
    inputs: Vec<&'s Arg>,
    /// Where the callee leaves the output each temporary is passed for.
    temporaries: Vec<(VarId, Output)>,
    /// The assignments of the outputs.
    outputs: &'s [Stmt],
}

/// The callee of a call, as this backend's layout knows it.
enum Target {
    Function(UserFunctionInfo),
    Method(MethodSite),
}

/// Where a callee leaves one of its outputs, in this backend's layout.
#[derive(Clone)]
enum Output {
    /// A value, in the slot of the output's variable.
    Scalar(VarIndex),
    /// A string, in the data region.
    String(StringVarInfo),
}

impl Target {
    /// The slot of each output.
    fn outputs(&self) -> &HashMap<Id, VarIndex> {
        match self {
            Target::Function(function) => &function.outputs,
            Target::Method(site) => &site.outputs,
        }
    }

    /// Where each string output is.
    fn string_outputs(&self) -> &HashMap<Id, StringVarInfo> {
        match self {
            Target::Function(function) => &function.string_outputs,
            Target::Method(site) => &site.string_outputs,
        }
    }
}

impl OutputCall<'_> {
    /// How many statements the call is.
    pub(super) fn len(&self) -> usize {
        match self {
            OutputCall::Assignment { parts, .. } => {
                parts
                    .iter()
                    .map(|part| 1 + part.call.outputs.len())
                    .sum::<usize>()
                    + 1
            }
            OutputCall::Statement(call) => 1 + call.outputs.len(),
        }
    }

    /// Whether this backend emits every part of the call.
    pub(super) fn can_emit(&self, lowering: &Lowering<'_>, ctx: &CompileContext) -> bool {
        match self {
            OutputCall::Assignment {
                parts,
                store: (target, stored),
            } => {
                // Each result is read as the next value on the stack
                // (REQ-LOW-lowering-174), which `emit_part` leaves there.
                let results: Vec<VarId> = parts.iter().map(|part| part.result).collect();
                let values: Vec<(&Expr, VarId)> =
                    parts.iter().map(|part| (part.value, part.result)).collect();
                reads_results_in_order(&values, stored)
                    && parts.iter().all(|part| {
                        part.call.can_emit(lowering, ctx, &results)
                            && operations_can_emit(part.value)
                    })
                    && expr_can_emit_reading(lowering, ctx, stored, &results)
                    && reaches(lowering, ctx, target, stored.ty())
            }
            OutputCall::Statement(call) => call.can_emit(lowering, ctx, &[]),
        }
    }
}

impl Call<'_> {
    /// Whether this backend passes the call's inputs, in which a read of one
    /// of `results` takes the value an earlier call left on the stack, and
    /// emits the assignment of each of its outputs.
    fn can_emit(&self, lowering: &Lowering<'_>, ctx: &CompileContext, results: &[VarId]) -> bool {
        let inputs = self.inputs.iter().copied();
        let passed = match &self.target {
            Target::Function(function) => passes(&function.params, inputs),
            Target::Method(site) => site.takes(inputs),
        };
        passed
            && args_can_emit_reading(lowering, ctx, self.inputs.iter().copied(), results)
            && self
                .outputs
                .iter()
                .all(|output| self.output_can_emit(lowering, ctx, output))
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
                    StrExprKind::Literal(_)
                    | StrExprKind::Call { .. }
                    | StrExprKind::InPlaceCall { .. } => None,
                };
                matches!(output, Some(Output::String(_))) && reaches_string(lowering, ctx, place)
            }
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
            | StmtKind::Return => false,
        }
    }

    /// Where the callee leaves the output whose temporary `expr` reads.
    fn read_output(&self, expr: &Expr) -> Option<&Output> {
        output_read(&self.temporaries, expr)
    }

    /// Where the callee leaves the output `temporary` is passed for.
    fn output(&self, temporary: &LoweredPlace) -> Option<&Output> {
        output_of(&self.temporaries, temporary)
    }
}

/// The calls at the head of `stmts`, all of whose statements have one span:
///
/// * an assignment whose value has calls with outputs: the part of each,
///   the call stored in a temporary then the assignments of its outputs,
///   then the assignment, whose value reads the temporaries;
/// * a call statement of a method, then the assignments of its outputs.
///
/// `None` when `stmts` does not start with one.
pub(super) fn output_call<'s>(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    stmts: &'s [Stmt],
) -> Option<OutputCall<'s>> {
    let [first, rest @ ..] = stmts else {
        return None;
    };
    let span = first.span();
    match first.kind() {
        StmtKind::Assign { .. } => {
            let mut parts = Vec::new();
            let mut rest = stmts;
            while let Some((part, len)) = part(lowering, ctx, rest, span) {
                parts.push(part);
                rest = &rest[len..];
            }
            let [stored, ..] = rest else {
                return None;
            };
            if parts.is_empty() || !same_span(stored.span(), span) {
                return None;
            }
            let StmtKind::Assign { place, value } = stored.kind() else {
                return None;
            };
            Some(OutputCall::Assignment {
                parts,
                store: (place, value),
            })
        }
        StmtKind::Call {
            callee: Callee::User(pou),
            args,
        } => {
            let mut call = call_target(lowering, ctx, *pou, args)?;
            let Target::Method(_) = call.target else {
                return None;
            };
            call.outputs = outputs_of(&call.temporaries, rest, span);
            Some(OutputCall::Statement(call))
        }
        StmtKind::Call { .. }
        | StmtKind::AssignStr { .. }
        | StmtKind::AssignBits { .. }
        | StmtKind::Copy { .. }
        | StmtKind::FbCall { .. }
        | StmtKind::If { .. }
        | StmtKind::Case { .. }
        | StmtKind::For { .. }
        | StmtKind::Loop { .. }
        | StmtKind::Exit(_)
        | StmtKind::Continue(_)
        | StmtKind::Return => None,
    }
}

/// The part at the head of `stmts`, at `span`, and how many statements it
/// is: the assignment of a call with outputs to a temporary, inside the
/// narrowing of its result, if any, then the assignments of its outputs.
fn part<'s>(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    stmts: &'s [Stmt],
    span: &SourceSpan,
) -> Option<(Part<'s>, usize)> {
    let [first, rest @ ..] = stmts else {
        return None;
    };
    let StmtKind::Assign { place, value } = first.kind() else {
        return None;
    };
    if !same_span(first.span(), span) {
        return None;
    }
    let result = whole(place)?;
    let ExprKind::Call {
        callee: Callee::User(pou),
        args,
    } = innermost(value).kind()
    else {
        return None;
    };
    let mut call = call_target(lowering, ctx, *pou, args)?;
    if call.temporaries.is_empty() {
        return None;
    }
    call.outputs = outputs_of(&call.temporaries, rest, span);
    let len = 1 + call.outputs.len();
    Some((
        Part {
            call,
            value,
            result,
        },
        len,
    ))
}

/// The assignments of outputs read from `temporaries` at the head of
/// `stmts`, at `span`.
fn outputs_of<'s>(
    temporaries: &[(VarId, Output)],
    stmts: &'s [Stmt],
    span: &SourceSpan,
) -> &'s [Stmt] {
    let count = stmts
        .iter()
        .take_while(|stmt| same_span(stmt.span(), span) && assigns_output(temporaries, stmt))
        .count();
    &stmts[..count]
}

/// The call of `pou` with `args`: its callee, the arguments for its inputs
/// and in-outs, and where it leaves the output each other argument passes a
/// temporary for, with no assignments of its outputs yet. A method's first
/// argument, its receiver, is neither.
fn call_target<'s>(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    pou: PouId,
    args: &'s [Arg],
) -> Option<Call<'s>> {
    let (target, receivers) = match user_function(lowering, ctx, pou) {
        Some(function) => (Target::Function(function), 0),
        None => (Target::Method(method_site(lowering, ctx, pou, args)?), 1),
    };
    let parameters = lowering.pous().function(pou)?.parameters();
    if parameters.len() != args.len() {
        return None;
    }
    let mut inputs = Vec::with_capacity(args.len());
    let mut temporaries = Vec::new();
    for (parameter, arg) in parameters.iter().zip(args).skip(receivers) {
        let name = Id::from(&lowering.variables().get(*parameter)?.name().to_string());
        let Some(slot) = target.outputs().get(&name) else {
            inputs.push(arg);
            continue;
        };
        let Arg::Ref(temporary) = arg else {
            return None;
        };
        let output = match target.string_outputs().get(&name) {
            Some(string) => Output::String(string.clone()),
            None => Output::Scalar(*slot),
        };
        temporaries.push((whole(temporary)?, output));
    }
    Some(Call {
        target,
        inputs,
        temporaries,
        outputs: &[],
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
    match call {
        OutputCall::Assignment {
            parts,
            store: (target, stored),
        } => resolve(lowering, ctx, target, span)?.emit_assign_fitting(
            emitter,
            ctx,
            |emitter, ctx| {
                for part in parts {
                    emit_part(emitter, ctx, lowering, part, span)?;
                }
                // Each result is the value its call left on the stack.
                emit_expr(emitter, ctx, lowering, stored)?;
                all_results_read(ctx, span)
            },
        ),
        OutputCall::Statement(call) => {
            // `output_call` makes a statement only of a method's call.
            let Target::Method(site) = &call.target else {
                return Err(no_output(span));
            };
            emit_method_call(
                emitter,
                ctx,
                lowering,
                site,
                call.inputs.iter().copied(),
                span,
            )?;
            call.emit_outputs(emitter, ctx, lowering)?;
            // Drop the result, if any, and the instance beneath it.
            if site.has_return_value {
                emitter.emit_pop();
            }
            emitter.emit_pop();
            Ok(())
        }
    }
}

/// Emits `part`, whose span is `span`: its call, which leaves its result on
/// the stack, then the stores of its outputs. The result stays on the stack
/// for the read of its temporary to take.
fn emit_part(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    part: &Part<'_>,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let call = &part.call;
    let inputs = call.inputs.iter().copied();
    match &call.target {
        Target::Function(function) => {
            emit_around(emitter, ctx, part.value, |emitter, ctx| {
                emit_user_call(emitter, ctx, lowering, function, inputs, span)
            })?;
            call.emit_outputs(emitter, ctx, lowering)?;
        }
        Target::Method(site) => emit_around(emitter, ctx, part.value, |emitter, ctx| {
            emit_method_call(emitter, ctx, lowering, site, inputs, span)?;
            call.emit_outputs(emitter, ctx, lowering)?;
            // Drop the instance from beneath the result.
            emitter.emit_swap();
            emitter.emit_pop();
            Ok(())
        })?,
    }
    ctx.results_on_stack.push(part.result);
    Ok(())
}

/// Whether `place` is the temporary of a call's result that this backend
/// left on the stack (`emit_part`), which a read of it takes: it is then on
/// the stack no longer.
pub(super) fn take_result(ctx: &mut CompileContext, place: &LoweredPlace) -> bool {
    let Some(variable) = whole(place) else {
        return false;
    };
    let Some(position) = ctx
        .results_on_stack
        .iter()
        .position(|result| *result == variable)
    else {
        return false;
    };
    ctx.results_on_stack.remove(position);
    true
}

/// Refuses an assignment that left a result of one of its calls on the
/// stack unread, which `can_emit` should have left to the AST route.
fn all_results_read(ctx: &mut CompileContext, span: &SourceSpan) -> Result<(), Diagnostic> {
    if ctx.results_on_stack.is_empty() {
        return Ok(());
    }
    ctx.results_on_stack.clear();
    Err(Diagnostic::internal_error_at(Label::span(
        span.clone(),
        "A result of a lowered call this backend did not read",
    )))
}

impl Call<'_> {
    /// Emits the stores of the call's outputs, each read where the callee
    /// left it.
    fn emit_outputs(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        lowering: &Lowering<'_>,
    ) -> Result<(), Diagnostic> {
        self.outputs
            .iter()
            .try_for_each(|output| emit_output(emitter, ctx, lowering, self, output))
    }
}

/// Emits `stmt`, the assignment of an output of `call` read from its
/// temporary, by reading the output where the callee left it.
fn emit_output(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    call: &Call<'_>,
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
                StrExprKind::Literal(_)
                | StrExprKind::Call { .. }
                | StrExprKind::InPlaceCall { .. } => None,
            };
            let Some(Output::String(string)) = output else {
                return Err(no_output(span));
            };
            emitter.emit_str_load_var(string.data_offset);
            resolve_string(lowering, ctx, place, span)?.emit_store(emitter, ctx)
        }
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

/// The variable `place` is the whole of.
fn whole(place: &LoweredPlace) -> Option<VarId> {
    place.path().is_empty().then(|| place.root())
}

/// Where the callee leaves the output `temporary` is passed for, of
/// `temporaries`.
fn output_of<'t>(
    temporaries: &'t [(VarId, Output)],
    temporary: &LoweredPlace,
) -> Option<&'t Output> {
    let temporary = whole(temporary)?;
    temporaries
        .iter()
        .find_map(|(passed, output)| (*passed == temporary).then_some(output))
}

/// Where the callee leaves the output whose temporary, of `temporaries`,
/// `expr` reads.
fn output_read<'t>(temporaries: &'t [(VarId, Output)], expr: &Expr) -> Option<&'t Output> {
    match expr.kind() {
        ExprKind::Read(temporary) => output_of(temporaries, temporary),
        ExprKind::Const(_)
        | ExprKind::Convert(_)
        | ExprKind::ConvertChecked(_)
        | ExprKind::Truncate { .. }
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

/// Whether `stmt` assigns an output read from one of `temporaries`.
fn assigns_output(temporaries: &[(VarId, Output)], stmt: &Stmt) -> bool {
    match stmt.kind() {
        StmtKind::Assign { value, .. } => output_read(temporaries, innermost(value)).is_some(),
        StmtKind::AssignStr { value, .. } => match value.kind() {
            StrExprKind::Read(temporary) => output_of(temporaries, temporary).is_some(),
            StrExprKind::Literal(_)
            | StrExprKind::Call { .. }
            | StrExprKind::InPlaceCall { .. } => false,
        },
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
        | StmtKind::Return => false,
    }
}

/// The internal error for an output `can_emit` should have left to the AST
/// route.
fn no_output(span: &SourceSpan) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span.clone(),
        "An output of a lowered call this backend does not read",
    ))
}
