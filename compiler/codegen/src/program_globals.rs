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
/// `VAR_GLOBAL` blocks (outside any `CONFIGURATION`), the configuration's
/// `VAR_GLOBAL`, then the `VAR_GLOBAL` of the resource that instantiates
/// `program_name`.
///
/// A resource's globals are visible only to the programs of that resource
/// (IEC 61131-3, 2.7.1), so the globals of any other resource are left out.
/// No name repeats across these blocks: the analyzer reports a second
/// global of the same name (P4014).
pub(crate) fn program_globals(
    library: &Library,
    config: Option<&ConfigurationDeclaration>,
    program_name: &Id,
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
        if let Some(resource) = find_program_resource(config, program_name) {
            globals.extend_from_slice(&resource.global_vars);
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{options::CompilerOptions, parse_program};

    /// Parses a configuration whose one resource instantiates the program
    /// type `instantiated`.
    fn library(instantiated: &str) -> Library {
        let source = format!(
            "
CONFIGURATION config
  VAR_GLOBAL
    from_config : INT;
  END_VAR
  RESOURCE resource1 ON PLC
    VAR_GLOBAL
      from_resource : INT;
    END_VAR
    TASK t (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM instance WITH t : {instantiated};
  END_RESOURCE
END_CONFIGURATION"
        );
        parse_program(&source, &FileId::default(), &CompilerOptions::default()).unwrap()
    }

    fn config(library: &Library) -> &ConfigurationDeclaration {
        library
            .elements
            .iter()
            .find_map(|element| match element {
                LibraryElementKind::ConfigurationDeclaration(config) => Some(config),
                _ => None,
            })
            .unwrap()
    }

    fn names(globals: &[VarDecl]) -> Vec<String> {
        globals
            .iter()
            .map(|decl| decl.identifier.to_string())
            .collect()
    }

    #[test]
    fn find_program_resource_when_resource_instantiates_program_then_returns_resource() {
        let library = library("main");

        let resource = find_program_resource(config(&library), &Id::from("main"));

        assert_eq!(resource.unwrap().name, Id::from("resource1"));
    }

    #[test]
    fn find_program_resource_when_no_resource_instantiates_program_then_none() {
        let library = library("other");

        let resource = find_program_resource(config(&library), &Id::from("main"));

        assert!(resource.is_none());
    }

    #[test]
    fn program_globals_when_resource_instantiates_program_then_configuration_then_resource_globals()
    {
        let library = library("main");

        let globals = program_globals(
            &library,
            Some(config(&library)),
            &Id::from("main"),
            &CodegenOptions::default(),
        );

        assert_eq!(names(&globals), vec!["from_config", "from_resource"]);
    }

    #[test]
    fn program_globals_when_resource_instantiates_other_program_then_resource_globals_excluded() {
        let library = library("other");

        let globals = program_globals(
            &library,
            Some(config(&library)),
            &Id::from("main"),
            &CodegenOptions::default(),
        );

        assert_eq!(names(&globals), vec!["from_config"]);
    }

    #[test]
    fn program_globals_when_system_uptime_enabled_then_uptime_globals_first() {
        let library = library("main");
        let options = CodegenOptions {
            system_uptime_global: true,
            ..CodegenOptions::default()
        };

        let globals = program_globals(
            &library,
            Some(config(&library)),
            &Id::from("main"),
            &options,
        );

        assert_eq!(
            names(&globals),
            vec![
                "__SYSTEM_UP_TIME",
                "__SYSTEM_UP_LTIME",
                "from_config",
                "from_resource"
            ]
        );
    }
}
