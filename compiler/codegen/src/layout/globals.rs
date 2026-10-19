//! The layout of the globals, from the first slot of the variable table: the
//! compiler-provided globals of the execution model, then the declared ones,
//! each in the model's order.

use ironplc_analyzer::system_globals::SYSTEM_UPTIME_GLOBALS;
use ironplc_analyzer::TypeEnvironment;
use ironplc_container::debug_section::{function_id, var_section, VarNameEntry};
use ironplc_container::{ContainerBuilder, VarIndex};
use ironplc_dsl::common::{TypeName, VarDecl};
use ironplc_dsl::core::Id;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_ir::execution::SystemGlobal;

use super::debug_type::resolve_iec_type_tag;
use super::names::ScopeId;
use super::storage::assign_variables;
use crate::compile::CompileContext;

/// Lays out the compiler-provided globals `system_globals`, then the
/// declared globals `global_vars`, from the first slot of the variable
/// table, under the globals' scope.
pub(crate) fn lay_out_globals(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    system_globals: &[SystemGlobal],
    global_vars: &[VarDecl],
    types: &TypeEnvironment,
) -> Result<(), Diagnostic> {
    ctx.scope = ScopeId::Global;
    assign_system_globals(ctx, system_globals, types)?;
    assign_variables(ctx, builder, global_vars, types)
}

/// Assigns the compiler-provided globals of the execution model, such as
/// `__SYSTEM_UP_TIME`, the next variable-table slots, in the model's order.
/// The model lists them first, and the VM writes them before every scan, so
/// they have no starting value to store.
fn assign_system_globals(
    ctx: &mut CompileContext,
    system_globals: &[SystemGlobal],
    types: &TypeEnvironment,
) -> Result<(), Diagnostic> {
    for kind in system_globals {
        let global = SYSTEM_UPTIME_GLOBALS
            .iter()
            .find(|global| global.kind == *kind)
            .ok_or_else(|| {
                Diagnostic::internal_error_at(Label::file(
                    ironplc_dsl::core::FileId::default(),
                    "System global has no entry in the system globals table",
                ))
            })?;
        let id = Id::from(global.name);
        let index = VarIndex::new(ctx.slot_count());
        ctx.variables.insert(ctx.scope, id.clone(), index);
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
    Ok(())
}
