//! Shared resolution of an invocation's callee.
//!
//! A function-block invocation (`inst(...)`) and a method call
//! (`inst.M(...)`) start the same way: find the declared type of the instance
//! variable, find that function block's declaration, and for a method walk
//! up the `EXTENDS` chain to the block that declares it. Any later analysis
//! that needs to know which declared parameter a call argument binds to --
//! a type check, a write analysis -- needs the same lookups. The rules used
//! to keep private copies of them; this module is the one copy, so they
//! cannot drift.
//!
//! Argument-to-parameter binding for a resolved callee lives beside the
//! parameter-assignment checks in `call_assignment_check`.

use std::collections::{HashMap, HashSet};

use ironplc_dsl::common::{
    FunctionBlockDeclaration, InitialValueAssignmentKind, Library, LibraryElementKind,
    MethodDeclaration, TypeName, VarDecl, VariableType,
};
use ironplc_dsl::core::Id;

/// The function blocks a library declares, by name.
pub(crate) struct FunctionBlocks<'a> {
    by_name: HashMap<TypeName, &'a FunctionBlockDeclaration>,
}

impl<'a> FunctionBlocks<'a> {
    pub(crate) fn from_library(lib: &'a Library) -> Self {
        let by_name = lib
            .elements
            .iter()
            .filter_map(|element| match element {
                LibraryElementKind::FunctionBlockDeclaration(fb) => Some((fb.name.clone(), fb)),
                _ => None,
            })
            .collect();
        FunctionBlocks { by_name }
    }

    /// The declaration of the function block named `name`, if the library
    /// declares one. Standard-library function blocks are not declared in
    /// the library and so are never found here.
    pub(crate) fn get(&self, name: &TypeName) -> Option<&'a FunctionBlockDeclaration> {
        self.by_name.get(name).copied()
    }

    pub(crate) fn contains(&self, name: &TypeName) -> bool {
        self.by_name.contains_key(name)
    }

    /// `fb_name`, then its `EXTENDS` base, then that base's base, and so
    /// on. Stops at a block seen before: a cycle is independently invalid
    /// (and rejected elsewhere), this is just a safety net.
    fn chain(&self, fb_name: &TypeName) -> impl Iterator<Item = &'a FunctionBlockDeclaration> + '_ {
        let mut current = self.get(fb_name);
        let mut visited: HashSet<TypeName> = HashSet::new();
        std::iter::from_fn(move || {
            let fb = current?;
            if !visited.insert(fb.name.clone()) {
                return None;
            }
            current = fb
                .oop
                .as_ref()
                .and_then(|oop| oop.base.as_ref())
                .and_then(|base| self.get(base));
            Some(fb)
        })
    }

    /// Resolves `method_name` against `fb_name`'s own methods, then its
    /// `EXTENDS` base, then that base's base, and so on (ADR-0041 Phase 1
    /// static dispatch). Returns the function block that actually declares
    /// the method (which may be a base, not `fb_name` itself) together
    /// with the method declaration.
    pub(crate) fn resolve_method(
        &self,
        fb_name: &TypeName,
        method_name: &Id,
    ) -> Option<(&'a FunctionBlockDeclaration, &'a MethodDeclaration)> {
        self.chain(fb_name).find_map(|fb| {
            fb.methods
                .iter()
                .find(|m| &m.name == method_name)
                .map(|method| (fb, method))
        })
    }

    /// The block in `fb_name`'s `EXTENDS` chain -- `fb_name` itself first --
    /// that declares the variable `field`, if any does.
    pub(crate) fn declaring_block(
        &self,
        fb_name: &TypeName,
        field: &Id,
    ) -> Option<&'a FunctionBlockDeclaration> {
        self.chain(fb_name).find(|fb| {
            fb.variables
                .iter()
                .any(|decl| decl.identifier.symbolic_id() == Some(field))
        })
    }
}

/// The function-block instances visible in the program organization unit
/// being walked, by variable name.
///
/// Two layers, the unit's own declarations shadowing the globals:
///
/// - The unit's own declarations. A walk records each as it meets it and
///   calls [`InstanceTypes::clear`] when it leaves the unit, exactly as the
///   rules that own a walk have always done. A `VAR_EXTERNAL` is one of
///   them: it names a global instance, and is how a unit reaches the
///   `VAR_GLOBAL` of a `CONFIGURATION`.
/// - The instances of the top-level `VAR_GLOBAL` declarations (an
///   extension), which every unit sees without a `VAR_EXTERNAL`. They are
///   collected once, up front, because the walk meets them before the units
///   that use them and must not forget them on leaving a unit.
#[derive(Default)]
pub(crate) struct InstanceTypes {
    var_to_fb: HashMap<Id, TypeName>,
    globals: HashMap<Id, TypeName>,
}

impl InstanceTypes {
    /// Starts a walk of `lib` with its top-level global instances visible.
    pub(crate) fn with_top_level_globals(lib: &Library) -> Self {
        let mut globals = HashMap::new();
        for element in &lib.elements {
            if let LibraryElementKind::GlobalVarDeclarations(decls) = element {
                for decl in decls {
                    if let Some((name, type_name)) = instance_type(decl) {
                        globals.insert(name.clone(), type_name.clone());
                    }
                }
            }
        }
        Self {
            var_to_fb: HashMap::new(),
            globals,
        }
    }

    /// Records `decl` when it declares a function-block instance, or refers
    /// to one through `VAR_EXTERNAL`; any other declaration is ignored.
    ///
    /// A `VAR_GLOBAL` is not the unit's own: a top-level one is already in
    /// the global layer, and one of a `CONFIGURATION` is reached only
    /// through a `VAR_EXTERNAL`.
    pub(crate) fn declare(&mut self, decl: &VarDecl) {
        if decl.var_type == VariableType::Global {
            return;
        }
        if let Some((name, type_name)) = instance_type(decl) {
            self.var_to_fb.insert(name.clone(), type_name.clone());
        }
    }

    /// The declared function-block type of the variable `instance`.
    pub(crate) fn type_of(&self, instance: &Id) -> Option<&TypeName> {
        self.var_to_fb
            .get(instance)
            .or_else(|| self.globals.get(instance))
    }

    /// Forgets the unit's own declarations, on leaving the unit that
    /// declared them. The top-level globals stay visible.
    pub(crate) fn clear(&mut self) {
        self.var_to_fb.clear();
    }
}

/// The variable name and function-block type of `decl`, when it may name a
/// function-block instance.
///
/// Type resolution has already turned every instance declaration,
/// member-initialized or not, into a function-block initializer, so for
/// those the initializer kind is the whole test. A `VAR_EXTERNAL` is not an
/// instance declaration: the parser cannot tell a function block type from
/// any other named type there, and type resolution leaves its initializer
/// `Simple`. Its type name is returned whatever it names; a type that is not
/// a function block then fails the caller's function-block lookup, exactly
/// as an undeclared instance does.
fn instance_type(decl: &VarDecl) -> Option<(&Id, &TypeName)> {
    let type_name = match (&decl.var_type, &decl.initializer) {
        (_, InitialValueAssignmentKind::FunctionBlock(init)) => &init.type_name,
        (VariableType::External, InitialValueAssignmentKind::Simple(init)) => &init.type_name,
        _ => return None,
    };
    decl.identifier.symbolic_id().map(|name| (name, type_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::parse_and_resolve_types_with_options;
    use ironplc_dsl::common::ProgramDeclaration;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{options::CompilerOptions, parse_program};

    fn oop_options() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        }
    }

    /// Parses only: these lookups read declarations as the parser leaves
    /// them, so no type resolution is needed.
    fn parse(program: &str) -> Library {
        parse_program(program, &FileId::default(), &oop_options()).unwrap()
    }

    const HIERARCHY: &str = "
FUNCTION_BLOCK FB_Base
METHOD Start
    ;
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
METHOD Stop
    ;
END_METHOD
END_FUNCTION_BLOCK";

    #[test]
    fn from_library_when_function_blocks_declared_then_finds_each_by_name() {
        let lib = parse(HIERARCHY);
        let fbs = FunctionBlocks::from_library(&lib);
        assert!(fbs.contains(&TypeName::from("FB_Base")));
        assert!(fbs.get(&TypeName::from("FB_Derived")).is_some());
        assert!(fbs.get(&TypeName::from("FB_Missing")).is_none());
    }

    #[test]
    fn resolve_method_when_declared_on_own_block_then_returns_own_block() {
        let lib = parse(HIERARCHY);
        let fbs = FunctionBlocks::from_library(&lib);
        let (owner, method) = fbs
            .resolve_method(&TypeName::from("FB_Derived"), &Id::from("Stop"))
            .unwrap();
        assert_eq!(TypeName::from("FB_Derived"), owner.name);
        assert_eq!(Id::from("Stop"), method.name);
    }

    #[test]
    fn resolve_method_when_declared_on_base_then_returns_base_block() {
        let lib = parse(HIERARCHY);
        let fbs = FunctionBlocks::from_library(&lib);
        let (owner, method) = fbs
            .resolve_method(&TypeName::from("FB_Derived"), &Id::from("Start"))
            .unwrap();
        assert_eq!(TypeName::from("FB_Base"), owner.name);
        assert_eq!(Id::from("Start"), method.name);
    }

    #[test]
    fn resolve_method_when_not_declared_anywhere_then_none() {
        let lib = parse(HIERARCHY);
        let fbs = FunctionBlocks::from_library(&lib);
        assert!(fbs
            .resolve_method(&TypeName::from("FB_Derived"), &Id::from("Reset"))
            .is_none());
    }

    #[test]
    fn declaring_block_when_field_inherited_then_returns_base_block() {
        let lib = parse(
            "
FUNCTION_BLOCK FB_Base
VAR
    count : INT;
END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    own : INT;
END_VAR
END_FUNCTION_BLOCK",
        );
        let fbs = FunctionBlocks::from_library(&lib);
        let derived = TypeName::from("FB_Derived");
        assert_eq!(
            Some(TypeName::from("FB_Base")),
            fbs.declaring_block(&derived, &Id::from("count"))
                .map(|fb| fb.name.clone())
        );
        assert_eq!(
            Some(derived.clone()),
            fbs.declaring_block(&derived, &Id::from("own"))
                .map(|fb| fb.name.clone())
        );
        assert!(fbs
            .declaring_block(&derived, &Id::from("missing"))
            .is_none());
    }

    #[test]
    fn resolve_method_when_extends_cycle_then_none() {
        let lib = parse(
            "
FUNCTION_BLOCK FB_A EXTENDS FB_B
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_B EXTENDS FB_A
END_FUNCTION_BLOCK",
        );
        let fbs = FunctionBlocks::from_library(&lib);
        assert!(fbs
            .resolve_method(&TypeName::from("FB_A"), &Id::from("Reset"))
            .is_none());
    }

    fn program(lib: &Library) -> &ProgramDeclaration {
        lib.elements
            .iter()
            .find_map(|element| match element {
                LibraryElementKind::ProgramDeclaration(program) => Some(program),
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn instance_types_when_instance_declared_then_type_of_finds_it() {
        // An instance declaration is late-bound until type resolution, so
        // the declarations must be resolved before they can be recorded.
        let (lib, _) = parse_and_resolve_types_with_options(
            "
FUNCTION_BLOCK FB_Base
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM main
VAR
    inst : FB_Base;
    with_init : FB_Base := (x := 1);
    count : INT;
END_VAR
END_PROGRAM",
            &oop_options(),
        );
        let mut instances = InstanceTypes::default();
        for decl in &program(&lib).variables {
            instances.declare(decl);
        }
        assert_eq!(
            Some(&TypeName::from("FB_Base")),
            instances.type_of(&Id::from("inst"))
        );
        assert_eq!(
            Some(&TypeName::from("FB_Base")),
            instances.type_of(&Id::from("with_init"))
        );
        assert_eq!(None, instances.type_of(&Id::from("count")));

        instances.clear();
        assert_eq!(None, instances.type_of(&Id::from("inst")));
    }

    #[test]
    fn instance_types_when_external_declared_then_type_of_finds_named_type() {
        let (lib, _) = parse_and_resolve_types_with_options(
            "
FUNCTION_BLOCK FB_Base
END_FUNCTION_BLOCK
PROGRAM main
VAR_EXTERNAL
    inst : FB_Base;
    count : INT;
END_VAR
END_PROGRAM",
            &oop_options(),
        );
        let mut instances = InstanceTypes::default();
        for decl in &program(&lib).variables {
            instances.declare(decl);
        }
        assert_eq!(
            Some(&TypeName::from("FB_Base")),
            instances.type_of(&Id::from("inst"))
        );
        // Recorded whatever it names: the function-block lookup rejects it.
        assert_eq!(
            Some(&TypeName::from("INT")),
            instances.type_of(&Id::from("count"))
        );
    }

    #[test]
    fn instance_types_when_top_level_global_then_visible_after_clear() {
        let (lib, _) = parse_and_resolve_types_with_options(
            "
FUNCTION_BLOCK FB_Base
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Other
END_FUNCTION_BLOCK
VAR_GLOBAL
    inst : FB_Base;
END_VAR
PROGRAM main
VAR
    inst : FB_Other;
END_VAR
END_PROGRAM",
            &CompilerOptions {
                allow_top_level_var_global: true,
                ..oop_options()
            },
        );
        let mut instances = InstanceTypes::with_top_level_globals(&lib);
        for decl in &program(&lib).variables {
            instances.declare(decl);
        }
        assert_eq!(
            Some(&TypeName::from("FB_Other")),
            instances.type_of(&Id::from("inst"))
        );

        instances.clear();
        assert_eq!(
            Some(&TypeName::from("FB_Base")),
            instances.type_of(&Id::from("inst"))
        );
    }
}
