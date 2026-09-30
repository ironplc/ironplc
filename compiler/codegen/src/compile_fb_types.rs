//! Registration of the user-defined function block types.
//!
//! Everything a caller needs to know about a user-defined function block
//! type -- its field layout, its type ID, the function ID of its body and
//! the shape of its methods -- is knowable from the AST alone. It is
//! registered here, before any variable is assigned a slot, so that
//! `assign_variables` can size an instance of the type and a call can find
//! its fields. The bodies themselves are compiled later, once the
//! variable offsets are known.

use std::collections::HashMap;

use ironplc_container::FunctionId;
use ironplc_dsl::common::{
    FunctionBlockDeclaration, InitialValueAssignmentKind, VarDecl, VariableType,
};

use crate::compile::{CompileContext, OpType, UserFbTypeInfo, UserMethodInfo, DEFAULT_OP_TYPE};

/// Registers every function block of `fb_decls` in `ctx.user_fb_types`.
///
/// Function IDs are positional: the function block bodies take the IDs
/// from 2 in declaration order, then come the `num_user_functions` user
/// FUNCTIONs, then the methods (OOP extension, ADR-0041 Phase 1).
pub(crate) fn register_user_fb_types(
    ctx: &mut CompileContext,
    fb_decls: &[&FunctionBlockDeclaration],
    num_user_functions: usize,
) {
    register_fields(ctx, fb_decls);
    register_methods(ctx, fb_decls, num_user_functions);
}

/// Registers each type's field indices, field op types and type ID.
fn register_fields(ctx: &mut CompileContext, fb_decls: &[&FunctionBlockDeclaration]) {
    for (next_function_id, fb_decl) in (2_u16..).zip(fb_decls.iter()) {
        let fb_name = fb_decl.name.name.to_string().to_uppercase();
        let mut field_indices: HashMap<String, u8> = HashMap::new();
        let mut field_op_types: HashMap<String, OpType> = HashMap::new();
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
        for (i, decl) in field_decls_tmp.iter().enumerate() {
            if let Some(id) = decl.identifier.symbolic_id() {
                let name = id.to_string().to_lowercase();
                field_indices.insert(name.clone(), i as u8);
                if let InitialValueAssignmentKind::Simple(_) = &decl.initializer {
                    if let Some(vti) = crate::type_info::decl_type_info(ctx, decl) {
                        field_op_types.insert(name, (vti.op_width, vti.signedness));
                    } else {
                        field_op_types.insert(name, DEFAULT_OP_TYPE);
                    }
                } else {
                    field_op_types.insert(name, DEFAULT_OP_TYPE);
                }
            }
        }

        let type_id = ctx.next_user_fb_type_id;
        ctx.next_user_fb_type_id += 1;
        ctx.user_fb_types.insert(
            fb_name,
            UserFbTypeInfo {
                type_id,
                num_fields: field_decls_tmp.len(),
                field_indices,
                function_id: FunctionId::new(next_function_id),
                var_offset: 0, // updated after program vars are assigned
                field_op_types,
                methods: HashMap::new(),
            },
        );
    }
}

/// Registers the METHOD declarations (OOP extension, ADR-0041 Phase 1):
/// their function IDs, parameter shapes, and return-value presence. Method
/// function IDs continue the sequence after every FB body and every user
/// FUNCTION; `param_var_off`/`max_stack_depth` stay `0` until the method is
/// actually compiled (mirrors `UserFbTypeInfo::var_offset`).
fn register_methods(
    ctx: &mut CompileContext,
    fb_decls: &[&FunctionBlockDeclaration],
    num_user_functions: usize,
) {
    let mut next_method_function_id = 2_u16 + fb_decls.len() as u16 + num_user_functions as u16;
    for fb_decl in fb_decls.iter() {
        let fb_name = fb_decl.name.name.to_string().to_uppercase();
        for method in &fb_decl.methods {
            let method_name = method.name.to_string().to_lowercase();
            let mut param_names_in_order: Vec<String> = Vec::new();
            let mut param_op_types: Vec<OpType> = Vec::new();
            for decl in &method.variables {
                if !decl.var_type.is_input_compatible() {
                    continue;
                }
                if let Some(id) = decl.identifier.symbolic_id() {
                    param_names_in_order.push(id.to_string().to_lowercase());
                }
                let op_type = if let InitialValueAssignmentKind::Simple(_) = &decl.initializer {
                    crate::type_info::decl_type_info(ctx, decl)
                        .map_or(DEFAULT_OP_TYPE, |vti| (vti.op_width, vti.signedness))
                } else {
                    DEFAULT_OP_TYPE
                };
                param_op_types.push(op_type);
            }

            let function_id = FunctionId::new(next_method_function_id);
            next_method_function_id += 1;

            if let Some(fb_info) = ctx.user_fb_types.get_mut(&fb_name) {
                fb_info.methods.insert(
                    method_name,
                    UserMethodInfo {
                        function_id,
                        param_var_off: 0, // updated once the method is compiled
                        num_params: param_names_in_order.len() as u16,
                        param_names_in_order,
                        param_op_types,
                        has_return_value: method.return_type.is_some(),
                        max_stack_depth: 0, // updated once the method is compiled
                    },
                );
            }
        }
    }
}
