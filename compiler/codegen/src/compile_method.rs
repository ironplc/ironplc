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

use ironplc_ir::lowered::{Nullability, PouKind, Program, TypeKind};
use ironplc_ir::{Diagnostic, Label};

use super::compile::{finalize_function, CompileContext, CompiledFunction, CurrentFunctionReturn};
use super::compile_expr::emit_load_var;
use super::compile_setup::emit_clear_result_slot;
use super::layout::MethodLayout;
use crate::emit::Emitter;

/// Rejects a method of `program` whose result is a `STRING` or a `WSTRING`,
/// which this backend does not compile yet (P9999).
pub(crate) fn reject_string_results(program: &Program) -> Result<(), Diagnostic> {
    let types = program.types();
    for pou in program.pous().ids() {
        if program.parts(pou).map(|parts| parts.kind) != Some(PouKind::Method) {
            continue;
        }
        let Some(result) = program
            .result(pou)
            .and_then(|var| program.variables().get(var))
        else {
            continue;
        };
        // A result returned through a parameter is a reference to the
        // caller's place.
        let ty = match types.reference(result.ty()) {
            Some((target, Nullability::NonNull)) => target,
            Some((_, Nullability::Nullable)) | None => result.ty(),
        };
        if matches!(types.kind(ty), Some(TypeKind::String(_))) {
            return Err(Diagnostic::not_implemented(Label::span(
                result.name().span(),
                "STRING return type of a METHOD",
            )));
        }
    }
    Ok(())
}

/// Compiles the body of the method `layout` lays out, in the method's scope;
/// the caller's is current again once the body is compiled.
pub(crate) fn compile_user_method(
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
    let caller = std::mem::replace(&mut ctx.scope, scope);

    let mut method_emitter = Emitter::new();

    crate::body::emit_prologue(&mut method_emitter, ctx)?;
    if !has_return_value {
        // No lowered variable holds the result slot of a method that
        // returns nothing, so it is cleared as this backend's own setup.
        emit_clear_result_slot(&mut method_emitter, ctx, return_var_index, return_op_type);
    }

    let saved_return_ctx = ctx.current_function_return.take();
    ctx.current_function_return = has_return_value.then_some(CurrentFunctionReturn::Scalar {
        var_index: return_var_index,
        op_type: return_op_type,
    });

    let saved_current_fn = ctx.current_function_id.take();
    ctx.current_function_id = Some(function_id);

    crate::body::emit_body(&mut method_emitter, ctx)?;

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

    let name = crate::body::pou_name(ctx, ctx.scope)?;
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
