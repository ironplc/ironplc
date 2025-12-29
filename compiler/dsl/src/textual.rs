//! Provides definitions of objects from IEC 61131-3 textual languages.
//!
//! See section 3.
use crate::common::{
    AddressAssignment, ConstantKind, EnumeratedValue, IntegerLiteral,
    SignedInteger, Subrange,
};
use crate::core::{Id, Located, SourceSpan};
use std::fmt;

use crate::fold::Fold;
use crate::visitor::Visitor;
use dsl_macro_derive::Recurse;

/// A body of a function bock (one of the possible types).
///
/// See section 3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Statements {
    pub body: Vec<StmtKind>,
}

/// A variable.
///
/// See section B.1.4.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub enum Variable {
    // A variable that maps to a hardware address.
    Direct(AddressAssignment),
    // A variable that maps to a symbolic name (essentially not a hardware address).
    Symbolic(SymbolicVariableKind),
}

impl From<SymbolicVariableKind> for Variable {
    fn from(item: SymbolicVariableKind) -> Self {
        match item {
            SymbolicVariableKind::Named(named) => {
                Variable::Symbolic(SymbolicVariableKind::Named(named))
            }
            SymbolicVariableKind::Array(array) => {
                Variable::Symbolic(SymbolicVariableKind::Array(array))
            }
            SymbolicVariableKind::Structured(structured) => {
                Variable::Symbolic(SymbolicVariableKind::Structured(structured))
            }
        }
    }
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Variable::Direct(assignment) => f.write_fmt(format_args!("{assignment}")),
            Variable::Symbolic(named) => f.write_fmt(format_args!("{named}")),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Recurse)]
pub enum SymbolicVariableKind {
    Named(NamedVariable),
    Array(ArrayVariable),
    Structured(StructuredVariable),
}

impl fmt::Display for SymbolicVariableKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymbolicVariableKind::Named(named) => f.write_fmt(format_args!("{named}")),
            SymbolicVariableKind::Array(array) => f.write_fmt(format_args!("{array}")),
            SymbolicVariableKind::Structured(structured) => {
                f.write_fmt(format_args!("{structured}"))
            }
        }
    }
}

impl Located for SymbolicVariableKind {
    fn span(&self) -> SourceSpan {
        match self {
            SymbolicVariableKind::Named(named) => named.span(),
            SymbolicVariableKind::Array(array) => array.span(),
            SymbolicVariableKind::Structured(structured) => structured.span(),
        }
    }
}

impl Variable {
    pub fn named(name: &str) -> Variable {
        Variable::Symbolic(SymbolicVariableKind::Named(NamedVariable {
            name: Id::from(name),
        }))
    }

    pub fn structured(record: &str, field: &str) -> Variable {
        Variable::Symbolic(SymbolicVariableKind::Structured(StructuredVariable {
            record: Box::new(SymbolicVariableKind::Named(NamedVariable {
                name: Id::from(record),
            })),
            field: Id::from(field),
        }))
    }
}

#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct NamedVariable {
    pub name: Id,
}

impl fmt::Display for NamedVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}", self.name))
    }
}

impl Located for NamedVariable {
    fn span(&self) -> SourceSpan {
        self.name.span()
    }
}

#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct ArrayVariable {
    /// The variable that is being accessed by subscript (the array).
    pub subscripted_variable: Box<SymbolicVariableKind>,
    /// The ordered set of subscripts. These should be expressions that
    /// evaluate to an index.
    pub subscripts: Vec<ExprKind>,
}

impl fmt::Display for ArrayVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // TODO format this
        f.write_fmt(format_args!(
            "{} {:?}",
            self.subscripted_variable, self.subscripts
        ))
    }
}

impl Located for ArrayVariable {
    fn span(&self) -> SourceSpan {
        self.subscripted_variable.as_ref().span()
    }
}

#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct StructuredVariable {
    pub record: Box<SymbolicVariableKind>,
    pub field: Id,
}

impl fmt::Display for StructuredVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{} {}", self.record.as_ref(), self.field))
    }
}

impl Located for StructuredVariable {
    fn span(&self) -> SourceSpan {
        SourceSpan::join2(self.record.as_ref(), &self.field)
    }
}

/// Function block invocation.
///
/// See section 3.2.3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct FbCall {
    /// Name of the variable that is associated with the function block
    /// call.
    pub var_name: Id,
    pub params: Vec<ParamAssignmentKind>,
    pub position: SourceSpan,
}

impl Located for FbCall {
    fn span(&self) -> SourceSpan {
        self.position.clone()
    }
}

/// A binary expression that produces a Boolean result by comparing operands.
///
/// See section 3.3.1.
#[derive(Debug, Clone, PartialEq, Recurse)]
pub struct CompareExpr {
    #[recurse(ignore)]
    pub op: CompareOp,
    pub left: ExprKind,
    pub right: ExprKind,
}

/// A binary expression that produces an arithmetic result by operating on
/// two operands.
///
/// See section 3.3.1.
#[derive(Debug, Clone, PartialEq, Recurse)]
pub struct BinaryExpr {
    #[recurse(ignore)]
    pub op: Operator,
    pub left: ExprKind,
    pub right: ExprKind,
}

/// A unary expression that produces a boolean or arithmetic result by
/// transforming the operand.
///
/// See section 3.3.1.
#[derive(Debug, Clone, PartialEq, Recurse)]
pub struct UnaryExpr {
    #[recurse(ignore)]
    pub op: UnaryOp,
    pub term: ExprKind,
}

#[derive(Debug, Clone, PartialEq, Recurse)]
pub struct Function {
    pub name: Id,
    pub param_assignment: Vec<ParamAssignmentKind>,
}

#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct LateBound {
    pub value: Id,
}

/// Expression that yields a value derived from the input(s) to the expression.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub enum ExprKind {
    Compare(Box<CompareExpr>),
    BinaryOp(Box<BinaryExpr>),
    UnaryOp(Box<UnaryExpr>),
    Expression(Box<ExprKind>),
    Const(ConstantKind),
    EnumeratedValue(EnumeratedValue),
    Variable(Variable),
    Function(Function),
    LateBound(LateBound),
}

impl ExprKind {
    pub fn compare(op: CompareOp, left: ExprKind, right: ExprKind) -> ExprKind {
        ExprKind::Compare(Box::new(CompareExpr { op, left, right }))
    }

    pub fn binary(op: Operator, left: ExprKind, right: ExprKind) -> ExprKind {
        ExprKind::BinaryOp(Box::new(BinaryExpr { op, left, right }))
    }

    pub fn unary(op: UnaryOp, term: ExprKind) -> ExprKind {
        ExprKind::UnaryOp(Box::new(UnaryExpr { op, term }))
    }

    pub fn named_variable(name: &str) -> ExprKind {
        ExprKind::Variable(Variable::named(name))
    }

    pub fn late_bound(name: &str) -> ExprKind {
        ExprKind::LateBound(LateBound {
            value: Id::from(name),
        })
    }

    pub fn integer_literal(value: &str) -> ExprKind {
        ExprKind::Const(ConstantKind::IntegerLiteral(IntegerLiteral {
            value: SignedInteger::new(value, SourceSpan::default()).unwrap(),
            data_type: None,
        }))
    }
}

/// Input argument to a function or function block invocation.
/// The input is mapped based on the order in a sequence. Also known
/// as a non-formal input.
///
/// See section 3.2.3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct PositionalInput {
    pub expr: ExprKind,
}

/// Input argument to a function or function block invocation.
/// The input is mapped based on the specified name. Also known as
/// a formal input.
///
/// See section 3.2.3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct NamedInput {
    pub name: Id,
    pub expr: ExprKind,
}

/// Output argument captured from a function or function block invocation.
///
/// See section 3.2.3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Output {
    #[recurse(ignore)]
    pub not: bool,
    pub src: Id,
    pub tgt: Variable,
}

#[derive(Debug, PartialEq, Clone, Recurse)]
pub enum ParamAssignmentKind {
    PositionalInput(PositionalInput),
    NamedInput(NamedInput),
    Output(Output),
}

impl ParamAssignmentKind {
    pub fn positional(expr: ExprKind) -> ParamAssignmentKind {
        ParamAssignmentKind::PositionalInput(PositionalInput { expr })
    }

    pub fn named(name: &str, expr: ExprKind) -> ParamAssignmentKind {
        ParamAssignmentKind::NamedInput(NamedInput {
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
#[derive(Debug, PartialEq, Clone, Recurse)]
#[allow(clippy::large_enum_variant)]
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
    #[recurse(ignore)]
    Return,
    // Exit statement.
    #[recurse(ignore)]
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
            .map(|input| ParamAssignmentKind::positional(ExprKind::late_bound(input)))
            .collect::<Vec<ParamAssignmentKind>>();

        StmtKind::assignment(
            Variable::named(output),
            ExprKind::Function(Function {
                name: Id::from(fb_name),
                param_assignment: assignments,
            }),
        )
    }
    pub fn fb_call_mapped(fb_instance_name: &str, inputs: Vec<(&str, &str)>) -> StmtKind {
        let assignments = inputs
            .into_iter()
            .map(|pair| {
                ParamAssignmentKind::named(
                    pair.0,
                    ExprKind::LateBound(LateBound {
                        value: Id::from(pair.1),
                    }),
                )
            })
            .collect::<Vec<ParamAssignmentKind>>();

        StmtKind::FbCall(FbCall {
            var_name: Id::from(fb_instance_name),
            params: assignments,
            position: SourceSpan::default(),
        })
    }

    pub fn assignment(target: Variable, value: ExprKind) -> StmtKind {
        StmtKind::Assignment(Assignment { target, value })
    }

    pub fn simple_assignment(target: &str, src: &str) -> StmtKind {
        StmtKind::Assignment(Assignment {
            target: Variable::named(target),
            value: ExprKind::LateBound(LateBound {
                value: Id::from(src),
            }),
        })
    }

    pub fn structured_assignment(target: &str, record: &str, field: &str) -> StmtKind {
        let variable = Variable::Symbolic(SymbolicVariableKind::Structured(StructuredVariable {
            record: Box::new(SymbolicVariableKind::Named(NamedVariable {
                name: Id::from(record),
            })),
            field: Id::from(field),
        }));
        StmtKind::Assignment(Assignment {
            target: Variable::named(target),
            value: ExprKind::Variable(variable),
        })
    }
}

/// Assigns a variable as the evaluation of an expression.
///
/// See section 3.3.2.1.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Assignment {
    pub target: Variable,
    pub value: ExprKind,
}

/// If selection statement.
///
/// See section 3.3.2.3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct If {
    pub expr: ExprKind,
    pub body: Vec<StmtKind>,
    pub else_ifs: Vec<ElseIf>,
    pub else_body: Vec<StmtKind>,
}

#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct ElseIf {
    pub expr: ExprKind,
    pub body: Vec<StmtKind>,
}

/// Case selection statement.
///
/// See section 3.3.2.3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Case {
    /// An expression, the result of which is used to select a particular case.
    pub selector: ExprKind,
    pub statement_groups: Vec<CaseStatementGroup>,
    pub else_body: Vec<StmtKind>,
}

/// A group of statements that can be selected within a case.
///
/// See section 3.3.2.3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct CaseStatementGroup {
    pub selectors: Vec<CaseSelectionKind>,
    pub statements: Vec<StmtKind>,
}

/// A particular value that selects a case statement group.
///
/// See section 3.3.2.3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub enum CaseSelectionKind {
    Subrange(Subrange),
    SignedInteger(SignedInteger),
    EnumeratedValue(EnumeratedValue),
}

/// The for loop statement.
///
/// See section 3.3.2.4.
#[derive(Debug, PartialEq, Clone, Recurse)]
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
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct While {
    pub condition: ExprKind,
    pub body: Vec<StmtKind>,
}

/// The repeat loop statement.
///
/// See section 3.3.2.4.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Repeat {
    pub until: ExprKind,
    pub body: Vec<StmtKind>,
}
