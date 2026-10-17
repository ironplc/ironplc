//! The ids that name the parts of a lowered program.
//!
//! Each id is a type of its own rather than an alias of an integer, so a
//! `PouId` cannot be passed where a `VarId` is expected (REQ-LOW-ir-019).
//! An id is a position in the table that allocates it, never a name, and
//! only that table makes one: nothing outside this crate builds an id from a
//! number, so an id always names an entry.
//!
//! Every id also carries the [`Stamp`] of the program builder whose table
//! allocated it. A table refuses an id with another program's stamp, so an
//! id from one lowered program cannot be used in another.

use std::fmt;
use std::sync::atomic::{AtomicU32, Ordering};

/// Which program builder made an id, a place or an expression.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct Stamp(u32);

impl Stamp {
    /// A stamp no other program builder has.
    pub(crate) fn fresh() -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

macro_rules! id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name {
            stamp: Stamp,
            index: u32,
        }

        impl $name {
            /// The id at position `index` of the table, stamped `stamp`, that
            /// allocates it.
            pub(crate) const fn new(stamp: Stamp, index: u32) -> Self {
                Self { stamp, index }
            }

            /// The position of the id in the table that allocated it.
            pub const fn index(self) -> u32 {
                self.index
            }

            /// The stamp of the program builder that allocated the id.
            pub(crate) fn stamp(self) -> Stamp {
                self.stamp
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!($prefix, "{}"), self.index)
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
