//! Provides definitions of objects from IEC 61131-3 textual languages.
//!
//! See section 3.
use crate::common::{
    AddressAssignment, ConstantKind, EnumeratedValue, Integer, IntegerLiteral, SignedInteger,
    Subrange, TypeName,
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
            SymbolicVariableKind::BitAccess(bit_access) => {
                Variable::Symbolic(SymbolicVariableKind::BitAccess(bit_access))
            }
            SymbolicVariableKind::Deref(deref) => {
                Variable::Symbolic(SymbolicVariableKind::Deref(deref))
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
    BitAccess(BitAccessVariable),
    Deref(DerefVariable),
}

impl fmt::Display for SymbolicVariableKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymbolicVariableKind::Named(named) => f.write_fmt(format_args!("{named}")),
            SymbolicVariableKind::Array(array) => f.write_fmt(format_args!("{array}")),
            SymbolicVariableKind::Structured(structured) => {
                f.write_fmt(format_args!("{structured}"))
            }
            SymbolicVariableKind::BitAccess(bit_access) => {
                f.write_fmt(format_args!("{bit_access}"))
            }
            SymbolicVariableKind::Deref(deref) => f.write_fmt(format_args!("{deref}")),
        }
    }
}

impl Located for SymbolicVariableKind {
    fn span(&self) -> SourceSpan {
        match self {
            SymbolicVariableKind::Named(named) => named.span(),
            SymbolicVariableKind::Array(array) => array.span(),
            SymbolicVariableKind::Structured(structured) => structured.span(),
            SymbolicVariableKind::BitAccess(bit_access) => bit_access.span(),
            SymbolicVariableKind::Deref(deref) => deref.span(),
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
    pub subscripts: Vec<Expr>,
}

impl fmt::Display for ArrayVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[", self.subscripted_variable)?;
        for (i, subscript) in self.subscripts.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{subscript}")?;
        }
        write!(f, "]")
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
        f.write_fmt(format_args!("{}.{}", self.record.as_ref(), self.field))
    }
}

impl Located for StructuredVariable {
    fn span(&self) -> SourceSpan {
        SourceSpan::join2(self.record.as_ref(), &self.field)
    }
}

/// Bit access on an integer-typed variable.
///
/// See section B.1.4.2.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct BitAccessVariable {
    /// The variable being bit-accessed.
    pub variable: Box<SymbolicVariableKind>,
    /// The bit index (unsigned integer).
    pub index: Integer,
}

impl fmt::Display for BitAccessVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.variable, self.index.value)
    }
}

impl Located for BitAccessVariable {
    fn span(&self) -> SourceSpan {
        SourceSpan::join2(self.variable.as_ref(), &self.index)
    }
}

/// Dereference of a pointer variable, used when the result is further
/// accessed (e.g., `PT^[0]` or `PT^.field`).
///
/// See section B.1.4.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct DerefVariable {
    /// The variable being dereferenced.
    pub variable: Box<SymbolicVariableKind>,
}

impl fmt::Display for DerefVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}^", self.variable)
    }
}

impl Located for DerefVariable {
    fn span(&self) -> SourceSpan {
        self.variable.span()
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
    pub left: Expr,
    pub right: Expr,
}

/// A binary expression that produces an arithmetic result by operating on
/// two operands.
///
/// See section 3.3.1.
#[derive(Debug, Clone, PartialEq, Recurse)]
pub struct BinaryExpr {
    #[recurse(ignore)]
    pub op: Operator,
    pub left: Expr,
    pub right: Expr,
}

/// A unary expression that produces a boolean or arithmetic result by
/// transforming the operand.
///
/// See section 3.3.1.
#[derive(Debug, Clone, PartialEq, Recurse)]
pub struct UnaryExpr {
    #[recurse(ignore)]
    pub op: UnaryOp,
    pub term: Expr,
}

#[derive(Debug, Clone, PartialEq, Recurse)]
pub struct Function {
    pub name: Id,
    pub param_assignment: Vec<ParamAssignmentKind>,
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(...)", self.name)
    }
}

#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct LateBound {
    pub value: Id,
}

impl fmt::Display for LateBound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

/// Wrapper around `ExprKind` that carries optional resolved type information.
///
/// The `resolved_type` field is populated by a later analysis pass. During
/// parsing and initial construction, it is always `None`.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Expr {
    pub kind: ExprKind,
    #[recurse(ignore)]
    pub resolved_type: Option<TypeName>,
}

impl Expr {
    /// Creates a new `Expr` with no resolved type.
    pub fn new(kind: ExprKind) -> Expr {
        Expr {
            kind,
            resolved_type: None,
        }
    }

    /// Creates a new `Expr` with a resolved type.
    pub fn with_type(kind: ExprKind, type_name: TypeName) -> Expr {
        Expr {
            kind,
            resolved_type: Some(type_name),
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

/// Expression that yields a value derived from the input(s) to the expression.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub enum ExprKind {
    Compare(Box<CompareExpr>),
    BinaryOp(Box<BinaryExpr>),
    UnaryOp(Box<UnaryExpr>),
    Expression(Box<Expr>),
    Const(ConstantKind),
    EnumeratedValue(EnumeratedValue),
    Variable(Variable),
    Function(Function),
    LateBound(LateBound),
    Ref(Box<Variable>),
    Deref(Box<Expr>),
    Null(SourceSpan),
}

impl ExprKind {
    pub fn compare(op: CompareOp, left: ExprKind, right: ExprKind) -> ExprKind {
        ExprKind::Compare(Box::new(CompareExpr {
            op,
            left: Expr::new(left),
            right: Expr::new(right),
        }))
    }

    pub fn binary(op: Operator, left: ExprKind, right: ExprKind) -> ExprKind {
        ExprKind::BinaryOp(Box::new(BinaryExpr {
            op,
            left: Expr::new(left),
            right: Expr::new(right),
        }))
    }

    pub fn unary(op: UnaryOp, term: ExprKind) -> ExprKind {
        ExprKind::UnaryOp(Box::new(UnaryExpr {
            op,
            term: Expr::new(term),
        }))
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

impl fmt::Display for ExprKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExprKind::Compare(expr) => {
                write!(f, "{} {} {}", expr.left, expr.op, expr.right)
            }
            ExprKind::BinaryOp(expr) => {
                write!(f, "{} {} {}", expr.left, expr.op, expr.right)
            }
            ExprKind::UnaryOp(expr) => {
                write!(f, "{}{}", expr.op, expr.term)
            }
            ExprKind::Expression(inner) => write!(f, "({})", inner),
            ExprKind::Const(constant) => write!(f, "{constant}"),
            ExprKind::EnumeratedValue(value) => write!(f, "{value}"),
            ExprKind::Variable(var) => write!(f, "{var}"),
            ExprKind::Function(func) => {
                write!(f, "{}(", func.name)?;
                for (i, param) in func.param_assignment.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{param}")?;
                }
                write!(f, ")")
            }
            ExprKind::LateBound(late) => write!(f, "{}", late.value),
            ExprKind::Ref(var) => write!(f, "REF({var})"),
            ExprKind::Deref(expr) => write!(f, "{expr}^"),
            ExprKind::Null(_) => write!(f, "NULL"),
        }
    }
}

/// Input argument to a function or function block invocation.
/// The input is mapped based on the order in a sequence. Also known
/// as a non-formal input.
///
/// See section 3.2.3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct PositionalInput {
    pub expr: Expr,
}

/// Input argument to a function or function block invocation.
/// The input is mapped based on the specified name. Also known as
/// a formal input.
///
/// See section 3.2.3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct NamedInput {
    pub name: Id,
    pub expr: Expr,
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
        ParamAssignmentKind::PositionalInput(PositionalInput {
            expr: Expr::new(expr),
        })
    }

    pub fn named(name: &str, expr: ExprKind) -> ParamAssignmentKind {
        ParamAssignmentKind::NamedInput(NamedInput {
            name: Id::from(name),
            expr: Expr::new(expr),
        })
    }
}

impl fmt::Display for ParamAssignmentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParamAssignmentKind::PositionalInput(input) => write!(f, "{}", input.expr),
            ParamAssignmentKind::NamedInput(input) => write!(f, "{} := {}", input.name, input.expr),
            ParamAssignmentKind::Output(output) => {
                if output.not {
                    write!(f, "NOT {} => {}", output.src, output.tgt)
                } else {
                    write!(f, "{} => {}", output.src, output.tgt)
                }
            }
        }
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

impl fmt::Display for CompareOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let symbol = match self {
            CompareOp::Or => "OR",
            CompareOp::Xor => "XOR",
            CompareOp::And => "AND",
            CompareOp::Eq => "=",
            CompareOp::Ne => "<>",
            CompareOp::Lt => "<",
            CompareOp::Gt => ">",
            CompareOp::LtEq => "<=",
            CompareOp::GtEq => ">=",
        };
        write!(f, "{symbol}")
    }
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

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let symbol = match self {
            Operator::Add => "+",
            Operator::Sub => "-",
            Operator::Mul => "*",
            Operator::Div => "/",
            Operator::Mod => "MOD",
            Operator::Pow => "**",
        };
        write!(f, "{symbol}")
    }
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

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let symbol = match self {
            UnaryOp::Neg => "-",
            UnaryOp::Not => "NOT",
        };
        write!(f, "{symbol}")
    }
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
    Exit(SourceSpan),
}

impl StmtKind {
    pub fn if_then(condition: ExprKind, body: Vec<StmtKind>) -> StmtKind {
        StmtKind::If(If {
            expr: Expr::new(condition),
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
            expr: Expr::new(condition),
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
        StmtKind::Assignment(Assignment {
            target,
            deref: false,
            value: Expr::new(value),
        })
    }

    pub fn simple_assignment(target: &str, src: &str) -> StmtKind {
        StmtKind::Assignment(Assignment {
            target: Variable::named(target),
            deref: false,
            value: Expr::new(ExprKind::LateBound(LateBound {
                value: Id::from(src),
            })),
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
            deref: false,
            value: Expr::new(ExprKind::Variable(variable)),
        })
    }
}

/// Assigns a variable as the evaluation of an expression.
///
/// See section 3.3.2.1.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Assignment {
    pub target: Variable,
    #[recurse(ignore)]
    pub deref: bool,
    pub value: Expr,
}

/// If selection statement.
///
/// See section 3.3.2.3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct If {
    pub expr: Expr,
    pub body: Vec<StmtKind>,
    pub else_ifs: Vec<ElseIf>,
    pub else_body: Vec<StmtKind>,
}

#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct ElseIf {
    pub expr: Expr,
    pub body: Vec<StmtKind>,
}

/// Case selection statement.
///
/// See section 3.3.2.3.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Case {
    /// An expression, the result of which is used to select a particular case.
    pub selector: Expr,
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
    pub from: Expr,
    pub to: Expr,
    pub step: Option<Expr>,
    pub body: Vec<StmtKind>,
}

/// The while loop statement.
///
/// See section 3.3.2.4.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct While {
    pub condition: Expr,
    pub body: Vec<StmtKind>,
}

/// The repeat loop statement.
///
/// See section 3.3.2.4.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct Repeat {
    pub until: Expr,
    pub body: Vec<StmtKind>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_variable_display_when_single_subscript_then_formats_with_brackets() {
        let array_var = ArrayVariable {
            subscripted_variable: Box::new(SymbolicVariableKind::Named(NamedVariable {
                name: Id::from("data"),
            })),
            subscripts: vec![Expr::new(ExprKind::integer_literal("0"))],
        };

        let result = format!("{}", array_var);

        assert_eq!(result, "data[0]");
    }

    #[test]
    fn array_variable_display_when_multiple_subscripts_then_formats_with_comma_separated() {
        let array_var = ArrayVariable {
            subscripted_variable: Box::new(SymbolicVariableKind::Named(NamedVariable {
                name: Id::from("matrix"),
            })),
            subscripts: vec![
                Expr::new(ExprKind::integer_literal("1")),
                Expr::new(ExprKind::integer_literal("2")),
            ],
        };

        let result = format!("{}", array_var);

        assert_eq!(result, "matrix[1, 2]");
    }

    #[test]
    fn array_variable_display_when_variable_subscript_then_formats_variable_name() {
        let array_var = ArrayVariable {
            subscripted_variable: Box::new(SymbolicVariableKind::Named(NamedVariable {
                name: Id::from("arr"),
            })),
            subscripts: vec![Expr::new(ExprKind::named_variable("i"))],
        };

        let result = format!("{}", array_var);

        assert_eq!(result, "arr[i]");
    }

    #[test]
    fn display_when_named_variable_then_name() {
        let v = Variable::named("x");
        assert_eq!(format!("{v}"), "x");
    }

    #[test]
    fn display_when_structured_variable_then_dot_notation() {
        let v = Variable::structured("rec", "field");
        assert_eq!(format!("{v}"), "rec.field");
    }

    #[test]
    fn display_when_bit_access_variable_then_dot_index() {
        let ba = BitAccessVariable {
            variable: Box::new(SymbolicVariableKind::Named(NamedVariable {
                name: Id::from("val"),
            })),
            index: Integer::new("3", SourceSpan::default()).unwrap(),
        };
        assert_eq!(format!("{ba}"), "val.3");
    }

    #[test]
    fn display_when_deref_variable_then_caret() {
        let d = DerefVariable {
            variable: Box::new(SymbolicVariableKind::Named(NamedVariable {
                name: Id::from("ptr"),
            })),
        };
        assert_eq!(format!("{d}"), "ptr^");
    }

    #[test]
    fn display_when_param_assignment_positional_then_value() {
        let pa = ParamAssignmentKind::positional(ExprKind::integer_literal("42"));
        assert_eq!(format!("{pa}"), "42");
    }

    #[test]
    fn display_when_param_assignment_named_then_name_assign_value() {
        let pa = ParamAssignmentKind::named("in1", ExprKind::integer_literal("5"));
        assert_eq!(format!("{pa}"), "in1 := 5");
    }

    #[test]
    fn display_when_param_assignment_output_then_arrow() {
        let pa = ParamAssignmentKind::Output(Output {
            not: false,
            src: Id::from("out1"),
            tgt: Variable::named("result"),
        });
        assert_eq!(format!("{pa}"), "out1 => result");
    }

    #[test]
    fn display_when_param_assignment_output_not_then_not_arrow() {
        let pa = ParamAssignmentKind::Output(Output {
            not: true,
            src: Id::from("out1"),
            tgt: Variable::named("result"),
        });
        assert_eq!(format!("{pa}"), "NOT out1 => result");
    }

    #[test]
    fn display_when_compare_op_then_symbol() {
        assert_eq!(format!("{}", CompareOp::Or), "OR");
        assert_eq!(format!("{}", CompareOp::Xor), "XOR");
        assert_eq!(format!("{}", CompareOp::And), "AND");
        assert_eq!(format!("{}", CompareOp::Eq), "=");
        assert_eq!(format!("{}", CompareOp::Ne), "<>");
        assert_eq!(format!("{}", CompareOp::Lt), "<");
        assert_eq!(format!("{}", CompareOp::Gt), ">");
        assert_eq!(format!("{}", CompareOp::LtEq), "<=");
        assert_eq!(format!("{}", CompareOp::GtEq), ">=");
    }

    #[test]
    fn display_when_operator_then_symbol() {
        assert_eq!(format!("{}", Operator::Add), "+");
        assert_eq!(format!("{}", Operator::Sub), "-");
        assert_eq!(format!("{}", Operator::Mul), "*");
        assert_eq!(format!("{}", Operator::Div), "/");
        assert_eq!(format!("{}", Operator::Mod), "MOD");
        assert_eq!(format!("{}", Operator::Pow), "**");
    }

    #[test]
    fn display_when_unary_op_then_symbol() {
        assert_eq!(format!("{}", UnaryOp::Neg), "-");
        assert_eq!(format!("{}", UnaryOp::Not), "NOT");
    }

    #[test]
    fn display_when_expr_kind_const_then_value() {
        let expr = ExprKind::integer_literal("10");
        assert_eq!(format!("{expr}"), "10");
    }

    #[test]
    fn display_when_expr_kind_variable_then_name() {
        let expr = ExprKind::named_variable("x");
        assert_eq!(format!("{expr}"), "x");
    }

    #[test]
    fn display_when_expr_kind_compare_then_formatted() {
        let expr = ExprKind::compare(
            CompareOp::Gt,
            ExprKind::named_variable("a"),
            ExprKind::integer_literal("0"),
        );
        assert_eq!(format!("{expr}"), "a > 0");
    }

    #[test]
    fn display_when_expr_kind_binary_then_formatted() {
        let expr = ExprKind::binary(
            Operator::Add,
            ExprKind::named_variable("x"),
            ExprKind::integer_literal("1"),
        );
        assert_eq!(format!("{expr}"), "x + 1");
    }

    #[test]
    fn display_when_expr_kind_unary_then_formatted() {
        let expr = ExprKind::unary(UnaryOp::Neg, ExprKind::named_variable("x"));
        assert_eq!(format!("{expr}"), "-x");
    }

    #[test]
    fn display_when_function_then_name_with_parens() {
        let func = Function {
            name: Id::from("ABS"),
            param_assignment: vec![],
        };
        assert_eq!(format!("{func}"), "ABS(...)");
    }

    #[test]
    fn display_when_late_bound_then_value() {
        let lb = LateBound {
            value: Id::from("my_val"),
        };
        assert_eq!(format!("{lb}"), "my_val");
    }

    #[test]
    fn display_when_symbolic_variable_kind_named_then_name() {
        let svk = SymbolicVariableKind::Named(NamedVariable {
            name: Id::from("foo"),
        });
        assert_eq!(format!("{svk}"), "foo");
    }
}
