//! Collects every global variable declaration in a library.

use std::convert::Infallible;

use ironplc_dsl::{
    common::{Library, VarDecl, VariableType},
    visitor::Visitor,
};

/// Returns every `VAR_GLOBAL` declaration in `lib`, in walk order.
///
/// The library a rule sees is the merge of every source and activated
/// library, so this covers globals declared in a `CONFIGURATION`, in a
/// `RESOURCE`, at the top level (the `--allow-top-level-var-global`
/// extension) and in a compatibility library alike. Located
/// declarations (`AT %IW0`) are included; callers that need a name
/// filter on `identifier.symbolic_id()`.
pub fn collect_global_var_decls(lib: &Library) -> Vec<VarDecl> {
    let mut collector = GlobalVarCollector { decls: Vec::new() };
    let Ok(()) = collector.walk(lib);
    collector.decls
}

struct GlobalVarCollector {
    decls: Vec<VarDecl>,
}

impl Visitor<Infallible> for GlobalVarCollector {
    type Value = ();

    fn visit_var_decl(&mut self, node: &VarDecl) -> Result<Self::Value, Infallible> {
        if node.var_type == VariableType::Global {
            self.decls.push(node.clone());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{options::CompilerOptions, parse_program};

    fn names(program: &str, options: &CompilerOptions) -> Vec<String> {
        let lib = parse_program(program, &FileId::default(), options).unwrap();
        collect_global_var_decls(&lib)
            .iter()
            .map(|d| d.identifier.to_string())
            .collect()
    }

    #[test]
    fn collect_global_var_decls_when_configuration_and_resource_globals_then_returns_both() {
        let found = names(
            "
CONFIGURATION config
  VAR_GLOBAL
    from_config : INT;
  END_VAR
  RESOURCE res ON PLC
    VAR_GLOBAL
      from_resource : INT;
    END_VAR
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : main;
  END_RESOURCE
END_CONFIGURATION

PROGRAM main
VAR
  local : INT;
END_VAR
END_PROGRAM",
            &CompilerOptions::default(),
        );
        assert_eq!(found, vec!["from_config", "from_resource"]);
    }

    #[test]
    fn collect_global_var_decls_when_top_level_global_then_returned() {
        let found = names(
            "
VAR_GLOBAL
  top : INT;
END_VAR

PROGRAM main
VAR
  local : INT;
END_VAR
END_PROGRAM",
            &CompilerOptions {
                allow_top_level_var_global: true,
                ..CompilerOptions::default()
            },
        );
        assert_eq!(found, vec!["top"]);
    }

    #[test]
    fn collect_global_var_decls_when_no_globals_then_empty() {
        let found = names(
            "
PROGRAM main
VAR
  local : INT;
END_VAR
END_PROGRAM",
            &CompilerOptions::default(),
        );
        assert!(found.is_empty());
    }
}
