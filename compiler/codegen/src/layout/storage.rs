//! The storage of one variable a global or program declaration declares:
//! its variable-table slot, the data region it reserves for a string, an
//! array, a structure or a function block instance, and its debug entry.

use std::collections::HashMap;

use ironplc_analyzer::semantic_type::SemanticType;
use ironplc_analyzer::TypeEnvironment;
use ironplc_container::debug_section::{
    function_id, iec_type_tag, StringLayoutEntry, VarNameEntry,
};
use ironplc_container::{ContainerBuilder, VarIndex};
use ironplc_dsl::common::{InitialValueAssignmentKind, VarDecl};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::type_id::TypeId;

use super::debug_type::{
    iec_type_tag_of, map_var_section, resolve_iec_type_tag, subrange_iec_type_tag,
};
use crate::compile::{
    string_region_size, CompileContext, FbInstanceInfo, StringVarInfo, VarTypeInfo,
    DEFAULT_STRING_MAX_LENGTH,
};
use crate::compile_call::standard_fb_layout;
use crate::compile_stmt::resolve_string_max_length;

/// Assigns variable table indices and type info for all variable declarations.
///
/// The storage each declaration needs is decided by the type the analyzer
/// resolved for it (`VarDecl::type_id`), not by the syntax of its
/// initializer: a structure, an array, a string, a function block instance, a
/// reference or a scalar.
pub(super) fn assign_variables(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    declarations: &[VarDecl],
    types: &TypeEnvironment,
) -> Result<(), Diagnostic> {
    for decl in declarations {
        if let Some(id) = decl.identifier.symbolic_id() {
            let index = VarIndex::new(ctx.slot_count());
            ctx.variables.insert(ctx.scope, id.clone(), index);
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
                ctx.var_types.insert(ctx.scope, id.clone(), type_info);
            }
            // A named subrange is known by its name; one spelled out in
            // place by its base type.
            let name = match types.name_of(type_id) {
                Some(name) => name.to_string().to_uppercase(),
                None => elementary_name(types, base_type),
            };
            (subrange_iec_type_tag(types, &representation), name)
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
                ctx.scope,
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
            let standard_block = types
                .get_by_id(type_id)
                .and_then(|attributes| attributes.standard_block);
            if let Some((fb_type_id, num_fields, field_map)) =
                standard_block.map(standard_fb_layout)
            {
                let field_types = standard_fb_field_types(types, type_id);
                // Standard library function block.
                let instance_size = num_fields as u32 * 8;
                let data_offset = crate::data_region::reserve(ctx, instance_size, &span)?;

                ctx.fb_instances.insert(
                    ctx.scope,
                    id.clone(),
                    FbInstanceInfo {
                        var_index: index,
                        type_id: fb_type_id,
                        data_offset,
                        field_indices: field_map,
                        field_types,
                        held: Vec::new(),
                    },
                );
            } else if let Some((num_fields, type_id, field_indices, field_types)) =
                ctx.user_fb_type(&fb_name).map(|user_fb| {
                    (
                        user_fb.num_fields,
                        user_fb.type_id,
                        user_fb.field_indices.clone(),
                        user_fb.field_types.clone(),
                    )
                })
            {
                // User-defined function block.
                let instance_size = num_fields as u32 * 8;
                let data_offset = crate::data_region::reserve(ctx, instance_size, &span)?;
                let held = crate::held_instances::reserve(ctx, &fb_name, data_offset, &span)?;

                ctx.fb_instances.insert(
                    ctx.scope,
                    id.clone(),
                    FbInstanceInfo {
                        var_index: index,
                        type_id,
                        data_offset,
                        field_indices,
                        field_types,
                        held,
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
            ctx.var_types.insert(ctx.scope, id.clone(), type_info);
            // Debug tag is DINT per REQ-EN-codegen-012; type_name is the
            // enum's debug name (e.g. "COLOR"), REQ-EN-codegen-092.
            let name = crate::compile_enum::debug_name(types, Some(type_id));
            (iec_type_tag::DINT, name)
        }
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
        ctx.var_types.insert(ctx.scope, id.clone(), type_info);
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

/// The type of each field of the standard function block `block`, from the
/// declared type of the field the type environment records: a `CTU_LINT`'s
/// `PV` and `CV` are operated as `LINT`s, and a `CTU`'s are stored as `INT`s.
fn standard_fb_field_types(types: &TypeEnvironment, block: TypeId) -> HashMap<String, VarTypeInfo> {
    types
        .block_fields(block)
        .unwrap_or_default()
        .iter()
        .filter_map(|field| {
            let field_type = &types.get_by_id(field.field_type?)?.representation;
            let info = crate::type_info::operand_type_info(field_type)?;
            Some((field.name.to_string().to_lowercase(), info))
        })
        .collect()
}
