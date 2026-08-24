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

use ironplc_container::{ContainerBuilder, FunctionId, VarIndex};
use ironplc_dsl::common::{
    FunctionBlockDeclaration, FunctionReturnType, InitialValueAssignmentKind, MethodDeclaration,
};
use ironplc_dsl::diagnostic::Diagnostic;

use ironplc_analyzer::TypeEnvironment;

use super::compile::{
    finalize_function, CompileContext, CompiledFunction, CurrentFunctionReturn, DEFAULT_OP_TYPE,
};
use super::compile_expr::emit_load_var;
use super::compile_setup::{emit_function_local_prologue, resolve_type_name};
use super::compile_stmt::compile_statements;
use crate::emit::Emitter;

/// Compiles every `METHOD` declared on `fb_decl`, in declaration order.
///
/// Must run after `fb_decl`'s own body has been compiled (so `ctx.variables`
/// still holds that type's field name -> `VarIndex` mappings, at
/// `field_var_off`) and before the caller restores `ctx.variables` back to
/// the program-level view. `var_offset` is threaded through and advanced
/// past each method's own param/local region as it's allocated, exactly
/// like `compile_user_function`'s `var_offset` in the outer driver.
pub(crate) fn compile_user_fb_methods(
    fb_decl: &FunctionBlockDeclaration,
    fb_name: &str,
    field_var_off: u16,
    var_offset: &mut VarIndex,
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    types: &TypeEnvironment,
) -> Result<Vec<CompiledFunction>, Diagnostic> {
    let mut compiled = Vec::new();

    for method in &fb_decl.methods {
        let method_name = method.name.to_string().to_lowercase();
        let function_id = ctx.user_fb_types[fb_name].methods[&method_name].function_id;
        let param_var_off = *var_offset;

        let result = compile_user_method(
            method,
            function_id,
            field_var_off,
            param_var_off,
            ctx,
            builder,
            types,
        )?;

        // `result.num_locals` spans from `field_var_off` (not
        // `param_var_off`) through this method's own locals -- see the
        // comment in `compile_user_method`. Advancing `var_offset` by
        // that full span means a type with N methods reserves
        // N * num_fields more table slots than strictly necessary (each
        // method's span re-covers the shared field region). Harmless
        // (a few extra flat-table slots, never aliased or read), just
        // not maximally compact; fine to tighten later if it matters.
        *var_offset = VarIndex::new(var_offset.raw() + result.num_locals);

        if let Some(info) = ctx
            .user_fb_types
            .get_mut(fb_name)
            .and_then(|fb| fb.methods.get_mut(&method_name))
        {
            info.param_var_off = param_var_off.raw();
            info.max_stack_depth = result.max_stack_depth;
        }

        compiled.push(result);
    }

    Ok(compiled)
}

/// Compiles a single method body. `param_var_off` is where this method's
/// own params/locals/return slot start; `field_var_off` is the owning
/// type's field region start (already populated in `ctx.variables` by the
/// caller, so field references inside the body resolve normally through
/// the existing variable-lookup machinery -- no special-casing needed
/// here beyond not touching those entries).
fn compile_user_method(
    method: &MethodDeclaration,
    function_id: FunctionId,
    field_var_off: u16,
    param_var_off: VarIndex,
    ctx: &mut CompileContext,
    _builder: &mut ContainerBuilder,
    _types: &TypeEnvironment,
) -> Result<CompiledFunction, Diagnostic> {
    let mut current_index = param_var_off;
    let mut num_params: u16 = 0;

    // First pass: input-compatible parameters (VAR_INPUT and VAR_IN_OUT).
    for decl in &method.variables {
        if !decl.var_type.is_input_compatible() {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(id.clone(), current_index);
            if let InitialValueAssignmentKind::Simple(simple) = &decl.initializer {
                if let Some(type_info) = resolve_type_name(&simple.type_name.name) {
                    ctx.var_types.insert(id.clone(), type_info);
                }
            }
            current_index = VarIndex::new(current_index.raw() + 1);
            num_params += 1;
        }
    }

    // Second pass: local variables (VAR, VAR_TEMP).
    for decl in &method.variables {
        if !decl.var_type.is_local() {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(id.clone(), current_index);
            if let InitialValueAssignmentKind::Simple(simple) = &decl.initializer {
                if let Some(type_info) = resolve_type_name(&simple.type_name.name) {
                    ctx.var_types.insert(id.clone(), type_info);
                }
            }
            current_index = VarIndex::new(current_index.raw() + 1);
        }
    }

    // Always allocate a return-value slot, even for a method with no
    // return type: `emit_function_local_prologue` unconditionally
    // zero-initializes "the return variable", so a void method gets one
    // harmless unused slot rather than special-casing the prologue call.
    let return_var_index = current_index;
    let return_id = method.name.clone();
    let has_return_value = method.return_type.is_some();

    let return_op_type = match &method.return_type {
        Some(FunctionReturnType::Named(type_name)) => resolve_type_name(&type_name.name)
            .map(|info| (info.op_width, info.signedness))
            .unwrap_or(DEFAULT_OP_TYPE),
        Some(FunctionReturnType::String(_)) | Some(FunctionReturnType::WString(_)) => {
            // STRING/WSTRING method returns aren't implemented in this
            // slice -- see specs/plans/2026-08-12-oop-method-declarations-static-dispatch.md.
            return Err(Diagnostic::todo(file!(), line!()));
        }
        None => DEFAULT_OP_TYPE,
    };
    current_index = VarIndex::new(current_index.raw() + 1);

    // Reported num_locals spans from the *type's field region* (not just
    // this method's own params/locals) through the end of this method's
    // own locals: the VM pushes a Frame with `instance_offset:
    // field_var_off, instance_count: <this value>`, so it must cover
    // both the field range (for `self` access) and this method's own
    // range in one contiguous bounds-check window.
    let num_locals = current_index.raw() - field_var_off;

    let mut method_emitter = Emitter::new();

    emit_function_local_prologue(
        &mut method_emitter,
        ctx,
        &method.variables,
        &return_id,
        return_var_index,
        return_op_type,
    )?;

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

    let finalized = finalize_function(&mut method_emitter, ctx);

    Ok(CompiledFunction {
        function_id,
        bytecode: finalized.bytecode,
        max_stack_depth: finalized.max_stack_depth,
        num_locals,
        num_params,
        name: method.name.to_string(),
        line_map: finalized.line_map,
    })
}
