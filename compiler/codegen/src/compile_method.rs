//! Compilation of `METHOD` declarations (OOP extension, ADR-0041 Phase 1
//! static dispatch).
//!
//! A method shares its owning function block type's field scratch region
//! (see `UserFbTypeInfo::var_offset`/`num_fields` in `compile.rs`) for
//! `self` access, and gets its own additional, non-shared param/local
//! scratch region allocated immediately after -- see the module doc on
//! `METHOD_CALL` in `ironplc_container::opcode` for the full calling
//! convention. Structurally this mirrors `compile_fn::compile_user_function`
//! closely; the differences are exactly the parts of that convention.

use ironplc_dsl::common::MethodDeclaration;
use ironplc_dsl::diagnostic::Diagnostic;

use super::compile::{finalize_function, CompileContext, CompiledFunction, CurrentFunctionReturn};
use super::compile_expr::emit_load_var;
use super::compile_setup::emit_clear_result_slot;
use super::compile_stmt::compile_statements;
use super::layout::MethodLayout;
use crate::emit::Emitter;

/// Compiles the body of `method`, which `layout` lays out, in the method's
/// scope; the caller's is current again once the body is compiled.
pub(crate) fn compile_user_method(
    method: &MethodDeclaration,
    layout: MethodLayout,
    ctx: &mut CompileContext,
) -> Result<CompiledFunction, Diagnostic> {
    let MethodLayout {
        function_id,
        scope,
        num_locals,
        num_params,
        return_var_index,
        return_op_type,
        has_return_value,
    } = layout;
    let return_id = method.name.clone();
    let caller = std::mem::replace(&mut ctx.scope, scope);

    let mut method_emitter = Emitter::new();

    crate::lowered::emit_prologue(
        &mut method_emitter,
        ctx,
        &method.variables,
        has_return_value.then_some(&return_id),
    )?;
    if !has_return_value {
        // No lowered variable holds the result slot of a method that
        // returns nothing, so it is cleared as this backend's own setup.
        emit_clear_result_slot(&mut method_emitter, ctx, return_var_index, return_op_type);
    }

    let body = ironplc_dsl::textual::Statements {
        body: method.body.clone(),
    };

    let saved_return_ctx = ctx.current_function_return.take();
    ctx.current_function_return = has_return_value.then_some(CurrentFunctionReturn::Scalar {
        var_index: return_var_index,
        op_type: return_op_type,
    });

    let saved_current_fn = ctx.current_function_id.take();
    ctx.current_function_id = Some(function_id);

    compile_statements(&mut method_emitter, ctx, &body)?;

    ctx.current_function_id = saved_current_fn;
    ctx.current_function_return = saved_return_ctx;

    if has_return_value {
        emit_load_var(&mut method_emitter, return_var_index, return_op_type);
        method_emitter.emit_ret();
    } else {
        method_emitter.emit_ret_void();
    }

    let finalized = finalize_function(&mut method_emitter, ctx)?;

    // The most a call of the method adds to its caller's stack.
    ctx.function_stack_depths
        .insert(function_id, finalized.max_stack_depth);

    let name = crate::lowered::pou_name(ctx, ctx.scope).unwrap_or_else(|| method.name.to_string());
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
