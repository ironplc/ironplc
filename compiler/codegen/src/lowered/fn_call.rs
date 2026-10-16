//! Emits a lowered call of a function or method with outputs, and a call
//! statement of a method.
//!
//! `r := F(a, q => x)` lowers to three parts, each with the assignment's
//! span (REQ-LOW-lowering-172): the call, stored in a temporary of the
//! result's type, with a reference to a temporary for each output; the
//! assignment of each output, read from its temporary; and the assignment
//! to `r`, read from the result's temporary. A method call whose result is
//! not used, `c.M(a, q => x);`, lowers to a `Call` statement, then the
//! assignment of each output (REQ-LOW-lowering-092).
//!
//! The parts are emitted together, as the route that reads the AST emits
//! the call (`compile_user_call.rs`, `compile_method.rs`): the call's
//! inputs, its `CALL` or `METHOD_CALL`, the stores of its outputs, then the
//! result, stored as the AST route stores it or dropped. Neither kind of
//! temporary has storage of its own. The result is the value the call
//! leaves on the stack, which stays there while the outputs are stored. An
//! output is the callee's own variable, which this backend's layout keeps
//! the output in and which keeps its value until the callee is called again
//! (ADR-0046), so a call passes no reference for it.

use std::collections::HashMap;

use ironplc_container::VarIndex;
use ironplc_dsl::core::{Id, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::{
    Arg, Callee, Expr, ExprKind, Place as LoweredPlace, PouId, Stmt, StmtKind, StrExprKind, VarId,
};
use ironplc_lowering::Lowering;

use super::expr::{emit_operation, emit_user_call};
use super::method::{emit_method_call, method_site, MethodSite};
use super::place::{resolve, resolve_string};
use super::string::reaches_string;
use super::{
    args_can_emit, op_type, operand, operation_can_emit, passes, reaches, same_span, user_function,
};
use crate::compile::{CompileContext, StringVarInfo, UserFunctionInfo};
use crate::compile_expr::emit_load_var;
use crate::emit::Emitter;

/// The parts of a call of a function or method with outputs, or of a call
/// statement of a method, at the head of a list of lowered statements.
pub(super) struct OutputCall<'s> {
    /// The callee, as this backend's layout knows it.
    target: Target,
    /// The call's arguments for the callee's inputs and in-outs, in order.
    inputs: Vec<&'s Arg>,
    /// Where the callee leaves the output each temporary is passed for.
    temporaries: Vec<(VarId, Output)>,
    /// The assignments of the outputs.
    outputs: &'s [Stmt],
    /// What the call's result goes to.
    form: Form<'s>,
}

/// A call's callee, its arguments for the callee's inputs and in-outs, and
/// where the callee leaves the output each temporary is passed for.
struct Parts<'s> {
    target: Target,
    inputs: Vec<&'s Arg>,
    temporaries: Vec<(VarId, Output)>,
}

/// The callee of a call, as this backend's layout knows it.
enum Target {
    Function(UserFunctionInfo),
    Method(MethodSite),
}

/// What a call's result goes to.
enum Form<'s> {
    /// The assignment of the call: `value`, stored in the result's
    /// temporary, is the call inside the narrowing of its result, if any;
    /// `store` is the assignment of the result, its target and its value, a
    /// read of the result's temporary inside the conversion the analyzer
    /// recorded.
    Assignment {
        value: &'s Expr,
        store: (&'s LoweredPlace, &'s Expr),
    },
    /// A call statement of a method, whose result, if any, is dropped.
    Statement,
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
        let call = match self.form {
            Form::Assignment { .. } => 2,
            Form::Statement => 1,
        };
        self.outputs.len() + call
    }

    /// Whether this backend emits every part of the call.
    pub(super) fn can_emit(&self, lowering: &Lowering<'_>, ctx: &CompileContext) -> bool {
        let inputs = self.inputs.iter().copied();
        let passed = match &self.target {
            Target::Function(function) => passes(&function.params, inputs),
            Target::Method(site) => site.takes(inputs),
        };
        let stored = match self.form {
            Form::Assignment {
                value,
                store: (target, stored),
            } => {
                operations_can_emit(value)
                    && operations_can_emit(stored)
                    && reaches(lowering, ctx, target, stored.ty())
            }
            Form::Statement => true,
        };
        passed
            && args_can_emit(lowering, ctx, self.inputs.iter().copied())
            && self
                .outputs
                .iter()
                .all(|output| self.output_can_emit(lowering, ctx, output))
            && stored
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

/// The call at the head of `stmts`, all of whose parts have one span:
///
/// * the assignment of a call with outputs: the call stored in a temporary,
///   the assignments of the outputs, then the assignment of a read of the
///   temporary;
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
        StmtKind::Assign { place, value } => {
            let result = whole(place)?;
            let ExprKind::Call {
                callee: Callee::User(pou),
                args,
            } = innermost(value).kind()
            else {
                return None;
            };
            let Parts {
                target,
                inputs,
                temporaries,
            } = call_target(lowering, ctx, *pou, args)?;
            if temporaries.is_empty() {
                return None;
            }
            let end = rest.iter().position(|stmt| reads(stmt, result))?;
            let StmtKind::Assign {
                place: assigned,
                value: stored,
            } = rest[end].kind()
            else {
                return None;
            };
            if !rest[..=end].iter().all(|stmt| same_span(stmt.span(), span)) {
                return None;
            }
            Some(OutputCall {
                target,
                inputs,
                temporaries,
                outputs: &rest[..end],
                form: Form::Assignment {
                    value,
                    store: (assigned, stored),
                },
            })
        }
        StmtKind::Call {
            callee: Callee::User(pou),
            args,
        } => {
            let Parts {
                target,
                inputs,
                temporaries,
            } = call_target(lowering, ctx, *pou, args)?;
            let Target::Method(_) = target else {
                return None;
            };
            let outputs = rest
                .iter()
                .take_while(|stmt| {
                    same_span(stmt.span(), span) && assigns_output(&temporaries, stmt)
                })
                .count();
            Some(OutputCall {
                target,
                inputs,
                temporaries,
                outputs: &rest[..outputs],
                form: Form::Statement,
            })
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

/// The callee of the call of `pou` with `args`, the arguments for its
/// inputs and in-outs, and where it leaves the output each other argument
/// passes a temporary for. A method's first argument, its receiver, is
/// neither.
fn call_target<'s>(
    lowering: &Lowering<'_>,
    ctx: &CompileContext,
    pou: PouId,
    args: &'s [Arg],
) -> Option<Parts<'s>> {
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
    Some(Parts {
        target,
        inputs,
        temporaries,
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
    let inputs = || call.inputs.iter().copied();
    let outputs = |emitter: &mut Emitter, ctx: &mut CompileContext| {
        call.outputs
            .iter()
            .try_for_each(|output| emit_output(emitter, ctx, lowering, call, output))
    };
    match (&call.target, &call.form) {
        (
            Target::Function(function),
            Form::Assignment {
                value,
                store: (target, stored),
            },
        ) => resolve(lowering, ctx, target, span)?.emit_assign_fitting(
            emitter,
            ctx,
            |emitter, ctx| {
                emit_around(emitter, ctx, value, |emitter, ctx| {
                    emit_user_call(emitter, ctx, lowering, function, inputs(), span)
                })?;
                outputs(emitter, ctx)?;
                // The result is the value the call left on the stack.
                emit_around(emitter, ctx, stored, |_, _| Ok(()))
            },
        ),
        (
            Target::Method(site),
            Form::Assignment {
                value,
                store: (target, stored),
            },
        ) => resolve(lowering, ctx, target, span)?.emit_assign_fitting(
            emitter,
            ctx,
            |emitter, ctx| {
                emit_around(emitter, ctx, value, |emitter, ctx| {
                    emit_method_call(emitter, ctx, lowering, site, inputs(), span)?;
                    outputs(emitter, ctx)?;
                    // Drop the instance from beneath the result.
                    emitter.emit_swap();
                    emitter.emit_pop();
                    Ok(())
                })?;
                emit_around(emitter, ctx, stored, |_, _| Ok(()))
            },
        ),
        (Target::Method(site), Form::Statement) => {
            emit_method_call(emitter, ctx, lowering, site, inputs(), span)?;
            outputs(emitter, ctx)?;
            // Drop the result, if any, and the instance beneath it.
            if site.has_return_value {
                emitter.emit_pop();
            }
            emitter.emit_pop();
            Ok(())
        }
        // `output_call` makes a statement only of a method's call.
        (Target::Function(_), Form::Statement) => Err(no_output(span)),
    }
}

/// Emits `stmt`, the assignment of an output of `call` read from its
/// temporary, by reading the output where the callee left it.
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
            StrExprKind::Literal(_) | StrExprKind::Call { .. } => false,
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
