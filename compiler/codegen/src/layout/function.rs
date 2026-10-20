//! The layout of each reachable user function, in library order, in the
//! variable-table slots after the program's. Within a function's run:
//!
//! 1. its `VAR_INPUT` and `VAR_IN_OUT`, in declaration order, which a call
//!    stores its arguments in;
//! 2. then its `VAR`, `VAR_TEMP` and `VAR_OUTPUT`, in declaration order;
//! 3. then its result;
//! 4. then the scratch slot of a structure result holding strings in an
//!    array, when it has one.
//!
//! A function's variables are recorded under its own scope, which its body
//! is compiled in, and which also sees some of the globals'
//! ([`super::names`]).

use std::collections::HashMap;

use ironplc_analyzer::TypeEnvironment;
use ironplc_container::debug_section::{var_section, VarNameEntry};
use ironplc_container::{ContainerBuilder, FunctionId, VarIndex};
use ironplc_dsl::common::{
    FunctionDeclaration, FunctionReturnType, InitialValueAssignmentKind, VariableType,
};
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::Diagnostic;

use super::debug_type::debug_type_for_return;
use super::names::ScopeId;
use crate::compile::{
    CompileContext, OpType, ParamPassing, StringReturnInfo, UserFunctionInfo, DEFAULT_OP_TYPE,
    NARROW_CHAR_WIDTH, WIDE_CHAR_WIDTH,
};
use crate::compile_fn::{
    aliased_string, in_out_value_type, param_passing, push_local_var_name,
    register_declared_string, register_string,
};
use crate::compile_stmt::resolve_string_spec_max_length;
use crate::type_info::{decl_type_info, named_type_info};

/// Where a user function's variables are, which its body is compiled
/// against.
pub(crate) struct FunctionLayout {
    /// The function's id in the container.
    pub(crate) function_id: FunctionId,
    /// The scope the function's variables are recorded under, which its
    /// body is compiled in.
    pub(crate) scope: ScopeId,
    /// How many slots the function's run takes.
    pub(crate) num_locals: u16,
    /// How many of them, at its start, hold the function's parameters.
    pub(crate) num_params: u16,
    /// The slot of the function's result.
    pub(crate) return_var_index: VarIndex,
    /// The type the result is loaded at: the declared return type, a
    /// subrange at its base type. A string or structure result lives in the
    /// data region, and its slot holds a data offset at the default type.
    pub(crate) return_op_type: OpType,
    /// Where in the data region the result is, when it is a string.
    pub(crate) return_string_info: Option<StringReturnInfo>,
}

/// Lays out `func_decl`, whose id is `function_id`, from the slot
/// `var_offset`, and records what a call of it needs in
/// `ctx.user_functions`.
pub(crate) fn lay_out_function(
    func_decl: &FunctionDeclaration,
    function_id: FunctionId,
    var_offset: VarIndex,
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    types: &TypeEnvironment,
) -> Result<FunctionLayout, Diagnostic> {
    // Record the function's variables under its own scope; the program's is
    // current again once the function is laid out.
    let scope = ScopeId::Function(function_id);
    let caller = std::mem::replace(&mut ctx.scope, scope);

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
            ctx.variables.insert(ctx.scope, id.clone(), current_index);
            push_local_var_name(ctx, current_index, function_id, decl, id, types);
            if decl.var_type == VariableType::InOut {
                // The slot holds a reference to the caller's variable; its
                // type info is the referenced value's, for reads and writes.
                let type_info = in_out_value_type(decl)?;
                ctx.var_types.insert(ctx.scope, id.clone(), type_info);
                ctx.in_out_params.insert(ctx.scope, id.clone(), ());
                current_index = VarIndex::new(current_index.raw() + 1);
                num_params += 1;
                continue;
            }
            // A parameter operates at its declared type, whatever kind of
            // declaration declares it: a subrange at its base type, an
            // enumeration as a `DINT`, a reference as its 64-bit index.
            if let Some(type_info) = decl_type_info(ctx, decl) {
                ctx.var_types.insert(ctx.scope, id.clone(), type_info);
            }
            register_declared_string(ctx, id, decl)?;
            if let InitialValueAssignmentKind::Reference(ref_init) = &decl.initializer {
                crate::compile_reference::register_reference_variable(
                    ctx,
                    builder,
                    types,
                    id,
                    current_index,
                    ref_init,
                )?;
            }
            current_index = VarIndex::new(current_index.raw() + 1);
            num_params += 1;
        }
    }

    // Second pass: local variables (VAR, VAR_TEMP) and outputs (VAR_OUTPUT),
    // which start again on every call as a local does.
    let mut outputs = HashMap::new();
    for decl in &func_decl.variables {
        if !decl.var_type.is_local() && decl.var_type != VariableType::Output {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(ctx.scope, id.clone(), current_index);
            if decl.var_type == VariableType::Output {
                outputs.insert(id.clone(), current_index);
            }
            push_local_var_name(ctx, current_index, function_id, decl, id, types);
            // A local operates at its declared type, as a parameter does.
            if let Some(type_info) = decl_type_info(ctx, decl) {
                ctx.var_types.insert(ctx.scope, id.clone(), type_info);
            }
            register_declared_string(ctx, id, decl)?;
            if let InitialValueAssignmentKind::Reference(ref_init) = &decl.initializer {
                crate::compile_reference::register_reference_variable(
                    ctx,
                    builder,
                    types,
                    id,
                    current_index,
                    ref_init,
                )?;
            }
            current_index = VarIndex::new(current_index.raw() + 1);
        }
    }

    // Assign the return variable (named same as the function).
    let return_var_index = current_index;
    let return_id = func_decl.name.clone();
    ctx.variables
        .insert(ctx.scope, return_id.clone(), return_var_index);

    // Debug entry for the return value. The result has no IEC variable
    // section; it is modeled as VAR_OUTPUT so a debugger surfaces it in the
    // "Outputs" scope (per the section->DAP-scope table in
    // specs/design/debugger-support.md).
    let (return_tag, return_type_name_str) = debug_type_for_return(&func_decl.return_type, types);
    ctx.debug_var_names.push(VarNameEntry {
        var_index: return_var_index,
        function_id,
        var_section: var_section::VAR_OUTPUT,
        iec_type_tag: return_tag,
        name: return_id.to_string(),
        type_name: return_type_name_str,
    });

    // Check if this function returns a STRING/WSTRING, spelled out or
    // through a type alias of one.
    let return_string = match &func_decl.return_type {
        FunctionReturnType::String(spec) | FunctionReturnType::WString(spec) => {
            let char_width = match &func_decl.return_type {
                FunctionReturnType::WString(_) => WIDE_CHAR_WIDTH,
                _ => NARROW_CHAR_WIDTH,
            };
            let max_length = resolve_string_spec_max_length(spec)?;
            Some((max_length, char_width, spec.keyword_span.clone()))
        }
        FunctionReturnType::Named(name) => aliased_string(ctx, types.id_of(name))
            .map(|(max_length, char_width)| (max_length, char_width, func_decl.name.span())),
    };
    let return_string_info = match return_string {
        Some((max_length, char_width, span)) => {
            let data_offset = register_string(ctx, &return_id, max_length, char_width, &span)?;
            Some(StringReturnInfo {
                data_offset,
                max_length,
                char_width,
            })
        }
        None => {
            let return_type_name = func_decl.return_type.to_type_name();
            if let Some(struct_type) = types.resolve_struct_type(&return_type_name) {
                // Return type is a struct — allocate data region space and
                // register the return variable so field assignments
                // (e.g. FUNC.X := val) work inside the function body.
                crate::compile_struct::allocate_struct_variable(
                    ctx,
                    builder,
                    struct_type,
                    &return_id,
                    return_var_index,
                    &func_decl.name.span(),
                )?;
            } else if let Some(type_info) = named_type_info(ctx, types, &return_type_name) {
                ctx.var_types
                    .insert(ctx.scope, return_id.clone(), type_info);
            }
            None
        }
    };
    // Captured here because the call site reads it from the caller's scope,
    // which does not see the function's variables.
    let return_struct_desc_index = ctx.struct_var(&return_id).map(|info| info.desc_index);
    // The run ends after the result, and after the scratch slot a structure
    // result holding strings in an array takes after it.
    current_index = ctx.next_free_slot();

    let num_locals = current_index.raw() - var_offset.raw();

    // The type the return value is loaded at: the declared return type, a
    // subrange at its base type. A string or structure return lives in the
    // data region, and its slot holds a data offset at the default type.
    let return_type_name = func_decl.return_type.to_type_name();
    let return_op_type = named_type_info(ctx, types, &return_type_name)
        .map(|info| (info.op_width, info.signedness))
        .unwrap_or(DEFAULT_OP_TYPE);

    // Record how a call site passes each parameter: by value, into the
    // parameter's string space, or by reference.
    let params: Vec<ParamPassing> = func_decl
        .variables
        .iter()
        .filter(|decl| decl.var_type.is_input_compatible())
        .map(|decl| param_passing(ctx, decl))
        .collect();

    let string_outputs = outputs
        .keys()
        .filter_map(|id| Some((id.clone(), ctx.string_var(id)?.clone())))
        .collect();

    // Record function metadata for use at call sites.
    ctx.user_functions.insert(
        func_decl.name.lower_case().to_string(),
        UserFunctionInfo {
            function_id,
            var_offset,
            num_params,
            params,
            return_string_info: return_string_info.clone(),
            return_struct_desc_index,
            outputs,
            string_outputs,
        },
    );

    ctx.scope = caller;

    Ok(FunctionLayout {
        function_id,
        scope,
        num_locals,
        num_params,
        return_var_index,
        return_op_type,
        return_string_info,
    })
}
