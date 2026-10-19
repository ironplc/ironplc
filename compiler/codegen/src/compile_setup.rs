//! Initial values and prologues for IEC 61131-3 code generation: the
//! bytecode that gives each variable of the program its starting value, and
//! each local of a function its value on every call. The layout their stores
//! address is decided first (`layout/`).

use ironplc_container::VarIndex;
use ironplc_dsl::common::{ResultVariable, VarDecl};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_lowering::initial_value::InitialValue;

use ironplc_analyzer::semantic_type::SemanticType;
use ironplc_analyzer::TypeEnvironment;

use super::compile::{
    emit_string_literal_load, CompileContext, OpType, OpWidth, Signedness, VarTypeInfo,
};
use super::compile_expr::{emit_store_var, emit_truncation};
use super::compile_initial_value::{
    char_width, emit_reference, emit_scalar, emit_string_store, is_cleared, required,
};
use super::compile_struct_init::Writes;
use crate::emit::Emitter;

/// Emits the bytecode that gives each variable `declarations` declare the
/// starting value the analyzer resolved for it, once, when the program
/// starts.
///
/// The variable table and the data region start cleared. A scalar of an
/// elementary type whose value leaves its slot cleared is not stored; an
/// enumeration, a subrange and a reference always are, and so is every
/// field of a structure.
pub(crate) fn emit_initial_values(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    declarations: &[VarDecl],
    types: &TypeEnvironment,
) -> Result<(), Diagnostic> {
    for decl in declarations {
        let Some(id) = decl.identifier.symbolic_id() else {
            continue;
        };
        let value = &required(decl, ctx)?;
        let span = decl.identifier.span();
        if let Some(struct_info) = ctx.struct_var(id).cloned() {
            crate::compile_struct_init::initialize_struct_variable(
                emitter,
                ctx,
                &struct_info,
                value,
                Writes::Once,
                &span,
            )?;
        } else if let Some(info) = ctx.struct_array_var(id).cloned() {
            crate::compile_struct_init::initialize_struct_array_variable(
                emitter,
                ctx,
                &info,
                value,
                Writes::Once,
                &span,
            )?;
        } else if ctx.array_var(id).is_some_and(|array| !array.is_ref) {
            // A `REF_TO ARRAY` registers array metadata too, but its slot
            // holds a reference, stored below.
            emit_array_initial_value(emitter, ctx, id, value, &span)?;
        } else if let Some(info) = ctx.string_var(id) {
            let (data_offset, max_length, char_width) =
                (info.data_offset, info.max_length, info.char_width);
            // Initialize the string header in the data region, then store
            // the characters. The literal is encoded at the variable's width
            // so the store's encoding check passes (ADR-0034).
            emitter.emit_str_init(data_offset, max_length, char_width);
            if let InitialValue::String(string) = value {
                emit_string_store(emitter, ctx, string, data_offset);
            }
        } else if let Some(fb_info) = ctx.fb_instance(id) {
            let data_offset = fb_info.data_offset;
            let var_index = fb_info.var_index;
            let held = fb_info.held.clone();
            // Store the data region byte offset into the variable slot, and
            // in each field that holds an instance, the offset of that
            // instance's fields.
            let offset_const = ctx.add_i32_constant(data_offset as i32);
            emitter.emit_load_const_i32(offset_const);
            emitter.emit_store_var_i32(var_index);
            crate::held_instances::emit_offsets(emitter, ctx, &held);

            // `timer : TON := (PT := T#100MS)` sets the instance's own
            // members. The slot offset has to be in place first, because
            // each member store addresses the instance through it.
            crate::compile_fb_init::emit_fb_instance_member_initializers(emitter, ctx, id, value)?;
        } else {
            let always = stored_even_when_cleared(types, decl);
            emit_slot_initial_value(emitter, ctx, id, value, always, &span)?;
        }
    }
    Ok(())
}

/// Whether a scalar variable is stored its starting value even when that
/// leaves its slot cleared: an enumeration or a subrange is, as the init
/// function has always stored them, and an elementary type is not.
fn stored_even_when_cleared(types: &TypeEnvironment, decl: &VarDecl) -> bool {
    decl.type_id
        .and_then(|id| types.get_by_id(id))
        .is_some_and(|attributes| {
            matches!(
                attributes.representation,
                SemanticType::Enumeration { .. } | SemanticType::Subrange { .. }
            )
        })
}

/// Emits the store of `value` into the variable `id`, which occupies one
/// variable-table slot. A scalar whose value leaves the slot cleared is not
/// stored unless `always`.
fn emit_slot_initial_value(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    id: &Id,
    value: &InitialValue,
    always: bool,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    match value {
        InitialValue::Scalar(scalar) => {
            if !always && scalar.value.is_zero() {
                return Ok(());
            }
            let var_index = ctx.var_index(id)?;
            let type_info = slot_type_info(ctx, id, span)?;
            let op_type = (type_info.op_width, type_info.signedness);
            emit_scalar(emitter, ctx, scalar, op_type, span)?;
            emit_truncation(emitter, type_info);
            emit_store_var(emitter, var_index, op_type);
        }
        InitialValue::Reference(reference) => {
            let var_index = ctx.var_index(id)?;
            emit_reference(emitter, ctx, reference)?;
            emitter.emit_store_var_i64(var_index);
        }
        // A variable codegen gave no storage for its aggregate value (a
        // function block it does not know) holds nothing to store.
        other if is_cleared(other) => {}
        _ => {
            return Err(Diagnostic::not_implemented(Label::span(
                span.clone(),
                "Initial value of a variable of this type",
            )))
        }
    }
    Ok(())
}

/// The storage type of the scalar variable `id`.
fn slot_type_info(
    ctx: &CompileContext,
    id: &Id,
    span: &SourceSpan,
) -> Result<VarTypeInfo, Diagnostic> {
    ctx.var_type_info(id).ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            span.clone(),
            "Scalar variable has no storage type",
        ))
    })
}

/// Emits the stores that give the array variable `id` its starting value
/// `value`: its data-region offset into its slot, the header of every STRING
/// element, and each element whose value would not leave it cleared.
fn emit_array_initial_value(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    id: &Id,
    value: &InitialValue,
    span: &SourceSpan,
) -> Result<(), Diagnostic> {
    let Some(array_info) = ctx.array_var(id) else {
        return Err(Diagnostic::internal_error());
    };
    let data_offset = array_info.data_offset;
    let var_index = array_info.var_index;
    let desc_index = array_info.desc_index;
    let element_vti = array_info.element_var_type_info;
    let is_string = array_info.is_string_element;

    let InitialValue::Array(elements) = value else {
        return Err(Diagnostic::internal_error_at(Label::span(
            span.clone(),
            "Array variable whose initial value is not an array",
        )));
    };

    // Store data_offset into the variable slot (like FB instances).
    let offset_const = ctx.add_i32_constant(data_offset as i32);
    emitter.emit_load_const_i32(offset_const);
    emitter.emit_store_var_i32(var_index);

    if is_string {
        // Initialize all string headers in the array.
        emitter.emit_str_init_array(var_index, desc_index);
    }
    for (i, element) in elements.iter().enumerate() {
        if is_cleared(element) {
            continue;
        }
        match element {
            // String literals are encoded at the element width so the array
            // element's encoding check passes.
            InitialValue::String(string) if is_string => {
                emit_string_literal_load(emitter, ctx, &string.chars, char_width(string));
                let idx_const = ctx.add_i32_constant(i as i32);
                emitter.emit_load_const_i32(idx_const);
                emitter.emit_str_store_array_elem(var_index, desc_index);
            }
            InitialValue::Scalar(scalar) => {
                let op_type = (element_vti.op_width, element_vti.signedness);
                emit_scalar(emitter, ctx, scalar, op_type, span)?;
                emit_truncation(emitter, element_vti);
                let idx_const = ctx.add_i32_constant(i as i32);
                emitter.emit_load_const_i32(idx_const);
                emitter.emit_store_array(var_index, desc_index);
            }
            InitialValue::Reference(reference) => {
                emit_reference(emitter, ctx, reference)?;
                let idx_const = ctx.add_i32_constant(i as i32);
                emitter.emit_load_const_i32(idx_const);
                emitter.emit_store_array(var_index, desc_index);
            }
            _ => {
                return Err(Diagnostic::internal_error_at(Label::span(
                    span.clone(),
                    "Array element value does not have the element's shape",
                )))
            }
        }
    }
    Ok(())
}

/// Emits a bytecode prologue that sets every local a function or method
/// starts again on each call to its resolved starting value: the
/// declarations the analyzer flagged (`VarDecl::reset_on_call`) and the
/// result (`result`, which the analyzer made; `None` for a method that
/// returns nothing). IEC 61131-3 requires functions
/// to be stateless, and the flat variable table (ADR-0046) keeps whatever
/// the last call left, so each value is stored whatever it is.
pub(crate) fn emit_function_local_prologue(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    variables: &[VarDecl],
    result: Option<&ResultVariable>,
    return_id: &Id,
    return_var_index: VarIndex,
    return_op_type: OpType,
) -> Result<(), Diagnostic> {
    for decl in variables {
        if !decl.reset_on_call {
            continue;
        }
        let Some(id) = decl.identifier.symbolic_id() else {
            continue;
        };
        let value = &required(decl, ctx)?;
        let span = decl.identifier.span();
        let var_index = ctx.var_index(id)?;

        if let Some(info) = ctx.string_var(id) {
            // Reset the header, which empties the string, then store the
            // characters.
            let (data_offset, max_length, char_width) =
                (info.data_offset, info.max_length, info.char_width);
            emitter.emit_str_init(data_offset, max_length, char_width);
            if let InitialValue::String(string) = value {
                emit_string_store(emitter, ctx, string, data_offset);
            }
            continue;
        }
        match value {
            InitialValue::Scalar(scalar) => {
                let type_info = ctx
                    .var_type_info(id)
                    .or_else(|| crate::type_info::decl_type_info(ctx, decl))
                    .ok_or_else(|| {
                        Diagnostic::internal_error_at(Label::span(
                            span.clone(),
                            "Scalar local has no storage type",
                        ))
                    })?;
                let op_type = (type_info.op_width, type_info.signedness);
                emit_scalar(emitter, ctx, scalar, op_type, &span)?;
                emit_truncation(emitter, type_info);
                emit_store_var(emitter, var_index, op_type);
            }
            InitialValue::Reference(reference) => {
                emit_reference(emitter, ctx, reference)?;
                emitter.emit_store_var_i64(var_index);
            }
            // A local of a type this backend gives no storage of its own
            // inside a function (a structure, an array) keeps its slot
            // cleared, as it always has.
            _ => {
                emit_zero_const(emitter, ctx, (OpWidth::W32, Signedness::Signed));
                emitter.emit_store_var_i32(var_index);
            }
        }
    }

    let Some(result) = result else {
        // A method that returns nothing still has a result slot, which
        // codegen allocates for it; it is cleared like a result.
        emit_zero_const(emitter, ctx, return_op_type);
        emit_store_var(emitter, return_var_index, return_op_type);
        return Ok(());
    };
    let result = result.variable().ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            return_id.span(),
            "Result has no resolved initial value",
        ))
    })?;
    let value = &required(result, ctx)?;
    if let Some(struct_info) = ctx.struct_var(return_id).cloned() {
        // The struct was registered under `return_var_index`, so
        // `struct_info.var_index` is that slot.
        crate::compile_struct_init::initialize_struct_variable(
            emitter,
            ctx,
            &struct_info,
            value,
            Writes::EveryCall,
            &return_id.span(),
        )?;
    } else if let Some(info) = ctx.string_var(return_id) {
        // STRING/WSTRING result: initialize the string header in the data
        // region, which empties it.
        emitter.emit_str_init(info.data_offset, info.max_length, info.char_width);
    } else if let InitialValue::Scalar(scalar) = value {
        emit_scalar(emitter, ctx, scalar, return_op_type, &return_id.span())?;
        emit_store_var(emitter, return_var_index, return_op_type);
    } else {
        emit_zero_const(emitter, ctx, return_op_type);
        emit_store_var(emitter, return_var_index, return_op_type);
    }

    Ok(())
}

/// Emits a LOAD_CONST instruction that pushes a zero value of the given type.
pub(crate) fn emit_zero_const(emitter: &mut Emitter, ctx: &mut CompileContext, op_type: OpType) {
    match op_type.0 {
        OpWidth::W32 => {
            let pool_index = ctx.add_i32_constant(0);
            emitter.emit_load_const_i32(pool_index);
        }
        OpWidth::W64 => {
            let pool_index = ctx.add_i64_constant(0);
            emitter.emit_load_const_i64(pool_index);
        }
        OpWidth::F32 => {
            let pool_index = ctx.add_f32_constant(0.0);
            emitter.emit_load_const_f32(pool_index);
        }
        OpWidth::F64 => {
            let pool_index = ctx.add_f64_constant(0.0);
            emitter.emit_load_const_f64(pool_index);
        }
    }
}
