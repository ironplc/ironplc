//! Variable setup and initialization for IEC 61131-3 code generation.
//!
//! Contains variable assignment, initial value emission, function local
//! prologue, and type name resolution. Separated from compile.rs to
//! keep module sizes within the 1000-line guideline.

use ironplc_container::debug_section::{function_id, iec_type_tag, var_section, VarNameEntry};
use ironplc_container::{ContainerBuilder, VarIndex, STRING_HEADER_BYTES};
use ironplc_dsl::common::{
    ElementaryTypeName, FunctionDeclaration, GenericTypeName, InitialValueAssignmentKind,
    ReferenceInitialValue, SpecificationKind, VarDecl, VariableType,
};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

use ironplc_analyzer::intermediate_type::IntermediateType;
use ironplc_analyzer::TypeEnvironment;

use super::compile::{
    CompileContext, FbInstanceInfo, OpType, OpWidth, Signedness, StringVarInfo, VarTypeInfo,
    DEFAULT_OP_TYPE,
};
use super::compile_call::resolve_fb_type;
use super::compile_expr::{compile_constant, emit_store_var, emit_truncation, resolve_variable};
use super::compile_stmt::resolve_string_max_length;
use crate::emit::Emitter;

/// Assigns variable table indices and type info for all variable declarations.
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

            // Resolve type info and collect debug metadata.
            let (type_tag, type_name_str) = match &decl.initializer {
                InitialValueAssignmentKind::Simple(simple) => {
                    // The global_var_decl parser produces Simple for all named
                    // types, including structs.  Detect struct types via the
                    // type environment and register them properly so that field
                    // access works in codegen.
                    if types.resolve_struct_type(&simple.type_name).is_some() {
                        crate::compile_struct::allocate_struct_variable(
                            ctx,
                            builder,
                            types,
                            &simple.type_name,
                            id,
                            index,
                            &decl.identifier.span(),
                        )?;
                        let type_name_str = simple.type_name.to_string().to_uppercase();
                        (iec_type_tag::OTHER, type_name_str)
                    } else if let Some(subrange_type) =
                        types.resolve_subrange_type(&simple.type_name)
                    {
                        // Named subrange type with explicit init (e.g., x : MY_RANGE := 75)
                        if let Some(type_info) =
                            crate::compile_struct::var_type_info_for_field(subrange_type)
                        {
                            ctx.var_types.insert(id.clone(), type_info);
                        }
                        let name = simple.type_name.to_string().to_uppercase();
                        (iec_type_tag::OTHER, name)
                    } else {
                        if let Some(type_info) = resolve_type_name(&simple.type_name.name) {
                            ctx.var_types.insert(id.clone(), type_info);
                        }
                        let tag = resolve_iec_type_tag(&simple.type_name.name);
                        let name = simple.type_name.name.to_string().to_uppercase();
                        (tag, name)
                    }
                }
                InitialValueAssignmentKind::String(string_init) => {
                    let max_length = resolve_string_max_length(string_init)?;

                    // Allocate space in the data region: [max_length: u16][cur_length: u16][data]
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
                    (iec_type_tag::STRING, "STRING".into())
                }
                InitialValueAssignmentKind::FunctionBlock(fb_init) => {
                    let fb_name = fb_init.type_name.to_string().to_uppercase();
                    if let Some((type_id, num_fields, field_map)) = resolve_fb_type(&fb_name) {
                        // Standard library function block.
                        let instance_size = num_fields as u32 * 8;
                        let data_offset = ctx.data_region_offset;
                        ctx.data_region_offset = ctx
                            .data_region_offset
                            .checked_add(instance_size)
                            .ok_or_else(|| {
                                Diagnostic::problem(
                                    Problem::NotImplemented,
                                    Label::span(decl.identifier.span(), "Data region overflow"),
                                )
                            })?;

                        ctx.fb_instances.insert(
                            id.clone(),
                            FbInstanceInfo {
                                var_index: index,
                                type_id,
                                data_offset,
                                field_indices: field_map,
                            },
                        );
                    } else if let Some(user_fb) = ctx.user_fb_types.get(&fb_name) {
                        // User-defined function block.
                        let instance_size = user_fb.num_fields as u32 * 8;
                        let data_offset = ctx.data_region_offset;
                        ctx.data_region_offset = ctx
                            .data_region_offset
                            .checked_add(instance_size)
                            .ok_or_else(|| {
                                Diagnostic::problem(
                                    Problem::NotImplemented,
                                    Label::span(decl.identifier.span(), "Data region overflow"),
                                )
                            })?;

                        ctx.fb_instances.insert(
                            id.clone(),
                            FbInstanceInfo {
                                var_index: index,
                                type_id: user_fb.type_id,
                                data_offset,
                                field_indices: user_fb.field_indices.clone(),
                            },
                        );
                    }
                    (iec_type_tag::OTHER, fb_name)
                }
                InitialValueAssignmentKind::Array(array_init) => {
                    let spec = match &array_init.spec {
                        SpecificationKind::Inline(array_subranges) => {
                            crate::compile_array::array_spec_from_inline(
                                array_subranges,
                                &decl.identifier.span(),
                            )?
                        }
                        SpecificationKind::Named(type_name) => {
                            let array_type =
                                types.resolve_array_type(type_name).ok_or_else(|| {
                                    Diagnostic::problem(
                                        Problem::NotImplemented,
                                        Label::span(type_name.span(), "Unknown array type"),
                                    )
                                })?;
                            let IntermediateType::Array {
                                element_type,
                                dimensions,
                            } = array_type
                            else {
                                unreachable!("resolve_array_type guarantees Array variant");
                            };
                            crate::compile_array::array_spec_from_named(element_type, dimensions)?
                        }
                    };
                    crate::compile_array::register_array_variable(
                        ctx,
                        builder,
                        id,
                        index,
                        &spec,
                        &decl.identifier.span(),
                    )?
                }
                InitialValueAssignmentKind::Reference(ref_init) => {
                    // References are stored as 64-bit variable-table indices (unsigned).
                    ctx.var_types.insert(
                        id.clone(),
                        VarTypeInfo {
                            op_width: OpWidth::W64,
                            signedness: Signedness::Unsigned,
                            storage_bits: 64,
                        },
                    );
                    crate::compile_array::register_ref_to_array_metadata(
                        ctx, builder, id, index, ref_init,
                    )?;
                    (iec_type_tag::OTHER, "REF_TO".into())
                }
                InitialValueAssignmentKind::Structure(struct_init) => {
                    crate::compile_struct::allocate_struct_variable(
                        ctx,
                        builder,
                        types,
                        &struct_init.type_name,
                        id,
                        index,
                        &decl.identifier.span(),
                    )?;
                    let type_name_str = struct_init.type_name.to_string().to_uppercase();
                    (iec_type_tag::OTHER, type_name_str)
                }
                InitialValueAssignmentKind::EnumeratedType(enum_init) => {
                    // Enum variables use DINT (W32/Signed/32-bit) per REQ-EN-010.
                    let type_info = crate::compile_enum::enum_var_type_info();
                    ctx.var_types.insert(id.clone(), type_info);
                    // Debug tag is DINT per REQ-EN-012; type_name is the
                    // user-defined enum name (e.g. "COLOR").
                    let name = enum_init.type_name.to_string().to_uppercase();
                    (iec_type_tag::DINT, name)
                }
                InitialValueAssignmentKind::Subrange(ref spec) => {
                    // Subrange variable (e.g., x : MY_RANGE or x : INT (1..100))
                    // Resolve VarTypeInfo from the subrange's base type.
                    let subrange_type = match spec {
                        SpecificationKind::Named(type_name) => {
                            types.resolve_subrange_type(type_name)
                        }
                        SpecificationKind::Inline(inline_spec) => {
                            let base_tn: ironplc_dsl::common::TypeName =
                                inline_spec.type_name.clone().into();
                            types.get(&base_tn).map(|attrs| &attrs.representation)
                        }
                    };
                    if let Some(st) = subrange_type {
                        if let Some(type_info) = crate::compile_struct::var_type_info_for_field(st)
                        {
                            ctx.var_types.insert(id.clone(), type_info);
                        }
                    }
                    let name = match spec {
                        SpecificationKind::Named(tn) => tn.to_string().to_uppercase(),
                        SpecificationKind::Inline(inline) => {
                            format!("{}", inline.type_name)
                        }
                    };
                    (iec_type_tag::OTHER, name)
                }
                InitialValueAssignmentKind::LateResolvedType(_) => {
                    // LateResolvedType should have been resolved before codegen.
                    // If we reach here, it indicates a bug in the compiler.
                    return Err(Diagnostic::internal_error(file!(), line!()));
                }
                // Other initializer kinds (EnumeratedValues, etc.)
                // do not yet have type info tracked in codegen.
                _ => (iec_type_tag::OTHER, String::new()),
            };

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

/// Maps a DSL VariableType to the debug section var_section encoding.
fn map_var_section(vt: &VariableType) -> u8 {
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

/// Maps an IEC 61131-3 type name to its debug type tag.
fn resolve_iec_type_tag(name: &Id) -> u8 {
    match ElementaryTypeName::try_from(name) {
        Ok(elem) => match elem {
            ElementaryTypeName::BOOL => iec_type_tag::BOOL,
            ElementaryTypeName::SINT => iec_type_tag::SINT,
            ElementaryTypeName::INT => iec_type_tag::INT,
            ElementaryTypeName::DINT => iec_type_tag::DINT,
            ElementaryTypeName::LINT => iec_type_tag::LINT,
            ElementaryTypeName::USINT => iec_type_tag::USINT,
            ElementaryTypeName::UINT => iec_type_tag::UINT,
            ElementaryTypeName::UDINT => iec_type_tag::UDINT,
            ElementaryTypeName::ULINT => iec_type_tag::ULINT,
            ElementaryTypeName::REAL => iec_type_tag::REAL,
            ElementaryTypeName::LREAL => iec_type_tag::LREAL,
            ElementaryTypeName::BYTE => iec_type_tag::BYTE,
            ElementaryTypeName::WORD => iec_type_tag::WORD,
            ElementaryTypeName::DWORD => iec_type_tag::DWORD,
            ElementaryTypeName::LWORD => iec_type_tag::LWORD,
            ElementaryTypeName::STRING => iec_type_tag::STRING,
            ElementaryTypeName::WSTRING => iec_type_tag::WSTRING,
            ElementaryTypeName::TIME => iec_type_tag::TIME,
            ElementaryTypeName::LTIME => iec_type_tag::LTIME,
            ElementaryTypeName::DATE => iec_type_tag::DATE,
            ElementaryTypeName::LDATE => iec_type_tag::LDATE,
            ElementaryTypeName::TimeOfDay => iec_type_tag::TIME_OF_DAY,
            ElementaryTypeName::LTimeOfDay => iec_type_tag::LTOD,
            ElementaryTypeName::DateAndTime => iec_type_tag::DATE_AND_TIME,
            ElementaryTypeName::LDateAndTime => iec_type_tag::LDT,
        },
        Err(()) => iec_type_tag::OTHER,
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
                    if let Some(struct_info) = ctx.struct_vars.get(id) {
                        let data_offset = struct_info.data_offset;
                        let var_index = struct_info.var_index;
                        let desc_index = struct_info.desc_index;
                        let fields: Vec<_> = struct_info
                            .fields
                            .iter()
                            .map(|f| crate::compile_struct::FieldInitInfo {
                                name: f.name.clone(),
                                slot_offset: f.slot_offset,
                                field_type: f.field_type.clone(),
                                op_type: f.op_type,
                                string_max_length: f.string_max_length,
                            })
                            .collect();

                        let offset_const = ctx.add_i32_constant(data_offset as i32);
                        emitter.emit_load_const_i32(offset_const);
                        emitter.emit_store_var_i32(var_index);

                        crate::compile_struct::initialize_struct_fields(
                            emitter,
                            ctx,
                            var_index,
                            desc_index,
                            data_offset,
                            &fields,
                            &[],
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

                        // Initialize the string header in the data region.
                        emitter.emit_str_init(data_offset, max_length);

                        // If there's an initial value, load and store it.
                        if let Some(chars) = &string_init.initial_value {
                            // Convert chars to Latin-1 bytes (STRING encoding per ADR-0016).
                            let bytes: Vec<u8> = chars.iter().map(|&ch| ch as u8).collect();
                            let pool_index = ctx.add_str_constant(bytes);
                            ctx.num_temp_bufs += 1;
                            emitter.emit_load_const_str(pool_index);
                            emitter.emit_str_store_var(data_offset);
                        }
                    }
                }
                InitialValueAssignmentKind::FunctionBlock(_) => {
                    if let Some(fb_info) = ctx.fb_instances.get(id) {
                        let data_offset = fb_info.data_offset;
                        let var_index = fb_info.var_index;
                        // Store the data region byte offset into the variable slot.
                        let offset_const = ctx.add_i32_constant(data_offset as i32);
                        emitter.emit_load_const_i32(offset_const);
                        emitter.emit_store_var_i32(var_index);
                    }
                }
                InitialValueAssignmentKind::Array(array_init) => {
                    if let Some(array_info) = ctx.array_vars.get(id) {
                        let data_offset = array_info.data_offset;
                        let var_index = array_info.var_index;
                        let desc_index = array_info.desc_index;
                        let element_vti = array_info.element_var_type_info;
                        let is_string = array_info.is_string_element;

                        // Store data_offset into the variable slot (like FB instances).
                        let offset_const = ctx.add_i32_constant(data_offset as i32);
                        emitter.emit_load_const_i32(offset_const);
                        emitter.emit_store_var_i32(var_index);

                        if is_string {
                            // Initialize all string headers in the array.
                            emitter.emit_str_init_array(var_index, desc_index);

                            // Emit STR_STORE_ARRAY_ELEM for each initial string value.
                            if !array_init.initial_values.is_empty() {
                                let values = crate::compile_array::flatten_array_initial_values(
                                    &array_init.initial_values,
                                )?;
                                for (i, value) in values.iter().enumerate() {
                                    compile_constant(emitter, ctx, value, DEFAULT_OP_TYPE)?;
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
                    if let Some(struct_info) = ctx.struct_vars.get(id) {
                        // Extract needed values before mutable borrow of ctx.
                        let data_offset = struct_info.data_offset;
                        let var_index = struct_info.var_index;
                        let desc_index = struct_info.desc_index;
                        let fields: Vec<_> = struct_info
                            .fields
                            .iter()
                            .map(|f| crate::compile_struct::FieldInitInfo {
                                name: f.name.clone(),
                                slot_offset: f.slot_offset,
                                field_type: f.field_type.clone(),
                                op_type: f.op_type,
                                string_max_length: f.string_max_length,
                            })
                            .collect();

                        // Store data_offset into the variable slot
                        let offset_const = ctx.add_i32_constant(data_offset as i32);
                        emitter.emit_load_const_i32(offset_const);
                        emitter.emit_store_var_i32(var_index);

                        // Initialize each field
                        crate::compile_struct::initialize_struct_fields(
                            emitter,
                            ctx,
                            var_index,
                            desc_index,
                            data_offset,
                            &fields,
                            &struct_init.elements_init,
                        )?;
                    }
                }
                InitialValueAssignmentKind::EnumeratedType(enum_init) => {
                    // Emit LOAD_CONST_I32(ordinal) + STORE_VAR_I32 per REQ-EN-020.
                    let var_index = ctx.var_index(id)?;
                    let op_type = DEFAULT_OP_TYPE;
                    let ordinal = if let Some(ev) = &enum_init.initial_value {
                        crate::compile_enum::resolve_enum_ordinal(&ctx.enum_map, ev)?
                    } else {
                        // No explicit init: use type declaration default (REQ-EN-021/022).
                        let type_upper = enum_init.type_name.to_string().to_uppercase();
                        crate::compile_enum::resolve_enum_default_ordinal(
                            &ctx.enum_map,
                            &type_upper,
                        )
                    };
                    let pool_index = ctx.add_i32_constant(ordinal);
                    emitter.emit_load_const_i32(pool_index);
                    emit_store_var(emitter, var_index, op_type);
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
                    let min_value: Option<i128> = match spec {
                        SpecificationKind::Named(type_name) => {
                            _types.get(type_name).and_then(|attrs| {
                                if let IntermediateType::Subrange { min_value, .. } =
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
    func_decl: &FunctionDeclaration,
    return_var_index: VarIndex,
    return_op_type: OpType,
) -> Result<(), Diagnostic> {
    // Re-initialize VAR locals (not Input parameters).
    for decl in &func_decl.variables {
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
                        emitter.emit_str_init(data_offset, max_length);

                        if let Some(chars) = &string_init.initial_value {
                            let bytes: Vec<u8> = chars.iter().map(|&ch| ch as u8).collect();
                            let pool_index = ctx.add_str_constant(bytes);
                            ctx.num_temp_bufs += 1;
                            emitter.emit_load_const_str(pool_index);
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
                InitialValueAssignmentKind::EnumeratedType(enum_init) => {
                    // Re-initialize enum locals per REQ-EN-023.
                    let ordinal = if let Some(ev) = &enum_init.initial_value {
                        crate::compile_enum::resolve_enum_ordinal(&ctx.enum_map, ev)?
                    } else {
                        let type_upper = enum_init.type_name.to_string().to_uppercase();
                        crate::compile_enum::resolve_enum_default_ordinal(
                            &ctx.enum_map,
                            &type_upper,
                        )
                    };
                    let pool_index = ctx.add_i32_constant(ordinal);
                    emitter.emit_load_const_i32(pool_index);
                    emit_store_var(emitter, var_index, op_type);
                }
                _ => {
                    // Other initializer kinds (FunctionBlock, etc.)
                    // are not expected in function locals; zero-fill as default.
                    emit_zero_const(emitter, ctx, op_type);
                    emit_store_var(emitter, var_index, op_type);
                }
            }
        }
    }

    // Zero-initialize the return variable.
    if let Some(struct_info) = ctx.struct_vars.get(&func_decl.name).cloned() {
        // Struct return: store data_offset into the return var slot and
        // zero all struct fields. Functions are stateless, so the struct
        // must be re-initialized on every call.
        let offset_const = ctx.add_i32_constant(struct_info.data_offset as i32);
        emitter.emit_load_const_i32(offset_const);
        emitter.emit_store_var_i32(return_var_index);

        let fields: Vec<_> = struct_info
            .fields
            .iter()
            .map(|f| crate::compile_struct::FieldInitInfo {
                name: f.name.clone(),
                slot_offset: f.slot_offset,
                field_type: f.field_type.clone(),
                op_type: f.op_type,
                string_max_length: f.string_max_length,
            })
            .collect();

        crate::compile_struct::initialize_struct_fields(
            emitter,
            ctx,
            return_var_index,
            struct_info.desc_index,
            struct_info.data_offset,
            &fields,
            &[],
        )?;
    } else if let Some(info) = ctx.string_vars.get(&func_decl.name) {
        // STRING return: initialize the string header in the data region.
        emitter.emit_str_init(info.data_offset, info.max_length);
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

/// Maps an IEC 61131-3 type name to its `VarTypeInfo`.
///
/// Returns `None` for unrecognized type names (e.g., user-defined types)
/// and for STRING/WSTRING which are handled separately.
pub(crate) fn resolve_type_name(name: &Id) -> Option<VarTypeInfo> {
    // Try as elementary type first (the common case), then fall back to
    // generic types mapped to their default concrete representation.
    // Generic types may reach codegen for expressions like `5 + 5` where
    // no concrete type context was available during type resolution.
    let elem = ElementaryTypeName::try_from(name)
        .or_else(|_| match GenericTypeName::try_from(name)? {
            GenericTypeName::AnyInt | GenericTypeName::AnyNum | GenericTypeName::AnyMagnitude => {
                Ok(ElementaryTypeName::DINT)
            }
            GenericTypeName::AnyReal => Ok(ElementaryTypeName::REAL),
            _ => Err(()),
        })
        .ok()?;
    match elem {
        ElementaryTypeName::SINT => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 8,
        }),
        ElementaryTypeName::INT => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 16,
        }),
        ElementaryTypeName::DINT => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 32,
        }),
        ElementaryTypeName::LINT => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Signed,
            storage_bits: 64,
        }),
        ElementaryTypeName::USINT => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 8,
        }),
        ElementaryTypeName::UINT => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 16,
        }),
        ElementaryTypeName::UDINT => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 32,
        }),
        ElementaryTypeName::ULINT => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }),
        ElementaryTypeName::REAL => Some(VarTypeInfo {
            op_width: OpWidth::F32,
            signedness: Signedness::Signed,
            storage_bits: 32,
        }),
        ElementaryTypeName::LREAL => Some(VarTypeInfo {
            op_width: OpWidth::F64,
            signedness: Signedness::Signed,
            storage_bits: 64,
        }),
        ElementaryTypeName::BOOL => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 1,
        }),
        ElementaryTypeName::BYTE => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 8,
        }),
        ElementaryTypeName::WORD => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 16,
        }),
        ElementaryTypeName::DWORD => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 32,
        }),
        ElementaryTypeName::LWORD => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }),
        ElementaryTypeName::TIME => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: 32,
        }),
        ElementaryTypeName::LTIME => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Signed,
            storage_bits: 64,
        }),
        ElementaryTypeName::DATE => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 32,
        }),
        ElementaryTypeName::TimeOfDay => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 32,
        }),
        ElementaryTypeName::DateAndTime => Some(VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Unsigned,
            storage_bits: 32,
        }),
        ElementaryTypeName::LDATE => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }),
        ElementaryTypeName::LTimeOfDay => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }),
        ElementaryTypeName::LDateAndTime => Some(VarTypeInfo {
            op_width: OpWidth::W64,
            signedness: Signedness::Unsigned,
            storage_bits: 64,
        }),
        // STRING and WSTRING are handled separately in codegen
        ElementaryTypeName::STRING | ElementaryTypeName::WSTRING => None,
    }
}
