//! User-defined function and function block compilation.
//!
//! Contains compilation of FUNCTION and FUNCTION_BLOCK declarations,
//! including variable setup, body compilation, and metadata registration.
//! Separated from compile.rs to keep module sizes within the 1000-line guideline.

use ironplc_container::{ContainerBuilder, VarIndex, STRING_HEADER_BYTES};
use ironplc_dsl::common::{
    FunctionBlockDeclaration, FunctionDeclaration, FunctionReturnType, InitialValueAssignmentKind,
    VarDecl, VariableType,
};
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

use ironplc_analyzer::{FunctionEnvironment, TypeEnvironment};

use super::compile::{
    CompileContext, CompiledFunction, OpType, OpWidth, Signedness, StringParamInfo,
    StringReturnInfo, StringVarInfo, UserFunctionInfo, VarTypeInfo, DEFAULT_OP_TYPE,
};
use super::compile_expr::emit_load_var;
use super::compile_setup::{emit_function_local_prologue, resolve_type_name};
use super::compile_stmt::{
    compile_body, compile_statements, resolve_string_max_length, resolve_string_spec_max_length,
};
use crate::emit::Emitter;

/// Compiles a single user-defined function body.
///
/// Saves and restores the context's variable mappings so that function-local
/// variables don't interfere with the program's namespace.
///
/// Variable layout within the function's region (starting at `var_offset`):
/// - Input parameters (in declaration order)
/// - Local variables (VAR)
/// - Return value variable (named same as function)
#[allow(clippy::too_many_arguments)]
pub(crate) fn compile_user_function(
    func_decl: &FunctionDeclaration,
    function_id: u16,
    var_offset: VarIndex,
    ctx: &mut CompileContext,
    functions: &FunctionEnvironment,
    builder: &mut ContainerBuilder,
    types: &TypeEnvironment,
    num_globals: u16,
) -> Result<CompiledFunction, Diagnostic> {
    // Save the program's variable mappings.
    let saved_variables = std::mem::take(&mut ctx.variables);
    let saved_var_types = std::mem::take(&mut ctx.var_types);
    let saved_string_vars = std::mem::take(&mut ctx.string_vars);
    let saved_array_vars = std::mem::take(&mut ctx.array_vars);
    let saved_struct_vars = std::mem::take(&mut ctx.struct_vars);

    // Re-insert global variable mappings so the function body can access them.
    for (id, index) in &saved_variables {
        if index.raw() < num_globals {
            ctx.variables.insert(id.clone(), *index);
        }
    }
    for (id, info) in &saved_var_types {
        if saved_variables
            .get(id)
            .is_some_and(|i| i.raw() < num_globals)
        {
            ctx.var_types.insert(id.clone(), *info);
        }
    }
    for (id, info) in &saved_string_vars {
        if saved_variables
            .get(id)
            .is_some_and(|i| i.raw() < num_globals)
        {
            ctx.string_vars.insert(id.clone(), info.clone());
        }
    }
    for (id, info) in &saved_struct_vars {
        if saved_variables
            .get(id)
            .is_some_and(|i| i.raw() < num_globals)
        {
            ctx.struct_vars.insert(id.clone(), info.clone());
        }
    }

    // Assign variable slots for the function's parameters and locals,
    // starting at var_offset. Input parameters come first (declaration order),
    // then local variables.
    let mut current_index = var_offset;
    let mut num_params: u16 = 0;

    // First pass: input-compatible parameters (VAR_INPUT and VAR_IN_OUT)
    // must come first for CALL arg passing.
    for decl in &func_decl.variables {
        if !decl.var_type.is_input_compatible() {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(id.clone(), current_index);
            match &decl.initializer {
                InitialValueAssignmentKind::Simple(simple) => {
                    if let Some(type_info) = resolve_type_name(&simple.type_name.name) {
                        ctx.var_types.insert(id.clone(), type_info);
                    }
                }
                InitialValueAssignmentKind::String(string_init) => {
                    let max_length = resolve_string_max_length(string_init)?;

                    let data_offset = ctx.data_region_offset;
                    let total_bytes = STRING_HEADER_BYTES as u32 + max_length as u32;
                    ctx.data_region_offset = ctx
                        .data_region_offset
                        .checked_add(total_bytes)
                        .ok_or_else(|| {
                            Diagnostic::problem(
                                Problem::NotImplemented,
                                Label::span(string_init.span(), "Data region overflow"),
                            )
                        })?;

                    if max_length > ctx.max_string_capacity {
                        ctx.max_string_capacity = max_length;
                    }

                    ctx.string_vars.insert(
                        id.clone(),
                        StringVarInfo {
                            data_offset,
                            max_length,
                        },
                    );
                }
                InitialValueAssignmentKind::Reference(ref_init) => {
                    ctx.var_types.insert(
                        id.clone(),
                        VarTypeInfo {
                            op_width: OpWidth::W64,
                            signedness: Signedness::Unsigned,
                            storage_bits: 64,
                        },
                    );
                    crate::compile_array::register_ref_to_array_metadata(
                        ctx,
                        builder,
                        id,
                        current_index,
                        ref_init,
                    )?;
                }
                _ => {}
            }
            current_index = VarIndex::new(current_index.raw() + 1);
            num_params += 1;
        }
    }

    // Second pass: local variables (VAR, VAR_TEMP).
    for decl in &func_decl.variables {
        if !decl.var_type.is_local() {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(id.clone(), current_index);
            match &decl.initializer {
                InitialValueAssignmentKind::Simple(simple) => {
                    if let Some(type_info) = resolve_type_name(&simple.type_name.name) {
                        ctx.var_types.insert(id.clone(), type_info);
                    }
                }
                InitialValueAssignmentKind::String(string_init) => {
                    let max_length = resolve_string_max_length(string_init)?;

                    let data_offset = ctx.data_region_offset;
                    let total_bytes = STRING_HEADER_BYTES as u32 + max_length as u32;
                    ctx.data_region_offset = ctx
                        .data_region_offset
                        .checked_add(total_bytes)
                        .ok_or_else(|| {
                            Diagnostic::problem(
                                Problem::NotImplemented,
                                Label::span(string_init.span(), "Data region overflow"),
                            )
                        })?;

                    if max_length > ctx.max_string_capacity {
                        ctx.max_string_capacity = max_length;
                    }

                    ctx.string_vars.insert(
                        id.clone(),
                        StringVarInfo {
                            data_offset,
                            max_length,
                        },
                    );
                }
                InitialValueAssignmentKind::Reference(ref_init) => {
                    ctx.var_types.insert(
                        id.clone(),
                        VarTypeInfo {
                            op_width: OpWidth::W64,
                            signedness: Signedness::Unsigned,
                            storage_bits: 64,
                        },
                    );
                    crate::compile_array::register_ref_to_array_metadata(
                        ctx,
                        builder,
                        id,
                        current_index,
                        ref_init,
                    )?;
                }
                _ => {}
            }
            current_index = VarIndex::new(current_index.raw() + 1);
        }
    }

    // Assign the return variable (named same as the function).
    let return_var_index = current_index;
    let return_id = func_decl.name.clone();
    ctx.variables.insert(return_id.clone(), return_var_index);

    // Check if this function returns a STRING/WSTRING.
    let return_string_info = match &func_decl.return_type {
        FunctionReturnType::String(spec) | FunctionReturnType::WString(spec) => {
            let max_length = resolve_string_spec_max_length(spec)?;

            let data_offset = ctx.data_region_offset;
            let total_bytes = STRING_HEADER_BYTES as u32 + max_length as u32;
            ctx.data_region_offset = ctx
                .data_region_offset
                .checked_add(total_bytes)
                .ok_or_else(|| Diagnostic::todo(file!(), line!()))?;

            if max_length > ctx.max_string_capacity {
                ctx.max_string_capacity = max_length;
            }

            ctx.string_vars.insert(
                return_id.clone(),
                StringVarInfo {
                    data_offset,
                    max_length,
                },
            );
            Some(StringReturnInfo {
                data_offset,
                max_length,
            })
        }
        FunctionReturnType::Named(_) => {
            let return_type_name = func_decl.return_type.to_type_name();
            if types.resolve_struct_type(&return_type_name).is_some() {
                // Return type is a struct — allocate data region space and
                // register the return variable so field assignments
                // (e.g. FUNC.X := val) work inside the function body.
                crate::compile_struct::allocate_struct_variable(
                    ctx,
                    builder,
                    types,
                    &return_type_name,
                    &return_id,
                    return_var_index,
                    &func_decl.name.span(),
                )?;
            } else if let Some(type_info) = resolve_type_name(&return_type_name.name) {
                ctx.var_types.insert(return_id.clone(), type_info);
            }
            None
        }
    };
    current_index = VarIndex::new(current_index.raw() + 1);

    let num_locals = current_index.raw() - var_offset.raw();

    // Determine return type's OpType.
    let return_type_name = func_decl.return_type.to_type_name();
    let return_op_type = resolve_type_name(&return_type_name.name)
        .map(|info| (info.op_width, info.signedness))
        .unwrap_or(DEFAULT_OP_TYPE);

    // Compile the function body.
    let mut func_emitter = Emitter::new();

    // Emit initialization prologue: IEC 61131-3 functions are stateless, so
    // local variables must be re-initialized on every call. The flat variable
    // table (ADR-0021) retains stale values between calls, so the prologue
    // resets non-parameter locals to their declared initial values (or zero).
    emit_function_local_prologue(
        &mut func_emitter,
        ctx,
        func_decl,
        return_var_index,
        return_op_type,
    )?;

    let body = ironplc_dsl::textual::Statements {
        body: func_decl.body.clone(),
    };
    compile_statements(&mut func_emitter, ctx, &body)?;

    // Load the return value and emit RET.
    if let Some(ref str_info) = return_string_info {
        // For STRING return: load the return string from the data region into
        // a temp buffer, leaving buf_idx on the stack for the caller.
        ctx.num_temp_bufs += 1;
        func_emitter.emit_str_load_var(str_info.data_offset);
    } else {
        emit_load_var(&mut func_emitter, return_var_index, return_op_type);
    }
    func_emitter.emit_ret();

    let bytecode = crate::optimize::optimize(func_emitter.bytecode(), &ctx.constants);
    let max_stack_depth = func_emitter.max_stack_depth();

    // Record function metadata for use at call sites.
    let func_name = func_decl.name.lower_case();

    // Record parameter OpTypes and STRING info from the function's declarations.
    let mut param_op_types: Vec<OpType> = Vec::new();
    let mut param_string_info: Vec<Option<StringParamInfo>> = Vec::new();
    for decl in &func_decl.variables {
        if !decl.var_type.is_input_compatible() {
            continue;
        }
        match &decl.initializer {
            InitialValueAssignmentKind::String(_) => {
                param_op_types.push(DEFAULT_OP_TYPE);
                if let Some(id) = decl.identifier.symbolic_id() {
                    if let Some(info) = ctx.string_vars.get(id) {
                        param_string_info.push(Some(StringParamInfo {
                            data_offset: info.data_offset,
                            max_length: info.max_length,
                        }));
                    } else {
                        param_string_info.push(None);
                    }
                } else {
                    param_string_info.push(None);
                }
            }
            InitialValueAssignmentKind::Reference(_) => {
                param_op_types.push((OpWidth::W64, Signedness::Unsigned));
                param_string_info.push(None);
            }
            _ => {
                if let Some(sig) = functions.get(&func_decl.name) {
                    if let Some(param) = sig
                        .parameters
                        .iter()
                        .filter(|p| p.is_input)
                        .nth(param_op_types.len())
                    {
                        param_op_types.push(
                            resolve_type_name(&param.param_type.name)
                                .map(|info| (info.op_width, info.signedness))
                                .unwrap_or(DEFAULT_OP_TYPE),
                        );
                    } else {
                        param_op_types.push(DEFAULT_OP_TYPE);
                    }
                } else {
                    param_op_types.push(DEFAULT_OP_TYPE);
                }
                param_string_info.push(None);
            }
        }
    }

    ctx.user_functions.insert(
        func_name.to_string(),
        UserFunctionInfo {
            function_id,
            var_offset,
            num_params,
            param_op_types,
            param_string_info,
            return_string_info,
            max_stack_depth,
        },
    );

    // Restore the program's variable mappings.
    ctx.variables = saved_variables;
    ctx.var_types = saved_var_types;
    ctx.string_vars = saved_string_vars;
    ctx.array_vars = saved_array_vars;
    ctx.struct_vars = saved_struct_vars;

    Ok(CompiledFunction {
        function_id,
        bytecode,
        max_stack_depth,
        num_locals,
        num_params,
        name: func_name.to_string(),
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
    function_id: u16,
    var_offset: u16,
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    _types: &TypeEnvironment,
    num_globals: u16,
) -> Result<CompiledFunction, Diagnostic> {
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

    // Save the program's variable mappings.
    let saved_variables = std::mem::take(&mut ctx.variables);
    let saved_var_types = std::mem::take(&mut ctx.var_types);
    let saved_string_vars = std::mem::take(&mut ctx.string_vars);
    let saved_array_vars = std::mem::take(&mut ctx.array_vars);
    let saved_struct_vars = std::mem::take(&mut ctx.struct_vars);
    let saved_fb_instances = std::mem::take(&mut ctx.fb_instances);

    // Re-insert global variable mappings so the FB body can access them.
    for (id, index) in &saved_variables {
        if index.raw() < num_globals {
            ctx.variables.insert(id.clone(), *index);
        }
    }
    for (id, info) in &saved_var_types {
        if saved_variables
            .get(id)
            .is_some_and(|i| i.raw() < num_globals)
        {
            ctx.var_types.insert(id.clone(), *info);
        }
    }
    for (id, info) in &saved_string_vars {
        if saved_variables
            .get(id)
            .is_some_and(|i| i.raw() < num_globals)
        {
            ctx.string_vars.insert(id.clone(), info.clone());
        }
    }
    for (id, info) in &saved_struct_vars {
        if saved_variables
            .get(id)
            .is_some_and(|i| i.raw() < num_globals)
        {
            ctx.struct_vars.insert(id.clone(), info.clone());
        }
    }

    // Assign variable slots for all FB fields, in the same order as field_decls.
    let mut current_index = VarIndex::new(var_offset);
    for decl in &field_decls {
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(id.clone(), current_index);
            match &decl.initializer {
                InitialValueAssignmentKind::Simple(simple) => {
                    if let Some(vti) = resolve_type_name(&simple.type_name.name) {
                        ctx.var_types.insert(id.clone(), vti);
                    }
                }
                InitialValueAssignmentKind::Reference(ref_init) => {
                    ctx.var_types.insert(
                        id.clone(),
                        VarTypeInfo {
                            op_width: OpWidth::W64,
                            signedness: Signedness::Unsigned,
                            storage_bits: 64,
                        },
                    );
                    crate::compile_array::register_ref_to_array_metadata(
                        ctx,
                        builder,
                        id,
                        current_index,
                        ref_init,
                    )?;
                }
                InitialValueAssignmentKind::String(string_init) => {
                    let max_length = resolve_string_max_length(string_init)?;

                    let data_offset = ctx.data_region_offset;
                    let total_bytes = STRING_HEADER_BYTES as u32 + max_length as u32;
                    ctx.data_region_offset = ctx
                        .data_region_offset
                        .checked_add(total_bytes)
                        .ok_or_else(|| {
                            Diagnostic::problem(
                                Problem::NotImplemented,
                                Label::span(string_init.span(), "Data region overflow"),
                            )
                        })?;

                    if max_length > ctx.max_string_capacity {
                        ctx.max_string_capacity = max_length;
                    }

                    ctx.string_vars.insert(
                        id.clone(),
                        StringVarInfo {
                            data_offset,
                            max_length,
                        },
                    );
                }
                _ => {}
            }
            current_index = VarIndex::new(current_index.raw() + 1);
        }
    }

    let num_locals = current_index.raw() - var_offset;

    // Compile the FB body.
    let mut fb_emitter = Emitter::new();
    compile_body(&mut fb_emitter, ctx, &fb_decl.body)?;
    fb_emitter.emit_ret_void();

    let bytecode = crate::optimize::optimize(fb_emitter.bytecode(), &ctx.constants);
    let max_stack_depth = fb_emitter.max_stack_depth();

    // Restore the program's variable mappings.
    ctx.variables = saved_variables;
    ctx.var_types = saved_var_types;
    ctx.string_vars = saved_string_vars;
    ctx.array_vars = saved_array_vars;
    ctx.struct_vars = saved_struct_vars;
    ctx.fb_instances = saved_fb_instances;

    Ok(CompiledFunction {
        function_id,
        bytecode,
        max_stack_depth,
        num_locals,
        num_params: 0,
        name: fb_name,
    })
}
