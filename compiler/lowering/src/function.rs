//! Calls of user-defined functions.
//!
//! A call of a function whose parameters are all `VAR_INPUT`s, `VAR_IN_OUT`s
//! and `VAR_OUTPUT`s of a scalar type, and whose result is a scalar, is a
//! `Call` of the function's POU with one argument for each parameter, in the
//! order the function declares them (REQ-LOW-lowering-071): a `Value` for a
//! `VAR_INPUT`, a `Ref` to the place its variable names for a `VAR_IN_OUT`
//! (REQ-LOW-lowering-075), and a `Ref` to a temporary of the parameter's
//! type for a `VAR_OUTPUT` (REQ-LOW-lowering-092). A `VAR_OUTPUT` may also be
//! a string. The analyzer wrote the call in that order, as its normal form
//! (`specs/design/call-binding.md`), and converted each input to its
//! parameter's type (ADR-0056).
//!
//! A call of a function with outputs is lowered only as the whole value of
//! an assignment, `r := F(a, q => x)`, inside the conversion the analyzer
//! recorded, if any. It lowers to three parts, in the order the AST route
//! runs them, each with the assignment's span:
//!
//! 1. the call, stored in a temporary of the result's type;
//! 2. the assignment of each output to its target, of the value the
//!    analyzer recorded for it: the call output `q`, a read of the temporary
//!    passed for `q`;
//! 3. the assignment, of a read of the result's temporary.
//!
//! So an output reaches its target, converted, only once the call returns,
//! and before the result does. A temporary belongs to the body the call is
//! in, and each call has its own.
//!
//! The function's POU is declared with its parameters and result the first
//! time a call reaches it. Its parameters and result are the variables its
//! body reads and writes, so a call and the body agree on them.
//!
//! Not lowered yet: a call of a function with outputs anywhere else, a
//! parameter other than an output, or a result, that is a string or an
//! aggregate (REQ-LOW-lowering-079), and a call with `EN` or `ENO`
//! (REQ-LOW-lowering-160).

use ironplc_dsl::common::{FunctionDeclaration, VariableType};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{
    Assignment, Expr as AstExpr, ExprKind as AstExprKind, Function, ParamAssignmentKind,
};
use ironplc_ir::execution::DebugName;
use ironplc_ir::lowered::{
    Arg, Owner, Param, Place, PouId, Projection, Stmt, TypeId, TypeKind, VarId,
};

use crate::expr::Lowered;
use crate::variables::BodyFrame;
use crate::{Body, CallOutputs, Lowering};

/// A lowered node, `None` when lowering does not support it, or something
/// inside it, yet.
type Made<T> = Result<Option<T>, Diagnostic>;

/// The section that declares a parameter of a function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    Input,
    InOut,
    Output,
}

/// A call's arguments, and the temporary it passes for each output of the
/// function, by the output's name.
struct CallArgs {
    pou: PouId,
    args: Vec<Arg>,
    temporaries: Vec<(Id, Place)>,
}

impl<'a> Lowering<'a> {
    /// The call `func`, the expression `expr`, of the user-defined function
    /// `function`.
    pub(crate) fn user_function_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        func: &Function,
        function: &'a FunctionDeclaration,
    ) -> Lowered {
        let Some(parameters) = parameters(function) else {
            return Ok(None);
        };
        if has_outputs(&parameters) {
            // A call of a function with outputs is the whole value of the
            // assignment `assignment_of_call` lowers, which stored the
            // call's result in a temporary first.
            return match self.call_result.clone() {
                Some((call, result)) if call == address(func) => self.read(result, expr.span()),
                Some(_) | None => Ok(None),
            };
        }
        let Some(call) = self.call_args(frame, func, function, &parameters, &expr.span())? else {
            return Ok(None);
        };
        self.program
            .call(call.pou, call.args, expr.span())
            .map(Some)
    }

    /// The call of a function with outputs that `value`, the value of an
    /// assignment, is, inside the conversion the analyzer recorded; `None`
    /// for any other value.
    pub(crate) fn call_with_outputs<'v>(
        &self,
        value: &'v AstExpr,
    ) -> Option<(&'v Function, &'a FunctionDeclaration)> {
        match &value.kind {
            AstExprKind::Expression(inner) => self.call_with_outputs(inner),
            AstExprKind::ImplicitConversion(conversion) => {
                self.call_with_outputs(&conversion.value)
            }
            AstExprKind::Function(func) => {
                let signature = self.analysis.context().functions().get(&func.name)?;
                if signature.builtin_function.is_some() {
                    return None;
                }
                let Some(Body::Function(function)) = self.body_named(&func.name) else {
                    return None;
                };
                let declares_outputs = function
                    .variables
                    .iter()
                    .any(|decl| decl.var_type == VariableType::Output);
                declares_outputs.then_some((func, function))
            }
            AstExprKind::Compare(_)
            | AstExprKind::BinaryOp(_)
            | AstExprKind::UnaryOp(_)
            | AstExprKind::Const(_)
            | AstExprKind::EnumeratedValue(_)
            | AstExprKind::Variable(_)
            | AstExprKind::MethodCall(_)
            | AstExprKind::LateBound(_)
            | AstExprKind::Ref(_)
            | AstExprKind::Deref(_)
            | AstExprKind::CallOutput(_)
            | AstExprKind::Null(_) => None,
        }
    }

    /// `assignment`, whose value is the call `func` of `function`, which has
    /// outputs, inside the conversion the analyzer recorded: the call stored
    /// in a temporary, the assignments of its outputs, then the assignment
    /// of the temporary (REQ-LOW-lowering-092).
    pub(crate) fn assignment_of_call(
        &mut self,
        frame: &BodyFrame,
        assignment: &Assignment,
        func: &Function,
        function: &'a FunctionDeclaration,
    ) -> Made<Vec<Stmt>> {
        // Every part is the statement the assignment is, for the line map
        // (REQ-LOW-lowering-063).
        let span = assignment.span();
        let Some(parameters) = parameters(function) else {
            return Ok(None);
        };
        let Some(call) = self.call_args(frame, func, function, &parameters, &span)? else {
            return Ok(None);
        };
        let Some(result_type) = self.result_type(call.pou) else {
            return Ok(None);
        };
        let value = self.program.call(call.pou, call.args, func.name.span())?;
        let result = self.temporary(frame, format!("{}()", function.name), result_type, &span)?;
        let mut stmts = vec![self.assign_value(result.clone(), value, span.clone())?];
        let Some(outputs) = self.outputs(
            frame,
            CallOutputs::Temporaries(call.temporaries),
            &func.param_assignment,
            &span,
        )?
        else {
            return Ok(None);
        };
        stmts.extend(outputs);
        let enclosing = self.call_result.replace((address(func), result));
        let stored = self.assignment(frame, assignment);
        self.call_result = enclosing;
        let Some(stored) = stored? else {
            return Ok(None);
        };
        stmts.push(stored);
        Ok(Some(stmts))
    }

    /// The arguments of the call `func` of `function`, whose parameters are
    /// `parameters`, in the body of `frame`, at `span`.
    fn call_args(
        &mut self,
        frame: &BodyFrame,
        func: &Function,
        function: &'a FunctionDeclaration,
        parameters: &[(&Id, Section)],
        span: &SourceSpan,
    ) -> Made<CallArgs> {
        let Some(pou) = self.function_pou(function, parameters)? else {
            return Ok(None);
        };
        let Some(params) = self.program.params(pou) else {
            return Ok(None);
        };
        // One named input for each input and in-out, in order, then the
        // outputs the call assigns: an `EN` or an `ENO` is not lowered yet.
        let mut inputs = func
            .param_assignment
            .iter()
            .filter(|assignment| !matches!(assignment, ParamAssignmentKind::Output(_)));
        let mut args = Vec::with_capacity(parameters.len());
        let mut temporaries = Vec::new();
        for ((name, section), param) in parameters.iter().zip(&params) {
            match (section, param) {
                (Section::Input | Section::InOut, _) => {
                    let Some(ParamAssignmentKind::NamedInput(input)) = inputs.next() else {
                        return Ok(None);
                    };
                    if input.name != **name {
                        return Ok(None);
                    }
                    let Some(arg) = self.arg(frame, *param, &input.expr)? else {
                        return Ok(None);
                    };
                    args.push(arg);
                }
                (Section::Output, Param::Ref(ty)) => {
                    let label = format!("{}.{}", function.name, name);
                    let temporary = self.temporary(frame, label, *ty, span)?;
                    args.push(Arg::Ref(temporary.clone()));
                    temporaries.push(((*name).clone(), temporary));
                }
                (Section::Output, Param::Value(_) | Param::Str(_)) => return Ok(None),
            }
        }
        if inputs.next().is_some() {
            return Ok(None);
        }
        // A `VAR_IN_OUT`'s variable is of its parameter's type, which the
        // analyzer checked; one lowered to another type stays on the AST.
        let passed: Vec<Param> = args.iter().map(Arg::param).collect();
        if passed != params {
            return Ok(None);
        }
        Ok(Some(CallArgs {
            pou,
            args,
            temporaries,
        }))
    }

    /// The POU of `function`, whose parameters are `parameters`, declared
    /// with its parameters and result the first time it is asked for; `None`
    /// when one of them is not a scalar, or, for an output, a string.
    fn function_pou(
        &mut self,
        function: &'a FunctionDeclaration,
        parameters: &[(&Id, Section)],
    ) -> Made<PouId> {
        let Some(frame) = self.frame(Body::Function(function))? else {
            return Ok(None);
        };
        if self.program.pous().function(frame.pou).is_some() {
            return Ok(Some(frame.pou));
        }
        let mut variables = Vec::with_capacity(parameters.len());
        for (name, section) in parameters {
            let string = *section == Section::Output;
            let Some(parameter) = self.scalar_variable(&frame, name, string)? else {
                return Ok(None);
            };
            variables.push(parameter);
        }
        // The result is the variable named after the function, unless that
        // name is a parameter's.
        let Some(result) = self.scalar_variable(&frame, &function.name, false)? else {
            return Ok(None);
        };
        if variables.contains(&result) {
            return Ok(None);
        }
        let span = function.name.span();
        self.program
            .declare_function(frame.pou, variables, result, &span)?;
        Ok(Some(frame.pou))
    }

    /// The variable `name` refers to in the body of `frame`, a function;
    /// `None` unless it is a variable of a scalar type, or a `VAR_IN_OUT` or
    /// `VAR_OUTPUT`, a reference to one. Where `string`, it may refer to a
    /// string.
    fn scalar_variable(&mut self, frame: &BodyFrame, name: &Id, string: bool) -> Made<VarId> {
        let Some(place) = self.named_place(frame, name, &name.span())? else {
            return Ok(None);
        };
        let types = self.program.types();
        let referred_string = matches!(place.path(), [Projection::Deref])
            && matches!(types.kind(place.ty()), Some(TypeKind::String(_)));
        let whole = matches!(place.path(), [] | [Projection::Deref]);
        let scalar = whole && types.operation_type(place.ty()).is_some();
        Ok((scalar || (string && referred_string)).then(|| place.root()))
    }

    /// The type of the result of the function `pou`.
    fn result_type(&self, pou: PouId) -> Option<TypeId> {
        let result = self.program.pous().function(pou)?.result();
        Some(self.program.variables().get(result)?.ty())
    }

    /// A new temporary of type `ty` for a call at `span` in the body of
    /// `frame`, named `label` for debug information. The label is not an
    /// identifier, so no name in the source refers to the temporary.
    fn temporary(
        &mut self,
        frame: &BodyFrame,
        label: String,
        ty: TypeId,
        span: &SourceSpan,
    ) -> Result<Place, Diagnostic> {
        let name = DebugName::new(label, span.clone());
        let temporary = self
            .program
            .push_variable(name, Owner::Pou(frame.pou), ty)?;
        self.program.var(temporary, span)
    }
}

/// The name and section of each parameter of `function`, in the order it
/// declares them; `None` when one is not a `VAR_INPUT`, a `VAR_IN_OUT` or a
/// `VAR_OUTPUT`.
fn parameters(function: &FunctionDeclaration) -> Option<Vec<(&Id, Section)>> {
    if !function.edge_variables.is_empty() {
        return None;
    }
    let mut parameters = Vec::new();
    for decl in &function.variables {
        let section = match decl.var_type {
            VariableType::Input => Section::Input,
            VariableType::InOut => Section::InOut,
            VariableType::Output => Section::Output,
            VariableType::Var | VariableType::VarTemp | VariableType::External => continue,
            VariableType::Global | VariableType::Access => return None,
        };
        parameters.push((decl.identifier.symbolic_id()?, section));
    }
    Some(parameters)
}

/// Whether one of `parameters` is a `VAR_OUTPUT`.
fn has_outputs(parameters: &[(&Id, Section)]) -> bool {
    parameters
        .iter()
        .any(|(_, section)| *section == Section::Output)
}

/// The address of `func`, which tells one call in the source from another.
fn address(func: &Function) -> usize {
    std::ptr::from_ref(func) as usize
}
