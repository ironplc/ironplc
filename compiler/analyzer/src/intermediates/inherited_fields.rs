//! Computes, for every `FUNCTION_BLOCK` with an `EXTENDS` clause, the set
//! of fields inherited from its ancestor chain.
//!
//! See `specs/plans/2026-07-20-twincat-extends-field-inheritance.md`.

use std::collections::{HashMap, HashSet};

use ironplc_dsl::common::{
    FunctionBlockDeclaration, Library, LibraryElementKind, TypeName, VarDecl,
};

/// For every `FUNCTION_BLOCK` with an `EXTENDS` clause, returns the
/// transitive list of fields inherited from its ancestor chain (not
/// including its own fields). Base-to-derived order, so a caller that
/// applies its own fields last gets correct shadowing (a redeclared
/// field on a derived function block wins over the ancestor's).
///
/// Assumes the `EXTENDS` graph is acyclic -- enforced by
/// `xform_toposort_declarations`'s `RecursiveCycle` check, which runs
/// earlier in the pipeline and is a hard failure. The recursion here
/// still guards against a cycle defensively (returning no further
/// fields once a name is seen again) rather than trusting that
/// invariant unconditionally.
///
/// A dangling `EXTENDS` target (no `FunctionBlockDeclaration` with that
/// name in the library) silently contributes no inherited fields --
/// existing behavior for referencing an undeclared base is unchanged by
/// this function.
pub fn collect_inherited_fields(lib: &Library) -> HashMap<TypeName, Vec<VarDecl>> {
    let by_name: HashMap<TypeName, &FunctionBlockDeclaration> = lib
        .elements
        .iter()
        .filter_map(|e| match e {
            LibraryElementKind::FunctionBlockDeclaration(fb) => Some((fb.name.clone(), fb)),
            _ => None,
        })
        .collect();

    let mut memo: HashMap<TypeName, Vec<VarDecl>> = HashMap::new();
    let mut result = HashMap::new();
    for fb in by_name.values() {
        if let Some(parent) = &fb.extends {
            let fields =
                resolve_own_and_inherited(parent.clone(), &by_name, &mut memo, &mut HashSet::new());
            result.insert(fb.name.clone(), fields);
        }
    }
    result
}

/// Returns `name`'s own fields plus everything it transitively inherits,
/// base-to-derived order. Memoized since a shared ancestor can be reached
/// through more than one descendant.
fn resolve_own_and_inherited(
    name: TypeName,
    by_name: &HashMap<TypeName, &FunctionBlockDeclaration>,
    memo: &mut HashMap<TypeName, Vec<VarDecl>>,
    visiting: &mut HashSet<TypeName>,
) -> Vec<VarDecl> {
    if let Some(cached) = memo.get(&name) {
        return cached.clone();
    }
    if visiting.contains(&name) {
        // Defensive: a cycle should already have been rejected by
        // toposort before this ever runs.
        return vec![];
    }
    visiting.insert(name.clone());

    let fields = match by_name.get(&name) {
        Some(fb) => {
            let mut fields = match &fb.extends {
                Some(parent) => resolve_own_and_inherited(parent.clone(), by_name, memo, visiting),
                None => vec![],
            };
            fields.extend(fb.variables.iter().cloned());
            fields
        }
        None => vec![],
    };

    visiting.remove(&name);
    memo.insert(name.clone(), fields.clone());
    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{options::CompilerOptions, parse_program};

    fn parse(program: &str) -> Library {
        let options = CompilerOptions {
            allow_oop_extensions: true,
            ..CompilerOptions::default()
        };
        parse_program(program, &FileId::default(), &options).unwrap()
    }

    #[test]
    fn collect_inherited_fields_when_no_extends_then_empty() {
        let lib = parse(
            "
FUNCTION_BLOCK FB_Plain
VAR
    x : BOOL;
END_VAR
END_FUNCTION_BLOCK",
        );

        let result = collect_inherited_fields(&lib);
        assert!(result.is_empty());
    }

    #[test]
    fn collect_inherited_fields_when_single_level_then_contains_base_fields() {
        let lib = parse(
            "
FUNCTION_BLOCK FB_Base
VAR
    bEnabled : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    bRunning : BOOL;
END_VAR
END_FUNCTION_BLOCK",
        );

        let result = collect_inherited_fields(&lib);
        let fields = result.get(&TypeName::from("FB_Derived")).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(
            fields[0].identifier.symbolic_id().unwrap().to_string(),
            "bEnabled"
        );
    }

    #[test]
    fn collect_inherited_fields_when_multi_level_then_transitively_included() {
        let lib = parse(
            "
FUNCTION_BLOCK FB_A
VAR
    a : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_B EXTENDS FB_A
VAR
    b : BOOL;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_C EXTENDS FB_B
VAR
    c : BOOL;
END_VAR
END_FUNCTION_BLOCK",
        );

        let result = collect_inherited_fields(&lib);
        let fields = result.get(&TypeName::from("FB_C")).unwrap();
        let names: Vec<String> = fields
            .iter()
            .map(|f| f.identifier.symbolic_id().unwrap().to_string())
            .collect();
        assert_eq!(names, vec!["a", "b"]);

        // FB_B itself only inherits from FB_A, not its own field "b".
        let fields = result.get(&TypeName::from("FB_B")).unwrap();
        let names: Vec<String> = fields
            .iter()
            .map(|f| f.identifier.symbolic_id().unwrap().to_string())
            .collect();
        assert_eq!(names, vec!["a"]);
    }

    #[test]
    fn collect_inherited_fields_when_shadowed_name_then_base_field_still_listed() {
        // This function only returns the *inherited* set; shadowing
        // (derived wins) is the responsibility of a caller that applies
        // its own fields after these, e.g. by inserting into a HashMap.
        let lib = parse(
            "
FUNCTION_BLOCK FB_Base
VAR
    state : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    state : BOOL;
END_VAR
END_FUNCTION_BLOCK",
        );

        let result = collect_inherited_fields(&lib);
        let fields = result.get(&TypeName::from("FB_Derived")).unwrap();
        assert_eq!(fields.len(), 1);
    }

    #[test]
    fn collect_inherited_fields_when_dangling_extends_then_empty_not_panic() {
        let lib = parse(
            "
FUNCTION_BLOCK FB_Derived EXTENDS FB_DoesNotExist
VAR
    x : BOOL;
END_VAR
END_FUNCTION_BLOCK",
        );

        let result = collect_inherited_fields(&lib);
        let fields = result.get(&TypeName::from("FB_Derived")).unwrap();
        assert!(fields.is_empty());
    }
}
