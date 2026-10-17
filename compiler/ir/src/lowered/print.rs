//! The text form of a lowered program, which lowering's tests compare against.
//!
//! An expression prints as a parenthesized operator, its type and its
//! operands: `(add i32s (read i32s v0^.f1) (const i32s 1))`; a call prints
//! as `call`, its callee, its type and its arguments, and an in-place call
//! as `call-in-place`, then the same, then each store of an output in
//! braces. A place prints as
//! its variable, then `^` for each dereference, `.f<n>` for each field and
//! `[...]` for the subscripts of each element.
//! A statement prints on one line, and one holding other statements prints
//! them on the lines after it, indented two spaces, and closes with `end`.

use std::fmt;

use super::call::{Arg, Callee, Intrinsic};
use super::expr::{BinaryOp, CompareOp, Const, Expr, ExprKind, ShortCircuitOp, UnaryOp};
use super::in_place::InPlaceCall;
use super::place::{Place, Projection};
use super::policy::{NonNumeric, ParseFailure, ParsePolicy};
use super::scalar::ScalarType;
use super::stmt::{CaseLabel, Stmt, StmtKind};
use super::str_expr::{StrExpr, StrExprKind};
use super::types::Encoding;

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.root())?;
        for projection in self.path() {
            match projection {
                Projection::Field(field) => write!(f, ".{field}")?,
                Projection::Index(subscripts) => {
                    let subscripts: Vec<String> =
                        subscripts.iter().map(ToString::to_string).collect();
                    write!(f, "[{}]", subscripts.join(", "))?
                }
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
            ExprKind::Const(constant) => match constant.integer_value(ty) {
                Some(value) => write!(f, "(const {ty} {value})"),
                None => match constant {
                    Const::Bool(value) => write!(f, "(const {ty} {value})"),
                    Const::F32(value) => write!(f, "(const {ty} {value:?})"),
                    Const::F64(value) => write!(f, "(const {ty} {value:?})"),
                    Const::I32(_) | Const::I64(_) => write!(f, "(const {ty} {constant:?})"),
                },
            },
            ExprKind::Read(place) => write!(f, "(read {ty} {place})"),
            ExprKind::RefTo(place) => write!(f, "(ref_to {place})"),
            ExprKind::Null => write!(f, "(null)"),
            ExprKind::Convert(value) => write!(f, "(convert {ty} {value})"),
            ExprKind::ConvertChecked(value) => write!(f, "(convert_checked {ty} {value})"),
            ExprKind::Truncate { width, value } => write!(f, "(truncate {} {value})", width.bits()),
            ExprKind::Unary { op, value } => write!(f, "({} {ty} {value})", unary(*op)),
            ExprKind::Binary { op, lhs, rhs } => write!(f, "({} {ty} {lhs} {rhs})", binary(*op)),
            ExprKind::Compare { op, lhs, rhs } => write!(f, "({} {lhs} {rhs})", compare(*op)),
            ExprKind::ShortCircuit { op, lhs, rhs } => {
                write!(f, "({} {lhs} {rhs})", short_circuit(*op))
            }
            ExprKind::Call { callee, args } => {
                match callee {
                    Callee::User(pou) => write!(f, "(call {pou} {ty}")?,
                    Callee::Intrinsic(intrinsic) => write!(f, "(call {intrinsic} {ty}")?,
                }
                write_args(f, args)?;
                f.write_str(")")
            }
            ExprKind::InPlaceCall(call) => write_in_place(f, call, &ty.to_string()),
        }
    }
}

impl fmt::Display for StrExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let shape = self.shape();
        let ty = match shape.encoding {
            Encoding::Narrow => "string",
            Encoding::Wide => "wstring",
        };
        match self.kind() {
            StrExprKind::Literal(chars) => {
                let text: String = chars.iter().collect();
                write!(f, "(literal {ty}[{}] {text:?})", shape.capacity)
            }
            StrExprKind::Read(place) => write!(f, "(read {ty}[{}] {place})", shape.capacity),
            StrExprKind::Call { callee, args } => {
                write!(f, "(call {callee} {ty}[{}]", shape.capacity)?;
                write_args(f, args)?;
                f.write_str(")")
            }
            StrExprKind::InPlaceCall { call, .. } => {
                write_in_place(f, call, &format!("{ty}[{}]", shape.capacity))
            }
        }
    }
}

/// An in-place call whose result is of type `ty`: its callee, `ty`, its
/// arguments, then each store, in braces.
fn write_in_place(f: &mut fmt::Formatter<'_>, call: &InPlaceCall, ty: &str) -> fmt::Result {
    write!(f, "(call-in-place {} {ty}", call.pou())?;
    write_args(f, call.args())?;
    call.stores()
        .iter()
        .try_for_each(|store| write!(f, " {{{store}}}"))?;
    f.write_str(")")
}

impl fmt::Display for Intrinsic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())?;
        if let Intrinsic::StringToU8(policy)
        | Intrinsic::StringToI8(policy)
        | Intrinsic::StringToU16(policy)
        | Intrinsic::StringToI16(policy)
        | Intrinsic::StringToU32(policy)
        | Intrinsic::StringToI32(policy)
        | Intrinsic::StringToU64(policy)
        | Intrinsic::StringToI64(policy)
        | Intrinsic::StringToF32(policy)
        | Intrinsic::StringToF64(policy) = self
        {
            write!(f, "[{policy}]")?;
        }
        Ok(())
    }
}

impl fmt::Display for ParsePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let non_numeric = match self.non_numeric {
            NonNumeric::Reject => "reject",
            NonNumeric::IgnoreTrailing => "ignore-trailing",
            NonNumeric::IgnoreSurrounding => "ignore-surrounding",
        };
        let failure = match self.failure {
            ParseFailure::Trap => "trap",
            ParseFailure::Zero => "zero",
        };
        write!(f, "{non_numeric},{failure}")
    }
}

/// Each of a call's `args`, after a space.
fn write_args(f: &mut fmt::Formatter<'_>, args: &[Arg]) -> fmt::Result {
    args.iter().try_for_each(|arg| match arg {
        Arg::Value(value) => write!(f, " {value}"),
        Arg::Str(value) => write!(f, " {value}"),
        Arg::Ref(place) => write!(f, " (ref {place})"),
    })
}

impl fmt::Display for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_stmt(f, self, 0)
    }
}

fn write_stmt(f: &mut fmt::Formatter<'_>, stmt: &Stmt, depth: usize) -> fmt::Result {
    let indent = "  ".repeat(depth);
    let block = |f: &mut fmt::Formatter<'_>, stmts: &[Stmt]| -> fmt::Result {
        stmts.iter().try_for_each(|stmt| {
            f.write_str("\n")?;
            write_stmt(f, stmt, depth + 1)
        })
    };
    f.write_str(&indent)?;
    match stmt.kind() {
        StmtKind::Assign { place, value } => write!(f, "assign {place} {value}"),
        StmtKind::AssignStr { place, value } => write!(f, "assign-str {place} {value}"),
        StmtKind::AssignBits {
            place,
            shift,
            bits,
            value,
        } => write!(f, "assign-bits {place} {shift} {bits} {value}"),
        StmtKind::Copy { dst, src } => write!(f, "copy {dst} {src}"),
        StmtKind::FbCall { instance, block } => write!(f, "fb-call {instance} {block}"),
        StmtKind::Call { callee, args } => {
            match callee {
                Callee::User(pou) => write!(f, "call {pou}")?,
                Callee::Intrinsic(intrinsic) => write!(f, "call {intrinsic}")?,
            }
            write_args(f, args)
        }
        StmtKind::If {
            cond,
            then,
            otherwise,
        } => {
            write!(f, "if {cond}")?;
            block(f, then)?;
            if !otherwise.is_empty() {
                write!(f, "\n{indent}else")?;
                block(f, otherwise)?;
            }
            write!(f, "\n{indent}end")
        }
        StmtKind::Case {
            selector,
            arms,
            otherwise,
        } => {
            write!(f, "case {selector}")?;
            for arm in arms {
                let labels: Vec<String> = arm
                    .labels()
                    .iter()
                    .map(|label| case_label(*label, selector.ty()))
                    .collect();
                write!(f, "\n{indent}of {}", labels.join(", "))?;
                block(f, arm.body())?;
            }
            if !otherwise.is_empty() {
                write!(f, "\n{indent}else")?;
                block(f, otherwise)?;
            }
            write!(f, "\n{indent}end")
        }
        StmtKind::For {
            id,
            control,
            from,
            to,
            step,
            body,
        } => {
            let step = case_label(CaseLabel::Value(*step), to.ty());
            write!(f, "for {id} {control} from {from} to {to} by {step}")?;
            block(f, body)?;
            write!(f, "\n{indent}end")
        }
        StmtKind::Loop {
            id,
            body,
            continuing,
        } => {
            write!(f, "loop {id}")?;
            block(f, body)?;
            if !continuing.is_empty() {
                write!(f, "\n{indent}continuing")?;
                block(f, continuing)?;
            }
            write!(f, "\n{indent}end")
        }
        StmtKind::Exit(id) => write!(f, "exit {id}"),
        StmtKind::Continue(id) => write!(f, "continue {id}"),
        StmtKind::Return => f.write_str("return"),
    }
}

/// A label, or a step, as the integer it is at type `ty`.
fn case_label(label: CaseLabel, ty: ScalarType) -> String {
    let value = |constant: Const| match constant.integer_value(ty) {
        Some(value) => value.to_string(),
        None => format!("{constant:?}"),
    };
    match label {
        CaseLabel::Value(constant) => value(constant),
        CaseLabel::Range(low, high) => format!("{}..{}", value(low), value(high)),
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
