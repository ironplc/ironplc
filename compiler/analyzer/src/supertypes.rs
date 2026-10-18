//! Which interfaces a function block or an interface converts to (OOP
//! extension).
//!
//! A function block instance may be used where an interface is required
//! when the block, or a base it `EXTENDS`, lists that interface in
//! `IMPLEMENTS`, or lists an interface that extends it. An interface value
//! may be used where an interface it extends is required.
//!
//! ```ignore
//! INTERFACE I_Base END_INTERFACE
//! INTERFACE I_Comm EXTENDS I_Base END_INTERFACE
//! FUNCTION_BLOCK FB_Serial IMPLEMENTS I_Comm END_FUNCTION_BLOCK
//! FUNCTION_BLOCK FB_Usb EXTENDS FB_Serial END_FUNCTION_BLOCK
//!     (* FB_Usb converts to I_Comm and I_Base; I_Comm converts to I_Base *)
//! ```
//!
//! Whether the block really provides the interface's members is the
//! `IMPLEMENTS` conformance check, not this relation.

use std::collections::{HashMap, HashSet};

use ironplc_dsl::common::TypeName;

/// The direct supertypes of each function block and interface: a function
/// block's `EXTENDS` base and `IMPLEMENTS` interfaces, and the interfaces an
/// interface `EXTENDS`.
#[derive(Debug, Default)]
pub struct Supertypes {
    direct: HashMap<TypeName, Vec<TypeName>>,
}

impl Supertypes {
    /// Records the direct supertypes of `name`.
    pub fn insert(&mut self, name: &TypeName, supertypes: Vec<TypeName>) {
        if !supertypes.is_empty() {
            self.direct.insert(name.clone(), supertypes);
        }
    }

    /// Whether `name` is `supertype` or has it among its direct or indirect
    /// supertypes. Stops at a cycle rather than following it: `EXTENDS`
    /// cycles are reported elsewhere.
    pub fn is_subtype_of(&self, name: &TypeName, supertype: &TypeName) -> bool {
        let mut seen = HashSet::new();
        let mut pending = vec![name];
        while let Some(current) = pending.pop() {
            if current == supertype {
                return true;
            }
            if !seen.insert(current) {
                continue;
            }
            if let Some(direct) = self.direct.get(current) {
                pending.extend(direct);
            }
        }
        false
    }

    /// Every direct or indirect supertype of `name`, each once, nearest
    /// first: for a function block, its bases and every interface it
    /// converts to. Stops at a cycle, as [`Self::is_subtype_of`] does.
    pub fn all_of(&self, name: &TypeName) -> Vec<TypeName> {
        let mut seen = HashSet::from([name]);
        let mut found = Vec::new();
        let mut pending = std::collections::VecDeque::from([name]);
        while let Some(current) = pending.pop_front() {
            for supertype in self.direct.get(current).into_iter().flatten() {
                if seen.insert(supertype) {
                    found.push(supertype.clone());
                    pending.push_back(supertype);
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(s: &str) -> TypeName {
        TypeName::from(s)
    }

    fn hierarchy() -> Supertypes {
        let mut supertypes = Supertypes::default();
        supertypes.insert(&name("I_Comm"), vec![name("I_Base")]);
        supertypes.insert(&name("FB_Serial"), vec![name("I_Comm")]);
        supertypes.insert(&name("FB_Usb"), vec![name("FB_Serial")]);
        supertypes
    }

    #[test]
    fn is_subtype_of_when_implements_directly_then_true() {
        assert!(hierarchy().is_subtype_of(&name("FB_Serial"), &name("I_Comm")));
    }

    #[test]
    fn is_subtype_of_when_base_implements_extended_interface_then_true() {
        assert!(hierarchy().is_subtype_of(&name("FB_Usb"), &name("I_Base")));
    }

    #[test]
    fn is_subtype_of_when_unrelated_then_false() {
        assert!(!hierarchy().is_subtype_of(&name("I_Base"), &name("I_Comm")));
    }

    #[test]
    fn all_of_when_base_implements_extended_interface_then_each_supertype_once() {
        let mut supertypes = hierarchy();
        supertypes.insert(&name("FB_Twice"), vec![name("I_Comm"), name("I_Base")]);

        assert_eq!(
            vec![name("FB_Serial"), name("I_Comm"), name("I_Base")],
            supertypes.all_of(&name("FB_Usb"))
        );
        assert_eq!(
            vec![name("I_Comm"), name("I_Base")],
            supertypes.all_of(&name("FB_Twice"))
        );
    }

    #[test]
    fn all_of_when_cycle_then_stops() {
        let mut supertypes = Supertypes::default();
        supertypes.insert(&name("A"), vec![name("B")]);
        supertypes.insert(&name("B"), vec![name("A")]);

        assert_eq!(vec![name("B")], supertypes.all_of(&name("A")));
    }

    #[test]
    fn is_subtype_of_when_cycle_then_false() {
        let mut supertypes = Supertypes::default();
        supertypes.insert(&name("A"), vec![name("B")]);
        supertypes.insert(&name("B"), vec![name("A")]);
        assert!(!supertypes.is_subtype_of(&name("A"), &name("C")));
    }
}
