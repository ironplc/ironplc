//! Emits a lowered statement.
//!
//! Each statement is emitted in the shape `compile_stmt.rs` and
//! `compile_loop.rs` give the statement it was lowered from, fused compare
//! and branch included, so a body runs the same instructions by either
//! route. The one difference is `Case`, whose selector is evaluated once
//! (REQ-LOW-codegen-124): it stays on the stack while the labels are tested,
//! and the arm that runs pops it first.

use ironplc_container::opcode::cmp_op;
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::lowered::{
    counts_down, CaseArm, CaseLabel, CompareOp, Const, Expr, ExprKind, LoopId,
    Place as LoweredPlace, Stmt, StmtKind,
};
use ironplc_lowering::Lowering;

use super::expr::{emit_constant, emit_expr, emit_truncate};
use super::{constant, op_type, read, storage_name};
use crate::compile::{CompileContext, OpType};
use crate::compile_expr::{
    classify_with_named, emit_add, emit_classified_cmp_br, emit_eq, emit_ge, emit_le, ClassifiedCmp,
};
use crate::compile_loop::LoopLabels;
use crate::compile_partial_access::Bits;
use crate::compile_stmt::{emit_return, record_position};
use crate::emit::{self, Emitter};

/// Emits `stmts`, the statements one top-level statement lowered to, in
/// order, at the source position of that statement.
pub(super) fn emit_stmts(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Lowering<'_>,
    stmts: &[Stmt],
) -> Result<(), Diagnostic> {
    let mut emission = Emission {
        lowering,
        loops: Vec::new(),
    };
    emission.sequence(emitter, ctx, stmts, false)
}

/// What emitting a statement needs besides the compile context.
struct Emission<'l, 'a> {
    lowering: &'l Lowering<'a>,
    /// The loops enclosing the statement being emitted, innermost last.
    loops: Vec<(LoopId, LoopLabels)>,
}

impl Emission<'_, '_> {
    /// Emits the statements `stmts`, each at its own source position.
    fn block(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        stmts: &[Stmt],
    ) -> Result<(), Diagnostic> {
        self.sequence(emitter, ctx, stmts, true)
    }

    /// Emits `stmts` in order, at each one's source position when `record`.
    /// The stores of a function block call's inputs and the call are emitted
    /// together, as the AST route emits a call (`fb_call.rs`), and so are the
    /// parts of an assignment of a call of a function with outputs
    /// (`fn_call.rs`).
    fn sequence(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        stmts: &[Stmt],
        record: bool,
    ) -> Result<(), Diagnostic> {
        let mut rest = stmts;
        while let [first, rest_after_first @ ..] = rest {
            // A call with a string result is emitted where its result is
            // read (`string_result.rs`).
            if super::string_result::is_moved(ctx, first) {
                rest = rest_after_first;
                continue;
            }
            if record {
                record_position(emitter, ctx, first.span());
            }
            let emitted = if let Some(call) = super::fb_call::call_site(self.lowering, ctx, rest) {
                super::fb_call::emit_call_site(emitter, ctx, self.lowering, &call, first.span())?;
                call.len()
            } else if let Some(call) = super::fn_call::output_call(self.lowering, ctx, rest) {
                super::fn_call::emit_output_call(emitter, ctx, self.lowering, &call, first.span())?;
                call.len()
            } else {
                self.stmt(emitter, ctx, first)?;
                1
            };
            rest = &rest[emitted..];
        }
        Ok(())
    }

    fn stmt(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        stmt: &Stmt,
    ) -> Result<(), Diagnostic> {
        let span = stmt.span();
        match stmt.kind() {
            StmtKind::Assign { place, value } => {
                let lowering = self.lowering;
                let storage = super::place::resolve(lowering, ctx, place, span)?;
                let push = |emitter: &mut Emitter, ctx: &mut CompileContext| {
                    emit_expr(emitter, ctx, lowering, value)
                };
                if super::place::reads_standard_block_field(lowering, ctx, value) {
                    storage.emit_assign(emitter, ctx, push)
                } else {
                    storage.emit_assign_fitting(emitter, ctx, push)
                }
            }
            StmtKind::AssignStr { place, value } => {
                super::string::emit_assign_str(emitter, ctx, self.lowering, place, value, span)
            }
            StmtKind::AssignBits {
                place,
                shift,
                bits,
                value,
            } => {
                let lowering = self.lowering;
                let storage = super::place::resolve(lowering, ctx, place, span)?;
                let width = storage.op_type().0;
                let selection = Bits::new(u32::from(*shift), u32::from(*bits));
                storage.emit_update(emitter, ctx, |emitter, ctx| {
                    selection.emit_insert(emitter, ctx, width, |emitter, ctx| {
                        emit_expr(emitter, ctx, lowering, value)
                    })
                })
            }
            // A call is emitted with the stores of its inputs (`sequence`);
            // one on its own has none.
            StmtKind::Copy { dst, src } => {
                super::aggregate::emit_copy(emitter, ctx, self.lowering, dst, src, span)
            }
            StmtKind::FbCall { instance, .. } => {
                let call = super::fb_call::bare_call(self.lowering, ctx, instance)
                    .ok_or_else(|| super::fb_call::no_instance(span))?;
                super::fb_call::emit_call_site(emitter, ctx, self.lowering, &call, span)
            }
            // `sequence` emits a call statement with its outputs.
            StmtKind::Call { .. } => Err(Diagnostic::internal_error_at(Label::span(
                span.clone(),
                "A lowered call statement outside the statements of its call",
            ))),
            StmtKind::If {
                cond,
                then,
                otherwise,
            } => self.if_(emitter, ctx, cond, then, otherwise),
            StmtKind::Case {
                selector,
                arms,
                otherwise,
            } => self.case(emitter, ctx, selector, arms, otherwise),
            StmtKind::For {
                id,
                control,
                from,
                to,
                step,
                body,
            } => self.for_(emitter, ctx, *id, control, (from, to, *step), body, span),
            StmtKind::Loop {
                id,
                body,
                continuing,
            } => self.loop_(emitter, ctx, *id, body, continuing),
            StmtKind::Exit(id) => {
                let exit = self.labels(*id, span)?.exit;
                emitter.emit_jmp(exit);
                Ok(())
            }
            StmtKind::Continue(id) => {
                let labels = self.labels(*id, span)?;
                labels.next_used = true;
                let next = labels.next;
                emitter.emit_jmp(next);
                Ok(())
            }
            StmtKind::Return => {
                emit_return(emitter, ctx);
                Ok(())
            }
        }
    }

    /// Pops a value, which already fits `place`, and stores it there.
    fn store(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        place: &LoweredPlace,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        super::place::resolve(self.lowering, ctx, place, span)?.emit_store_fitting(emitter, ctx)
    }

    fn load(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        place: &LoweredPlace,
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        super::place::resolve(self.lowering, ctx, place, span)?.emit_load(emitter, ctx)
    }

    /// As `compile_if`: the `else` branch is jumped over only when there is
    /// one.
    fn if_(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        cond: &Expr,
        then: &[Stmt],
        otherwise: &[Stmt],
    ) -> Result<(), Diagnostic> {
        let next = emitter.create_label();
        self.branch_unless(emitter, ctx, cond, next)?;
        self.block(emitter, ctx, then)?;
        if otherwise.is_empty() {
            emitter.bind_label(next);
            return Ok(());
        }
        let end = emitter.create_label();
        emitter.emit_jmp(end);
        emitter.bind_label(next);
        self.block(emitter, ctx, otherwise)?;
        emitter.bind_label(end);
        Ok(())
    }

    /// Jumps to `target` unless `cond` is true, with one `CMP_BR_*` when the
    /// condition compares a variable with a constant.
    fn branch_unless(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        cond: &Expr,
        target: emit::Label,
    ) -> Result<(), Diagnostic> {
        if let Some(classified) = self.classify(ctx, cond) {
            return emit_classified_cmp_br(emitter, classified, false, target);
        }
        emit_expr(emitter, ctx, self.lowering, cond)?;
        emitter.emit_jmp_if_not(target);
        Ok(())
    }

    /// `cond` as one `CMP_BR_*`, when it compares a signed integer variable
    /// with a constant, as `try_classify_cmp` finds it in the AST.
    fn classify(&self, ctx: &mut CompileContext, cond: &Expr) -> Option<ClassifiedCmp> {
        let ExprKind::Compare { op, lhs, rhs } = cond.kind() else {
            return None;
        };
        let cmp = match op {
            CompareOp::Eq => cmp_op::EQ,
            CompareOp::Ne => cmp_op::NE,
            CompareOp::Lt => cmp_op::LT_S,
            CompareOp::Le => cmp_op::LE_S,
            CompareOp::Gt => cmp_op::GT_S,
            CompareOp::Ge => cmp_op::GE_S,
        };
        let (place, value, cmp) = match read(lhs).zip(constant(rhs)) {
            Some((place, value)) => (place, value, cmp),
            None => (read(rhs)?, constant(lhs)?, cmp_op::commute(cmp)?),
        };
        self.classify_place(ctx, place, value, lhs.ty(), cmp)
    }

    /// `place <cmp> value` as one `CMP_BR_*`, when `place` is a signed integer
    /// variable.
    fn classify_place(
        &self,
        ctx: &mut CompileContext,
        place: &LoweredPlace,
        value: Const,
        ty: ironplc_ir::lowered::ScalarType,
        cmp: u8,
    ) -> Option<ClassifiedCmp> {
        let value = i64::try_from(value.integer_value(ty)?).ok()?;
        let name = storage_name(self.lowering, place)?;
        classify_with_named(ctx, &name, value, cmp)
    }

    /// Evaluates the selector once and keeps it on the stack while the labels
    /// are tested. Each label tests a copy; the arm whose label matches pops
    /// the selector before it runs, and `otherwise` pops it when none does.
    fn case(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        selector: &Expr,
        arms: &[CaseArm],
        otherwise: &[Stmt],
    ) -> Result<(), Diagnostic> {
        let op = operation(selector)?;
        emit_expr(emitter, ctx, self.lowering, selector)?;
        let with_selector = emitter.stack_depth();
        let end = emitter.create_label();
        for arm in arms {
            // An arm without a label never runs.
            let Some((last, others)) = arm.labels().split_last() else {
                continue;
            };
            let next_arm = emitter.create_label();
            let body = (!others.is_empty()).then(|| emitter.create_label());
            for label in others {
                let next_label = emitter.create_label();
                test_label(emitter, ctx, op, *label, next_label);
                if let Some(body) = body {
                    emitter.emit_jmp(body);
                }
                emitter.bind_label(next_label);
            }
            test_label(emitter, ctx, op, *last, next_arm);
            if let Some(body) = body {
                emitter.bind_label(body);
            }
            emitter.emit_pop();
            self.block(emitter, ctx, arm.body())?;
            emitter.emit_jmp(end);
            emitter.bind_label(next_arm);
            emitter.reset_stack_depth(with_selector);
        }
        emitter.emit_pop();
        self.block(emitter, ctx, otherwise)?;
        emitter.bind_label(end);
        Ok(())
    }

    /// As `compile_for`, with the loop test fused into one `CMP_BR_*` when the
    /// bound is a constant, and the truncation of the control variable left
    /// out when constant bounds keep it within its type.
    #[allow(clippy::too_many_arguments)]
    fn for_(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        id: LoopId,
        control: &LoweredPlace,
        (from, to, step): (&Expr, &Expr, Const),
        body: &[Stmt],
        span: &SourceSpan,
    ) -> Result<(), Diagnostic> {
        let ty = to.ty();
        let op = operation(to)?;
        let down = counts_down(step, ty);
        emit_expr(emitter, ctx, self.lowering, from)?;
        self.store(emitter, ctx, control, span)?;

        let head = emitter.create_label();
        let end = emitter.create_label();
        emitter.bind_label(head);
        let within = if down { cmp_op::GE_S } else { cmp_op::LE_S };
        let fused =
            constant(to).and_then(|bound| self.classify_place(ctx, control, bound, ty, within));
        match fused {
            Some(classified) => emit_classified_cmp_br(emitter, classified, false, end)?,
            None => {
                self.load(emitter, ctx, control, span)?;
                emit_expr(emitter, ctx, self.lowering, to)?;
                if down {
                    emit_ge(emitter, op);
                } else {
                    emit_le(emitter, op);
                }
                emitter.emit_jmp_if_not(end);
            }
        }

        let next = self.loop_body(emitter, ctx, id, end, body)?;
        bind_next(emitter, next);

        self.load(emitter, ctx, control, span)?;
        emit_constant(emitter, ctx, step);
        emit_add(emitter, op);
        let types = self.lowering.types();
        if let Some(width) = types.narrowed_storage(control.ty()) {
            if !stays_within(types, control, from, to, step) {
                emit_truncate(emitter, width, op.1, span)?;
            }
        }
        self.store(emitter, ctx, control, span)?;
        emitter.emit_jmp(head);
        emitter.bind_label(end);
        Ok(())
    }

    /// A `Loop`. The two shapes `WHILE` and `REPEAT` lower to are emitted as
    /// `compile_while` and `compile_repeat` emit them; any other repeats
    /// `body` then `continuing`.
    fn loop_(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        id: LoopId,
        body: &[Stmt],
        continuing: &[Stmt],
    ) -> Result<(), Diagnostic> {
        if let [tail] = continuing {
            if let Some(until) = exit_when(tail, id) {
                return self.repeat(emitter, ctx, id, body, until);
            }
        }
        if continuing.is_empty() {
            if let Some((head, rest)) = body.split_first() {
                if let Some(cond) = exit_unless(head, id) {
                    return self.while_(emitter, ctx, id, cond, rest);
                }
            }
        }
        let head = emitter.create_label();
        let end = emitter.create_label();
        emitter.bind_label(head);
        let next = self.loop_body(emitter, ctx, id, end, body)?;
        bind_next(emitter, next);
        self.block(emitter, ctx, continuing)?;
        emitter.emit_jmp(head);
        emitter.bind_label(end);
        Ok(())
    }

    /// `WHILE cond DO body`, as `compile_while` emits it.
    fn while_(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        id: LoopId,
        cond: &Expr,
        body: &[Stmt],
    ) -> Result<(), Diagnostic> {
        let end = emitter.create_label();
        if let Some(classified) = self.classify(ctx, cond) {
            let start = emitter.create_label();
            emit_classified_cmp_br(emitter, classified, false, end)?;
            emitter.bind_label(start);
            let next = self.loop_body(emitter, ctx, id, end, body)?;
            bind_next(emitter, next);
            emit_classified_cmp_br(emitter, classified, true, start)?;
            emitter.bind_label(end);
            return Ok(());
        }
        let head = emitter.create_label();
        emitter.bind_label(head);
        emit_expr(emitter, ctx, self.lowering, cond)?;
        emitter.emit_jmp_if_not(end);
        let next = self.loop_body(emitter, ctx, id, end, body)?;
        bind_next(emitter, next);
        emitter.emit_jmp(head);
        emitter.bind_label(end);
        Ok(())
    }

    /// `REPEAT body UNTIL until`, as `compile_repeat` emits it.
    fn repeat(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        id: LoopId,
        body: &[Stmt],
        until: &Expr,
    ) -> Result<(), Diagnostic> {
        let head = emitter.create_label();
        let end = emitter.create_label();
        // Classified before the body, as `compile_repeat` does, so the
        // constant pool holds the same constants in the same order.
        let fused = self.classify(ctx, until);
        emitter.bind_label(head);
        let next = self.loop_body(emitter, ctx, id, end, body)?;
        bind_next(emitter, next);
        match fused {
            Some(classified) => emit_classified_cmp_br(emitter, classified, false, head)?,
            None => {
                emit_expr(emitter, ctx, self.lowering, until)?;
                emitter.emit_jmp_if_not(head);
            }
        }
        emitter.bind_label(end);
        Ok(())
    }

    /// Emits the body of the loop `id`, whose `Exit` goes to `exit`, and
    /// returns the label its `Continue` goes to when it has one, for the
    /// caller to bind where the next iteration starts.
    fn loop_body(
        &mut self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        id: LoopId,
        exit: emit::Label,
        body: &[Stmt],
    ) -> Result<Option<emit::Label>, Diagnostic> {
        let labels = LoopLabels {
            exit,
            next: emitter.create_label(),
            next_used: false,
        };
        self.loops.push((id, labels));
        let emitted = self.block(emitter, ctx, body);
        let (_, labels) = self.loops.pop().ok_or_else(Diagnostic::internal_error)?;
        emitted?;
        Ok(labels.next_used.then_some(labels.next))
    }

    /// The labels of the enclosing loop `id`.
    fn labels(&mut self, id: LoopId, span: &SourceSpan) -> Result<&mut LoopLabels, Diagnostic> {
        self.loops
            .iter_mut()
            .rev()
            .find(|(loop_id, _)| *loop_id == id)
            .map(|(_, labels)| labels)
            .ok_or_else(|| {
                Diagnostic::internal_error_at(Label::span(
                    span.clone(),
                    format!("{id} does not enclose the statement"),
                ))
            })
    }
}

/// Binds the label a `Continue` goes to, when the body has one. Binding a
/// label ends the peephole window, so it is bound only then.
fn bind_next(emitter: &mut Emitter, next: Option<emit::Label>) {
    if let Some(label) = next {
        emitter.bind_label(label);
    }
}

/// Jumps to `fail` unless the selector on the stack matches `label`, leaving
/// the selector there either way.
fn test_label(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    op: OpType,
    label: CaseLabel,
    fail: emit::Label,
) {
    match label {
        CaseLabel::Value(value) => {
            emitter.emit_dup();
            emit_constant(emitter, ctx, value);
            emit_eq(emitter, op);
            emitter.emit_jmp_if_not(fail);
        }
        CaseLabel::Range(low, high) => {
            emitter.emit_dup();
            emit_constant(emitter, ctx, low);
            emit_ge(emitter, op);
            emitter.emit_jmp_if_not(fail);
            emitter.emit_dup();
            emit_constant(emitter, ctx, high);
            emit_le(emitter, op);
            emitter.emit_jmp_if_not(fail);
        }
    }
}

/// The condition of `stmt` when it is `If cond Exit(id)`: the tail a
/// `REPEAT` lowers to.
fn exit_when(stmt: &Stmt, id: LoopId) -> Option<&Expr> {
    let (cond, then, otherwise) = if_parts(stmt)?;
    (otherwise.is_empty() && is_exit(then, id)).then_some(cond)
}

/// The condition of `stmt` when it is `If cond else Exit(id)`: the head a
/// `WHILE` lowers to.
fn exit_unless(stmt: &Stmt, id: LoopId) -> Option<&Expr> {
    let (cond, then, otherwise) = if_parts(stmt)?;
    (then.is_empty() && is_exit(otherwise, id)).then_some(cond)
}

/// The condition, `then` and `otherwise` of `stmt` when it is an `If`.
fn if_parts(stmt: &Stmt) -> Option<(&Expr, &[Stmt], &[Stmt])> {
    match stmt.kind() {
        StmtKind::If {
            cond,
            then,
            otherwise,
        } => Some((cond, then, otherwise)),
        StmtKind::Assign { .. }
        | StmtKind::AssignBits { .. }
        | StmtKind::AssignStr { .. }
        | StmtKind::Copy { .. }
        | StmtKind::FbCall { .. }
        | StmtKind::Call { .. }
        | StmtKind::Case { .. }
        | StmtKind::For { .. }
        | StmtKind::Loop { .. }
        | StmtKind::Exit(_)
        | StmtKind::Continue(_)
        | StmtKind::Return => None,
    }
}

fn is_exit(stmts: &[Stmt], id: LoopId) -> bool {
    matches!(stmts, [stmt] if matches!(stmt.kind(), StmtKind::Exit(exit) if *exit == id))
}

/// Whether every value a `For` stores in `control` is within its type: the
/// first, every one the body sees, and the one after the last, so the
/// increment needs no truncation. Only constant bounds show it.
fn stays_within(
    types: &ironplc_ir::lowered::TypeTable,
    control: &LoweredPlace,
    from: &Expr,
    to: &Expr,
    step: Const,
) -> bool {
    let ty = to.ty();
    let integer = |expr: &Expr| constant(expr)?.integer_value(ty);
    let (Some((min, max)), Some(from), Some(to), Some(step)) = (
        types.value_range(control.ty()),
        integer(from),
        integer(to),
        step.integer_value(ty),
    ) else {
        return false;
    };
    let within = |value: i128| (min..=max).contains(&value);
    within(from) && within(to) && within(to + step)
}

/// The operation type of `expr`.
fn operation(expr: &Expr) -> Result<OpType, Diagnostic> {
    op_type(expr.ty()).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            expr.span().clone(),
            "A lowered expression has no operation type",
        ))
    })
}
