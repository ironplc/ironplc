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
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{Expr, MethodCall, MethodReceiver, ParamAssignmentKind};

use ironplc_analyzer::TypeEnvironment;

use super::compile::{
    finalize_function, CompileContext, CompiledFunction, CurrentFunctionReturn, DEFAULT_OP_TYPE,
};
use super::compile_expr::{compile_expr, emit_load_var};
use super::compile_setup::emit_function_local_prologue;
use super::compile_stmt::compile_statements;
use super::type_info::{decl_type_info, resolve_type_name};
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

        // A method's parameters, locals and result name belong to the
        // method, not to the function block or to whichever method is
        // compiled next. `compile_user_function` gets this by starting
        // from an empty map (`compile_fn.rs:78`); a method cannot,
        // because the enclosing type's field mappings have to stay
        // visible for `self` access -- so snapshot and restore instead.
        // Without this, a name declared by one method still resolves in
        // the next, to a `VarIndex` that may fall outside that method's
        // frame bounds window.
        let saved_variables = ctx.variables.clone();
        let saved_var_types = ctx.var_types.clone();

        let compiled_method = compile_user_method(
            method,
            function_id,
            field_var_off,
            param_var_off,
            ctx,
            builder,
            types,
        );

        ctx.variables = saved_variables;
        ctx.var_types = saved_var_types;

        let result = compiled_method?;

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
    for decl in method.all_variables() {
        if !decl.var_type.is_input_compatible() {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(id.clone(), current_index);
            if let InitialValueAssignmentKind::Simple(_) = &decl.initializer {
                if let Some(type_info) = decl_type_info(ctx, decl) {
                    ctx.var_types.insert(id.clone(), type_info);
                }
            }
            current_index = VarIndex::new(current_index.raw() + 1);
            num_params += 1;
        }
    }

    // Second pass: local variables (VAR, VAR_TEMP).
    for decl in method.all_variables() {
        if !decl.var_type.is_local() {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(id.clone(), current_index);
            if let InitialValueAssignmentKind::Simple(_) = &decl.initializer {
                if let Some(type_info) = decl_type_info(ctx, decl) {
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
        Some(FunctionReturnType::String(spec)) | Some(FunctionReturnType::WString(spec)) => {
            // STRING/WSTRING method returns aren't implemented in this
            // slice.
            return Err(Diagnostic::not_implemented(Label::span(
                spec.keyword_span.clone(),
                "STRING return type of a METHOD",
            )));
        }
        None => DEFAULT_OP_TYPE,
    };

    // Bind the method's own name to that slot, as
    // `compile_user_function` does at `compile_fn.rs:258`, so a body
    // that produces its result the standard way -- `GetSpeed := speed`
    // -- resolves the name to the slot the epilogue loads before `RET`.
    // Only when the method declares a return type: one without a return
    // type has no result to assign, and the analyzer rejects the
    // assignment rather than letting it reach here.
    if has_return_value {
        ctx.variables.insert(return_id.clone(), return_var_index);
        if let Some(FunctionReturnType::Named(type_name)) = &method.return_type {
            if let Some(type_info) = resolve_type_name(&type_name.name) {
                ctx.var_types.insert(return_id.clone(), type_info);
            }
        }
    }

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

    let finalized = finalize_function(&mut method_emitter, ctx)?;

    Ok(CompiledFunction {
        function_id,
        bytecode: finalized.bytecode,
        max_stack_depth: finalized.max_stack_depth,
        max_temp_depth: finalized.max_temp_depth,
        num_locals,
        num_params,
        name: method.name.to_string(),
        line_map: finalized.line_map,
    })
}

/// Compiles a method call (`instance.MethodName(args)`, OOP extension,
/// ADR-0041 Phase 1 static dispatch). See `METHOD_CALL`'s doc comment in
/// `ironplc_container::opcode` for the calling convention.
///
/// Scoped to methods declared directly on the instance's own function
/// block type. A method reached only via the instance type's `EXTENDS`
/// chain (already accepted by `rule_method_call_declared` at the
/// semantic-analysis level) is not yet supported here: a derived type's
/// data-region layout doesn't currently reserve storage for a base
/// type's fields at all (`compile_user_function_block`'s field list
/// comes from `fb_decl.variables` only, never flattened with inherited
/// fields), so there is nothing correct to copy-in/copy-out from.
///
/// Leaves the instance reference on the stack with the return value, if
/// the method has one, on top of it, and returns whether it does. The
/// caller decides what to keep.
fn compile_method_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    call: &MethodCall,
) -> Result<bool, Diagnostic> {
    // `THIS^.M()` / `SUPER^.M()` receivers parse but are rejected earlier by
    // `rule_method_call_declared`, so codegen only ever sees a named instance.
    let instance = match &call.receiver {
        MethodReceiver::Instance(id) => id,
        MethodReceiver::SelfRef(self_ref) => {
            return Err(Diagnostic::todo_with_span(self_ref.span()))
        }
    };

    let fb_info = ctx
        .fb_instances
        .get(instance)
        .ok_or_else(|| Diagnostic::todo_with_span(call.span()))?;
    let type_id = fb_info.type_id;
    let var_index = fb_info.var_index;

    let fb_name = ctx
        .user_fb_types
        .iter()
        .find(|(_, info)| info.type_id == type_id)
        .map(|(name, _)| name.clone())
        .ok_or_else(|| Diagnostic::todo_with_span(call.span()))?;
    let fb_type_info = &ctx.user_fb_types[&fb_name];

    let method_name = call.method.to_string().to_lowercase();
    let method_info = fb_type_info
        .methods
        .get(&method_name)
        .ok_or_else(|| Diagnostic::todo_with_span(call.span()))?;

    let function_id = method_info.function_id;
    let field_var_off = ironplc_container::VarIndex::new(fb_type_info.var_offset);
    let num_fields = fb_type_info.num_fields as u8;
    let param_var_off = ironplc_container::VarIndex::new(method_info.param_var_off);
    let num_params = method_info.num_params;
    let param_names_in_order = method_info.param_names_in_order.clone();
    let param_op_types = method_info.param_op_types.clone();
    let has_return_value = method_info.has_return_value;
    let max_stack_depth = method_info.max_stack_depth;

    emitter.emit_fb_load_instance(var_index);

    // Resolve named/positional args to the method's declared VAR_INPUT
    // order (arity and name validity already checked by
    // rule_method_call_declared, so any argument that doesn't line up
    // here indicates a codegen bug, not a user error -- hence `todo`
    // rather than a user-facing diagnostic).
    let mut ordered_args: Vec<Option<&Expr>> = vec![None; param_names_in_order.len()];
    for param in &call.params {
        match param {
            ParamAssignmentKind::PositionalInput(p) => {
                if let Some(slot) = ordered_args.iter_mut().find(|s| s.is_none()) {
                    *slot = Some(&p.expr);
                }
            }
            ParamAssignmentKind::NamedInput(n) => {
                let name = n.name.to_string().to_lowercase();
                if let Some(idx) = param_names_in_order.iter().position(|p| *p == name) {
                    ordered_args[idx] = Some(&n.expr);
                }
            }
            ParamAssignmentKind::Output(_) => {
                // Methods have no VAR_OUTPUT `=>` call syntax in this slice.
            }
        }
    }

    for (i, arg) in ordered_args.iter().enumerate() {
        let expr = arg.ok_or_else(|| Diagnostic::todo_with_span(call.span()))?;
        let op_type = param_op_types.get(i).copied().unwrap_or(DEFAULT_OP_TYPE);
        compile_expr(emitter, ctx, expr, op_type)?;
    }

    // Record a call-graph edge; unlike FB_CALL, a method call is always
    // user-defined (no intrinsic-FB branch), so this is unconditional.
    ctx.record_call_edge(function_id);

    emitter.emit_method_call(
        function_id,
        field_var_off,
        num_fields,
        param_var_off,
        num_params,
        has_return_value,
        max_stack_depth,
    );

    Ok(has_return_value)
}

/// Compiles a method call in expression position: the call, then drops the
/// instance reference from beneath the return value, leaving only the value.
/// `rule_method_call_declared` rejects an expression call to a method
/// without a return type (P4057), so reaching one here is a compiler bug.
pub(crate) fn compile_method_call_expression(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    call: &MethodCall,
) -> Result<(), Diagnostic> {
    let has_return_value = compile_method_call(emitter, ctx, call)?;
    if !has_return_value {
        return Err(Diagnostic::todo_with_span(call.span()));
    }
    emitter.emit_swap();
    emitter.emit_pop();
    Ok(())
}

/// Compiles a method call in statement position: the call, then discards
/// the return value (if any) and the instance reference beneath it.
pub(crate) fn compile_method_call_statement(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    call: &MethodCall,
) -> Result<(), Diagnostic> {
    let has_return_value = compile_method_call(emitter, ctx, call)?;
    if has_return_value {
        emitter.emit_pop();
    }
    emitter.emit_pop();
    Ok(())
}
