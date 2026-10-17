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

use std::collections::HashMap;

use ironplc_container::{FunctionId, VarIndex};
use ironplc_dsl::common::{
    FunctionBlockDeclaration, FunctionReturnType, InitialValueAssignmentKind, MethodDeclaration,
    TypeName, VariableType,
};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::textual::{MethodCall, MethodReceiver, ParamAssignmentKind};

use ironplc_analyzer::TypeEnvironment;

use super::compile::{
    finalize_function, CompileContext, CompiledFunction, CurrentBody, CurrentFunctionReturn,
    ParamPassing, StringVarInfo, DEFAULT_OP_TYPE,
};
use super::compile_call_output::{store_outputs, CallOutputs};
use super::compile_expr::{compile_expr, emit_load_var};
use super::compile_fn::{param_passing, register_string_variable};
use super::compile_setup::emit_function_local_prologue;
use super::compile_stmt::compile_statements;
use super::compile_user_call::compile_string_arg;
use super::type_info::{decl_type_info, named_type_info};
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
        let saved_string_vars = ctx.string_vars.clone();

        let compiled_method = compile_user_method(
            &fb_decl.name.name,
            method,
            function_id,
            field_var_off,
            param_var_off,
            ctx,
            types,
        );

        ctx.variables = saved_variables;
        ctx.var_types = saved_var_types;
        ctx.string_vars = saved_string_vars;

        let CompiledMethod {
            compiled: result,
            params,
            outputs,
            string_outputs,
        } = compiled_method?;

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
            info.outputs = outputs;
            info.string_outputs = string_outputs;
            for param in &mut info.params {
                if let Some(passing) = params.get(&param.name) {
                    param.passing = passing.clone();
                }
            }
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
    block: &Id,
    method: &MethodDeclaration,
    function_id: FunctionId,
    field_var_off: u16,
    param_var_off: VarIndex,
    ctx: &mut CompileContext,
    types: &TypeEnvironment,
) -> Result<CompiledMethod, Diagnostic> {
    let mut current_index = param_var_off;
    let mut num_params: u16 = 0;

    // First pass: input-compatible parameters (VAR_INPUT and VAR_IN_OUT).
    for decl in method.all_variables() {
        if !decl.var_type.is_input_compatible() {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(id.clone(), current_index);
            // A parameter operates at its declared type, whatever kind of
            // declaration declares it, as a function's does.
            if let Some(type_info) = decl_type_info(ctx, decl) {
                ctx.var_types.insert(id.clone(), type_info);
            }
            // A call copies a string argument into the parameter's run.
            if let InitialValueAssignmentKind::String(string_init) = &decl.initializer {
                register_string_variable(ctx, id, string_init)?;
            }
            current_index = VarIndex::new(current_index.raw() + 1);
            num_params += 1;
        }
    }

    // Second pass: local variables (VAR, VAR_TEMP) and outputs (VAR_OUTPUT),
    // which start again on every call as a local does.
    let mut outputs = HashMap::new();
    for decl in method.all_variables() {
        if !decl.var_type.is_local() && decl.var_type != VariableType::Output {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(id.clone(), current_index);
            if decl.var_type == VariableType::Output {
                outputs.insert(id.clone(), current_index);
            }
            // A local operates at its declared type, as a parameter does.
            if let Some(type_info) = decl_type_info(ctx, decl) {
                ctx.var_types.insert(id.clone(), type_info);
            }
            // The prologue initializes a string local or output's run at
            // every call.
            if let InitialValueAssignmentKind::String(string_init) = &decl.initializer {
                register_string_variable(ctx, id, string_init)?;
            }
            current_index = VarIndex::new(current_index.raw() + 1);
        }
    }

    // Always allocate a return-value slot, even for a method with no
    // return type: `emit_function_local_prologue` clears it, so a void
    // method gets one harmless unused slot rather than special-casing the
    // prologue call.
    let return_var_index = current_index;
    let return_id = method.name.clone();
    let has_return_value = method.return_type.is_some();

    let return_op_type = match &method.return_type {
        Some(FunctionReturnType::Named(type_name)) => named_type_info(ctx, types, type_name)
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
            if let Some(type_info) = named_type_info(ctx, types, type_name) {
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

    let saved_body = ctx.current_body.replace(CurrentBody::Method {
        block: block.clone(),
        method: method.name.clone(),
    });
    compile_statements(&mut method_emitter, ctx, &body)?;
    ctx.current_body = saved_body;

    ctx.current_function_id = saved_current_fn;
    ctx.current_function_return = saved_return_ctx;

    if has_return_value {
        emit_load_var(&mut method_emitter, return_var_index, return_op_type);
        method_emitter.emit_ret();
    } else {
        method_emitter.emit_ret_void();
    }

    let finalized = finalize_function(&mut method_emitter, ctx)?;

    let params = method
        .all_variables()
        .filter(|decl| decl.var_type.is_input_compatible())
        .filter_map(|decl| {
            Some((
                decl.identifier.symbolic_id()?.clone(),
                param_passing(ctx, decl),
            ))
        })
        .collect();
    let string_outputs = outputs
        .keys()
        .filter_map(|id| Some((id.clone(), ctx.string_vars.get(id)?.clone())))
        .collect();

    Ok(CompiledMethod {
        compiled: CompiledFunction {
            function_id,
            bytecode: finalized.bytecode,
            max_stack_depth: finalized.max_stack_depth,
            max_temp_depth: finalized.max_temp_depth,
            num_locals,
            num_params,
            name: method.name.to_string(),
            line_map: finalized.line_map,
        },
        params,
        outputs,
        string_outputs,
    })
}

/// A compiled method, how a call passes each of its input parameters, the
/// slot of each of its outputs, and where each string output is.
struct CompiledMethod {
    compiled: CompiledFunction,
    params: HashMap<Id, ParamPassing>,
    outputs: HashMap<Id, VarIndex>,
    string_outputs: HashMap<Id, StringVarInfo>,
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
    // `THIS^.M()` / `SUPER^.M()` receivers parse but are rejected earlier by
    // `rule_method_call_declared`, so codegen only ever sees a named instance.
    let instance = match &call.receiver {
        MethodReceiver::Instance(id) => id,
        MethodReceiver::SelfRef(self_ref) => {
            return Err(Diagnostic::internal_error_at(Label::span(
                self_ref.span(),
                "THIS or SUPER receiver of a method call",
            )))
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
    let fb_type_info = ctx.user_fb_types.get(&declaring).ok_or_else(|| {
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
    let max_stack_depth = method_info.max_stack_depth;
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
