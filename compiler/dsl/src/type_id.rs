//! The numeric identity of a concrete type.

/// The identity of a concrete type within one compilation.
///
/// Two values have the same type exactly when their `TypeId`s are equal. The
/// id says nothing else on its own: the analyzer's `TypeEnvironment`, the
/// only place that allocates one, answers every other question about the
/// type it identifies -- what it is, and the name it was declared with when
/// it has one.
///
/// A `TypeId` always identifies one type. A generic category such as
/// `ANY_INT` is a set of types and has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TypeId(u32);

impl TypeId {
    /// Wraps a raw id. Only the type environment allocates ids; anything else
    /// that builds one from a number has an id no environment knows.
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

    #[test]
    fn from_raw_when_raw_then_round_trips() {
        assert_eq!(TypeId::from_raw(256).raw(), 256);
    }
}
