use crate::dsl::{Constant, DirectVariable};

#[derive(Debug, PartialEq, Clone)]
pub enum Variable {
    DirectVariable(DirectVariable),
    SymbolicVariable(String),
    // A structured variable that may be nested. This data type is definitely
    // incorrect because it doesn't support array types
    MultiElementVariable(Vec<String>),
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

impl StmtKind {
    pub fn fb_assign(fb_name: &str, inputs: Vec<&str>, output: &str) -> StmtKind {
        let assignments = inputs
            .into_iter()
            .map(|input| ParamAssignment::Input {
                name: None,
                expr: ExprKind::Variable(Variable::SymbolicVariable(String::from(input))),
            })
            .collect::<Vec<ParamAssignment>>();

        StmtKind::Assignment {
            target: Variable::SymbolicVariable(String::from(output)),
            value: ExprKind::Function {
                name: String::from(fb_name),
                param_assignment: assignments,
            },
        }
    }
    pub fn fb_call_mapped(fb_name: &str, inputs: Vec<(&str, &str)>) -> StmtKind {
        let assignments = inputs
            .into_iter()
            .map(|pair| ParamAssignment::Input {
                name: Some(String::from(pair.0)),
                expr: ExprKind::Variable(Variable::SymbolicVariable(String::from(pair.1))),
            })
            .collect::<Vec<ParamAssignment>>();

        StmtKind::FbCall {
            name: String::from(fb_name),
            params: assignments,
        }
    }

    pub fn assignment(target: &str, src: Vec<&str>) -> StmtKind {
        let variable = match src.len() {
            1 => Variable::SymbolicVariable(String::from(src[0])),
            _ => {
                let src = src
                    .into_iter()
                    .map(|part| String::from(part))
                    .collect::<Vec<String>>();
                Variable::MultiElementVariable(src)
            }
        };

        StmtKind::Assignment {
            target: Variable::SymbolicVariable(String::from(target)),
            value: ExprKind::Variable(variable),
        }
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
    Const(Constant),
    Variable(Variable),
    Function {
        name: String,
        param_assignment: Vec<ParamAssignment>,
    },
}

impl ExprKind {
    pub fn boxed_symbolic_variable(name: &str) -> Box<ExprKind> {
        Box::new(ExprKind::symbolic_variable(name))
    }

    pub fn symbolic_variable(name: &str) -> ExprKind {
        ExprKind::Variable(Variable::SymbolicVariable(String::from(name)))
    }

    pub fn integer_literal(value: i128) -> ExprKind {
        ExprKind::Const(Constant::IntegerLiteral(1))
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
