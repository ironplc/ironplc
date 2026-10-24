//! Reference (`REF_TO`) variable registration for IEC 61131-3 code generation.
//!
//! A reference variable is stored as a 64-bit unsigned value, in one of the
//! forms `ironplc_container::reference` describes: [`reference_to`] makes a
//! string's a data-region reference to its header, and any other variable's
//! its variable-table index. When the target is an array, the variable also
//! carries array metadata so that `PT^[idx]` can be compiled with the deref
//! array opcodes, and when the target is a string or an array of strings,
//! the descriptor that `STR_LOAD_ARRAY_DEREF` and `STR_STORE_ARRAY_DEREF`
//! reach the strings with ([`StringRefInfo`]). When the target is an array
//! or a structure, it carries the descriptor that sizes the whole of it, for
//! `COPY_REGION_DEREF`. Every declaration site
//! (program, function parameter, function local, function block) registers
//! references through [`register_reference_variable`], or, given the type
//! referenced, [`register_reference_of_type`].

use ironplc_analyzer::{SemanticType, TypeEnvironment};
use ironplc_container::reference::data_region_reference;
use ironplc_container::{CharWidth, ContainerBuilder, FieldType, VarIndex};
use ironplc_dsl::common::{ReferenceInitializer, TypeName};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::Diagnostic;

use super::compile::{CompileContext, OpWidth, Signedness, VarTypeInfo, DEFAULT_STRING_MAX_LENGTH};
use super::compile_array::{
    array_spec_from_named, compute_dimensions, var_type_info_to_type_byte, ArraySpec, ArrayVarInfo,
};

/// The strings a reference variable refers to, a string or an array of
/// strings, as `STR_LOAD_ARRAY_DEREF` and `STR_STORE_ARRAY_DEREF` reach them
/// through the reference: a string is the one element of a one-element
/// array.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StringRefInfo {
    /// The descriptor of the array of strings the reference refers to.
    pub(crate) desc_index: u16,
    /// The encoding of each string, which a value stored through the
    /// reference must be produced at (ADR-0034).
    pub(crate) char_width: CharWidth,
}

/// The reference to the variable `name`, as `REF(name)` makes it: a
/// data-region reference to the header of a string, which lives in the data
/// region, and a variable reference to the slot of any other variable
/// (`specs/design/var-in-out-parameters.md`, "References").
pub(crate) fn reference_to(ctx: &CompileContext, name: &Id) -> Result<u64, Diagnostic> {
    match ctx.string_var(name) {
        Some(string) => Ok(data_region_reference(string.data_offset)),
        None => Ok(u64::from(ctx.var_index(name)?.raw())),
    }
}

/// Registers a `REF_TO` variable: records its 64-bit unsigned storage and,
/// when the target is an array, the array metadata needed to compile
/// `PT^[idx]`.
///
/// The target is resolved through the type environment rather than by
/// inspecting the AST, so `REF_TO ARRAY[0..3] OF INT`, `REF_TO ARR4` (a named
/// array type) and an alias of either all register the same metadata.
pub(crate) fn register_reference_variable(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    types: &TypeEnvironment,
    id: &Id,
    var_index: VarIndex,
    ref_init: &ReferenceInitializer,
) -> Result<(), Diagnostic> {
    let declaring = TypeName { name: id.clone() };
    let target = types.resolve_reference_target(&declaring, &ref_init.target)?;
    register_reference_of_type(ctx, builder, id, var_index, &target)
}

/// Registers a `REF_TO` variable whose target is of type `target`.
pub(crate) fn register_reference_of_type(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    id: &Id,
    var_index: VarIndex,
    target: &SemanticType,
) -> Result<(), Diagnostic> {
    // References are stored as 64-bit variable-table indices (unsigned).
    ctx.var_types.insert(
        ctx.scope,
        id.clone(),
        VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        },
    );

    if let SemanticType::Array {
        element_type,
        dimensions,
    } = target
    {
        let spec = array_spec_from_named(element_type, dimensions, &id.span())?;
        register_ref_to_array_metadata(ctx, builder, id, var_index, &spec)?;
    } else if let SemanticType::Structure { .. } = target {
        // A structure is laid out as a flat array of slots, as a structure
        // variable's region is (`compile_struct::allocate_struct_variable`).
        // One whose size is not known has no descriptor, and no copy through
        // the reference is emitted.
        if let Ok(total_slots) = target.slot_count() {
            let desc_index = builder.add_array_descriptor(FieldType::Slot as u8, total_slots, 0);
            ctx.region_refs.insert(ctx.scope, id.clone(), desc_index);
        }
    } else if let SemanticType::String {
        max_len,
        char_width,
    } = target
    {
        let max_len = max_len.map_or(DEFAULT_STRING_MAX_LENGTH, |len| len as u16);
        register_string_ref(ctx, builder, id, 1, max_len, *char_width);
    }
    Ok(())
}

/// Registers array metadata for a `REF_TO ARRAY` variable so that
/// `PT^[idx]` can be compiled with deref array opcodes.
///
/// No data region space is allocated — the reference parameter
/// points to an array in the caller's scope.
fn register_ref_to_array_metadata(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    id: &Id,
    var_index: VarIndex,
    spec: &ArraySpec,
) -> Result<(), Diagnostic> {
    let span = id.span();
    let element_vti = if spec.ref_to {
        VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }
    } else {
        super::type_info::resolve_type_name(&spec.element_type_name).unwrap_or(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 32,
        })
    };
    let element_type_byte = var_type_info_to_type_byte(&element_vti);
    let (dimensions, total_elements) = compute_dimensions(&spec.dimensions, &span)?;
    let desc_index = builder.add_array_descriptor(element_type_byte, total_elements, 0);
    // An array of strings, not of references to them. Its string
    // descriptor is the one that sizes the whole array.
    let region_desc = if let (false, Some(max_len), Some(char_width)) =
        (spec.ref_to, spec.string_max_len, spec.string_char_width)
    {
        register_string_ref(ctx, builder, id, total_elements, max_len, char_width)
    } else {
        desc_index
    };
    ctx.region_refs.insert(ctx.scope, id.clone(), region_desc);
    ctx.array_vars.insert(
        ctx.scope,
        id.clone(),
        ArrayVarInfo {
            var_index,
            desc_index,
            data_offset: 0,
            element_var_type_info: element_vti,
            total_elements,
            dimensions,
            is_string_element: false,
            string_max_len: 0,
            string_char_width: CharWidth::Narrow,
            is_ref: true,
        },
    );
    Ok(())
}

/// Records that the reference `id` refers to `total_elements` strings of
/// `max_len` code units `char_width` wide each, with the descriptor that
/// `STR_LOAD_ARRAY_DEREF` and `STR_STORE_ARRAY_DEREF` reach them with, and
/// returns that descriptor.
fn register_string_ref(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    id: &Id,
    total_elements: u32,
    max_len: u16,
    char_width: CharWidth,
) -> u16 {
    let element_type = FieldType::string(char_width) as u8;
    let desc_index = builder.add_array_descriptor(element_type, total_elements, max_len);
    ctx.string_refs.insert(
        ctx.scope,
        id.clone(),
        StringRefInfo {
            desc_index,
            char_width,
        },
    );
    desc_index
}
