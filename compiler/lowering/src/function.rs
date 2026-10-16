//! Calls of user-defined functions.
//!
//! A call of a function whose parameters are all `VAR_INPUT`s and
//! `VAR_IN_OUT`s of a scalar type, and whose result is a scalar, is a `Call`
//! of the function's POU with one argument for each parameter, in the order
//! the function declares them (REQ-LOW-lowering-071): a `Value` for a
//! `VAR_INPUT`, and a `Ref` to the place its variable names for a
//! `VAR_IN_OUT` (REQ-LOW-lowering-075). The analyzer wrote the call in that
//! order, as its normal form (`specs/design/call-binding.md`), and converted
//! each input to its parameter's type (ADR-0056).
//!
//! The function's POU is declared with its parameters and result the first
//! time a call reaches it. Its parameters and result are the variables its
//! body reads and writes, so a call and the body agree on them.
//!
//! Not lowered yet: a function with a `VAR_OUTPUT` (REQ-LOW-lowering-092), a
//! parameter or result that is a string or an aggregate
//! (REQ-LOW-lowering-079), and a call with `EN` or `ENO`
//! (REQ-LOW-lowering-160).

use ironplc_dsl::common::{FunctionDeclaration, VariableType};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::textual::{Expr as AstExpr, Function, ParamAssignmentKind};
use ironplc_ir::lowered::{Arg, Param, PouId, Projection, VarId};

use crate::expr::Lowered;
use crate::variables::BodyFrame;
use crate::{Body, Lowering};

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
        let Some(names) = parameters(function) else {
            return Ok(None);
        };
        let Some(pou) = self.function_pou(function, &names)? else {
            return Ok(None);
        };
        let Some(params) = self.program.params(pou) else {
            return Ok(None);
        };
        // One named input for each parameter, in order: an `EN`, an `ENO` or
        // an output is not lowered yet.
        if func.param_assignment.len() != names.len() {
            return Ok(None);
        }
        let mut args = Vec::with_capacity(names.len());
        for ((assignment, name), param) in func.param_assignment.iter().zip(&names).zip(&params) {
            let ParamAssignmentKind::NamedInput(input) = assignment else {
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
        // A `VAR_IN_OUT`'s variable is of its parameter's type, which the
        // analyzer checked; one lowered to another type stays on the AST.
        let passed: Vec<Param> = args.iter().map(Arg::param).collect();
        if passed != params {
            return Ok(None);
        }
        self.program.call(pou, args, expr.span()).map(Some)
    }

    /// The POU of `function`, whose parameters are named `names`, declared
    /// with its parameters and result the first time it is asked for; `None`
    /// when one of them is not a scalar.
    fn function_pou(
        &mut self,
        function: &'a FunctionDeclaration,
        names: &[&Id],
    ) -> Result<Option<PouId>, Diagnostic> {
        let Some(frame) = self.frame(Body::Function(function))? else {
            return Ok(None);
        };
        if self.program.pous().function(frame.pou).is_some() {
            return Ok(Some(frame.pou));
        }
        let mut parameters = Vec::with_capacity(names.len());
        for name in names {
            let Some(parameter) = self.scalar_variable(&frame, name)? else {
                return Ok(None);
            };
            parameters.push(parameter);
        }
        // The result is the variable named after the function.
        let Some(result) = self.scalar_variable(&frame, &function.name)? else {
            return Ok(None);
        };
        let span = function.name.span();
        self.program
            .declare_function(frame.pou, parameters, result, &span)?;
        Ok(Some(frame.pou))
    }

    /// The variable `name` refers to in the body of `frame`, a function;
    /// `None` unless it is a variable of a scalar type, or a `VAR_IN_OUT`, a
    /// reference to one.
    fn scalar_variable(
        &mut self,
        frame: &BodyFrame,
        name: &Id,
    ) -> Result<Option<VarId>, Diagnostic> {
        let Some(place) = self.named_place(frame, name, &name.span())? else {
            return Ok(None);
        };
        let whole = matches!(place.path(), [] | [Projection::Deref]);
        let scalar = whole && self.program.types().operation_type(place.ty()).is_some();
        Ok(scalar.then(|| place.root()))
    }
}

/// The name of each parameter of `function`, in the order it declares them;
/// `None` when one is not a `VAR_INPUT` or a `VAR_IN_OUT`.
fn parameters(function: &FunctionDeclaration) -> Option<Vec<&Id>> {
    if !function.edge_variables.is_empty() {
        return None;
    }
    let mut names = Vec::new();
    for decl in &function.variables {
        match decl.var_type {
            VariableType::Input | VariableType::InOut => names.push(decl.identifier.symbolic_id()?),
            VariableType::Var | VariableType::VarTemp | VariableType::External => {}
            VariableType::Output | VariableType::Global | VariableType::Access => return None,
        }
    }
    Some(names)
}
