use crate::dsl::{Constant, DirectVariable};

#[derive(Debug, PartialEq, Clone)]
pub enum Variable {
    DirectVariable(DirectVariable),
    SymbolicVariable(String),
}

#[derive(Debug, PartialEq, Clone)]
pub enum StmtKind {
    Assignment{
        target: Variable,
        value: ExprKind,
    },
    If {
        // TODO how to handle else else if (that should probably be a nested if)
        expr: ExprKind,
        body: Vec<StmtKind>,
        else_body: Vec<StmtKind>,
    },
    FbCall {
        name: String,
        params: Vec<ParamAssignment>
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum CompareOp {
    Or,
    Xor,
    And,
    Eq,
    Ne,
    Lt,
    Gt,
    LtEq,
    GtEq,
}

#[derive(Debug, PartialEq, Clone)]
pub enum Operator {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

#[derive(Debug, PartialEq, Clone)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ExprKind {
    Compare {
        op: CompareOp,
        terms: Vec<ExprKind>
    },
    BinaryOp {
        ops: Vec<Operator>,
        terms: Vec<ExprKind>,
    },
    UnaryOp {
        op: UnaryOp,

    },
    Const {
        value: Constant,
    },
    Variable {
        value: Variable,
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ParamAssignment {
    pub name: Option<String>,
    pub expr: ExprKind,
}