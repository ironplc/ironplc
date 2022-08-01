use crate::dsl::{Constant, DirectVariable};

#[derive(Debug, PartialEq, Clone)]
pub enum Variable {
    DirectVariable(DirectVariable),
    SymbolicVariable(String),
    // A structured variable that may be nested. This data type is definitely
    // incorrect because it doesn't support array types
    MultiElementVariable(Vec<String>)
}

#[derive(Debug, PartialEq, Clone)]
pub enum StmtKind {
    Assignment {
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
        params: Vec<ParamAssignment>,
    },
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
        terms: Vec<ExprKind>,
    },
    BinaryOp {
        ops: Vec<Operator>,
        terms: Vec<ExprKind>,
    },
    UnaryOp {
        op: UnaryOp,
        term: Box<ExprKind>,
    },
    Const {
        value: Constant,
    },
    Variable {
        value: Variable,
    },
    Function {
        name: String,
        param_assignment: Vec<ParamAssignment>
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum ParamAssignment {
    Input {
        name: Option<String>,
        expr: ExprKind,
    },
    Output {
        not: bool,
        src: String,
        tgt: Variable,
    },
}
pub struct InputParamAssignment {
    pub name: Option<String>,
    pub expr: ExprKind,
}
