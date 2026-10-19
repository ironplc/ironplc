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

use ironplc_dsl::common::{MethodDeclaration, TypeName};
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{MethodCall, MethodReceiver, ParamAssignmentKind};

use super::compile::{
    finalize_function, CompileContext, CompiledFunction, CurrentFunctionReturn, ParamPassing,
};
use super::compile_call_output::{store_outputs, CallOutputs};
use super::compile_expr::{compile_expr, emit_load_var};
use super::compile_setup::emit_function_local_prologue;
use super::compile_stmt::compile_statements;
use super::compile_user_call::compile_string_arg;
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

    emit_function_local_prologue(
        &mut method_emitter,
        ctx,
        &method.variables,
        has_return_value.then_some(&method.result),
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

    // The most a call of the method adds to its caller's stack.
    ctx.function_stack_depths
        .insert(function_id, finalized.max_stack_depth);

    ctx.scope = caller;

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
/// The method is the one the analyzer resolved the call to
/// ([`ironplc_analyzer::MethodOwners`]): the instance's block's own, or the
/// nearest block it `EXTENDS` that declares one. A method a base block
/// declares runs on the instance only when the base block has no fields: a
/// derived block's data-region layout does not yet reserve storage for a
/// base block's fields (`compile_user_function_block`'s field list comes from
/// `fb_decl.variables` only, never flattened with inherited fields), so
/// there is nothing correct to copy in and out. Such a call is reported as
/// not implemented.
///
/// Leaves the instance reference on the stack with the return value, if
/// the method has one, on top of it, and returns whether it does. The
/// caller decides what to keep.
fn compile_method_call(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    call: &MethodCall,
) -> Result<bool, Diagnostic> {
    // `THIS^.M()` / `SUPER^.M()` pass analysis but are not compiled yet.
    let instance = match &call.receiver {
        MethodReceiver::Instance(id) => id,
        MethodReceiver::SelfRef(self_ref) => {
            return Err(Diagnostic::not_implemented(Label::span(
                self_ref.span(),
                format!("method call through {}", self_ref.kind.spelling()),
            )))
        }
    };

    let fb_info = ctx
        .fb_instance(instance)
        .ok_or_else(|| Diagnostic::todo_with_span(call.span()))?;
    let type_id = fb_info.type_id;
    let var_index = fb_info.var_index;

    let fb_name = ctx
        .user_fb_types
        .iter()
        .find(|(_, info)| info.type_id == type_id)
        .map(|(name, _)| name.clone())
        .ok_or_else(|| Diagnostic::todo_with_span(call.span()))?;

    // `rule_method_call_declared` rejects a call no block in the chain
    // declares a method for (P4046).
    let declaring = ctx
        .method_owners
        .declaring_block(&TypeName::from(fb_name.as_str()), &call.method)
        .map(|block| block.to_string().to_uppercase())
        .ok_or_else(|| {
            Diagnostic::internal_error_at(Label::span(
                call.span(),
                "Method call the analyzer resolved to no method",
            ))
        })?;
    let fb_type_info = ctx.user_fb_type(&declaring).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            call.span(),
            "Method of a function block that has no layout",
        ))
    })?;
    if declaring != fb_name && fb_type_info.num_fields > 0 {
        return Err(Diagnostic::not_implemented(Label::span(
            call.span(),
            "Method of a base function block that declares fields, called on an instance of a derived block",
        )));
    }

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
    let has_return_value = method_info.has_return_value;
    let outputs = method_info.outputs.clone();

    // The analyzer wrote the call in normal form (`call-binding.md`), and
    // `rule_method_call_declared` requires every input (P4018), so the call
    // names each parameter once, in declaration order. Anything else is a
    // compiler bug.
    let mut inputs = Vec::with_capacity(call.params.len());
    for param in &call.params {
        match param {
            ParamAssignmentKind::NamedInput(input) => inputs.push(input),
            ParamAssignmentKind::PositionalInput(input) => {
                return Err(Diagnostic::internal_error_at(Label::span(
                    input.expr.span(),
                    "Positional argument of a method call the analyzer did not bind",
                )))
            }
            // Stored in their targets after the call.
            ParamAssignmentKind::Output(_) => {}
        }
    }
    let params = &method_info.params;
    let in_order = inputs.len() == params.len()
        && inputs
            .iter()
            .zip(params)
            .all(|(input, param)| input.name == param.name);
    if !in_order {
        return Err(Diagnostic::internal_error_at(Label::span(
            call.span(),
            "Method call whose arguments are not in normal form",
        )));
    }
    // A VAR_IN_OUT is passed by reference, which this route cannot do
    // (`var-in-out-parameters.md`); lowering does.
    if let Some((input, _)) = inputs
        .iter()
        .zip(params)
        .find(|(_, param)| matches!(param.passing, ParamPassing::Reference))
    {
        return Err(Diagnostic::not_implemented(Label::span(
            input.name.span(),
            "VAR_IN_OUT parameter of a METHOD",
        )));
    }
    let passings: Vec<ParamPassing> = params.iter().map(|param| param.passing.clone()).collect();

    emitter.emit_fb_load_instance(var_index);

    for (input, passing) in inputs.iter().zip(passings) {
        match passing {
            ParamPassing::String(param) => {
                compile_string_arg(emitter, ctx, &input.expr, &param, &call.span())?;
            }
            // A reference was refused above.
            ParamPassing::Value | ParamPassing::Reference => {
                compile_expr(emitter, ctx, &input.expr)?;
            }
        }
    }

    // Record a call-graph edge; unlike FB_CALL, a method call is always
    // user-defined (no intrinsic-FB branch), so this is unconditional.
    ctx.record_call_edge(function_id);

    let max_stack_depth = ctx.function_stack_depth(function_id, &call.span())?;
    emitter.emit_method_call(
        function_id,
        field_var_off,
        num_fields,
        param_var_off,
        num_params,
        has_return_value,
        max_stack_depth,
    );

    // A method leaves its outputs in its own output variables; storing them
    // leaves the instance and the return value on the stack.
    store_outputs(emitter, ctx, CallOutputs::Variables(outputs), &call.params)?;

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
    // `rule_method_call_declared` rejects an expression-position call to a
    // method without a return value (P4057).
    if !has_return_value {
        return Err(Diagnostic::internal_error_at(Label::span(
            call.span(),
            "Method without a return value used as an expression",
        )));
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
