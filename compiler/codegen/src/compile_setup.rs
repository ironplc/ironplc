//! Variable setup and initialization for IEC 61131-3 code generation.
//!
//! Contains variable assignment, initial value emission, function local
//! prologue, and type name resolution. Separated from compile.rs to
//! keep module sizes within the 1000-line guideline.

use crate::initial_value::InitialValue;
use ironplc_container::debug_section::{
    function_id, iec_type_tag, var_section, StringLayoutEntry, VarNameEntry,
};
use ironplc_container::{ContainerBuilder, VarIndex};
use ironplc_dsl::common::{
    FunctionReturnType, InitialValueAssignmentKind, ResultVariable, TypeName, VarDecl, VariableType,
};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::{Diagnostic, Label};

use ironplc_analyzer::semantic_type::SemanticType;
use ironplc_analyzer::system_globals::SYSTEM_UPTIME_GLOBALS;
use ironplc_analyzer::TypeEnvironment;

use super::compile::{
    char_width_for_string_type, emit_string_literal_load, string_region_size, CompileContext,
    FbInstanceInfo, OpType, OpWidth, Signedness, StringVarInfo, VarTypeInfo,
};
use super::compile_call::resolve_fb_type;
use super::compile_expr::{emit_store_var, emit_truncation};
use super::compile_initial_value::{
    char_width, emit_reference, emit_scalar, emit_string_store, is_cleared, required,
};
use super::compile_struct_init::Writes;
use crate::emit::Emitter;

/// Assigns variable table indices and type info for all variable declarations.
///
/// The storage each declaration needs is decided by the type the analyzer
/// resolved for it (`VarDecl::type_id`): a structure, an array, a string, a
/// function block instance, a reference or a scalar. A string's length is
/// the one its resolved starting value records, because a sized string has
/// the unsized string's type id.
pub(crate) fn assign_variables(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    declarations: &[VarDecl],
    types: &TypeEnvironment,
) -> Result<(), Diagnostic> {
    for decl in declarations {
        if let Some(id) = decl.identifier.symbolic_id() {
            let index = VarIndex::new(ctx.variables.len() as u16);
            ctx.variables.insert(id.clone(), index);
            let (type_tag, type_name_str) = assign_storage(ctx, builder, decl, id, index, types)?;
            ctx.debug_var_names.push(VarNameEntry {
                var_index: index,
                function_id: function_id::GLOBAL_SCOPE,
                var_section: map_var_section(&decl.var_type),
                iec_type_tag: type_tag,
                name: id.to_string(),
                type_name: type_name_str,
            });
        }
    }
    Ok(())
}

/// Assigns the implicit uptime globals (`__SYSTEM_UP_TIME`,
/// `__SYSTEM_UP_LTIME`) the first variable-table slots. The VM writes them
/// before every scan, so they have no starting value to store.
pub(crate) fn assign_system_uptime_globals(ctx: &mut CompileContext, types: &TypeEnvironment) {
    for global in &SYSTEM_UPTIME_GLOBALS {
        let id = Id::from(global.name);
        let index = VarIndex::new(ctx.variables.len() as u16);
        ctx.variables.insert(id.clone(), index);
        let type_name = TypeName::from(global.type_name);
        ctx.debug_var_names.push(VarNameEntry {
            var_index: index,
            function_id: function_id::GLOBAL_SCOPE,
            var_section: var_section::VAR_GLOBAL,
            iec_type_tag: resolve_iec_type_tag(types, &type_name),
            name: id.to_string(),
            type_name: type_name.name.to_string().to_uppercase(),
        });
    }
}

/// Lays out the storage of the variable `decl` declares, which has the
/// variable-table slot `index`, and returns its debug `(iec_type_tag,
/// type_name)`.
fn assign_storage(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    decl: &VarDecl,
    id: &Id,
    index: VarIndex,
    types: &TypeEnvironment,
) -> Result<(u8, String), Diagnostic> {
    let span = decl.identifier.span();
    let Some(type_id) = decl.type_id else {
        return Ok((iec_type_tag::OTHER, String::new()));
    };
    let representation = types
        .get_by_id(type_id)
        .map(|attributes| attributes.representation.clone())
        .ok_or_else(|| {
            Diagnostic::internal_error_at(Label::span(
                span.clone(),
                "Variable type is absent from the type environment",
            ))
        })?;
    let declared_name = || {
        types
            .name_of(type_id)
            .map(|name| name.to_string().to_uppercase())
            .unwrap_or_default()
    };

    Ok(match &representation {
        SemanticType::Structure { .. } => {
            crate::compile_struct::allocate_struct_variable(
                ctx,
                builder,
                &representation,
                id,
                index,
                &span,
            )?;
            (iec_type_tag::STRUCT, declared_name())
        }
        SemanticType::String { .. } => {
            let InitialValue::String(value) = required(decl, ctx)? else {
                return Err(Diagnostic::internal_error_at(Label::span(
                    span,
                    "STRING variable whose initial value is not a string",
                )));
            };
            let max_length = value.max_length;
            let char_width = char_width(&value);

            // Allocate space in the data region: [max_length: u16][cur_length: u16][data]
            let total_bytes = string_region_size(max_length, char_width);
            let data_offset = crate::data_region::reserve(ctx, total_bytes, &span)?;

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
            ctx.debug_string_layouts.push(StringLayoutEntry {
                var_index: index,
                data_offset,
                max_length,
            });
            if char_width.is_wide() {
                ctx.has_wide_string = true;
                (iec_type_tag::WSTRING, "WSTRING".into())
            } else {
                (iec_type_tag::STRING, "STRING".into())
            }
        }
        SemanticType::FunctionBlock { .. } => {
            // The instance's members are stored by `emit_initial_values`,
            // which runs after the instance has its slot offset -- each
            // member store addresses the instance through it. Nothing to do
            // here but size it.
            let fb_name = declared_name();
            if let Some((type_id, num_fields, field_map)) = resolve_fb_type(&fb_name) {
                // Standard library function block.
                let instance_size = num_fields as u32 * 8;
                let data_offset = crate::data_region::reserve(ctx, instance_size, &span)?;

                ctx.fb_instances.insert(
                    id.clone(),
                    FbInstanceInfo {
                        var_index: index,
                        type_id,
                        data_offset,
                        field_indices: field_map,
                    },
                );
            } else if let Some((num_fields, type_id, field_indices)) =
                ctx.user_fb_types.get(&fb_name).map(|user_fb| {
                    (
                        user_fb.num_fields,
                        user_fb.type_id,
                        user_fb.field_indices.clone(),
                    )
                })
            {
                // User-defined function block.
                let instance_size = num_fields as u32 * 8;
                let data_offset = crate::data_region::reserve(ctx, instance_size, &span)?;

                ctx.fb_instances.insert(
                    id.clone(),
                    FbInstanceInfo {
                        var_index: index,
                        type_id,
                        data_offset,
                        field_indices,
                    },
                );
            }
            (iec_type_tag::FB_INSTANCE, fb_name)
        }
        SemanticType::Array { element_type, .. } => {
            // An array whose elements are structures is laid out as one flat
            // run of slots rather than one slot per element, so it registers
            // through its own path.
            if let SemanticType::Structure { .. } = element_type.as_ref() {
                let debug_type_name = types
                    .name_of(type_id)
                    .map(|name| name.to_string().to_uppercase())
                    .unwrap_or_else(|| {
                        let element = types
                            .element_type(type_id)
                            .and_then(|element| types.name_of(element))
                            .map(|name| name.to_string().to_uppercase())
                            .unwrap_or_default();
                        format!("ARRAY OF {element}")
                    });
                let SemanticType::Array { dimensions, .. } = &representation else {
                    return Err(Diagnostic::internal_error());
                };
                crate::compile_array_struct::register_struct_array_variable(
                    ctx,
                    builder,
                    id,
                    index,
                    element_type,
                    &debug_type_name,
                    dimensions,
                    &span,
                )?
            } else {
                let SemanticType::Array { dimensions, .. } = &representation else {
                    return Err(Diagnostic::internal_error());
                };
                // An element that is a reference is known by the name of
                // the type it refers to.
                let target_name = types
                    .element_type(type_id)
                    .and_then(|element| types.referenced_type(element))
                    .and_then(|target| types.name_of(target))
                    .map(|name| name.name.clone());
                let spec = crate::compile_array::array_spec_from_type(
                    element_type,
                    dimensions,
                    target_name,
                    &span,
                )?;
                crate::compile_array::register_array_variable(
                    ctx, builder, id, index, &spec, &span,
                )?
            }
        }
        SemanticType::Reference { target_type } => {
            crate::compile_reference::register_reference_of_type(
                ctx,
                builder,
                id,
                index,
                target_type,
            )?;
            (iec_type_tag::OTHER, "REF_TO".into())
        }
        SemanticType::Enumeration { .. } => {
            // Enum variables use DINT (W32/Signed/32-bit) per REQ-EN-codegen-010.
            let type_info = crate::compile_enum::enum_var_type_info();
            ctx.var_types.insert(id.clone(), type_info);
            // Debug tag is DINT per REQ-EN-codegen-012; type_name is the
            // enum's debug name (e.g. "COLOR"), REQ-EN-codegen-092.
            let name = crate::compile_enum::debug_name(types, Some(type_id));
            (iec_type_tag::DINT, name)
        }
        SemanticType::Subrange { base_type, .. } => {
            // A subrange operates as its base type.
            if let Some(type_info) = crate::compile_struct::var_type_info_for_field(&representation)
            {
                ctx.var_types.insert(id.clone(), type_info);
            }
            // A named subrange is known by its name; one spelled out in
            // place by its base type.
            let name = match types.name_of(type_id) {
                Some(name) => name.to_string().to_uppercase(),
                None => elementary_name(types, base_type),
            };
            (iec_type_tag::OTHER, name)
        }
        SemanticType::Function { .. } => (iec_type_tag::OTHER, String::new()),
        elementary => {
            if let Some(type_info) = crate::type_info::decl_type_info(ctx, decl) {
                ctx.var_types.insert(id.clone(), type_info);
            }
            // An alias of an elementary type is known by the elementary
            // type, as the analyzer resolves the alias.
            let name = elementary_name(types, elementary);
            let tag = resolve_iec_type_tag(types, &TypeName::from(name.as_str()));
            (tag, name)
        }
    })
}

/// The upper-case name of the elementary type `representation` is, or an
/// empty name when it is not one.
fn elementary_name(types: &TypeEnvironment, representation: &SemanticType) -> String {
    types
        .elementary_type_name_for(representation)
        .map(|name| name.name.to_string().to_uppercase())
        .unwrap_or_default()
}

/// Maps a DSL VariableType to the debug section var_section encoding.
pub(crate) fn map_var_section(vt: &VariableType) -> u8 {
    match vt {
        VariableType::Var => var_section::VAR,
        VariableType::VarTemp => var_section::VAR_TEMP,
        VariableType::Input => var_section::VAR_INPUT,
        VariableType::Output => var_section::VAR_OUTPUT,
        VariableType::InOut => var_section::VAR_IN_OUT,
        VariableType::External => var_section::VAR_EXTERNAL,
        VariableType::Global => var_section::VAR_GLOBAL,
        VariableType::Access => var_section::VAR,
    }
}

/// The debug type tag of the type `type_name` names: the type's id when it
/// is elementary, else `OTHER`.
fn resolve_iec_type_tag(types: &TypeEnvironment, type_name: &TypeName) -> u8 {
    types
        .id_of(type_name)
        .and_then(ironplc_analyzer::type_id::elementary_debug_tag)
        .unwrap_or(iec_type_tag::OTHER)
}

/// Computes the debug `(iec_type_tag, type_name)` pair for a function- or
/// FB-local variable declaration, without the data-region allocation of the
/// program/global path (see [`assign_variables`]), so it is safe to call from
/// the per-function slot-assignment loops in `compile_fn`. Composite or
/// unsupported declarations fall back to [`iec_type_tag::OTHER`] with a
/// best-effort type name.
pub(crate) fn debug_type_for_decl(decl: &VarDecl, types: &TypeEnvironment) -> (u8, String) {
    match &decl.initializer {
        InitialValueAssignmentKind::Simple(simple) => (
            resolve_iec_type_tag(types, &simple.type_name),
            simple.type_name.name.to_string().to_uppercase(),
        ),
        InitialValueAssignmentKind::String(string_init) => {
            if char_width_for_string_type(&string_init.width).is_wide() {
                (iec_type_tag::WSTRING, "WSTRING".into())
            } else {
                (iec_type_tag::STRING, "STRING".into())
            }
        }
        InitialValueAssignmentKind::Reference(_) => (iec_type_tag::OTHER, "REF_TO".into()),
        // A local aggregate keeps its contents in the data region and its slot
        // holds their offset, exactly as a program-level one does.
        InitialValueAssignmentKind::Structure(struct_init) => (
            iec_type_tag::STRUCT,
            struct_init.type_name.to_string().to_uppercase(),
        ),
        InitialValueAssignmentKind::FunctionBlock(fb_init) => (
            iec_type_tag::FB_INSTANCE,
            fb_init.type_name.to_string().to_uppercase(),
        ),
        InitialValueAssignmentKind::EnumeratedType(_)
        | InitialValueAssignmentKind::EnumeratedValues(_) => (
            iec_type_tag::DINT,
            crate::compile_enum::debug_name(types, decl.type_id),
        ),
        _ => (iec_type_tag::OTHER, String::new()),
    }
}

/// Computes the debug `(iec_type_tag, type_name)` pair for a user
/// function's return variable, derived from its declared return type.
pub(crate) fn debug_type_for_return(
    return_type: &FunctionReturnType,
    types: &TypeEnvironment,
) -> (u8, String) {
    match return_type {
        FunctionReturnType::String(_) => (iec_type_tag::STRING, "STRING".into()),
        FunctionReturnType::WString(_) => (iec_type_tag::WSTRING, "WSTRING".into()),
        FunctionReturnType::Named(_) => {
            let type_name = return_type.to_type_name();
            (
                resolve_iec_type_tag(types, &type_name),
                type_name.name.to_string().to_uppercase(),
            )
        }
    }
}

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
        if let Some(struct_info) = ctx.struct_vars.get(id).cloned() {
            crate::compile_struct_init::initialize_struct_variable(
                emitter,
                ctx,
                &struct_info,
                value,
                Writes::Once,
                &span,
            )?;
        } else if let Some(info) = ctx.struct_array_vars.get(id).cloned() {
            crate::compile_struct_init::initialize_struct_array_variable(
                emitter,
                ctx,
                &info,
                value,
                Writes::Once,
                &span,
            )?;
        } else if ctx.array_vars.get(id).is_some_and(|array| !array.is_ref) {
            // A `REF_TO ARRAY` registers array metadata too, but its slot
            // holds a reference, stored below.
            emit_array_initial_value(emitter, ctx, id, value, &span)?;
        } else if let Some(info) = ctx.string_vars.get(id) {
            let (data_offset, max_length, char_width) =
                (info.data_offset, info.max_length, info.char_width);
            // Initialize the string header in the data region, then store
            // the characters. The literal is encoded at the variable's width
            // so the store's encoding check passes (ADR-0034).
            emitter.emit_str_init(data_offset, max_length, char_width);
            if let InitialValue::String(string) = value {
                emit_string_store(emitter, ctx, string, data_offset);
            }
        } else if let Some(fb_info) = ctx.fb_instances.get(id) {
            let data_offset = fb_info.data_offset;
            let var_index = fb_info.var_index;
            // Store the data region byte offset into the variable slot.
            let offset_const = ctx.add_i32_constant(data_offset as i32);
            emitter.emit_load_const_i32(offset_const);
            emitter.emit_store_var_i32(var_index);

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
    let Some(array_info) = ctx.array_vars.get(id) else {
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

        if let Some(info) = ctx.string_vars.get(id) {
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
    if let Some(struct_info) = ctx.struct_vars.get(return_id).cloned() {
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
    } else if let Some(info) = ctx.string_vars.get(return_id) {
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
