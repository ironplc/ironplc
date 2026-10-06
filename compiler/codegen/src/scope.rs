//! The variable mappings one body is compiled against.
//!
//! A user-defined function, a function block and a program each resolve names
//! through their own set of mappings: where a variable lives in the variable
//! table, and what is known about its type. [`CompileContext`] holds the active
//! set as a handful of loose fields; [`Scope`] is the same set as a value, so a
//! body's mappings can be built, installed and put back as a unit.

use std::collections::{HashMap, HashSet};

use ironplc_container::VarIndex;
use ironplc_dsl::core::Id;

use crate::compile::{CompileContext, FbInstanceInfo, StringVarInfo, VarTypeInfo};
use crate::compile_array::ArrayVarInfo;
use crate::compile_array_struct::StructArrayVarInfo;
use crate::compile_struct::StructVarInfo;

/// The variable mappings one body is compiled against.
///
/// These are the same collections, with the same key and value types, as the
/// fields of the same names on [`CompileContext`]; [`CompileContext::swap_scope`]
/// moves a whole `Scope` in and out of them.
#[derive(Default)]
pub(crate) struct Scope {
    /// Maps variable identifiers to their variable table indices.
    pub(crate) variables: HashMap<Id, VarIndex>,
    /// Maps variable identifiers to their type information.
    pub(crate) var_types: HashMap<Id, VarTypeInfo>,
    /// Maps STRING variable identifiers to their data region metadata.
    pub(crate) string_vars: HashMap<Id, StringVarInfo>,
    /// Maps array variable identifiers to their metadata.
    pub(crate) array_vars: HashMap<Id, ArrayVarInfo>,
    /// Maps structure variable identifiers to their metadata.
    pub(crate) struct_vars: HashMap<Id, StructVarInfo>,
    /// Maps top-level `ARRAY OF <struct>` variable identifiers to their metadata.
    pub(crate) struct_array_vars: HashMap<Id, StructArrayVarInfo>,
    /// Maps FB instance variable identifiers to their metadata.
    pub(crate) fb_instances: HashMap<Id, FbInstanceInfo>,
    /// The `VAR_IN_OUT` parameters of the function being compiled.
    pub(crate) in_out_params: HashSet<Id>,
}

impl Scope {
    /// The scope the body of a user-defined function is compiled against.
    ///
    /// `program` is the program-level scope and `num_globals` the number of
    /// global variables, which occupy variable table indices below it.
    ///
    /// - `variables` keeps the entries whose index is below `num_globals`.
    ///   `var_types`, `string_vars`, `struct_vars` and `struct_array_vars` keep
    ///   the entries whose identifier is one of those variables.
    /// - `array_vars` starts empty, so a global array is not visible.
    /// - `fb_instances` is the program-level map, unfiltered.
    /// - `in_out_params` starts empty; the function's own `VAR_IN_OUT`
    ///   declarations fill it.
    pub(crate) fn for_function_body(program: &Scope, num_globals: u16) -> Scope {
        Scope {
            fb_instances: program.fb_instances.clone(),
            ..Scope::globals_of(program, num_globals)
        }
    }

    /// The scope the body of a user-defined function block is compiled against.
    ///
    /// `program` is the program-level scope and `num_globals` the number of
    /// global variables, which occupy variable table indices below it.
    ///
    /// - `variables`, `var_types`, `string_vars`, `struct_vars` and
    ///   `struct_array_vars` keep what [`Scope::for_function_body`] keeps.
    /// - `array_vars` starts empty, so a global array is not visible.
    /// - `fb_instances` starts empty.
    /// - `in_out_params` is the program-level set, unfiltered.
    pub(crate) fn for_fb_body(program: &Scope, num_globals: u16) -> Scope {
        Scope {
            in_out_params: program.in_out_params.clone(),
            ..Scope::globals_of(program, num_globals)
        }
    }

    /// The five collections that carry the program's globals into a body,
    /// filtered to the globals; every other collection is empty.
    ///
    /// A global is a variable whose index in `program.variables` is below
    /// `num_globals`. The filter is by that index: an entry whose identifier is
    /// not in `program.variables` is dropped.
    fn globals_of(program: &Scope, num_globals: u16) -> Scope {
        let is_global = |id: &Id| {
            program
                .variables
                .get(id)
                .is_some_and(|index| index.raw() < num_globals)
        };
        Scope {
            variables: program
                .variables
                .iter()
                .filter(|(_, index)| index.raw() < num_globals)
                .map(|(id, index)| (id.clone(), *index))
                .collect(),
            var_types: keep_globals(&program.var_types, is_global),
            string_vars: keep_globals(&program.string_vars, is_global),
            struct_vars: keep_globals(&program.struct_vars, is_global),
            struct_array_vars: keep_globals(&program.struct_array_vars, is_global),
            ..Scope::default()
        }
    }
}

/// Clones the entries of `entries` whose identifier satisfies `is_global`.
fn keep_globals<V: Clone>(
    entries: &HashMap<Id, V>,
    is_global: impl Fn(&Id) -> bool,
) -> HashMap<Id, V> {
    entries
        .iter()
        .filter(|(id, _)| is_global(id))
        .map(|(id, value)| (id.clone(), value.clone()))
        .collect()
}

impl CompileContext {
    /// Installs `scope` as the active variable mappings and returns the ones
    /// it replaces.
    ///
    /// All eight collections are replaced wholesale, so nothing of the previous
    /// scope remains visible until it is swapped back in.
    pub(crate) fn swap_scope(&mut self, scope: Scope) -> Scope {
        Scope {
            variables: std::mem::replace(&mut self.variables, scope.variables),
            var_types: std::mem::replace(&mut self.var_types, scope.var_types),
            string_vars: std::mem::replace(&mut self.string_vars, scope.string_vars),
            array_vars: std::mem::replace(&mut self.array_vars, scope.array_vars),
            struct_vars: std::mem::replace(&mut self.struct_vars, scope.struct_vars),
            struct_array_vars: std::mem::replace(
                &mut self.struct_array_vars,
                scope.struct_array_vars,
            ),
            fb_instances: std::mem::replace(&mut self.fb_instances, scope.fb_instances),
            in_out_params: std::mem::replace(&mut self.in_out_params, scope.in_out_params),
        }
    }
}

#[cfg(test)]
mod tests {
    use ironplc_analyzer::SemanticType;
    use ironplc_container::{CharWidth, SlotIndex};
    use rstest::rstest;

    use super::*;
    use crate::compile::{OpWidth, Signedness};

    /// Identifiers are laid out around `NUM_GLOBALS`: two globals (the second
    /// is the last one below the limit), the first non-global (exactly at the
    /// limit), a later local, and one that is in no `variables` entry at all.
    const NUM_GLOBALS: u16 = 2;
    const GLOBAL: &str = "global";
    const LAST_GLOBAL: &str = "last_global";
    const FIRST_LOCAL: &str = "first_local";
    const LOCAL: &str = "local";
    const ORPHAN: &str = "orphan";

    /// Each entry's `tag` is carried in a field of its value, so a test can tell
    /// that a kept entry is the program's entry and not a look-alike.
    fn tag_of(name: &str) -> u32 {
        match name {
            GLOBAL => 100,
            LAST_GLOBAL => 101,
            FIRST_LOCAL => 102,
            LOCAL => 103,
            _ => 104,
        }
    }

    fn type_info(name: &str) -> VarTypeInfo {
        VarTypeInfo {
            op_width: OpWidth::W32,
            signedness: Signedness::Signed,
            storage_bits: tag_of(name) as u8,
        }
    }

    fn string_info(name: &str) -> StringVarInfo {
        StringVarInfo {
            data_offset: tag_of(name),
            max_length: 80,
            char_width: CharWidth::Narrow,
        }
    }

    fn struct_info(name: &str) -> StructVarInfo {
        StructVarInfo {
            var_index: VarIndex::new(0),
            data_offset: tag_of(name),
            total_slots: SlotIndex::new(0),
            desc_index: 0,
            fields: Vec::new(),
            field_index: HashMap::new(),
            scratch_var_index: None,
            string_array_descs: HashMap::new(),
            element_strings: Vec::new(),
        }
    }

    fn struct_array_info(name: &str) -> StructArrayVarInfo {
        StructArrayVarInfo {
            var_index: VarIndex::new(0),
            desc_index: 0,
            data_offset: tag_of(name),
            element_type: SemanticType::Bool,
            dimensions: Vec::new(),
            scratch_var_index: None,
            element_strings: Vec::new(),
        }
    }

    fn array_info(name: &str) -> ArrayVarInfo {
        ArrayVarInfo {
            var_index: VarIndex::new(0),
            desc_index: 0,
            data_offset: tag_of(name),
            element_var_type_info: type_info(name),
            total_elements: 0,
            dimensions: Vec::new(),
            is_string_element: false,
            string_max_len: 0,
            string_char_width: CharWidth::Narrow,
            is_ref: false,
        }
    }

    fn fb_info(name: &str) -> FbInstanceInfo {
        FbInstanceInfo {
            var_index: VarIndex::new(0),
            type_id: 0,
            data_offset: tag_of(name),
            field_indices: HashMap::new(),
        }
    }

    fn id(name: &str) -> Id {
        Id::from(name)
    }

    /// The identifiers in a collection, sorted, as strings.
    fn names<'a>(ids: impl Iterator<Item = &'a Id>) -> Vec<String> {
        let mut names: Vec<String> = ids.map(|id| id.to_string()).collect();
        names.sort();
        names
    }

    fn expected(names: &[&str]) -> Vec<String> {
        let mut expected: Vec<String> = names.iter().map(|name| name.to_string()).collect();
        expected.sort();
        expected
    }

    const GLOBALS: [&str; 2] = [GLOBAL, LAST_GLOBAL];
    const EVERY_ID: [&str; 5] = [GLOBAL, LAST_GLOBAL, FIRST_LOCAL, LOCAL, ORPHAN];

    /// A program-level scope with something in every collection. `variables`
    /// has the two globals, the first local at index `NUM_GLOBALS` and a later
    /// local; every other collection has an entry for each of the five
    /// identifiers, including the one `variables` does not know.
    fn program_scope() -> Scope {
        let mut scope = Scope::default();
        scope.variables.insert(id(GLOBAL), VarIndex::new(0));
        scope.variables.insert(id(LAST_GLOBAL), VarIndex::new(1));
        scope.variables.insert(id(FIRST_LOCAL), VarIndex::new(2));
        scope.variables.insert(id(LOCAL), VarIndex::new(7));
        for name in EVERY_ID {
            scope.var_types.insert(id(name), type_info(name));
            scope.string_vars.insert(id(name), string_info(name));
            scope.array_vars.insert(id(name), array_info(name));
            scope.struct_vars.insert(id(name), struct_info(name));
            scope
                .struct_array_vars
                .insert(id(name), struct_array_info(name));
            scope.fb_instances.insert(id(name), fb_info(name));
            scope.in_out_params.insert(id(name));
        }
        scope
    }

    type Constructor = fn(&Scope, u16) -> Scope;

    #[rstest]
    #[case::function(Scope::for_function_body)]
    #[case::fb(Scope::for_fb_body)]
    fn constructor_when_ids_straddle_num_globals_then_variables_keeps_only_indices_below_it(
        #[case] construct: Constructor,
    ) {
        let scope = construct(&program_scope(), NUM_GLOBALS);

        assert_eq!(names(scope.variables.keys()), expected(&GLOBALS));
        assert_eq!(scope.variables[&id(GLOBAL)], VarIndex::new(0));
        assert_eq!(scope.variables[&id(LAST_GLOBAL)], VarIndex::new(1));
    }

    #[rstest]
    #[case::function(Scope::for_function_body)]
    #[case::fb(Scope::for_fb_body)]
    fn constructor_when_ids_straddle_num_globals_then_dependent_collections_keep_only_global_ids(
        #[case] construct: Constructor,
    ) {
        let scope = construct(&program_scope(), NUM_GLOBALS);

        assert_eq!(names(scope.var_types.keys()), expected(&GLOBALS));
        assert_eq!(names(scope.string_vars.keys()), expected(&GLOBALS));
        assert_eq!(names(scope.struct_vars.keys()), expected(&GLOBALS));
        assert_eq!(names(scope.struct_array_vars.keys()), expected(&GLOBALS));
    }

    #[rstest]
    #[case::function(Scope::for_function_body)]
    #[case::fb(Scope::for_fb_body)]
    fn constructor_when_global_has_entries_then_they_are_the_programs_entries(
        #[case] construct: Constructor,
    ) {
        let scope = construct(&program_scope(), NUM_GLOBALS);

        let last_global = id(LAST_GLOBAL);
        assert_eq!(
            scope.var_types[&last_global].storage_bits,
            tag_of(LAST_GLOBAL) as u8
        );
        assert_eq!(
            scope.string_vars[&last_global].data_offset,
            tag_of(LAST_GLOBAL)
        );
        assert_eq!(
            scope.struct_vars[&last_global].data_offset,
            tag_of(LAST_GLOBAL)
        );
        assert_eq!(
            scope.struct_array_vars[&last_global].data_offset,
            tag_of(LAST_GLOBAL)
        );
    }

    #[rstest]
    #[case::function(Scope::for_function_body)]
    #[case::fb(Scope::for_fb_body)]
    fn constructor_when_program_has_array_vars_then_array_vars_is_empty(
        #[case] construct: Constructor,
    ) {
        let program = program_scope();
        assert!(program.array_vars.contains_key(&id(GLOBAL)));

        let scope = construct(&program, NUM_GLOBALS);

        assert!(scope.array_vars.is_empty());
    }

    #[rstest]
    #[case::function(Scope::for_function_body)]
    #[case::fb(Scope::for_fb_body)]
    fn constructor_when_num_globals_is_zero_then_no_variable_carries_over(
        #[case] construct: Constructor,
    ) {
        let scope = construct(&program_scope(), 0);

        assert!(scope.variables.is_empty());
        assert!(scope.var_types.is_empty());
        assert!(scope.string_vars.is_empty());
        assert!(scope.struct_vars.is_empty());
        assert!(scope.struct_array_vars.is_empty());
    }

    #[test]
    fn for_function_body_when_program_has_fb_instances_then_all_carry_over_unfiltered() {
        let scope = Scope::for_function_body(&program_scope(), NUM_GLOBALS);

        assert_eq!(names(scope.fb_instances.keys()), expected(&EVERY_ID));
        assert_eq!(
            scope.fb_instances[&id(LOCAL)].data_offset,
            tag_of(LOCAL),
            "the entry is the program's, not a look-alike"
        );
    }

    #[test]
    fn for_function_body_when_program_has_in_out_params_then_none_carry_over() {
        let program = program_scope();
        assert!(!program.in_out_params.is_empty());

        let scope = Scope::for_function_body(&program, NUM_GLOBALS);

        assert!(scope.in_out_params.is_empty());
    }

    #[test]
    fn for_fb_body_when_program_has_fb_instances_then_none_carry_over() {
        let program = program_scope();
        assert!(!program.fb_instances.is_empty());

        let scope = Scope::for_fb_body(&program, NUM_GLOBALS);

        assert!(scope.fb_instances.is_empty());
    }

    #[test]
    fn for_fb_body_when_program_has_in_out_params_then_all_carry_over_unfiltered() {
        let scope = Scope::for_fb_body(&program_scope(), NUM_GLOBALS);

        assert_eq!(names(scope.in_out_params.iter()), expected(&EVERY_ID));
    }

    #[test]
    fn swap_scope_when_called_then_installs_new_scope_and_returns_previous() {
        let mut ctx = CompileContext::new();
        ctx.swap_scope(program_scope());

        let mut replacement = Scope::default();
        replacement.variables.insert(id("other"), VarIndex::new(9));
        replacement.in_out_params.insert(id("other"));
        let previous = ctx.swap_scope(replacement);

        assert_eq!(names(ctx.variables.keys()), expected(&["other"]));
        assert_eq!(names(ctx.in_out_params.iter()), expected(&["other"]));
        assert!(ctx.var_types.is_empty());
        assert!(ctx.string_vars.is_empty());
        assert!(ctx.array_vars.is_empty());
        assert!(ctx.struct_vars.is_empty());
        assert!(ctx.struct_array_vars.is_empty());
        assert!(ctx.fb_instances.is_empty());
        assert_eq!(previous.variables.len(), 4);
        assert_eq!(previous.var_types.len(), EVERY_ID.len());
        assert_eq!(previous.string_vars.len(), EVERY_ID.len());
        assert_eq!(previous.array_vars.len(), EVERY_ID.len());
        assert_eq!(previous.struct_vars.len(), EVERY_ID.len());
        assert_eq!(previous.struct_array_vars.len(), EVERY_ID.len());
        assert_eq!(previous.fb_instances.len(), EVERY_ID.len());
        assert_eq!(previous.in_out_params.len(), EVERY_ID.len());
    }

    #[test]
    fn swap_scope_when_used_to_enter_and_leave_a_body_then_program_scope_is_restored() {
        let mut ctx = CompileContext::new();
        ctx.swap_scope(program_scope());

        // Enter a body the way the compile functions do: take the program
        // scope out, build the body's scope from it, install that.
        let program = ctx.swap_scope(Scope::default());
        ctx.swap_scope(Scope::for_function_body(&program, NUM_GLOBALS));
        assert_eq!(names(ctx.variables.keys()), expected(&GLOBALS));

        // Leave it: the program scope goes back and the body's comes out.
        let body = ctx.swap_scope(program);

        assert_eq!(names(body.variables.keys()), expected(&GLOBALS));
        assert_eq!(
            names(ctx.variables.keys()),
            expected(&[GLOBAL, LAST_GLOBAL, FIRST_LOCAL, LOCAL])
        );
        assert_eq!(names(ctx.array_vars.keys()), expected(&EVERY_ID));
    }
}
