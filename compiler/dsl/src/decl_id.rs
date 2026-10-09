//! The numeric identity of a variable declaration.

/// The identity of one variable declaration within one compilation.
///
/// Two references name the same variable exactly when they carry the same
/// `DeclId`. The analyzer gives one to every variable declaration -- each
/// global, each variable of a POU in every section, each function block
/// field, each method variable, the implicit result variable of a function
/// or method, and each global the compiler provides -- and records on every
/// variable reference the `DeclId` its scope rules resolve the name to. A
/// back end keys storage by it and never resolves a name itself.
///
/// The id says nothing else on its own: the declaration it identifies does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeclId(u32);

impl DeclId {
    /// Wraps a raw id. Only the analyzer allocates ids; anything else that
    /// builds one from a number has an id no declaration carries.
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    /// The raw id.
    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::VarDecl;
    use crate::core::Id;
    use crate::textual::NamedVariable;

    #[test]
    fn from_raw_when_raw_then_round_trips() {
        assert_eq!(DeclId::from_raw(7).raw(), 7);
    }

    #[test]
    fn eq_when_declarations_differ_only_by_decl_id_then_equal() {
        let mut bound = VarDecl::simple("x", "INT");
        bound.decl_id = Some(DeclId::from_raw(3));

        assert_eq!(bound, VarDecl::simple("x", "INT"));
    }

    #[test]
    fn eq_when_references_differ_only_by_decl_id_then_equal() {
        let mut bound = NamedVariable::new(Id::from("x"));
        bound.decl_id = Some(DeclId::from_raw(3));

        assert_eq!(bound, NamedVariable::new(Id::from("x")));
    }
}
