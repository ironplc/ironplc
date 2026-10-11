//! The ids that name the parts of a lowered program.
//!
//! Each id is a type of its own rather than an alias of an integer, so a
//! `PouId` cannot be passed where a `VarId` is expected (REQ-LOW-ir-019).
//! Lowering allocates them; an id is a position in a table lowering keeps,
//! never a name.

use std::fmt;

macro_rules! id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(u32);

        impl $name {
            /// The id at position `index` of the table that allocates it.
            pub const fn new(index: u32) -> Self {
                Self(index)
            }

            /// The position of the id in the table that allocated it.
            pub const fn index(self) -> u32 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($prefix, "{}"), self.0)
            }
        }
    };
}

id!(
    /// One variable that is not a field: a global, a parameter, a local, a
    /// `VAR_TEMP`, or the instance parameter of a body.
    VarId,
    "v"
);
id!(
    /// A program, function, function block or method.
    PouId,
    "pou"
);
id!(
    /// A field of a structure, function block or program, by position.
    FieldIdx,
    "f"
);
id!(
    /// One `Loop` or `For`.
    LoopId,
    "loop"
);
id!(
    /// A type of the lowered program's type table.
    ///
    /// The lowered program numbers its types itself rather than reusing the
    /// analyzer's ids: a program's instance type and the reference type of a
    /// body's instance parameter have no analyzer id, and a backend never
    /// needs to see one.
    TypeId,
    "t"
);
