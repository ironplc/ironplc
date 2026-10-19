//! User-defined function and function block compilation.
//!
//! Contains compilation of FUNCTION and FUNCTION_BLOCK declarations,
//! including variable setup, body compilation, and metadata registration.
//! Separated from compile.rs to keep module sizes within the 1000-line guideline.

use ironplc_container::debug_section::{iec_type_tag, VarNameEntry};
use ironplc_container::{ContainerBuilder, FunctionId, VarIndex};
use ironplc_dsl::common::{
    FunctionBlockDeclaration, FunctionDeclaration, InitialValueAssignmentKind, StringInitializer,
    VarDecl, VariableType,
};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};

use ironplc_analyzer::TypeEnvironment;

use super::compile::{
    char_width_for_string_type, finalize_function, string_region_size, CompileContext,
    CompiledFunction, CurrentBody, CurrentFunctionReturn, ParamPassing, StringParamInfo,
    StringVarInfo, VarTypeInfo,
};
use super::compile_expr::emit_load_var;
use super::compile_setup::emit_function_local_prologue;
use super::compile_stmt::{compile_body, compile_statements, resolve_string_max_length};
use super::layout::{debug_type_for_decl, map_var_section, FunctionLayout};
use super::scope::Scope;
use super::type_info::{decl_type_info, resolve_type_name};
use crate::emit::Emitter;

/// Reserves the run of the data region the string variable `id`, which
/// `string_init` declares, is held in, and records it in `ctx.string_vars`.
/// Every declared string of a function, function block or method is held
/// in a run of its own, sized to its declared capacity.
#[track_caller]
pub(crate) fn register_string_variable(
    ctx: &mut CompileContext,
    id: &Id,
    string_init: &StringInitializer,
) -> Result<(), Diagnostic> {
    let max_length = resolve_string_max_length(string_init)?;
    let char_width = char_width_for_string_type(&string_init.width);

    let total_bytes = string_region_size(max_length, char_width);
    let data_offset = crate::data_region::reserve(ctx, total_bytes, &string_init.span())?;

    if max_length > ctx.max_string_capacity {
        ctx.max_string_capacity = max_length;
    }

    ctx.string_vars.insert(
        id.clone(),
        StringVarInfo {
            data_offset,
            max_length,
            char_width,
        },
    );
    Ok(())
}

/// How a call site passes an argument to the input parameter `decl`
/// declares, once the callee's variables have their storage: by reference
/// to a `VAR_IN_OUT`, copied into the parameter's run of the data region for
/// a string, and by value otherwise.
pub(crate) fn param_passing(ctx: &CompileContext, decl: &VarDecl) -> ParamPassing {
    if decl.var_type == VariableType::InOut {
        return ParamPassing::Reference;
    }
    let string = match &decl.initializer {
        InitialValueAssignmentKind::String(_) => decl
            .identifier
            .symbolic_id()
            .and_then(|id| ctx.string_var(id)),
        _ => None,
    };
    string.map_or(ParamPassing::Value, |info| {
        ParamPassing::String(StringParamInfo {
            data_offset: info.data_offset,
            max_length: info.max_length,
            char_width: info.char_width,
        })
    })
}

/// Records a debug [`VarNameEntry`] for a function- or FB-local variable
/// (parameter or local) owned by `function_id`. Mirrors the program/global
/// emission in `layout::storage::assign_variables`, but tags the entry with
/// the owning function so a debugger can filter a frame's variables to the
/// current stack frame (plus globals).
pub(crate) fn push_local_var_name(
    ctx: &mut CompileContext,
    var_index: VarIndex,
    function_id: FunctionId,
    decl: &VarDecl,
    id: &Id,
    types: &TypeEnvironment,
) {
    let (tag, type_name) = debug_type_for_decl(decl, types);
    // A VAR_IN_OUT parameter's slot holds a reference to the caller's
    // variable, so it is described as one rather than as the value.
    let (tag, type_name) = if decl.var_type == VariableType::InOut {
        (iec_type_tag::OTHER, format!("REF_TO {type_name}"))
    } else {
        (tag, type_name)
    };
    ctx.debug_var_names.push(VarNameEntry {
        var_index,
        function_id,
        var_section: map_var_section(&decl.var_type),
        iec_type_tag: tag,
        name: id.to_string(),
        type_name,
    });
}

/// Returns the type of the value a function's `VAR_IN_OUT` parameter
/// refers to.
///
/// A `VAR_IN_OUT` parameter is passed as a reference to a single slot, so
/// only elementary types are supported. A string, array, structure,
/// reference or function block instance lives in the data region (or is
/// itself a reference), and passing one by reference is not implemented.
pub(crate) fn in_out_value_type(decl: &VarDecl) -> Result<VarTypeInfo, Diagnostic> {
    if let InitialValueAssignmentKind::Simple(simple) = &decl.initializer {
        if let Some(type_info) = resolve_type_name(&simple.type_name.name) {
            return Ok(type_info);
        }
    }
    Err(Diagnostic::not_implemented(Label::span(
        decl.identifier.span(),
        "VAR_IN_OUT parameter of a type other than an elementary type",
    )))
}

/// Compiles the body of the user function `func_decl`, which `layout` lays
/// out, against the function's own variable mappings; the program's are put
/// back once the body is compiled.
pub(crate) fn compile_user_function(
    func_decl: &FunctionDeclaration,
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
    let program_scope = ctx.swap_scope(scope);

    let mut func_emitter = Emitter::new();

    // Emit initialization prologue: IEC 61131-3 functions are stateless, so
    // the locals the analyzer flags, and the result, start again at their
    // resolved values on every call. The flat variable table (ADR-0046)
    // retains stale values between calls otherwise.
    emit_function_local_prologue(
        &mut func_emitter,
        ctx,
        &func_decl.variables,
        Some(&func_decl.result),
        &func_decl.name,
        return_var_index,
        return_op_type,
    )?;

    let body = ironplc_dsl::textual::Statements {
        body: func_decl.body.clone(),
    };

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

    let saved_body = ctx
        .current_body
        .replace(CurrentBody::Pou(func_decl.name.clone()));
    compile_statements(&mut func_emitter, ctx, &body)?;
    ctx.current_body = saved_body;

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

    // Restore the program's variable mappings.
    ctx.swap_scope(program_scope);

    Ok(CompiledFunction {
        function_id,
        bytecode: finalized.bytecode,
        max_stack_depth: finalized.max_stack_depth,
        max_temp_depth: finalized.max_temp_depth,
        num_locals,
        num_params,
        name: func_decl.name.lower_case().to_string(),
        line_map: finalized.line_map,
    })
}

/// Compiles a single user-defined function block body.
///
/// Saves and restores the context's variable mappings so that FB-local
/// variables don't interfere with the program's namespace.
///
/// All FB fields (VAR_INPUT, VAR_OUTPUT, VAR) are mapped to contiguous
/// variable table slots starting at `var_offset`, in declaration order.
/// The VM's copy-in/copy-out logic mirrors this ordering.
pub(crate) fn compile_user_function_block(
    fb_decl: &FunctionBlockDeclaration,
    function_id: FunctionId,
    var_offset: u16,
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    types: &TypeEnvironment,
    num_globals: u16,
) -> Result<(CompiledFunction, Scope), Diagnostic> {
    let fb_name = fb_decl.name.name.to_string().to_uppercase();

    // Collect fields in a stable order: inputs first, then outputs, then locals.
    // This matches the data-region layout used by the VM's copy-in/copy-out.
    let mut field_decls: Vec<&VarDecl> = Vec::new();
    for decl in &fb_decl.variables {
        if decl.var_type == VariableType::Input {
            field_decls.push(decl);
        }
    }
    for decl in &fb_decl.variables {
        if decl.var_type == VariableType::Output {
            field_decls.push(decl);
        }
    }
    for decl in &fb_decl.variables {
        if decl.var_type == VariableType::Var {
            field_decls.push(decl);
        }
    }

    // Swap the program's variable mappings for the FB type's own.
    let program_scope = ctx.swap_scope(Scope::default());
    ctx.swap_scope(Scope::for_fb_body(&program_scope, num_globals));

    // Assign variable slots for all FB fields, in the same order as field_decls.
    let mut current_index = VarIndex::new(var_offset);
    for decl in &field_decls {
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(id.clone(), current_index);
            push_local_var_name(ctx, current_index, function_id, decl, id, types);
            // A field operates at its declared type, as a function's local
            // does.
            if let Some(type_info) = decl_type_info(ctx, decl) {
                ctx.var_types.insert(id.clone(), type_info);
            }
            match &decl.initializer {
                InitialValueAssignmentKind::Reference(ref_init) => {
                    crate::compile_reference::register_reference_variable(
                        ctx,
                        builder,
                        types,
                        id,
                        current_index,
                        ref_init,
                    )?;
                }
                InitialValueAssignmentKind::String(string_init) => {
                    register_string_variable(ctx, id, string_init)?;
                }
                _ => {}
            }
            current_index = VarIndex::new(current_index.raw() + 1);
        }
    }

    let num_locals = current_index.raw() - var_offset;

    // Compile the FB body.
    // Mark this FB's body as the current caller so nested CALL / FB_CALL
    // emissions record edges into ctx.call_graph (mirrors the same setup
    // in compile_user_function and the SCAN-body site).
    let saved_current_fn = ctx.current_function_id.take();
    ctx.current_function_id = Some(function_id);

    let mut fb_emitter = Emitter::new();
    let saved_body = ctx
        .current_body
        .replace(CurrentBody::Pou(fb_decl.name.name.clone()));
    compile_body(&mut fb_emitter, ctx, &fb_decl.body, &fb_decl.name.span())?;
    ctx.current_body = saved_body;
    fb_emitter.emit_ret_void();

    ctx.current_function_id = saved_current_fn;

    let finalized = finalize_function(&mut fb_emitter, ctx)?;

    // Note: `ctx`'s variable mappings are intentionally NOT restored
    // here (unlike `compile_user_function`). This type's METHODs (OOP
    // extension, ADR-0041 Phase 1) are compiled next, right after this
    // function returns, and need `ctx.variables` to still hold this
    // type's field mappings for `self` access. The program-level scope is
    // returned instead, and the caller (`compile_program_with_functions`)
    // swaps it back in once both this body and its methods are compiled.

    Ok((
        CompiledFunction {
            function_id,
            bytecode: finalized.bytecode,
            max_stack_depth: finalized.max_stack_depth,
            max_temp_depth: finalized.max_temp_depth,
            num_locals,
            num_params: 0,
            name: fb_name,
            line_map: finalized.line_map,
        },
        program_scope,
    ))
}
