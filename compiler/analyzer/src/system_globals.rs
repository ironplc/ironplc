//! The globals the compiler provides when `allow_system_uptime_global` is on.
//!
//! The `__` prefix marks a name only the compiler can provide (see
//! `specs/design/compatibility-libraries.md`, *Non-Goals*). Every place that
//! seeds, types, lays out or guards these names reads this one table, so a
//! new reserved global is added here and nowhere else.

use ironplc_dsl::common::{VarDecl, VariableType};
use ironplc_dsl::core::Id;

use crate::symbol_environment::{ScopeKind, SymbolEnvironment};

/// A compiler-provided global: its name and the name of its type.
pub struct SystemGlobal {
    pub name: &'static str,
    pub type_name: &'static str,
}

/// The implicit uptime globals, in variable-table order: the VM writes
/// slot 0 and slot 1 with these, so codegen lays them out first.
pub const SYSTEM_UPTIME_GLOBALS: [SystemGlobal; 2] = [
    SystemGlobal {
        name: "__SYSTEM_UP_TIME",
        type_name: "TIME",
    },
    SystemGlobal {
        name: "__SYSTEM_UP_LTIME",
        type_name: "LTIME",
    },
];

/// The declarations of the compiler-provided globals the analysis declared,
/// in [`SYSTEM_UPTIME_GLOBALS`] order: each a `VAR_GLOBAL` of its type, with
/// the type id and the declaration identity its symbol records, so a back
/// end lays them out like any other global and every reference to one finds
/// its storage. Empty when the analysis provided none.
pub fn declarations(symbols: &SymbolEnvironment) -> Vec<VarDecl> {
    SYSTEM_UPTIME_GLOBALS
        .iter()
        .filter_map(|global| {
            let info = symbols
                .find(&Id::from(global.name), &ScopeKind::Global)
                .filter(|info| info.compiler_provided)?;
            let mut decl =
                VarDecl::simple(global.name, global.type_name).with_type(VariableType::Global);
            decl.type_id = info.type_id;
            decl.decl_id = info.decl_id;
            Some(decl)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ironplc_dsl::core::Id;
    use ironplc_parser::options::CompilerOptions;
    use spec_test_macro::spec_test;

    use super::declarations;
    use crate::symbol_environment::ScopeKind;
    use crate::test_helpers::parse_and_resolve_types_with_options;

    fn declarations_with(allow_system_uptime_global: bool) -> Vec<ironplc_dsl::common::VarDecl> {
        let options = CompilerOptions {
            allow_system_uptime_global,
            ..CompilerOptions::default()
        };
        let (_, context) =
            parse_and_resolve_types_with_options("PROGRAM main END_PROGRAM", &options);
        declarations(context.symbols())
    }

    #[spec_test(REQ_VB_analyzer_003)]
    #[test]
    fn declarations_when_uptime_globals_provided_then_each_carries_its_symbols_decl_id() {
        let options = CompilerOptions {
            allow_system_uptime_global: true,
            ..CompilerOptions::default()
        };
        let (_, context) =
            parse_and_resolve_types_with_options("PROGRAM main END_PROGRAM", &options);

        let decls = declarations(context.symbols());

        assert_eq!(decls.len(), 2);
        let symbol = |name: &str| {
            context
                .symbols()
                .find(&Id::from(name), &ScopeKind::Global)
                .and_then(|info| info.decl_id)
        };
        assert!(decls[0].decl_id.is_some());
        assert_eq!(decls[0].decl_id, symbol("__SYSTEM_UP_TIME"));
        assert_eq!(decls[1].decl_id, symbol("__SYSTEM_UP_LTIME"));
        assert_ne!(decls[0].decl_id, decls[1].decl_id);
    }

    #[test]
    fn declarations_when_uptime_globals_not_provided_then_empty() {
        assert!(declarations_with(false).is_empty());
    }
}
