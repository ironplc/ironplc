//! Provides definitions of objects from IEC 61131-3 textual languages.
//!
//! See section 3.
use crate::common::{AddressAssignment, Constant, EnumeratedValue, SignedInteger, Subrange};
use crate::core::{Id, SourceLoc};
use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, PartialEq, Clone)]
pub enum Variable {
    AddressAssignment(AddressAssignment),
    SymbolicVariable(SymbolicVariable),
    // A structured variable that may be nested. This data type is definitely
    // incorrect because it doesn't support array types
    MultiElementVariable(Vec<Id>),
}

impl Variable {
    pub fn symbolic(name: &str) -> Variable {
        Variable::SymbolicVariable(SymbolicVariable {
            name: Id::from(name),
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct SymbolicVariable {
    pub name: Id,
}

/// Function block invocation.
///
/// See section 3.2.3.
#[derive(Debug, PartialEq, Clone)]
pub struct FbCall {
    /// Name of the variable that is associated with the function block
    /// call.
    pub var_name: Id,
    pub params: Vec<ParamAssignment>,
    pub position: SourceLoc,
}

/// Expressions (instructions).
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
        name: Id,
        param_assignment: Vec<ParamAssignment>,
    },
}

impl ExprKind {
    pub fn boxed_symbolic_variable(name: &str) -> Box<ExprKind> {
        Box::new(ExprKind::symbolic_variable(name))
    }

    pub fn symbolic_variable(name: &str) -> ExprKind {
        ExprKind::Variable(Variable::symbolic(name))
    }

    pub fn integer_literal(value: i128) -> ExprKind {
        ExprKind::Const(Constant::IntegerLiteral(1))
    }
}

/// Input argument to a function or function block invocation.
/// The input is mapped based on the order in a sequence. Also known
/// as a non-formal input.
///
/// See section 3.2.3.
#[derive(Debug, PartialEq, Clone)]
pub struct PositionalInput {
    pub expr: ExprKind,
}

/// Input argument to a function or function block invocation.
/// The input is mapped based on the specified name. Also known as
/// a formal input.
///
/// See section 3.2.3.
#[derive(Debug, PartialEq, Clone)]
pub struct NamedInput {
    pub name: Id,
    pub expr: ExprKind,
}

/// Output argument captured from a function or function block invocation.
///
/// See section 3.2.3.
#[derive(Debug, PartialEq, Clone)]
pub struct Output {
    pub not: bool,
    pub src: Id,
    pub tgt: Variable,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ParamAssignment {
    PositionalInput(PositionalInput),
    NamedInput(NamedInput),
    Output(Output),
}

impl ParamAssignment {
    pub fn positional(expr: ExprKind) -> ParamAssignment {
        ParamAssignment::PositionalInput(PositionalInput { expr })
    }

    pub fn named(name: &str, expr: ExprKind) -> ParamAssignment {
        ParamAssignment::NamedInput(NamedInput {
            name: Id::from(name),
            expr,
        })
    }
}

/// Comparison operators.
///
/// See section 3.2.2, especially table 52.
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

/// Arithmetic operators.
///
/// See section 3.2.2, especially table 52.
#[derive(Debug, PartialEq, Clone)]
pub enum Operator {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
}

/// Local operators (with single operand).
///
/// See section 3.2.2, especially table 52.
#[derive(Debug, PartialEq, Clone)]
pub enum UnaryOp {
    Neg,
    // Compliment operator (for Boolean values)
    Not,
}

/// Statements.
///
/// See section 3.3.2.
#[derive(Debug, PartialEq, Clone)]
pub enum StmtKind {
    Assignment(Assignment),
    // Function and function block control
    FbCall(FbCall),
    // Selection statements
    If(If),
    Case(Case),
    // Iteration statements
    For(For),
    While(While),
    Repeat(Repeat),
    // Exit statement.
    Exit,
}

impl StmtKind {
    pub fn if_then(condition: ExprKind, body: Vec<StmtKind>) -> StmtKind {
        StmtKind::If(If {
            expr: condition,
            body,
            else_ifs: vec![],
            else_body: vec![],
        })
    }

    pub fn if_then_else(
        condition: ExprKind,
        body: Vec<StmtKind>,
        else_body: Vec<StmtKind>,
    ) -> StmtKind {
        StmtKind::If(If {
            expr: condition,
            body,
            else_ifs: vec![],
            else_body,
        })
    }

    pub fn fb_assign(fb_name: &str, inputs: Vec<&str>, output: &str) -> StmtKind {
        let assignments = inputs
            .into_iter()
            .map(|input| ParamAssignment::positional(ExprKind::symbolic_variable(input)))
            .collect::<Vec<ParamAssignment>>();

        StmtKind::assignment(
            Variable::symbolic(output),
            ExprKind::Function {
                name: Id::from(fb_name),
                param_assignment: assignments,
            },
        )
    }
    pub fn fb_call_mapped(fb_name: &str, inputs: Vec<(&str, &str)>) -> StmtKind {
        let assignments = inputs
            .into_iter()
            .map(|pair| {
                ParamAssignment::named(pair.0, ExprKind::Variable(Variable::symbolic(pair.1)))
            })
            .collect::<Vec<ParamAssignment>>();

        StmtKind::FbCall(FbCall {
            var_name: Id::from(fb_name),
            params: assignments,
            position: SourceLoc::default(),
        })
    }

    pub fn assignment(target: Variable, value: ExprKind) -> StmtKind {
        StmtKind::Assignment(Assignment { target, value })
    }

    pub fn simple_assignment(target: &str, src: Vec<&str>) -> StmtKind {
        let variable = match src.len() {
            1 => Variable::symbolic(src[0]),
            _ => {
                let src = src.into_iter().map(Id::from).collect::<Vec<Id>>();
                Variable::MultiElementVariable(src)
            }
        };

        StmtKind::Assignment(Assignment {
            target: Variable::symbolic(target),
            value: ExprKind::Variable(variable),
        })
    }
}

/// Assigns a variable as the evaluation of an expression.
///
/// See section 3.3.2.1.
#[derive(Debug, PartialEq, Clone)]
pub struct Assignment {
    pub target: Variable,
    pub value: ExprKind,
}

/// If selection statement.
///
/// See section 3.3.2.3.
#[derive(Debug, PartialEq, Clone)]
pub struct If {
    // TODO how to handle else else if (that should probably be a nested if)
    pub expr: ExprKind,
    pub body: Vec<StmtKind>,
    pub else_ifs: Vec<(ExprKind, Vec<StmtKind>)>,
    pub else_body: Vec<StmtKind>,
}

/// Case selection statement.
///
/// See section 3.3.2.3.
#[derive(Debug, PartialEq, Clone)]
pub struct Case {
    /// An expression, the result of which is used to select a particular case.
    pub selector: ExprKind,
    pub statement_groups: Vec<CaseStatementGroup>,
    pub else_body: Vec<StmtKind>,
}

/// A group of statements that can be selected within a case.
///
/// See section 3.3.2.3.
#[derive(Debug, PartialEq, Clone)]
pub struct CaseStatementGroup {
    pub selectors: Vec<CaseSelection>,
    pub statements: Vec<StmtKind>,
}

/// A particular value that selects a case statement group.
///
/// See section 3.3.2.3.
#[derive(Debug, PartialEq, Clone)]
pub enum CaseSelection {
    Subrange(Subrange),
    SignedInteger(SignedInteger),
    EnumeratedValue(EnumeratedValue),
}

/// The for loop statement.
///
/// See section 3.3.2.4.
#[derive(Debug, PartialEq, Clone)]
pub struct For {
    /// The variable that is assigned and contains the value for each loop iteration.
    pub control: Id,
    pub from: ExprKind,
    pub to: ExprKind,
    pub step: Option<ExprKind>,
    pub body: Vec<StmtKind>,
}

/// The while loop statement.
///
/// See section 3.3.2.4.
#[derive(Debug, PartialEq, Clone)]
pub struct While {
    pub condition: ExprKind,
    pub body: Vec<StmtKind>,
}

/// The repeat loop statement.
///
/// See section 3.3.2.4.
#[derive(Debug, PartialEq, Clone)]
pub struct Repeat {
    pub until: ExprKind,
    pub body: Vec<StmtKind>,
}
