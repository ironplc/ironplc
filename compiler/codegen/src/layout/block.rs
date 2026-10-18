//! The layout of each reachable user function block, in library order, in
//! the variable-table slots after the functions'.
//!
//! Before the program's variables are laid out, each block type is
//! registered (`lay_out_block_types`): its type id, the VM index of each
//! field (inputs, then outputs, then locals), and the function id and
//! parameters of each method, so that an instance of the block can be laid
//! out. Then each block is laid out (`lay_out_block`):
//!
//! 1. its copy slots, one for each field, in field order, which the VM
//!    copies an instance's fields into while the block's body or a method
//!    runs;
//! 2. then each method, in declaration order: its `VAR_INPUT` and
//!    `VAR_IN_OUT`, then its `VAR`, `VAR_TEMP` and `VAR_OUTPUT`, then its
//!    result, as a function's.
//!
//! A block's fields are recorded under its own scope, and each method's
//! variables under the method's, which also sees its block's. Each body is
//! compiled in its scope, which also sees some of the globals'
//! ([`super::names`]).

use std::collections::HashMap;

use ironplc_analyzer::TypeEnvironment;
use ironplc_container::{ContainerBuilder, FbTypeId, FunctionId, VarIndex};
use ironplc_dsl::common::{
    FunctionBlockDeclaration, FunctionReturnType, InitialValueAssignmentKind, MethodDeclaration,
    VarDecl, VariableType,
};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};

use super::names::ScopeId;
use crate::compile::{
    CompileContext, FbFieldIndex, OpType, ParamPassing, UserFbTypeInfo, UserMethodInfo,
    UserMethodParam, VarTypeInfo, DEFAULT_OP_TYPE, DEFAULT_VAR_TYPE_INFO, MAX_USER_FB_FIELDS,
};
use crate::compile_fn::{param_passing, push_local_var_name, register_declared_string};
use crate::held_instances::HeldField;
use crate::type_info::{decl_type_info, named_type_info};

/// Where a user function block's copy slots and its methods' variables
/// are, which their bodies are compiled against.
pub(crate) struct BlockLayout {
    /// The block body's id in the container.
    pub(crate) function_id: FunctionId,
    /// The scope the block's copy slots are recorded under, which its body
    /// is compiled in.
    pub(crate) scope: ScopeId,
    /// How many copy slots the block takes, one for each field.
    pub(crate) num_locals: u16,
    /// The layout of each of the block's methods, in declaration order.
    pub(crate) methods: Vec<MethodLayout>,
}

/// Where a method's variables are, which its body is compiled against.
pub(crate) struct MethodLayout {
    /// The method's id in the container.
    pub(crate) function_id: FunctionId,
    /// The scope the method's variables are recorded under, which its body
    /// is compiled in, and which sees its block's too.
    pub(crate) scope: ScopeId,
    /// The slots the VM's frame for the method spans, from the first of
    /// its block's copy slots through the method's result.
    pub(crate) num_locals: u16,
    /// How many input parameters the method takes.
    pub(crate) num_params: u16,
    /// The slot of the method's result, which a method without a return
    /// type has too.
    pub(crate) return_var_index: VarIndex,
    /// The type the result is loaded at.
    pub(crate) return_op_type: OpType,
    /// Whether the method declares a return type.
    pub(crate) has_return_value: bool,
}

/// Registers each block of `fb_decls` and its methods in
/// `ctx.user_fb_types`, before any instance of one is laid out. The
/// methods' function ids follow the blocks' and the `num_functions` user
/// functions'.
pub(crate) fn lay_out_block_types(
    ctx: &mut CompileContext,
    fb_decls: &[&FunctionBlockDeclaration],
    num_functions: u16,
    types: &TypeEnvironment,
) -> Result<(), Diagnostic> {
    for (next_function_id, fb_decl) in (2_u16..).zip(fb_decls.iter()) {
        let fb_name = fb_decl.name.name.to_string().to_uppercase();
        let mut field_indices: HashMap<String, u8> = HashMap::new();
        let mut field_types: HashMap<String, VarTypeInfo> = HashMap::new();
        let mut held_fields = Vec::new();
        let mut field_decls_tmp: Vec<&VarDecl> = Vec::new();

        for decl in &fb_decl.variables {
            if decl.var_type == VariableType::Input {
                field_decls_tmp.push(decl);
            }
        }
        for decl in &fb_decl.variables {
            if decl.var_type == VariableType::Output {
                field_decls_tmp.push(decl);
            }
        }
        for decl in &fb_decl.variables {
            if decl.var_type == VariableType::Var {
                field_decls_tmp.push(decl);
            }
        }
        // `FB_CALL` and `METHOD_CALL` count a block's fields, and
        // `FB_LOAD_PARAM` and `FB_STORE_PARAM` name one, by a byte, so a
        // block with more fields is a limit of the format (P9997).
        if field_decls_tmp.len() > MAX_USER_FB_FIELDS {
            return Err(Diagnostic::not_supported(Label::span(
                fb_decl.name.name.span(),
                format!("Function block with more than {MAX_USER_FB_FIELDS} fields"),
            ))
            .with_context("fields", &field_decls_tmp.len().to_string()));
        }
        for (i, decl) in field_decls_tmp.iter().enumerate() {
            if let Some(id) = decl.identifier.symbolic_id() {
                let name = id.to_string().to_lowercase();
                // At most `MAX_USER_FB_FIELDS` fields, so the index fits.
                let index = i as FbFieldIndex;
                field_indices.insert(name.clone(), index);
                // A field is operated and stored at its declared type,
                // whatever kind of declaration declares it: a subrange at its
                // base type.
                let field_type = decl_type_info(ctx, decl).unwrap_or(DEFAULT_VAR_TYPE_INFO);
                field_types.insert(name.clone(), field_type);
                if let Some(block) = crate::held_instances::held_block(types, decl) {
                    held_fields.push(HeldField { name, index, block });
                }
            }
        }

        let type_id = ctx.next_user_fb_type_id;
        ctx.next_user_fb_type_id = FbTypeId::new(type_id.raw() + 1);
        ctx.user_fb_types.insert(
            fb_name,
            UserFbTypeInfo {
                type_id,
                num_fields: field_decls_tmp.len(),
                field_indices,
                function_id: FunctionId::new(next_function_id),
                var_offset: 0, // set when the block is laid out
                field_types,
                held_fields,
                methods: HashMap::new(),
            },
        );
    }

    // Pre-scan METHOD declarations (OOP extension, ADR-0041 Phase 1) to
    // register their function IDs, parameter shapes, and return-value
    // presence -- all knowable from the AST alone, same reasoning as the
    // FB pre-scan above. Method function IDs continue the sequence after
    // every FB body and every user FUNCTION, so they must be registered
    // only after both counts are known; `param_var_off` stays `0` until
    // the block is laid out (mirrors `UserFbTypeInfo::var_offset`).
    let mut next_method_function_id = 2_u16 + fb_decls.len() as u16 + num_functions;
    for fb_decl in fb_decls.iter() {
        let fb_name = fb_decl.name.name.to_string().to_uppercase();
        for method in &fb_decl.methods {
            let method_name = method.name.to_string().to_lowercase();
            let mut params: Vec<UserMethodParam> = Vec::new();
            for decl in &method.variables {
                if !decl.var_type.is_input_compatible() {
                    continue;
                }
                if let Some(id) = decl.identifier.symbolic_id() {
                    // A string's passing is known once the method is
                    // compiled, which gives the string its storage.
                    let passing = match decl.var_type {
                        VariableType::InOut => ParamPassing::Reference,
                        _ => ParamPassing::Value,
                    };
                    params.push(UserMethodParam {
                        name: id.clone(),
                        passing,
                    });
                }
            }

            let function_id = FunctionId::new(next_method_function_id);
            next_method_function_id += 1;

            if let Some(fb_info) = ctx.user_fb_types.get_mut(&fb_name) {
                fb_info.methods.insert(
                    method_name,
                    UserMethodInfo {
                        function_id,
                        param_var_off: 0, // set when the block is laid out
                        num_params: params.len() as u16,
                        params,
                        has_return_value: method.return_type.is_some(),
                        outputs: HashMap::new(), // set when the block is laid out
                        string_outputs: HashMap::new(), // set when the block is laid out
                    },
                );
            }
        }
    }

    Ok(())
}

/// Lays out `fb_decl`'s copy slots, then each of its methods, from the slot
/// `var_offset`, which it advances past them, and records where they are
/// in `ctx.user_fb_types`.
pub(crate) fn lay_out_block(
    fb_decl: &FunctionBlockDeclaration,
    var_offset: &mut VarIndex,
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    types: &TypeEnvironment,
) -> Result<BlockLayout, Diagnostic> {
    let fb_name = fb_decl.name.name.to_string().to_uppercase();
    let field_var_off = var_offset.raw();

    // Update the var_offset in the registered type info.
    let fb_type = ctx.user_fb_types.get_mut(&fb_name).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            fb_decl.name.span(),
            "Function block was not registered before it was laid out",
        ))
    })?;
    let function_id = fb_type.function_id;
    fb_type.var_offset = field_var_off;

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

    // Record the block's fields under its own scope; the program's is
    // current again once the block and its methods are laid out.
    let scope = ScopeId::Block(function_id);
    let caller = std::mem::replace(&mut ctx.scope, scope);

    // Assign variable slots for all FB fields, in the same order as field_decls.
    let mut current_index = VarIndex::new(field_var_off);
    for decl in &field_decls {
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(ctx.scope, id.clone(), current_index);
            push_local_var_name(ctx, current_index, function_id, decl, id, types);
            // A field operates at its declared type, as a function's local
            // does.
            if let Some(type_info) = decl_type_info(ctx, decl) {
                ctx.var_types.insert(ctx.scope, id.clone(), type_info);
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
                // A field of a type alias of a string is left as it was: a
                // block's string fields have no initialized run (issue 1753).
                InitialValueAssignmentKind::String(_) => {
                    register_declared_string(ctx, id, decl)?;
                }
                _ => {}
            }
            current_index = VarIndex::new(current_index.raw() + 1);
        }
    }

    let num_locals = current_index.raw() - field_var_off;

    *var_offset = VarIndex::new(var_offset.raw() + num_locals);

    // Each method's variables are recorded under its own scope, so that a
    // name one method declares does not resolve in the next.
    let mut methods = Vec::with_capacity(fb_decl.methods.len());
    for method in &fb_decl.methods {
        methods.push(lay_out_method(
            &fb_name,
            function_id,
            method,
            field_var_off,
            var_offset,
            ctx,
            types,
        )?);
        ctx.scope = scope;
    }

    ctx.scope = caller;

    Ok(BlockLayout {
        function_id,
        scope,
        num_locals,
        methods,
    })
}

/// Lays out the variables of `method`, a method of the block `fb_name`
/// whose body is `block` and whose copy slots start at `field_var_off`,
/// from the slot `var_offset`, which it advances past them, under the
/// method's own scope, which it leaves current. Records where they are in
/// the method's entry of `ctx.user_fb_types`.
fn lay_out_method(
    fb_name: &str,
    block: FunctionId,
    method: &MethodDeclaration,
    field_var_off: u16,
    var_offset: &mut VarIndex,
    ctx: &mut CompileContext,
    types: &TypeEnvironment,
) -> Result<MethodLayout, Diagnostic> {
    let method_name = method.name.to_string().to_lowercase();
    let function_id = ctx.user_fb_types[fb_name].methods[&method_name].function_id;
    let param_var_off = *var_offset;
    let scope = ScopeId::Method {
        block,
        method: function_id,
    };
    ctx.scope = scope;

    let mut current_index = param_var_off;
    let mut num_params: u16 = 0;

    // First pass: input-compatible parameters (VAR_INPUT and VAR_IN_OUT).
    for decl in method.all_variables() {
        if !decl.var_type.is_input_compatible() {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            ctx.variables.insert(ctx.scope, id.clone(), current_index);
            // A parameter operates at its declared type, whatever kind of
            // declaration declares it, as a function's does.
            if let Some(type_info) = decl_type_info(ctx, decl) {
                ctx.var_types.insert(ctx.scope, id.clone(), type_info);
            }
            // A call copies a string argument into the parameter's run.
            register_declared_string(ctx, id, decl)?;
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
            ctx.variables.insert(ctx.scope, id.clone(), current_index);
            if decl.var_type == VariableType::Output {
                outputs.insert(id.clone(), current_index);
            }
            // A local operates at its declared type, as a parameter does.
            if let Some(type_info) = decl_type_info(ctx, decl) {
                ctx.var_types.insert(ctx.scope, id.clone(), type_info);
            }
            // The prologue initializes a string local or output's run at
            // every call.
            register_declared_string(ctx, id, decl)?;
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

    // Bind the method's own name to that slot, as `lay_out_function`
    // does for a function's result, so a body
    // that produces its result the standard way -- `GetSpeed := speed`
    // -- resolves the name to the slot the epilogue loads before `RET`.
    // Only when the method declares a return type: one without a return
    // type has no result to assign, and the analyzer rejects the
    // assignment rather than letting it reach here.
    if has_return_value {
        ctx.variables
            .insert(ctx.scope, return_id.clone(), return_var_index);
        if let Some(FunctionReturnType::Named(type_name)) = &method.return_type {
            if let Some(type_info) = named_type_info(ctx, types, type_name) {
                ctx.var_types
                    .insert(ctx.scope, return_id.clone(), type_info);
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

    let params: HashMap<Id, ParamPassing> = method
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
        .filter_map(|id| Some((id.clone(), ctx.string_var(id)?.clone())))
        .collect();

    // `num_locals` spans from `field_var_off` (not `param_var_off`) through
    // this method's own locals -- see the comment above. Advancing
    // `var_offset` by that full span means a type with N methods reserves
    // N * num_fields more table slots than strictly necessary (each
    // method's span re-covers the shared field region). Harmless (a few
    // extra flat-table slots, never aliased or read), just not maximally
    // compact; fine to tighten later if it matters.
    *var_offset = VarIndex::new(var_offset.raw() + num_locals);

    if let Some(info) = ctx
        .user_fb_types
        .get_mut(fb_name)
        .and_then(|fb| fb.methods.get_mut(&method_name))
    {
        info.param_var_off = param_var_off.raw();
        info.outputs = outputs;
        info.string_outputs = string_outputs;
        for param in &mut info.params {
            if let Some(passing) = params.get(&param.name) {
                param.passing = passing.clone();
            }
        }
    }

    Ok(MethodLayout {
        function_id,
        scope,
        num_locals,
        num_params,
        return_var_index,
        return_op_type,
        has_return_value,
    })
}
