//! The layout of the program's variables, in the variable-table slots after
//! the globals', in declaration order. A `VAR_EXTERNAL` is its global, so it
//! takes no slot of its own.

use ironplc_analyzer::TypeEnvironment;
use ironplc_container::ContainerBuilder;
use ironplc_dsl::common::{ProgramDeclaration, VarDecl, VariableType};
use ironplc_dsl::diagnostic::Diagnostic;

use super::names::ScopeId;
use super::storage::assign_variables;
use crate::compile::CompileContext;

/// Lays out the variables of `program` other than its `VAR_EXTERNAL`s,
/// under the program's scope, which it leaves current, and returns their
/// declarations, whose starting values `init` stores.
pub(crate) fn lay_out_program(
    ctx: &mut CompileContext,
    builder: &mut ContainerBuilder,
    program: &ProgramDeclaration,
    types: &TypeEnvironment,
) -> Result<Vec<VarDecl>, Diagnostic> {
    let local_vars: Vec<VarDecl> = program
        .variables
        .iter()
        .filter(|v| v.var_type != VariableType::External)
        .cloned()
        .collect();
    ctx.scope = ScopeId::Program;
    assign_variables(ctx, builder, &local_vars, types)?;
    Ok(local_vars)
}
