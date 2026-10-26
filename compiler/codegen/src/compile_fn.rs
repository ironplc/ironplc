//! User-defined function and function block compilation.
//!
//! Contains compilation of the bodies of the lowered program's functions and
//! function blocks. Separated from compile.rs to keep module sizes within
//! the 1000-line guideline.

use ironplc_container::FunctionId;
use ironplc_ir::Diagnostic;

use super::compile::{finalize_function, CompileContext, CompiledFunction, CurrentFunctionReturn};
use super::compile_expr::emit_load_var;
use super::layout::names::ScopeId;
use super::layout::FunctionLayout;
use crate::emit::Emitter;

/// Compiles the body of the user function `layout` lays out, in the
/// function's scope; the caller's is current again once the body is
/// compiled.
pub(crate) fn compile_user_function(
    layout: FunctionLayout,
    ctx: &mut CompileContext,
) -> Result<CompiledFunction, Diagnostic> {
    let FunctionLayout {
        function_id,
        scope,
        num_locals,
        num_params,
        return_var_index,
        return_op_type,
        return_string_info,
    } = layout;
    let caller = std::mem::replace(&mut ctx.scope, scope);

    let mut func_emitter = Emitter::new();

    // Emit initialization prologue: IEC 61131-3 functions are stateless, so
    // the locals the analyzer flags, and the result, start again at their
    // resolved values on every call. The flat variable table (ADR-0046)
    // retains stale values between calls otherwise.
    crate::body::emit_prologue(&mut func_emitter, ctx)?;

    // Make the return descriptor visible to nested `RETURN` statements so an
    // early RETURN produces the function's value before the RET opcode (rather
    // than emitting a bare RET_VOID, which would leave the caller's stack
    // empty and trigger a stack underflow on the assignment).
    let saved_return_ctx = ctx.current_function_return.take();
    ctx.current_function_return = Some(if let Some(ref str_info) = return_string_info {
        CurrentFunctionReturn::String {
            data_offset: str_info.data_offset,
        }
    } else {
        CurrentFunctionReturn::Scalar {
            var_index: return_var_index,
            op_type: return_op_type,
        }
    });

    // Mark this function as the current caller so nested CALL / FB_CALL
    // emissions record edges into ctx.call_graph.
    let saved_current_fn = ctx.current_function_id.take();
    ctx.current_function_id = Some(function_id);

    crate::body::emit_body(&mut func_emitter, ctx)?;

    ctx.current_function_id = saved_current_fn;
    ctx.current_function_return = saved_return_ctx;

    // Load the return value and emit RET.
    if let Some(ref str_info) = return_string_info {
        // For STRING return: load the return string from the data region into
        // a temp buffer, leaving buf_idx on the stack for the caller.
        func_emitter.emit_str_load_var(str_info.data_offset);
    } else {
        emit_load_var(&mut func_emitter, return_var_index, return_op_type);
    }
    func_emitter.emit_ret();

    let finalized = finalize_function(&mut func_emitter, ctx)?;

    // The most a call of the function adds to its caller's stack.
    ctx.function_stack_depths
        .insert(function_id, finalized.max_stack_depth);

    let name = crate::body::pou_name(ctx, ctx.scope)?.to_lowercase();
    ctx.scope = caller;

    Ok(CompiledFunction {
        function_id,
        bytecode: finalized.bytecode,
        max_stack_depth: finalized.max_stack_depth,
        max_temp_depth: finalized.max_temp_depth,
        num_locals,
        num_params,
        name,
        line_map: finalized.line_map,
    })
}

/// Compiles the body of the user function block whose id is `function_id`
/// and whose `num_locals` copy slots the layout gave it, in the block's
/// scope, `scope`; the caller's is current again once the body is compiled.
pub(crate) fn compile_user_function_block(
    function_id: FunctionId,
    scope: ScopeId,
    num_locals: u16,
    ctx: &mut CompileContext,
) -> Result<CompiledFunction, Diagnostic> {
    let caller = std::mem::replace(&mut ctx.scope, scope);
    let fb_name = crate::body::pou_name(ctx, scope)?.to_uppercase();

    // Compile the FB body.
    // Mark this FB's body as the current caller so nested CALL / FB_CALL
    // emissions record edges into ctx.call_graph (mirrors the same setup
    // in compile_user_function and the SCAN-body site).
    let saved_current_fn = ctx.current_function_id.take();
    ctx.current_function_id = Some(function_id);

    let mut fb_emitter = Emitter::new();
    // A `VAR_TEMP` starts again on every call, from the lowered program; the
    // AST gives a function block's none.
    if crate::body::restarts(ctx)? {
        crate::body::emit_prologue(&mut fb_emitter, ctx)?;
    }
    crate::body::emit_body(&mut fb_emitter, ctx)?;
    fb_emitter.emit_ret_void();

    ctx.current_function_id = saved_current_fn;

    let finalized = finalize_function(&mut fb_emitter, ctx)?;

    ctx.scope = caller;

    Ok(CompiledFunction {
        function_id,
        bytecode: finalized.bytecode,
        max_stack_depth: finalized.max_stack_depth,
        max_temp_depth: finalized.max_temp_depth,
        num_locals,
        num_params: 0,
        name: fb_name,
        line_map: finalized.line_map,
    })
}
