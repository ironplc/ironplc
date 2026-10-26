//! Emits a lowered in-place call, and a call statement of a method or of a
//! function whose result is a structure.
//!
//! An in-place call is a call of a function or method with outputs, or of a
//! function whose result is a string, and the stores of its outputs
//! (REQ-LOW-lowering-172, REQ-LOW-lowering-179). A method call whose result
//! is not used, `c.M(a, q => x);`, lowers to a `Call` statement, then the
//! store of each output (REQ-LOW-lowering-092). A call of a function whose
//! result is a structure, `p := MK(v, n => k)`, lowers to a `Call`
//! statement, the store of each output, then the copy of the result to its
//! target (REQ-LOW-lowering-186): the call leaves the offset of the
//! function's structure on the stack, the outputs are stored, and
//! `COPY_REGION` copies the structure to the target.
//!
//! Each is emitted where its expression is evaluated (REQ-LOW-codegen-175):
//! the call's inputs, `STR_INIT` of a string result, its `CALL` or
//! `METHOD_CALL`, then the stores of its outputs, with the result on the
//! stack throughout, or dropped after a call statement. No temporary has
//! storage of its own. The result is the value, or the string's buffer, the
//! call leaves on the stack. An output is the callee's own variable, which
//! this backend's layout keeps the output in and which keeps its value until
//! the callee is called again (ADR-0046), so a call passes no reference for
//! it.

use std::collections::HashMap;

use ironplc_container::VarIndex;
use ironplc_ir::lowered::Program;
use ironplc_ir::lowered::{
    Arg, Callee, Expr, ExprKind, InPlaceCall, Place as LoweredPlace, PouId, Stmt, StmtKind,
    StrExprKind, VarId,
};
use ironplc_ir::SourceSpan;
use ironplc_ir::{Diagnostic, Label};

use super::aggregate::{emit_copy_from_stack, reaches_region};
use super::call_args::passes;
use super::expr::{emit_operation, emit_user_call};
use super::method::{emit_method_call, method_site, MethodSite};
use super::place::{resolve, resolve_string};
use super::string::reaches_string;
use super::{
    args_can_emit, callee_function, op_type, operand, operation_can_emit, reaches, same_span,
};
use crate::compile::{CompileContext, StringVarInfo, UserFunctionInfo};
use crate::compile_expr::emit_load_var;
use crate::compile_partial_access::Bits;
use crate::emit::Emitter;

/// A call of a function or method, and the stores of its outputs.
pub(super) struct Call<'s> {
    /// The callee, as this backend's layout knows it.
    target: Target,
    /// The call's arguments for the callee's inputs and in-outs, in order.
    inputs: Vec<&'s Arg>,
    /// Where the callee leaves the output each temporary is passed for.
    temporaries: Vec<(VarId, Output)>,
    /// The stores of the outputs.
    outputs: &'s [Stmt],
    /// The target the result of a call of a function whose result is a
    /// structure is copied to.
    result: Option<&'s LoweredPlace>,
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
    fn outputs(&self) -> &HashMap<VarId, VarIndex> {
        match self {
            Target::Function(function) => &function.outputs,
            Target::Method(site) => &site.outputs,
        }
    }

    /// Where each string output is.
    fn string_outputs(&self) -> &HashMap<VarId, StringVarInfo> {
        match self {
            Target::Function(function) => &function.string_outputs,
            Target::Method(site) => &site.string_outputs,
        }
    }
}

impl Call<'_> {
    /// How many statements a call statement is: the call, the stores of its
    /// outputs and the copy of its result, if any.
    pub(super) fn len(&self) -> usize {
        1 + self.outputs.len() + usize::from(self.result.is_some())
    }

    /// Whether this backend passes the call's inputs and emits the store of
    /// each of its outputs.
    pub(super) fn can_emit(&self, lowering: &Program, ctx: &CompileContext) -> bool {
        let inputs = self.inputs.iter().copied();
        let passed = match &self.target {
            Target::Function(function) => passes(&function.params, inputs),
            Target::Method(site) => site.takes(inputs),
        };
        passed
            && args_can_emit(lowering, ctx, self.inputs.iter().copied())
            && self
                .outputs
                .iter()
                .all(|output| self.output_can_emit(lowering, ctx, output))
            && self
                .result
                .is_none_or(|target| reaches_region(lowering, ctx, target))
    }

    /// Whether this backend emits `stmt`, the store of an output read from
    /// its temporary.
    fn output_can_emit(&self, lowering: &Program, ctx: &CompileContext, stmt: &Stmt) -> bool {
        match stmt.kind() {
            StmtKind::Assign { place, value } => {
                matches!(self.read_output(innermost(value)), Some(Output::Scalar(_)))
                    && operations_can_emit(value)
                    && reaches(lowering, ctx, place, value.ty())
            }
            StmtKind::AssignBits { place, value, .. } => {
                matches!(self.read_output(innermost(value)), Some(Output::Scalar(_)))
                    && operations_can_emit(value)
                    && lowering
                        .types()
                        .operation_type(place.ty())
                        .is_some_and(|ty| reaches(lowering, ctx, place, ty))
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
            StmtKind::Copy { .. }
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

/// The call statement at the head of `stmts`, then the stores of its
/// outputs, all with the call's span: of a method, or of a function whose
/// result is a structure, followed by the copy of the result to its target.
/// `None` when `stmts` does not start with one.
pub(super) fn output_call<'s>(
    lowering: &Program,
    ctx: &CompileContext,
    stmts: &'s [Stmt],
) -> Option<Call<'s>> {
    let [first, rest @ ..] = stmts else {
        return None;
    };
    let StmtKind::Call {
        callee: Callee::User(pou),
        args,
    } = first.kind()
    else {
        return None;
    };
    let mut call = call_target(lowering, ctx, *pou, args)?;
    call.outputs = outputs_of(&call.temporaries, rest, first.span());
    let result = match &call.target {
        Target::Method(_) => None,
        Target::Function(function) => {
            function.return_struct_desc_index?;
            Some(result_copy(
                args,
                &rest[call.outputs.len()..],
                first.span(),
            )?)
        }
    };
    call.result = result;
    Some(call)
}

/// The target of `copy`, the first of `stmts`, when it copies the whole of
/// the temporary `args` passes for a function's result, with the call's
/// span.
fn result_copy<'s>(args: &[Arg], stmts: &'s [Stmt], span: &SourceSpan) -> Option<&'s LoweredPlace> {
    let [Arg::Ref(result), ..] = args else {
        return None;
    };
    let copy = stmts.first()?;
    let StmtKind::Copy { dst, src } = copy.kind() else {
        return None;
    };
    let copies_result = same_span(copy.span(), span) && whole(src)? == whole(result)?;
    copies_result.then_some(dst)
}

/// Whether this backend emits `call`, an in-place call whose value is used:
/// a call of a function that returns a value or of a method with a result,
/// each of whose stores moves one of its outputs.
pub(super) fn in_place_can_emit(
    lowering: &Program,
    ctx: &CompileContext,
    call: &InPlaceCall,
) -> bool {
    in_place(lowering, ctx, call).is_some_and(|emitted| {
        let has_value = match &emitted.target {
            Target::Function(function) => {
                function.return_string_info.is_none() && function.return_struct_desc_index.is_none()
            }
            Target::Method(site) => site.has_return_value,
        };
        has_value && emitted.can_emit(lowering, ctx)
    })
}

/// Whether this backend emits `call`, an in-place call of a function whose
/// result is a string, each of whose stores moves one of its outputs.
pub(super) fn str_in_place_can_emit(
    lowering: &Program,
    ctx: &CompileContext,
    call: &InPlaceCall,
) -> bool {
    in_place(lowering, ctx, call).is_some_and(|emitted| {
        let returns_string = match &emitted.target {
            Target::Function(function) => function.return_string_info.is_some(),
            Target::Method(_) => false,
        };
        returns_string && emitted.can_emit(lowering, ctx)
    })
}

/// `call` as this backend emits it: its callee, its inputs, and its stores,
/// which are each the store of an output.
fn in_place<'s>(
    lowering: &Program,
    ctx: &CompileContext,
    call: &'s InPlaceCall,
) -> Option<Call<'s>> {
    let mut emitted = call_target(lowering, ctx, call.pou(), call.args())?;
    let stores = call.stores();
    if !stores
        .iter()
        .all(|store| assigns_output(&emitted.temporaries, store))
    {
        return None;
    }
    emitted.outputs = stores;
    Some(emitted)
}

/// Emits `call`, an in-place call at `span`, which `in_place_can_emit` or
/// `str_in_place_can_emit` accepted: the call, which leaves its result on
/// the stack, then the stores of its outputs.
pub(super) fn emit_in_place(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Program,
    call: &InPlaceCall,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let emitted = in_place(lowering, ctx, call).ok_or_else(|| no_output(span))?;
    let inputs = emitted.inputs.iter().copied();
    match &emitted.target {
        Target::Function(function) => {
            emit_user_call(emitter, ctx, lowering, function, inputs, span)?;
            emitted.emit_outputs(emitter, ctx, lowering)
        }
        Target::Method(site) => {
            emit_method_call(emitter, ctx, lowering, site, inputs, span)?;
            emitted.emit_outputs(emitter, ctx, lowering)?;
            // Drop the instance from beneath the result.
            emitter.emit_swap();
            emitter.emit_pop();
            Ok(())
        }
    }
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
/// temporary for, with no stores of its outputs yet. A method's first
/// argument, its receiver, is neither, nor is a function's result
/// parameter, whose result the call leaves on the stack.
fn call_target<'s>(
    lowering: &Program,
    ctx: &CompileContext,
    pou: PouId,
    args: &'s [Arg],
) -> Option<Call<'s>> {
    let signature = lowering.pous().function(pou)?;
    let (target, receivers) = match callee_function(ctx, pou) {
        Some(function) => {
            let result_parameter = usize::from(signature.result_parameter().is_some());
            (Target::Function(function), result_parameter)
        }
        None => (Target::Method(method_site(lowering, ctx, pou, args)?), 1),
    };
    let parameters = signature.parameters();
    if parameters.len() != args.len() {
        return None;
    }
    let mut inputs = Vec::with_capacity(args.len());
    let mut temporaries = Vec::new();
    for (parameter, arg) in parameters.iter().zip(args).skip(receivers) {
        let Some(slot) = target.outputs().get(parameter) else {
            inputs.push(arg);
            continue;
        };
        let Arg::Ref(temporary) = arg else {
            return None;
        };
        let output = match target.string_outputs().get(parameter) {
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
        result: None,
    })
}

/// Emits `call`, a call statement at `span`, then the stores of its
/// outputs: of a method, then drops its result, if any, and the instance
/// beneath it; of a function whose result is a structure, then copies the
/// structure the call leaves to its target.
pub(super) fn emit_call_statement(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    lowering: &Program,
    call: &Call<'_>,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let site = match &call.target {
        Target::Method(site) => site,
        Target::Function(function) => {
            // `output_call` makes a statement of a function's call only when
            // its result is a structure, which it copies.
            let (Some(target), Some(desc)) = (call.result, function.return_struct_desc_index)
            else {
                return Err(no_output(span));
            };
            emit_user_call(
                emitter,
                ctx,
                lowering,
                function,
                call.inputs.iter().copied(),
                span,
            )?;
            call.emit_outputs(emitter, ctx, lowering)?;
            return emit_copy_from_stack(emitter, ctx, lowering, target, desc, span);
        }
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
    if site.has_return_value {
        emitter.emit_pop();
    }
    emitter.emit_pop();
    Ok(())
}

impl Call<'_> {
    /// Emits the stores of the call's outputs, each read where the callee
    /// left it.
    fn emit_outputs(
        &self,
        emitter: &mut Emitter,
        ctx: &mut CompileContext,
        lowering: &Program,
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
    lowering: &Program,
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
        StmtKind::AssignBits {
            place,
            shift,
            bits,
            value,
        } => {
            let slot = match call.read_output(innermost(value)) {
                Some(Output::Scalar(slot)) => *slot,
                Some(Output::String(_)) | None => return Err(no_output(span)),
            };
            let at = op_type(innermost(value).ty()).ok_or_else(|| no_output(span))?;
            let storage = resolve(lowering, ctx, place, span)?;
            let width = storage.op_type().0;
            let selection = Bits::new(u32::from(*shift), u32::from(*bits));
            storage.emit_update(emitter, ctx, |emitter, ctx| {
                selection.emit_insert(emitter, ctx, width, |emitter, ctx| {
                    emit_around(emitter, ctx, value, |emitter, _| {
                        emit_load_var(emitter, slot, at);
                        Ok(())
                    })
                })
            })
        }
        StmtKind::Copy { .. }
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
        StmtKind::Assign { value, .. } | StmtKind::AssignBits { value, .. } => {
            output_read(temporaries, innermost(value)).is_some()
        }
        StmtKind::AssignStr { value, .. } => match value.kind() {
            StrExprKind::Read(temporary) => output_of(temporaries, temporary).is_some(),
            StrExprKind::Literal(_)
            | StrExprKind::Call { .. }
            | StrExprKind::InPlaceCall { .. } => false,
        },
        StmtKind::Copy { .. }
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

/// The internal error for an output `can_emit` should have rejected.
fn no_output(span: &SourceSpan) -> Diagnostic {
    Diagnostic::internal_error_at(Label::span(
        span.clone(),
        "An output of a lowered call this backend does not read",
    ))
}
