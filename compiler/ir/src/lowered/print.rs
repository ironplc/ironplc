//! The text form of a lowered program, which lowering's tests compare against.
//!
//! An expression prints as a parenthesized operator, its type and its
//! operands: `(add i32s (read i32s v0^.f1) (const i32s 1))`. A place prints as
//! its variable, then `^` for each dereference and `.f<n>` for each field.

use std::fmt;

use super::expr::{BinaryOp, CompareOp, Const, Expr, ExprKind, ShortCircuitOp, UnaryOp};
use super::place::{Place, Projection};
use super::stmt::{Stmt, StmtKind};

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.root())?;
        for projection in self.path() {
            match projection {
                Projection::Field(field) => write!(f, ".{field}")?,
                Projection::Deref => f.write_str("^")?,
            }
        }
        Ok(())
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ty = self.ty();
        match self.kind() {
            ExprKind::Const(constant) => match (constant, constant.integer_value(ty)) {
                (_, Some(value)) => write!(f, "(const {ty} {value})"),
                (Const::Bool(value), None) => write!(f, "(const {ty} {value})"),
                (Const::F32(value), None) => write!(f, "(const {ty} {value:?})"),
                (Const::F64(value), None) => write!(f, "(const {ty} {value:?})"),
                (Const::I32(_) | Const::I64(_), None) => write!(f, "(const {ty} {constant:?})"),
            },
            ExprKind::Read(place) => write!(f, "(read {ty} {place})"),
            ExprKind::Convert(value) => write!(f, "(convert {ty} {value})"),
            ExprKind::Truncate { width, value } => write!(f, "(truncate {} {value})", width.bits()),
            ExprKind::Unary { op, value } => write!(f, "({} {ty} {value})", unary(*op)),
            ExprKind::Binary { op, lhs, rhs } => write!(f, "({} {ty} {lhs} {rhs})", binary(*op)),
            ExprKind::Compare { op, lhs, rhs } => write!(f, "({} {lhs} {rhs})", compare(*op)),
            ExprKind::ShortCircuit { op, lhs, rhs } => {
                write!(f, "({} {lhs} {rhs})", short_circuit(*op))
            }
        }
    }
}

impl fmt::Display for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            StmtKind::Assign { place, value } => write!(f, "assign {place} {value}"),
        }
    }
}

fn unary(op: UnaryOp) -> &'static str {
    match op {
        UnaryOp::Neg => "neg",
        UnaryOp::BitNot => "bit_not",
        UnaryOp::BoolNot => "bool_not",
    }
}

fn binary(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "add",
        BinaryOp::Sub => "sub",
        BinaryOp::Mul => "mul",
        BinaryOp::Div => "div",
        BinaryOp::Mod => "mod",
        BinaryOp::BitAnd => "bit_and",
        BinaryOp::BitOr => "bit_or",
        BinaryOp::BitXor => "bit_xor",
        BinaryOp::BoolAnd => "bool_and",
        BinaryOp::BoolOr => "bool_or",
        BinaryOp::BoolXor => "bool_xor",
    }
}

fn compare(op: CompareOp) -> &'static str {
    match op {
        CompareOp::Eq => "eq",
        CompareOp::Ne => "ne",
        CompareOp::Lt => "lt",
        CompareOp::Le => "le",
        CompareOp::Gt => "gt",
        CompareOp::Ge => "ge",
    }
}

fn short_circuit(op: ShortCircuitOp) -> &'static str {
    match op {
        ShortCircuitOp::AndThen => "and_then",
        ShortCircuitOp::OrElse => "or_else",
    }
}
