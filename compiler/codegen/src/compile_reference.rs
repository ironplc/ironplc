//! Reference (`REF_TO`) variable registration for IEC 61131-3 code generation.
//!
//! A reference variable is stored as a 64-bit unsigned variable-table index.
//! When the target is an array, the variable also carries array metadata so
//! that `PT^[idx]` can be compiled with the deref array opcodes. Every
//! declaration site (program, function parameter, function local, function
//! block) registers references through [`register_reference_variable`].

use ironplc_container::{CharWidth, ContainerBuilder, VarIndex};
use ironplc_dsl::common::{ReferenceInitializer, ReferenceTarget};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::Diagnostic;

use super::compile::{CompileContext, OpWidth, Signedness, VarTypeInfo};
use super::compile_array::{
    array_spec_from_inline, compute_dimensions, var_type_info_to_type_byte, ArrayVarInfo,
};

/// Registers a `REF_TO` variable: records its 64-bit unsigned storage and,
/// when the target is an array, the array metadata needed to compile
/// `PT^[idx]`.
pub(crate) fn register_reference_variable(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    id: &Id,
    var_index: VarIndex,
    ref_init: &ReferenceInitializer,
) -> Result<(), Diagnostic> {
    // References are stored as 64-bit variable-table indices (unsigned).
    ctx.var_types.insert(
        id.clone(),
        VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        },
    );
    register_ref_to_array_metadata(ctx, builder, id, var_index, ref_init)
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
    ref_init: &ReferenceInitializer,
) -> Result<(), Diagnostic> {
    // Only inline array targets (`REF_TO ARRAY[0..3] OF INT`) are registered
    // here. A named target (`REF_TO ARR4` where `ARR4` is an array type) is
    // not, so `PT^[idx]` on it fails with P9999; #1580 registers named
    // targets in the fix that follows this prefactor.
    if let ReferenceTarget::Array(subranges) = &ref_init.target {
        let span = id.span();
        let spec = array_spec_from_inline(subranges, &span)?;
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
        ctx.array_vars.insert(
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
    }
    Ok(())
}
