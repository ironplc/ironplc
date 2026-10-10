//! Variable setup and initialization for IEC 61131-3 code generation.
//!
//! Contains variable assignment, initial value emission, function local
//! prologue, and type name resolution. Separated from compile.rs to
//! keep module sizes within the 1000-line guideline.

use ironplc_container::debug_section::{
    function_id, iec_type_tag, var_section, StringLayoutEntry, VarNameEntry,
};
use ironplc_container::{ContainerBuilder, VarIndex};
use ironplc_dsl::common::{
    ConstantKind, FunctionReturnType, InitialValueAssignmentKind, ReferenceInitialValue,
    SpecificationKind, SubrangeSpecificationKind, TypeName, VarDecl, VariableType,
};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};

use ironplc_analyzer::semantic_type::SemanticType;
use ironplc_analyzer::system_globals::SYSTEM_UPTIME_GLOBALS;
use ironplc_analyzer::TypeEnvironment;
use ironplc_dsl::type_id::TypeId;
use std::collections::HashMap;

use super::compile::{
    char_width_for_string_type, emit_string_literal_load, string_region_size, CompileContext,
    FbInstanceInfo, OpType, OpWidth, StringVarInfo, DEFAULT_OP_TYPE, DEFAULT_STRING_MAX_LENGTH,
};
use super::compile_call::resolve_fb_type;
use super::compile_expr::{compile_constant, emit_store_var, emit_truncation, resolve_variable};
use super::compile_stmt::resolve_string_max_length;
use crate::emit::Emitter;

/// Assigns variable table indices and type info for all variable declarations.
///
/// The storage each declaration needs is decided by the type the analyzer
/// resolved for it (`VarDecl::type_id`), not by the syntax of its
/// initializer: a structure, an array, a string, a function block instance, a
/// reference or a scalar.
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
            (subrange_iec_type_tag(types, &representation), name)
        }
        // A declaration that names its type with a `SimpleInitializer` (every
        // `VAR_GLOBAL` declaration does, as does one naming a string type)
        // gets from `emit_initial_values` only what a scalar gets: no string
        // header, no data-region offset in its slot, no `NULL` and no
        // enumeration ordinal. Until its starting value is read from its
        // type rather than from its initializer, it keeps the scalar storage
        // and debug entry that match.
        SemanticType::String { .. }
        | SemanticType::Array { .. }
        | SemanticType::FunctionBlock { .. }
        | SemanticType::Reference { .. }
        | SemanticType::Enumeration { .. }
            if matches!(decl.initializer, InitialValueAssignmentKind::Simple(_)) =>
        {
            assign_scalar_storage(ctx, decl, id, types, type_id)
        }
        SemanticType::String {
            max_len,
            char_width,
        } => {
            let (max_length, region_span) = match &decl.initializer {
                InitialValueAssignmentKind::String(string_init) => {
                    (resolve_string_max_length(string_init)?, string_init.span())
                }
                _ => (
                    max_len
                        .map(|len| len as u16)
                        .unwrap_or(DEFAULT_STRING_MAX_LENGTH),
                    span,
                ),
            };
            let char_width = *char_width;

            // Allocate space in the data region: [max_length: u16][cur_length: u16][data]
            let total_bytes = string_region_size(max_length, char_width);
            let data_offset = crate::data_region::reserve(ctx, total_bytes, &region_span)?;

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
            // A member initializer (`(PT := T#100MS)`) is applied by
            // `emit_initial_values`, which runs after the instance has its
            // slot offset -- each member store addresses the instance
            // through it. Nothing to do here but size it.
            let fb_name = declared_name();
            if let Some((type_id, num_fields, field_map)) = resolve_fb_type(&fb_name) {
                let field_op_types = standard_fb_field_op_types(ctx, decl);
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
                        field_op_types,
                    },
                );
            } else if let Some((num_fields, type_id, field_indices, field_op_types)) =
                ctx.user_fb_types.get(&fb_name).map(|user_fb| {
                    (
                        user_fb.num_fields,
                        user_fb.type_id,
                        user_fb.field_indices.clone(),
                        user_fb.field_op_types.clone(),
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
                        field_op_types,
                    },
                );
            }
            (iec_type_tag::FB_INSTANCE, fb_name)
        }
        SemanticType::Array {
            element_type,
            dimensions,
        } => {
            if let SemanticType::Structure { .. } = element_type.as_ref() {
                // An array whose elements are structures is laid out as one
                // flat run of slots rather than one slot per element, so it
                // registers through its own path. An array spelled out in
                // place is known by its element type's name.
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
        SemanticType::Function { .. } => (iec_type_tag::OTHER, String::new()),
        _ => assign_scalar_storage(ctx, decl, id, types, type_id),
    })
}

/// Records how the scalar variable `decl` declares, of the type `type_id`,
/// operates, and returns its debug `(iec_type_tag, type_name)`.
///
/// An alias of an elementary type is known by the elementary type, as the
/// analyzer resolves the alias; any other type by its own name.
fn assign_scalar_storage(
    ctx: &mut CompileContext,
    decl: &VarDecl,
    id: &Id,
    types: &TypeEnvironment,
    type_id: TypeId,
) -> (u8, String) {
    if let Some(type_info) = crate::type_info::decl_type_info(ctx, decl) {
        ctx.var_types.insert(id.clone(), type_info);
    }
    let elementary = types
        .get_by_id(type_id)
        .and_then(|attrs| types.elementary_type_name_for(&attrs.representation));
    match elementary {
        Some(name) => (
            resolve_iec_type_tag(types, &name),
            name.to_string().to_uppercase(),
        ),
        None => (
            iec_type_tag_of(types, type_id),
            types
                .name_of(type_id)
                .map(|name| name.to_string().to_uppercase())
                .unwrap_or_default(),
        ),
    }
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
/// is elementary, its base type's tag when it is a subrange (or an alias of
/// one), else `OTHER`.
fn resolve_iec_type_tag(types: &TypeEnvironment, type_name: &TypeName) -> u8 {
    types
        .id_of(type_name)
        .map(|type_id| iec_type_tag_of(types, type_id))
        .unwrap_or(iec_type_tag::OTHER)
}

/// The debug type tag of the type `type_id`: the type's id when it is
/// elementary, its base type's tag when it is a subrange (or an alias of
/// one), else `OTHER`.
fn iec_type_tag_of(types: &TypeEnvironment, type_id: TypeId) -> u8 {
    if let Some(tag) = ironplc_analyzer::type_id::elementary_debug_tag(type_id) {
        return tag;
    }
    match types.get_by_id(type_id).map(|attrs| &attrs.representation) {
        Some(subrange @ SemanticType::Subrange { .. }) => subrange_iec_type_tag(types, subrange),
        _ => iec_type_tag::OTHER,
    }
}

/// The debug type tag of a subrange type: the tag of the elementary type it
/// is a range of (REQ-SR-023). A subrange's slot holds its value at the base
/// type's width and signedness, so it must render as that type -- a `LINT`
/// subrange rendered as 32 bits shows only the low word. `OTHER` when the
/// base type is not elementary.
/// The operation type of each field of the standard function block instance
/// `decl` declares, from the declared type of the field: a `CTU_LINT`'s `PV`
/// and `CV` are operated as `LINT`s.
fn standard_fb_field_op_types(ctx: &CompileContext, decl: &VarDecl) -> HashMap<String, OpType> {
    let Some(SemanticType::FunctionBlock { fields, .. }) =
        decl.type_id.and_then(|id| ctx.types.get(&id))
    else {
        return HashMap::new();
    };
    fields
        .iter()
        .filter_map(|field| {
            let info = crate::type_info::operand_type_info(&field.field_type)?;
            Some((
                field.name.to_string().to_lowercase(),
                (info.op_width, info.signedness),
            ))
        })
        .collect()
}

fn subrange_iec_type_tag(types: &TypeEnvironment, subrange: &SemanticType) -> u8 {
    let mut base = subrange;
    while let SemanticType::Subrange { base_type, .. } = base {
        base = base_type;
    }
    types
        .elementary_type_name_for(base)
        .and_then(|name| types.id_of(&name))
        .and_then(ironplc_analyzer::type_id::elementary_debug_tag)
        .unwrap_or(iec_type_tag::OTHER)
}

/// The debug type name of a subrange variable: the declared type's name, or
/// the base type's name for an inline subrange.
fn subrange_debug_type_name(spec: &SubrangeSpecificationKind) -> String {
    match spec {
        SpecificationKind::Named(tn) => tn.to_string().to_uppercase(),
        SpecificationKind::Inline(inline) => format!("{}", inline.type_name),
    }
}

/// Computes the debug `(iec_type_tag, type_name)` pair for a function- or
/// FB-local variable declaration. This mirrors the best-effort resolution
/// the program/global path performs (see [`assign_variables`]) but without
/// its side-effecting data-region allocation, so it is safe to call from
/// the per-function slot-assignment loops in `compile_fn`. Composite or
/// unsupported initializers fall back to [`iec_type_tag::OTHER`] with a
/// best-effort type name, matching the global behavior.
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
        InitialValueAssignmentKind::Subrange(spec) => {
            let tag = match &spec.spec {
                SpecificationKind::Named(type_name) => resolve_iec_type_tag(types, type_name),
                SpecificationKind::Inline(inline) => {
                    resolve_iec_type_tag(types, &inline.type_name.clone().into())
                }
            };
            (tag, subrange_debug_type_name(&spec.spec))
        }
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

/// Emits bytecode to initialize variables that have declared initial values.
///
/// For scalar variables with a `SimpleInitializer`, emits load-constant +
/// truncate (if narrow) + store-variable instructions.
///
/// For STRING variables, emits STR_INIT to set up the data region header,
/// then optionally LOAD_CONST_STR + STR_STORE_VAR for the initial value.
pub(crate) fn emit_initial_values(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    declarations: &[VarDecl],
    _types: &TypeEnvironment,
) -> Result<(), Diagnostic> {
    for decl in declarations {
        if let Some(id) = decl.identifier.symbolic_id() {
            match &decl.initializer {
                InitialValueAssignmentKind::Simple(simple) => {
                    // The global_var_decl parser produces Simple for all
                    // named types, including structs.  If the variable was
                    // registered as a struct during assign_variables,
                    // initialize it like a Structure initializer.
                    if let Some(struct_info) = ctx.struct_vars.get(id).cloned() {
                        crate::compile_struct_init::initialize_struct_variable(
                            emitter,
                            ctx,
                            &struct_info,
                            &[],
                            &decl.identifier.span(),
                        )?;
                    } else if let Some(constant) = &simple.initial_value {
                        let var_index = ctx.var_index(id)?;
                        let type_info = ctx.var_type_info(id);
                        let op_type = type_info
                            .map(|ti| (ti.op_width, ti.signedness))
                            .unwrap_or(DEFAULT_OP_TYPE);

                        compile_constant(emitter, ctx, constant, op_type)?;

                        if let Some(ti) = type_info {
                            emit_truncation(emitter, ti);
                        }

                        emit_store_var(emitter, var_index, op_type);
                    }
                }
                InitialValueAssignmentKind::String(string_init) => {
                    if let Some(info) = ctx.string_vars.get(id) {
                        let data_offset = info.data_offset;
                        let max_length = info.max_length;
                        let char_width = info.char_width;

                        // Initialize the string header in the data region.
                        emitter.emit_str_init(data_offset, max_length, char_width);

                        // If there's an initial value, load and store it. The
                        // literal is encoded at the variable's width so the
                        // store's encoding check passes (ADR-0034).
                        if let Some(lit) = &string_init.initial_value {
                            emit_string_literal_load(emitter, ctx, &lit.value, char_width);
                            emitter.emit_str_store_var(data_offset);
                        }
                    }
                }
                InitialValueAssignmentKind::FunctionBlock(fb_init) => {
                    if let Some(fb_info) = ctx.fb_instances.get(id) {
                        let data_offset = fb_info.data_offset;
                        let var_index = fb_info.var_index;
                        // Store the data region byte offset into the variable slot.
                        let offset_const = ctx.add_i32_constant(data_offset as i32);
                        emitter.emit_load_const_i32(offset_const);
                        emitter.emit_store_var_i32(var_index);

                        // `timer : TON := (PT := T#100MS)` sets the instance's
                        // own members. The slot offset has to be in place
                        // first, because each member store addresses the
                        // instance through it.
                        crate::compile_fb_init::emit_fb_instance_member_initializers(
                            emitter,
                            ctx,
                            id,
                            &fb_init.init,
                        )?;
                    }
                }
                InitialValueAssignmentKind::Array(array_init) => {
                    // An array of structures holds the data region offset in
                    // its variable slot, like a structure variable does. Its
                    // element field values are left zeroed, matching what an
                    // array-of-struct field of a structure gets today; only
                    // the headers of its STRING fields are written.
                    if let Some(struct_array_info) = ctx.struct_array_vars.get(id) {
                        if !array_init.initial_values.is_empty() {
                            return Err(Diagnostic::not_implemented(Label::span(
                                decl.identifier.span(),
                                "Initial values for an array of structures",
                            )));
                        }
                        let data_offset = struct_array_info.data_offset;
                        let var_index = struct_array_info.var_index;
                        let scratch_var_index = struct_array_info.scratch_var_index;
                        let element_strings = struct_array_info.element_strings.clone();
                        let offset_const = ctx.add_i32_constant(data_offset as i32);
                        emitter.emit_load_const_i32(offset_const);
                        emitter.emit_store_var_i32(var_index);
                        crate::compile_struct_init::initialize_element_strings(
                            emitter,
                            ctx,
                            data_offset,
                            scratch_var_index,
                            &element_strings,
                            &decl.identifier.span(),
                        )?;
                    } else if let Some(array_info) = ctx.array_vars.get(id) {
                        let data_offset = array_info.data_offset;
                        let var_index = array_info.var_index;
                        let desc_index = array_info.desc_index;
                        let element_vti = array_info.element_var_type_info;
                        let is_string = array_info.is_string_element;
                        let element_char_width = array_info.string_char_width;

                        // Store data_offset into the variable slot (like FB instances).
                        let offset_const = ctx.add_i32_constant(data_offset as i32);
                        emitter.emit_load_const_i32(offset_const);
                        emitter.emit_store_var_i32(var_index);

                        if is_string {
                            // Initialize all string headers in the array.
                            emitter.emit_str_init_array(var_index, desc_index);

                            // Emit STR_STORE_ARRAY_ELEM for each initial string value.
                            // String literals are encoded at the element width so
                            // the array element's encoding check passes.
                            if !array_init.initial_values.is_empty() {
                                let values = crate::compile_array::flatten_array_initial_values(
                                    &array_init.initial_values,
                                )?;
                                for (i, value) in values.iter().enumerate() {
                                    if let ConstantKind::CharacterString(lit) = value {
                                        emit_string_literal_load(
                                            emitter,
                                            ctx,
                                            &lit.value,
                                            element_char_width,
                                        );
                                    } else {
                                        compile_constant(emitter, ctx, value, DEFAULT_OP_TYPE)?;
                                    }
                                    let idx_const = ctx.add_i32_constant(i as i32);
                                    emitter.emit_load_const_i32(idx_const);
                                    emitter.emit_str_store_array_elem(var_index, desc_index);
                                }
                            }
                        } else {
                            // Emit STORE_ARRAY for each initial value.
                            if !array_init.initial_values.is_empty() {
                                let values = crate::compile_array::flatten_array_initial_values(
                                    &array_init.initial_values,
                                )?;
                                let element_op_type =
                                    (element_vti.op_width, element_vti.signedness);
                                for (i, value) in values.iter().enumerate() {
                                    compile_constant(emitter, ctx, value, element_op_type)?;
                                    emit_truncation(emitter, element_vti);
                                    let idx_const = ctx.add_i32_constant(i as i32);
                                    emitter.emit_load_const_i32(idx_const);
                                    emitter.emit_store_array(var_index, desc_index);
                                }
                            }
                        }
                    }
                }
                InitialValueAssignmentKind::Reference(ref_init) => {
                    let var_index = ctx.var_index(id)?;
                    match &ref_init.initial_value {
                        Some(ReferenceInitialValue::Ref(target_var)) => {
                            // REF(var) → load the target variable's index as a u64 constant.
                            let target_index = resolve_variable(ctx, target_var)?;
                            let pool_index = ctx.add_i64_constant(target_index.into());
                            emitter.emit_load_const_i64(pool_index);
                        }
                        _ => {
                            // NULL or no initializer → store null sentinel (u64::MAX).
                            let pool_index = ctx.add_i64_constant(u64::MAX as i64);
                            emitter.emit_load_const_i64(pool_index);
                        }
                    }
                    emitter.emit_store_var_i64(var_index);
                }
                InitialValueAssignmentKind::Structure(struct_init) => {
                    if let Some(struct_info) = ctx.struct_vars.get(id).cloned() {
                        crate::compile_struct_init::initialize_struct_variable(
                            emitter,
                            ctx,
                            &struct_info,
                            &struct_init.elements_init,
                            &decl.identifier.span(),
                        )?;
                    }
                }
                InitialValueAssignmentKind::EnumeratedType(_)
                | InitialValueAssignmentKind::EnumeratedValues(_) => {
                    // Emit LOAD_CONST_I32(ordinal) + STORE_VAR_I32 per REQ-EN-codegen-020.
                    let var_index = ctx.var_index(id)?;
                    emit_enum_initial_value(emitter, ctx, decl, var_index)?;
                }
                InitialValueAssignmentKind::Subrange(ref spec) => {
                    // Initialize subrange variable to its lower bound (min_value)
                    // per IEC 61131-3 §2.4.3.1 (default is the "leftmost value").
                    let var_index = ctx.var_index(id)?;
                    let type_info = ctx.var_type_info(id);
                    let op_type = type_info
                        .map(|ti| (ti.op_width, ti.signedness))
                        .unwrap_or(DEFAULT_OP_TYPE);

                    // Extract min_value from the type environment or inline spec
                    let min_value: Option<i128> = match &spec.spec {
                        SpecificationKind::Named(type_name) => {
                            _types.get(type_name).and_then(|attrs| {
                                if let SemanticType::Subrange { min_value, .. } =
                                    &attrs.representation
                                {
                                    Some(*min_value)
                                } else {
                                    None
                                }
                            })
                        }
                        SpecificationKind::Inline(inline_spec) => {
                            inline_spec.subrange.start.as_signed_integer().map(|si| {
                                if si.is_neg {
                                    -(si.value.value as i128)
                                } else {
                                    si.value.value as i128
                                }
                            })
                        }
                    };

                    if let Some(min_val) = min_value {
                        match op_type.0 {
                            OpWidth::W32 => {
                                let pool_index = ctx.add_i32_constant(min_val as i32);
                                emitter.emit_load_const_i32(pool_index);
                            }
                            OpWidth::W64 => {
                                let pool_index = ctx.add_i64_constant(min_val as i64);
                                emitter.emit_load_const_i64(pool_index);
                            }
                            _ => {
                                let pool_index = ctx.add_i32_constant(min_val as i32);
                                emitter.emit_load_const_i32(pool_index);
                            }
                        }

                        if let Some(ti) = type_info {
                            emit_truncation(emitter, ti);
                        }

                        emit_store_var(emitter, var_index, op_type);
                    }
                }
                // Other initializer kinds (EnumeratedValues, etc.)
                // do not yet support initial values in codegen.
                _ => {}
            }
        }
    }
    Ok(())
}

/// Emits `LOAD_CONST_I32(ordinal)` + `STORE_VAR_I32` storing the ordinal the
/// enumeration variable `decl` starts at (REQ-EN-codegen-020). Every
/// enumeration operates as a `DINT` (REQ-EN-codegen-003), so no truncation
/// follows.
fn emit_enum_initial_value(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    decl: &VarDecl,
    var_index: VarIndex,
) -> Result<(), Diagnostic> {
    let ordinal = crate::compile_enum::initial_ordinal(ctx, decl)?.ok_or_else(|| {
        Diagnostic::internal_error_at(Label::span(
            decl.identifier.span(),
            "Enumeration initial value for a declaration that is not an enumeration",
        ))
    })?;
    let pool_index = ctx.add_i32_constant(ordinal);
    emitter.emit_load_const_i32(pool_index);
    emit_store_var(emitter, var_index, DEFAULT_OP_TYPE);
    Ok(())
}

/// Emits a bytecode prologue that re-initializes a function's non-parameter
/// local variables and return variable on every call. IEC 61131-3 requires
/// functions to be stateless (locals must not retain values between calls).
///
/// For locals with a declared initial value, emits the same LOAD_CONST +
/// TRUNC + STORE_VAR sequence that `emit_initial_values()` uses. For locals
/// without an initializer and for the return variable, emits a zero-store.
pub(crate) fn emit_function_local_prologue(
    emitter: &mut Emitter,
    ctx: &mut CompileContext,
    variables: &[VarDecl],
    return_id: &Id,
    return_var_index: VarIndex,
    return_op_type: OpType,
) -> Result<(), Diagnostic> {
    // Re-initialize VAR locals (not Input parameters).
    for decl in variables {
        if decl.var_type != VariableType::Var {
            continue;
        }
        if let Some(id) = decl.identifier.symbolic_id() {
            let var_index = ctx.var_index(id)?;
            let type_info = ctx.var_type_info(id);
            let op_type = type_info
                .map(|ti| (ti.op_width, ti.signedness))
                .unwrap_or(DEFAULT_OP_TYPE);

            match &decl.initializer {
                InitialValueAssignmentKind::Simple(simple) => {
                    if let Some(constant) = &simple.initial_value {
                        // Has an explicit initial value: emit LOAD_CONST + TRUNC + STORE.
                        compile_constant(emitter, ctx, constant, op_type)?;
                        if let Some(ti) = type_info {
                            emit_truncation(emitter, ti);
                        }
                    } else {
                        // No initializer: zero-fill.
                        emit_zero_const(emitter, ctx, op_type);
                    }
                    emit_store_var(emitter, var_index, op_type);
                }
                InitialValueAssignmentKind::String(string_init) => {
                    // Re-initialize STRING locals: emit STR_INIT to reset the
                    // header, then optionally load the initial value.
                    if let Some(info) = ctx.string_vars.get(id) {
                        let data_offset = info.data_offset;
                        let max_length = info.max_length;
                        let char_width = info.char_width;
                        emitter.emit_str_init(data_offset, max_length, char_width);

                        if let Some(lit) = &string_init.initial_value {
                            emit_string_literal_load(emitter, ctx, &lit.value, char_width);
                            emitter.emit_str_store_var(data_offset);
                        }
                    }
                }
                InitialValueAssignmentKind::Reference(ref_init) => {
                    match &ref_init.initial_value {
                        Some(ReferenceInitialValue::Ref(target_var)) => {
                            let target_index = resolve_variable(ctx, target_var)?;
                            let pool_index = ctx.add_i64_constant(target_index.into());
                            emitter.emit_load_const_i64(pool_index);
                        }
                        _ => {
                            // NULL or no initializer: store null sentinel (u64::MAX).
                            let pool_index = ctx.add_i64_constant(u64::MAX as i64);
                            emitter.emit_load_const_i64(pool_index);
                        }
                    }
                    emitter.emit_store_var_i64(var_index);
                }
                InitialValueAssignmentKind::EnumeratedType(_)
                | InitialValueAssignmentKind::EnumeratedValues(_) => {
                    // Re-initialize enum locals per REQ-EN-codegen-023.
                    emit_enum_initial_value(emitter, ctx, decl, var_index)?;
                }
                _ => {
                    // Other initializer kinds; zero-fill as default.
                    // `Structure` and `Array` reach this arm, and zero-filling
                    // discards their declared field values.
                    emit_zero_const(emitter, ctx, op_type);
                    emit_store_var(emitter, var_index, op_type);
                }
            }
        }
    }

    // Zero-initialize the return variable.
    if let Some(struct_info) = ctx.struct_vars.get(return_id).cloned() {
        // Struct return: store data_offset into the return var slot and
        // zero all struct fields. Functions are stateless, so the struct
        // must be re-initialized on every call. The struct was registered
        // under `return_var_index`, so `struct_info.var_index` is that slot.
        crate::compile_struct_init::initialize_struct_variable(
            emitter,
            ctx,
            &struct_info,
            &[],
            &return_id.span(),
        )?;
    } else if let Some(info) = ctx.string_vars.get(return_id) {
        // STRING/WSTRING return: initialize the string header in the data region.
        emitter.emit_str_init(info.data_offset, info.max_length, info.char_width);
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
