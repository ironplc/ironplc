//! Finding a program or global variable by name in the debug section.
//!
//! A program's own variables and its globals share one scope, so a name means
//! at most one of them. Tests and tools that refer to a variable the way the
//! program does look it up here rather than assuming where codegen put it.

use crate::debug_section::{function_id, DebugSection, VarNameEntry};

impl DebugSection {
    /// The `VAR_NAME` entries of the program and its globals. A function's
    /// locals are not among them.
    pub fn program_variables(&self) -> impl Iterator<Item = &VarNameEntry> {
        self.var_names
            .iter()
            .filter(|entry| entry.function_id == function_id::GLOBAL_SCOPE)
    }

    /// The program or global variable called `name`, matched without regard
    /// to case as IEC identifiers are.
    pub fn program_variable(&self, name: &str) -> Option<&VarNameEntry> {
        self.program_variables()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
    }
}

#[cfg(test)]
mod tests {
    use std::string::String;
    use std::vec;

    use crate::debug_section::{iec_type_tag, var_section, DebugSection, VarNameEntry};
    use crate::id_types::{FunctionId, VarIndex};

    fn entry(index: u16, function_id: FunctionId, name: &str) -> VarNameEntry {
        VarNameEntry {
            var_index: VarIndex::new(index),
            function_id,
            var_section: var_section::VAR,
            iec_type_tag: iec_type_tag::DINT,
            name: String::from(name),
            type_name: String::from("DINT"),
        }
    }

    fn section() -> DebugSection {
        DebugSection {
            var_names: vec![
                entry(0, FunctionId::GLOBAL_SCOPE, "Count"),
                entry(1, FunctionId::new(1), "local"),
            ],
            ..DebugSection::default()
        }
    }

    #[test]
    fn program_variable_when_name_differs_in_case_then_finds_entry() {
        let debug = section();

        let found = debug.program_variable("COUNT").unwrap();

        assert_eq!(found.var_index, VarIndex::new(0));
    }

    #[test]
    fn program_variable_when_only_function_local_has_name_then_none() {
        assert!(section().program_variable("local").is_none());
    }

    #[test]
    fn program_variables_when_function_local_present_then_excludes_it() {
        let debug = section();

        let names: vec::Vec<&str> = debug
            .program_variables()
            .map(|entry| entry.name.as_str())
            .collect();

        assert_eq!(names, ["Count"]);
    }
}
