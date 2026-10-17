//! Calls of user-defined functions and methods.
//!
//! A call of a function whose parameters are all `VAR_INPUT`s, `VAR_IN_OUT`s
//! and `VAR_OUTPUT`s of a scalar type, and whose result is a scalar, is a
//! `Call` of the function's POU with one argument for each parameter, in the
//! order the function declares them (REQ-LOW-lowering-071): a `Value` for a
//! `VAR_INPUT`, a `Ref` to the place its variable names for a `VAR_IN_OUT`
//! (REQ-LOW-lowering-075), and a `Ref` to a temporary of the parameter's
//! type for a `VAR_OUTPUT` (REQ-LOW-lowering-092). A `VAR_INPUT` may also be
//! a string, held by value and passed a `Str` of its encoding
//! (REQ-LOW-lowering-072), and a `VAR_OUTPUT` may be one too. The analyzer
//! wrote the call in that order, as its normal form
//! (`specs/design/call-binding.md`), and converted each input to its
//! parameter's type (ADR-0056).
//!
//! A method is called as a function is, with a `Ref` to the receiver's place
//! first, for its instance parameter (REQ-LOW-lowering-076). Its result, if
//! it has one, is a scalar. A call whose result is not used is a `Call`
//! statement, followed by the assignments of its outputs.
//!
//! A call of a function or method with outputs is lowered as a value only
//! in the value of an assignment, ahead of which it is lowered as its part
//! (`hoist.rs`, REQ-LOW-lowering-172):
//!
//! 1. the call, stored in a temporary of the result's type;
//! 2. the assignment of each output to its target, of the value the
//!    analyzer recorded for it: the call output `q`, a read of the temporary
//!    passed for `q`.
//!
//! The call's value is a read of the result's temporary. So an output
//! reaches its target, converted, once the call returns, and before the
//! assignment does. A temporary belongs to the body the call is in, and each
//! call has its own.
//!
//! The function's or method's POU is declared with its parameters and
//! result the first time a call reaches it. Its parameters and result are
//! the variables its body reads and writes, so a call and the body agree on
//! them.
//!
//! A function whose result is a string returns it through its result
//! parameter, its first, to a temporary each call passes
//! (REQ-LOW-lowering-079); a call of one is moved out of the statement it is
//! in (`string_result.rs`).
//!
//! Not lowered yet: a call with outputs anywhere else, or where running it
//! first would change the order (REQ-LOW-lowering-174), a `VAR_IN_OUT` that
//! is a string, a string result of a function with outputs or of a method,
//! a parameter or a result that is an aggregate (REQ-LOW-lowering-079), a
//! call with `EN` or `ENO` (REQ-LOW-lowering-160), a call through `THIS^` or
//! `SUPER^`, and a call of a method its receiver's block inherits from a
//! block whose type it does not extend in the lowered program
//! (REQ-LOW-lowering-177). The route that reads the AST passes no string to
//! a `VAR_IN_OUT` (`compile_reference_arg`).

use ironplc_analyzer::symbol_environment::Resolution;
use ironplc_dsl::common::{
    FunctionBlockDeclaration, FunctionDeclaration, LibraryElementKind, TypeName, VarDecl,
    VariableType,
};
use ironplc_dsl::core::{Id, Located, SourceSpan};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::oop::MethodDeclaration;
use ironplc_dsl::textual::{
    Expr as AstExpr, Function, MethodCall, MethodReceiver, ParamAssignmentKind,
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

/// The section that declares a parameter of a function or method.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    Input,
    InOut,
    Output,
}

/// A call of a user-defined function or method, as the source writes it,
/// and the declaration it calls.
#[derive(Clone, Copy, Debug)]
pub(crate) enum UserCall<'v, 'a> {
    Function {
        call: &'v Function,
        function: &'a FunctionDeclaration,
    },
    /// A call of a method its receiver's block declares.
    Method {
        call: &'v MethodCall,
        block: &'a FunctionBlockDeclaration,
        method: &'a MethodDeclaration,
    },
}

impl<'v, 'a> UserCall<'v, 'a> {
    /// The call's arguments, in normal form.
    fn params(self) -> &'v [ParamAssignmentKind] {
        match self {
            UserCall::Function { call, .. } => &call.param_assignment,
            UserCall::Method { call, .. } => &call.params,
        }
    }

    /// The name of the function or method, for the debug names of the
    /// call's temporaries.
    fn callee(self) -> &'a Id {
        match self {
            UserCall::Function { function, .. } => &function.name,
            UserCall::Method { method, .. } => &method.name,
        }
    }

    /// Where the call names its callee.
    fn name_span(self) -> SourceSpan {
        match self {
            UserCall::Function { call, .. } => call.name.span(),
            UserCall::Method { call, .. } => call.method.span(),
        }
    }

    /// The address of the call, which tells one call in the source from
    /// another.
    pub(crate) fn address(self) -> usize {
        match self {
            UserCall::Function { call, .. } => std::ptr::from_ref(call) as usize,
            UserCall::Method { call, .. } => std::ptr::from_ref(call) as usize,
        }
    }

    /// The name and section of each parameter, in the order the callee
    /// declares them; `None` when one is not a `VAR_INPUT`, a `VAR_IN_OUT`
    /// or a `VAR_OUTPUT`.
    fn parameters(self) -> Option<Vec<(&'a Id, Section)>> {
        match self {
            UserCall::Function { function, .. } if function.edge_variables.is_empty() => {
                parameters(&function.variables)
            }
            UserCall::Method { method, .. }
                if method.edge_variables.is_empty() && method.implicit_variables.is_empty() =>
            {
                parameters(&method.variables)
            }
            UserCall::Function { .. } | UserCall::Method { .. } => None,
        }
    }
}

/// A call's arguments, the temporary it passes for each output, by the
/// output's name, and the one it passes for a string result.
pub(crate) struct CallArgs {
    pub(crate) pou: PouId,
    pub(crate) args: Vec<Arg>,
    temporaries: Vec<(Id, Place)>,
    /// The temporary the function writes its string result to
    /// (REQ-LOW-lowering-079); `None` for any other callee.
    pub(crate) result: Option<Place>,
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
        self.user_call(
            frame,
            expr,
            UserCall::Function {
                call: func,
                function,
            },
        )
    }

    /// The method call `call`, the expression `expr`.
    pub(crate) fn method_call_expr(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        call: &MethodCall,
    ) -> Lowered {
        match self.method_call(frame, call) {
            Some(call) => self.user_call(frame, expr, call),
            None => Ok(None),
        }
    }

    /// The call `call`, the expression `expr`, a value.
    fn user_call(&mut self, frame: &BodyFrame, expr: &AstExpr, call: UserCall<'_, 'a>) -> Lowered {
        let Some(parameters) = call.parameters() else {
            return Ok(None);
        };
        if has_outputs(&parameters) {
            return self.hoisted_call(frame, expr, call, &parameters);
        }
        let Some(args) = self.call_args(frame, call, &parameters, &expr.span())? else {
            return Ok(None);
        };
        if self.result_type(args.pou).is_none() {
            // The analyzer rejects a value of a method without a result
            // (P4057).
            return Ok(None);
        }
        self.program
            .call(args.pou, args.args, expr.span())
            .map(Some)
    }

    /// The method call statement `call`: a `Call` statement, whose result,
    /// if any, is discarded, then the assignments of its outputs.
    pub(crate) fn method_call_statement(
        &mut self,
        frame: &BodyFrame,
        call: &MethodCall,
    ) -> Made<Vec<Stmt>> {
        // Every part is the statement the call is, for the line map
        // (REQ-LOW-lowering-063).
        let span = call.span();
        let Some(user_call) = self.method_call(frame, call) else {
            return Ok(None);
        };
        let Some(parameters) = user_call.parameters() else {
            return Ok(None);
        };
        let Some(args) = self.call_args(frame, user_call, &parameters, &span)? else {
            return Ok(None);
        };
        let mut stmts = vec![self.program.call_stmt(args.pou, args.args, span.clone())?];
        let Some(outputs) = self.outputs(
            frame,
            CallOutputs::Temporaries(args.temporaries),
            &call.params,
            &span,
        )?
        else {
            return Ok(None);
        };
        stmts.extend(outputs);
        Ok(Some(stmts))
    }

    /// The call `call`, the expression `expr`, of a function or method with
    /// outputs, whose parameters are `parameters`, in the value of the
    /// assignment being lowered (`hoist.rs`): a read of the temporary its
    /// part stores its result in. The part is the call stored in that
    /// temporary, then the assignments of the call's outputs from their
    /// temporaries (REQ-LOW-lowering-092, REQ-LOW-lowering-172). `None`
    /// outside an assignment.
    fn hoisted_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        call: UserCall<'_, 'a>,
        parameters: &[(&Id, Section)],
    ) -> Lowered {
        let Some(hoisting) = &self.hoisting else {
            return Ok(None);
        };
        if let Some(result) = hoisting.result(call.address()) {
            return self.read(result.clone(), expr.span());
        }
        let span = hoisting.span().clone();
        let Some(args) = self.call_args(frame, call, parameters, &span)? else {
            return Ok(None);
        };
        let Some(result_type) = self.result_type(args.pou) else {
            return Ok(None);
        };
        let value = self.program.call(args.pou, args.args, call.name_span())?;
        let label = format!("{}()", call.callee());
        let result = self.temporary(frame, label, result_type, &span)?;
        let mut part = vec![self.assign_value(result.clone(), value, span.clone())?];
        let Some(outputs) = self.outputs(
            frame,
            CallOutputs::Temporaries(args.temporaries),
            call.params(),
            &span,
        )?
        else {
            return Ok(None);
        };
        part.extend(outputs);
        let Some(hoisting) = self.hoisting.as_mut() else {
            return Ok(None);
        };
        hoisting.push(call.address(), part, result.clone());
        self.read(result, expr.span())
    }

    /// The method `call` calls in the body of `frame`: the one the block of
    /// its receiver, an instance, declares, or else the nearest block it
    /// extends, which the analyzer resolved (ADR-0041). `None` for a call
    /// through `THIS^` or `SUPER^`. The receiver of an inherited method is
    /// passed for an instance of the block that declares it, which
    /// `call_args` accepts when the receiver's type extends that block's
    /// (REQ-LOW-lowering-177).
    fn method_call<'v>(&self, frame: &BodyFrame, call: &'v MethodCall) -> Option<UserCall<'v, 'a>> {
        let MethodReceiver::Instance(instance) = &call.receiver else {
            return None;
        };
        let context = self.analysis.context();
        let Resolution::Variable { info, .. } =
            context.symbols().resolve_variable(instance, &frame.scope)
        else {
            return None;
        };
        let block_name = context.types().name_of(info.type_id?)?;
        let declaring = self
            .method_owners
            .declaring_block(block_name, &call.method)?;
        let block = self.block_named(declaring)?;
        let method = block
            .methods
            .iter()
            .find(|method| method.name == call.method)?;
        Some(UserCall::Method {
            call,
            block,
            method,
        })
    }

    /// The function block the library declares by the name `name`.
    fn block_named(&self, name: &TypeName) -> Option<&'a FunctionBlockDeclaration> {
        self.analysis
            .library()
            .elements
            .iter()
            .find_map(|element| match element {
                LibraryElementKind::FunctionBlockDeclaration(block)
                    if TypeName::from(&block.name.name.to_string()) == *name =>
                {
                    Some(block)
                }
                LibraryElementKind::DataTypeDeclaration(_)
                | LibraryElementKind::FunctionDeclaration(_)
                | LibraryElementKind::FunctionBlockDeclaration(_)
                | LibraryElementKind::ProgramDeclaration(_)
                | LibraryElementKind::ConfigurationDeclaration(_)
                | LibraryElementKind::GlobalVarDeclarations(_)
                | LibraryElementKind::InterfaceDeclaration(_) => None,
            })
    }

    /// The arguments of `call`, at `span`, when its callee declares no
    /// outputs; `None` for a call of a function or method with outputs, which
    /// a call in an expression is lowered as only in an assignment's value
    /// (`hoist.rs`).
    pub(crate) fn call_args_without_outputs(
        &mut self,
        frame: &BodyFrame,
        call: UserCall<'_, 'a>,
        span: &SourceSpan,
    ) -> Made<CallArgs> {
        let Some(parameters) = call.parameters() else {
            return Ok(None);
        };
        if has_outputs(&parameters) {
            return Ok(None);
        }
        self.call_args(frame, call, &parameters, span)
    }

    /// The arguments of `call`, whose callee's parameters are `parameters`,
    /// in the body of `frame`, at `span`: for a method, a `Ref` to the
    /// receiver first.
    fn call_args(
        &mut self,
        frame: &BodyFrame,
        call: UserCall<'_, 'a>,
        parameters: &[(&Id, Section)],
        span: &SourceSpan,
    ) -> Made<CallArgs> {
        let Some(pou) = self.callee_pou(call, parameters)? else {
            return Ok(None);
        };
        let Some(params) = self.program.params(pou) else {
            return Ok(None);
        };
        let mut args = Vec::with_capacity(params.len());
        let mut declared = params.iter();
        if let UserCall::Method { call, .. } = call {
            let MethodReceiver::Instance(instance) = &call.receiver else {
                return Ok(None);
            };
            let Some(receiver) = self.named_place(frame, instance, &instance.span())? else {
                return Ok(None);
            };
            // The instance parameter.
            declared.next();
            args.push(Arg::Ref(receiver));
        }
        // A function whose result is a string writes it through its result
        // parameter, to a new temporary of the string's type
        // (REQ-LOW-lowering-079).
        let returns_through = self
            .program
            .pous()
            .function(pou)
            .and_then(|signature| signature.result_parameter())
            .is_some();
        let mut result = None;
        if returns_through {
            let Some(Param::Ref(ty)) = declared.next() else {
                return Ok(None);
            };
            let temporary = self.temporary(frame, format!("{}()", call.callee()), *ty, span)?;
            args.push(Arg::Ref(temporary.clone()));
            result = Some(temporary);
        }
        // One named input for each input and in-out, in order, then the
        // outputs the call assigns: an `EN` or an `ENO` is not lowered yet.
        let mut inputs = call
            .params()
            .iter()
            .filter(|assignment| !matches!(assignment, ParamAssignmentKind::Output(_)));
        let mut temporaries = Vec::new();
        for ((name, section), param) in parameters.iter().zip(declared) {
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
                    let label = format!("{}.{}", call.callee(), name);
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
        // A `VAR_IN_OUT`'s variable, and a receiver, is of its parameter's
        // type, which the analyzer checked, or a receiver of a block that
        // extends the method's; one lowered to another type, such as a
        // receiver of a block whose type records no base, stays on the AST.
        let types = self.program.types();
        let passes = args.len() == params.len()
            && args
                .iter()
                .zip(&params)
                .all(|(arg, param)| arg.param().passes_for(*param, types));
        if !passes {
            return Ok(None);
        }
        Ok(Some(CallArgs {
            pou,
            args,
            temporaries,
            result,
        }))
    }

    /// The POU `call` calls, whose parameters are `parameters`, declared
    /// with its parameters and result the first time it is asked for; `None`
    /// when one of them is not a scalar, or, for an input or an output, a
    /// string.
    fn callee_pou(&mut self, call: UserCall<'_, 'a>, parameters: &[(&Id, Section)]) -> Made<PouId> {
        let (body, result) = match call {
            UserCall::Function { function, .. } => (Body::Function(function), Some(&function.name)),
            UserCall::Method { block, method, .. } => (
                Body::Method { block, method },
                method.return_type.as_ref().map(|_| &method.name),
            ),
        };
        let Some(frame) = self.frame(body)? else {
            return Ok(None);
        };
        if self.program.pous().function(frame.pou).is_some() {
            return Ok(Some(frame.pou));
        }
        let mut variables = Vec::with_capacity(parameters.len() + 1);
        if let Some(instance) = &frame.instance {
            variables.push(instance.this);
        }
        for (name, section) in parameters {
            let string = matches!(section, Section::Input | Section::Output);
            let Some(parameter) = self.scalar_variable(&frame, name, string)? else {
                return Ok(None);
            };
            variables.push(parameter);
        }
        // A function's string result is its result parameter, first among
        // its parameters (REQ-LOW-lowering-079).
        let result_parameter = match call {
            UserCall::Function { function, .. } => self.result_parameter(&frame, &function.name)?,
            UserCall::Method { .. } => None,
        };
        if let Some(result_parameter) = result_parameter {
            variables.insert(0, result_parameter);
        }
        // Any other result is the variable named after the function or
        // method, unless that name is a parameter's.
        let result = match (result_parameter, result) {
            (None, Some(name)) => match self.scalar_variable(&frame, name, false)? {
                Some(result) if !variables.contains(&result) => Some(result),
                Some(_) | None => return Ok(None),
            },
            (Some(_), _) | (None, None) => None,
        };
        let span = call.callee().span();
        match (call, result_parameter, result) {
            (UserCall::Function { .. }, Some(_), _) => {
                self.program
                    .declare_function_returning_through(frame.pou, variables, &span)?;
            }
            (UserCall::Function { .. }, None, Some(result)) => {
                self.program
                    .declare_function(frame.pou, variables, result, &span)?;
            }
            (UserCall::Method { .. }, _, result) => {
                self.program
                    .declare_method(frame.pou, variables, result, &span)?;
            }
            (UserCall::Function { .. }, None, None) => return Ok(None),
        }
        Ok(Some(frame.pou))
    }

    /// The result parameter of the function named `name`, whose body is
    /// `frame`, when its result is a string: the variable its body reaches
    /// the result through (REQ-LOW-lowering-079). `None` for any other
    /// result.
    fn result_parameter(&mut self, frame: &BodyFrame, name: &Id) -> Made<VarId> {
        let Some(place) = self.named_place(frame, name, &name.span())? else {
            return Ok(None);
        };
        let through = matches!(place.path(), [Projection::Deref]);
        let string = matches!(
            self.program.types().kind(place.ty()),
            Some(TypeKind::String(_))
        );
        Ok((through && string).then(|| place.root()))
    }

    /// The variable `name` refers to in the body of `frame`, a function or
    /// method; `None` unless it is a variable of a scalar type, or a
    /// `VAR_IN_OUT` or `VAR_OUTPUT`, a reference to one. Where `string`, it
    /// may also be a string, as a `VAR_INPUT` is, or refer to one, as a
    /// `VAR_OUTPUT` does.
    fn scalar_variable(&mut self, frame: &BodyFrame, name: &Id, string: bool) -> Made<VarId> {
        let Some(place) = self.named_place(frame, name, &name.span())? else {
            return Ok(None);
        };
        let types = self.program.types();
        let whole = matches!(place.path(), [] | [Projection::Deref]);
        let scalar = whole && types.operation_type(place.ty()).is_some();
        let of_string = whole && matches!(types.kind(place.ty()), Some(TypeKind::String(_)));
        Ok((scalar || (string && of_string)).then(|| place.root()))
    }

    /// The type of the result of the function or method `pou`; `None` for a
    /// method without one.
    fn result_type(&self, pou: PouId) -> Option<TypeId> {
        let result = self.program.pous().function(pou)?.result()?;
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

/// The name and section of each parameter `variables` declares, in order;
/// `None` when one is not a `VAR_INPUT`, a `VAR_IN_OUT` or a `VAR_OUTPUT`.
fn parameters(variables: &[VarDecl]) -> Option<Vec<(&Id, Section)>> {
    let mut parameters = Vec::new();
    for decl in variables {
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
