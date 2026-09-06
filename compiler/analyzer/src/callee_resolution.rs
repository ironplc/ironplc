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
    MethodDeclaration, TypeName, VarDecl,
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
        let mut current = self.get(fb_name);
        let mut visited: HashSet<TypeName> = HashSet::new();

        while let Some(fb) = current {
            // Guards against an EXTENDS cycle causing an infinite loop.
            // Cycles are also independently invalid (and expected to be
            // rejected elsewhere); this is just a safety net.
            if !visited.insert(fb.name.clone()) {
                return None;
            }

            if let Some(method) = fb.methods.iter().find(|m| &m.name == method_name) {
                return Some((fb, method));
            }

            current = fb
                .oop
                .as_ref()
                .and_then(|oop| oop.base.as_ref())
                .and_then(|base| self.get(base));
        }

        None
    }
}

/// The function-block instances declared in the program organization unit
/// being walked, by variable name.
///
/// Instances are declared per unit, so a walk records each declaration as
/// it meets it and calls [`InstanceTypes::clear`] when it leaves the unit,
/// exactly as the rules that own a walk have always done.
#[derive(Default)]
pub(crate) struct InstanceTypes {
    var_to_fb: HashMap<Id, TypeName>,
}

impl InstanceTypes {
    /// Records `decl` when it declares a function-block instance; any other
    /// declaration is ignored.
    ///
    /// An instance declared with a member initializer, `inst : FB := (x :=
    /// 1)`, keeps the structure-shaped initializer through type resolution,
    /// so its type alone says whether it is an instance; `is_function_block`
    /// answers that for the caller's view of the declared types.
    pub(crate) fn declare(
        &mut self,
        decl: &VarDecl,
        is_function_block: &dyn Fn(&TypeName) -> bool,
    ) {
        let fb_type = match &decl.initializer {
            InitialValueAssignmentKind::FunctionBlock(init) => Some(&init.type_name),
            InitialValueAssignmentKind::Structure(init) if is_function_block(&init.type_name) => {
                Some(&init.type_name)
            }
            _ => None,
        };
        if let (Some(fb_type), Some(name)) = (fb_type, decl.identifier.symbolic_id()) {
            self.var_to_fb.insert(name.clone(), fb_type.clone());
        }
    }

    /// The declared function-block type of the variable `instance`.
    pub(crate) fn type_of(&self, instance: &Id) -> Option<&TypeName> {
        self.var_to_fb.get(instance)
    }

    /// Forgets every instance, on leaving the unit that declared them.
    pub(crate) fn clear(&mut self) {
        self.var_to_fb.clear();
    }
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
            instances.declare(decl, &|type_name| *type_name == TypeName::from("FB_Base"));
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
}
