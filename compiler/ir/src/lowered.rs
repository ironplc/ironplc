//! The lowered program: a program in which every decision the language makes
//! has already been made, and every name already refers to the thing it
//! names. See `specs/design/lowered-program.md`.
//!
//! Lowering builds the nodes through the constructors here, which check the
//! invariants of the design and return a P9998 diagnostic naming the
//! constructor when one is violated (REQ-LOW-ir-081). A [`ProgramBuilder`]
//! owns a program's tables and builds every node that refers into them. The fields of every node
//! are private, so the constructors are the only way to build one
//! (REQ-LOW-ir-080), and a backend can rely on the invariants without checking
//! them again.
//!
//! The lowered program grows one construct at a time. Today it holds scalar
//! expressions, places reached through fields, elements and dereferences,
//! the assignment of a scalar value, and control flow.

mod expr;
mod ids;
mod invariant;
mod loops;
mod place;
mod print;
mod program;
mod scalar;
mod stmt;
mod types;
mod variables;

pub use expr::{BinaryOp, CompareOp, Const, Expr, ExprKind, ShortCircuitOp, UnaryOp};
pub use ids::{FieldIdx, LoopId, PouId, TypeId, VarId};
pub use loops::{counts_down, ForParts, LoopScope};
pub use place::{Place, Projection};
pub use program::ProgramBuilder;
pub use scalar::{ScalarType, Signedness};
pub use stmt::{store_narrowing, CaseArm, CaseLabel, Stmt, StmtKind};
pub use types::{
    Encoding, EnumerationValue, Field, IntWidth, NarrowWidth, Nullability, Section, StringShape,
    TemporalKind, TypeEntry, TypeKind, TypeTable, WideWidth,
};
pub use variables::{Owner, PouTable, Variable, VariableTable};

#[cfg(test)]
mod tests;
