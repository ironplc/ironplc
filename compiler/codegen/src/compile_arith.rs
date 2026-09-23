//! The arithmetic operators, in both spellings: the operator expression
//! `a + b` and the function form `ADD(a, b, ...)`.
//!
//! Both spellings compile here so that they cannot diverge: the function
//! form folds its inputs from the left with the same opcode the operator
//! expression emits.

use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{BinaryExpr, Function, Operator};

use super::compile::{CompileContext, OpType};
use super::compile_call::compile_left_fold;
use super::compile_expr::{compile_expr, emit_arithmetic_op};
use crate::emit::Emitter;

/// Compiles the arithmetic operator expression `binary`, leaving the result
/// on the stack.
///
/// Both operands compile at `op_type`, the operation type of the enclosing
/// expression.
pub(crate) fn compile_binary_arith(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    binary: &BinaryExpr,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    compile_expr(emitter, ctx, &binary.left, op_type)?;
    compile_expr(emitter, ctx, &binary.right, op_type)?;
    emit_arithmetic_op(emitter, &binary.op, op_type);
    Ok(())
}

/// Compiles a call to the function form of the arithmetic operator `op`,
/// folding its inputs from the left: `ADD(a, b, c)` is `(a + b) + c`.
pub(crate) fn compile_arith_fold(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    func: &Function,
    op: &Operator,
    op_type: OpType,
) -> Result<(), Diagnostic> {
    compile_left_fold(emitter, ctx, func, op_type, |emitter, op_type| {
        emit_arithmetic_op(emitter, op, op_type)
    })
}
