//! Collects the global variable declarations the compiled program can reach.
//!
//! Codegen allocates these declarations at the start of the variable table
//! (indices `0..G`), before the program's own variables. A `VAR_EXTERNAL`
//! in the program resolves by name to one of them.

use ironplc_analyzer::system_globals::SYSTEM_UPTIME_GLOBALS;
use ironplc_dsl::common::{Library, LibraryElementKind, VarDecl, VariableType};
use ironplc_dsl::configuration::{ConfigurationDeclaration, ResourceDeclaration};
use ironplc_dsl::core::Id;

use crate::compile::CodegenOptions;

/// Returns the global declarations to allocate for the compiled program, in
/// variable-table order.
///
/// The order is: the system uptime globals (when enabled), top-level
/// `VAR_GLOBAL` blocks (outside any `CONFIGURATION`), then the
/// configuration's `VAR_GLOBAL`.
pub(crate) fn program_globals(
    library: &Library,
    config: Option<&ConfigurationDeclaration>,
    options: &CodegenOptions,
) -> Vec<VarDecl> {
    let mut globals: Vec<VarDecl> = Vec::new();

    if options.system_uptime_global {
        for global in &SYSTEM_UPTIME_GLOBALS {
            globals.push(
                VarDecl::simple(global.name, global.type_name).with_type(VariableType::Global),
            );
        }
    }

    for element in &library.elements {
        if let LibraryElementKind::GlobalVarDeclarations(decls) = element {
            globals.extend_from_slice(decls);
        }
    }

    if let Some(config) = config {
        globals.extend_from_slice(&config.global_var);
    }

    globals
}

/// Finds the `RESOURCE` of `config` that instantiates the program type
/// `program_name`.
///
/// `PROGRAM <instance> WITH <task> : <type>` names the program *type*, so the
/// match is on `type_name`. Returns `None` when no resource instantiates the
/// program.
pub(crate) fn find_program_resource<'a>(
    config: &'a ConfigurationDeclaration,
    program_name: &Id,
) -> Option<&'a ResourceDeclaration> {
    config.resource_decl.iter().find(|resource| {
        resource
            .programs
            .iter()
            .any(|program| &program.type_name == program_name)
    })
}
