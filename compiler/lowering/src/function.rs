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
//! A call of a function or method with outputs in an expression is an
//! in-place call (REQ-LOW-lowering-172): the call, then the store of each
//! output in its target, of the value the analyzer recorded for it, the call
//! output `q` a read of the temporary passed for `q`. The expression runs it
//! where the source evaluates the call (REQ-LOW-lowering-174), so an output
//! reaches its target, converted, once the call returns and before anything
//! the source evaluates after the call. A temporary belongs to the body the
//! call is in, and each call has its own.
//!
//! The function's or method's POU is declared with its parameters and
//! result the first time a call reaches it. Its parameters and result are
//! the variables its body reads and writes, so a call and the body agree on
//! them.
//!
//! A function whose result is a string or a structure returns it through
//! its result parameter, its first, to a temporary each call passes
//! (REQ-LOW-lowering-079). A call of one whose result is a string is a
//! string in-place call (`string_result.rs`), and a call of one whose result
//! is a structure is a call statement whose result is copied to its target
//! (`structure_result.rs`).
//!
//! An input that is an array or a structure is passed by copy
//! (REQ-LOW-lowering-075).
//!
//! Not lowered yet: a `VAR_IN_OUT` that is a string, a string or structure
//! result of a method, an in-out or an output that is an aggregate, a result
//! that is an array (REQ-LOW-lowering-079), a call with `EN` or `ENO` (REQ-LOW-lowering-160),
//! a call through `THIS^` or `SUPER^`, and a call of a method its receiver's
//! block inherits from a block whose type it does not extend in the lowered
//! program (REQ-LOW-lowering-177).

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
    Arg, Expr, Owner, Param, Place, PouId, Projection, Stmt, TypeId, TypeKind, VarId,
};

use crate::variables::{BodyFrame, CallOutputs};
use crate::{Body, Lowered, Lowering, Unlowered};

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

    /// The body of the function or method the call calls.
    fn body(self) -> Body<'a> {
        match self {
            UserCall::Function { function, .. } => Body::Function(function),
            UserCall::Method { block, method, .. } => Body::Method { block, method },
        }
    }

    /// The name and section of each parameter, in the order the callee
    /// declares them; `None` when one is not a `VAR_INPUT`, a `VAR_IN_OUT`
    /// or a `VAR_OUTPUT`.
    fn parameters(self) -> Option<Vec<(&'a Id, Section)>> {
        body_parameters(self.body())
    }
}

/// The name and section of each parameter of the function or method `body`,
/// in the order it declares them; `None` when one is not a `VAR_INPUT`, a
/// `VAR_IN_OUT` or a `VAR_OUTPUT`, and for any other body.
fn body_parameters(body: Body<'_>) -> Option<Vec<(&Id, Section)>> {
    match body {
        Body::Function(function) if function.edge_variables.is_empty() => {
            parameters(&function.variables)
        }
        Body::Method { method, .. }
            if method.edge_variables.is_empty() && method.implicit_variables.is_empty() =>
        {
            parameters(&method.variables)
        }
        Body::Function(_) | Body::Method { .. } | Body::Program(_) | Body::FunctionBlock(_) => None,
    }
}

/// A call's arguments, the temporary it passes for each output, by the
/// output's name, and the one it passes for a string or structure result.
pub(crate) struct CallArgs {
    pub(crate) pou: PouId,
    pub(crate) args: Vec<Arg>,
    temporaries: Vec<(Id, Place)>,
    /// The temporary the function writes its string or structure result to
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
    ) -> Lowered<Expr> {
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
    ) -> Lowered<Expr> {
        let call = self.method_call(frame, call).ok_or(Unlowered::NotYet)?;
        self.user_call(frame, expr, call)
    }

    /// The call `call`, the expression `expr`, a value: a `Call`, or an
    /// in-place call of a callee with outputs.
    fn user_call(
        &mut self,
        frame: &BodyFrame,
        expr: &AstExpr,
        call: UserCall<'_, 'a>,
    ) -> Lowered<Expr> {
        let span = expr.span();
        let (args, stores) = self.call_with_stores(frame, call, &span)?;
        if self.result_type(args.pou).is_none() {
            // The analyzer rejects a value of a method without a result
            // (P4057), a function whose result is a string is a string, and
            // one whose result is a structure is only assigned whole.
            return Err(Unlowered::NotYet);
        }
        match stores {
            Some(stores) => Ok(self
                .program
                .call_in_place(args.pou, args.args, stores, span)?),
            None => Ok(self.program.call(args.pou, args.args, span)?),
        }
    }

    /// The arguments of `call`, at `span`, and, when its callee has
    /// outputs, the store of each output the call assigns, from the
    /// temporary passed for it (REQ-LOW-lowering-092).
    pub(crate) fn call_with_stores(
        &mut self,
        frame: &BodyFrame,
        call: UserCall<'_, 'a>,
        span: &SourceSpan,
    ) -> Lowered<(CallArgs, Option<Vec<Stmt>>)> {
        let parameters = call.parameters().ok_or(Unlowered::NotYet)?;
        let mut args = self.call_args(frame, call, &parameters, span)?;
        if !has_outputs(&parameters) {
            return Ok((args, None));
        }
        let temporaries = std::mem::take(&mut args.temporaries);
        let stores = self.outputs(
            frame,
            CallOutputs::Temporaries(temporaries),
            call.params(),
            span,
        )?;
        Ok((args, Some(stores)))
    }

    /// The method call statement `call`: a `Call` statement, whose result,
    /// if any, is discarded, then the assignments of its outputs.
    pub(crate) fn method_call_statement(
        &mut self,
        frame: &BodyFrame,
        call: &MethodCall,
    ) -> Lowered<Vec<Stmt>> {
        // Every part is the statement the call is, for the line map
        // (REQ-LOW-lowering-063).
        let span = call.span();
        let user_call = self.method_call(frame, call).ok_or(Unlowered::NotYet)?;
        let (args, stores) = self.call_with_stores(frame, user_call, &span)?;
        let mut stmts = vec![self.program.call_stmt(args.pou, args.args, span)?];
        stmts.extend(stores.unwrap_or_default());
        Ok(stmts)
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
                LibraryElementKind::FunctionBlockDeclaration(block) if block.name == *name => {
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

    /// The arguments of `call`, whose callee's parameters are `parameters`,
    /// in the body of `frame`, at `span`: for a method, a `Ref` to the
    /// receiver first.
    fn call_args(
        &mut self,
        frame: &BodyFrame,
        call: UserCall<'_, 'a>,
        parameters: &[(&Id, Section)],
        span: &SourceSpan,
    ) -> Lowered<CallArgs> {
        let callee = call.callee().span();
        let pou = self.callee_pou(call.body(), parameters, &callee)?;
        let params = self.program.params(pou).ok_or(Unlowered::NotYet)?;
        let mut args = Vec::with_capacity(params.len());
        let mut declared = params.iter();
        if let UserCall::Method { call, .. } = call {
            let MethodReceiver::Instance(instance) = &call.receiver else {
                return Err(Unlowered::NotYet);
            };
            let receiver = self.named_place(frame, instance, &instance.span())?;
            // The instance parameter.
            declared.next();
            args.push(Arg::Ref(receiver));
        }
        // A function whose result is a string or a structure writes it
        // through its result parameter, to a new temporary of the result's
        // type (REQ-LOW-lowering-079).
        let returns_through = self
            .program
            .pous()
            .function(pou)
            .and_then(|signature| signature.result_parameter())
            .is_some();
        let mut result = None;
        if returns_through {
            let Some(Param::Ref(ty)) = declared.next() else {
                return Err(Unlowered::NotYet);
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
                        return Err(Unlowered::NotYet);
                    };
                    if input.name != **name {
                        return Err(Unlowered::NotYet);
                    }
                    args.push(self.arg(frame, *param, &input.expr)?);
                }
                (Section::Output, Param::Ref(ty)) => {
                    let label = format!("{}.{}", call.callee(), name);
                    let temporary = self.temporary(frame, label, *ty, span)?;
                    args.push(Arg::Ref(temporary.clone()));
                    temporaries.push(((*name).clone(), temporary));
                }
                (Section::Output, Param::Value(_) | Param::Str(_) | Param::Copy(_)) => {
                    return Err(Unlowered::NotYet)
                }
            }
        }
        if inputs.next().is_some() {
            return Err(Unlowered::NotYet);
        }
        // A `VAR_IN_OUT`'s variable, and a receiver, is of its parameter's
        // type, which the analyzer checked, or a receiver of a block that
        // extends the method's; one lowered to another type, such as a
        // receiver of a block whose type records no base, is not lowered.
        let types = self.program.types();
        let passes = args.len() == params.len()
            && args
                .iter()
                .zip(&params)
                .all(|(arg, param)| arg.param().passes_for(*param, types));
        if !passes {
            return Err(Unlowered::NotYet);
        }
        Ok(CallArgs {
            pou,
            args,
            temporaries,
            result,
        })
    }

    /// The POU of the function or method `body`, declared with its
    /// parameters and result the first time a call reaches it or the whole
    /// program is lowered. Any other body, and one whose parameters are not
    /// ones lowering expresses yet, is not lowered.
    pub(crate) fn signature_pou(&mut self, body: Body<'a>, span: &SourceSpan) -> Lowered<PouId> {
        let parameters = body_parameters(body).ok_or(Unlowered::NotYet)?;
        self.callee_pou(body, &parameters, span)
    }

    /// The POU of the function or method `body`, whose parameters are
    /// `parameters`, declared with its parameters and result, at `span`,
    /// the first time it is asked for. A parameter that is not a scalar, or,
    /// for an input or an output, a string, or, for an input, an array or a
    /// structure, is not lowered yet.
    fn callee_pou(
        &mut self,
        body: Body<'a>,
        parameters: &[(&Id, Section)],
        span: &SourceSpan,
    ) -> Lowered<PouId> {
        let result = match body {
            Body::Function(function) => Some(&function.name),
            Body::Method { method, .. } => method.return_type.as_ref().map(|_| &method.name),
            Body::Program(_) | Body::FunctionBlock(_) => return Err(Unlowered::NotYet),
        };
        let frame = self.frame(body)?;
        if self.program.pous().function(frame.pou).is_some() {
            return Ok(frame.pou);
        }
        let mut variables = Vec::with_capacity(parameters.len() + 1);
        if let Some(instance) = &frame.instance {
            variables.push(instance.this);
        }
        for (name, section) in parameters {
            let parameter = match section {
                // An array, structure or function block input is passed by
                // copy (REQ-LOW-lowering-075).
                Section::Input => match self.aggregate_variable(&frame, name)? {
                    Some(aggregate) => aggregate,
                    None => self.scalar_variable(&frame, name, true)?,
                },
                Section::Output => self.scalar_variable(&frame, name, true)?,
                Section::InOut => self.scalar_variable(&frame, name, false)?,
            };
            variables.push(parameter);
        }
        // A function's string or structure result is its result parameter,
        // first among its parameters (REQ-LOW-lowering-079).
        let result_parameter = match body {
            Body::Function(function) => self.result_parameter(&frame, &function.name)?,
            Body::Method { .. } | Body::Program(_) | Body::FunctionBlock(_) => None,
        };
        if let Some(result_parameter) = result_parameter {
            variables.insert(0, result_parameter);
        }
        // A method's array or structure result is a variable of its body
        // that no call reads (REQ-LOW-lowering-187).
        let result = match (body, result) {
            (Body::Method { .. }, Some(name)) => match self.aggregate_variable(&frame, name)? {
                Some(aggregate) => {
                    self.aggregate_results.insert(frame.pou, aggregate);
                    None
                }
                None => Some(name),
            },
            (
                Body::Function(_) | Body::Method { .. } | Body::Program(_) | Body::FunctionBlock(_),
                result,
            ) => result,
        };
        // Any other result is the variable named after the function or
        // method, unless that name is a parameter's.
        let result = match (result_parameter, result) {
            (None, Some(name)) => {
                let result = self.scalar_variable(&frame, name, false)?;
                if variables.contains(&result) {
                    return Err(Unlowered::NotYet);
                }
                Some(result)
            }
            (Some(_), _) | (None, None) => None,
        };
        match (body, result_parameter, result) {
            (Body::Function(_), Some(_), _) => {
                self.program
                    .declare_function_returning_through(frame.pou, variables, span)?;
            }
            (Body::Function(_), None, Some(result)) => {
                self.program
                    .declare_function(frame.pou, variables, result, span)?;
            }
            (Body::Method { .. }, _, result) => {
                self.program
                    .declare_method(frame.pou, variables, result, span)?;
            }
            (Body::Function(_), None, None) | (Body::Program(_) | Body::FunctionBlock(_), _, _) => {
                return Err(Unlowered::NotYet)
            }
        }
        Ok(frame.pou)
    }

    /// The result parameter of the function named `name`, whose body is
    /// `frame`, when its result is a string or a structure: the variable its
    /// body reaches the result through (REQ-LOW-lowering-079). `None` for
    /// any other result.
    fn result_parameter(&mut self, frame: &BodyFrame, name: &Id) -> Lowered<Option<VarId>> {
        let place = self.named_place(frame, name, &name.span())?;
        let through = matches!(place.path(), [Projection::Deref]);
        let returned = matches!(
            self.program.types().kind(place.ty()),
            Some(TypeKind::String(_) | TypeKind::Structure { .. })
        );
        Ok((through && returned).then(|| place.root()))
    }

    /// The variable `name` refers to in the body of `frame`, a function or
    /// method, when it is the whole of an array, a structure or a function
    /// block instance, as an input of one is; `None` otherwise.
    fn aggregate_variable(&mut self, frame: &BodyFrame, name: &Id) -> Lowered<Option<VarId>> {
        let place = self.named_place(frame, name, &name.span())?;
        let aggregate = place.path().is_empty()
            && matches!(
                self.program.types().kind(place.ty()),
                Some(
                    TypeKind::Array { .. }
                        | TypeKind::Structure { .. }
                        | TypeKind::FunctionBlock { .. }
                )
            );
        Ok(aggregate.then(|| place.root()))
    }

    /// The variable `name` refers to in the body of `frame`, a function or
    /// method, when it is a variable of a scalar type, or a `VAR_IN_OUT` or
    /// `VAR_OUTPUT`, a reference to one. Where `string`, it may also be a
    /// string, as a `VAR_INPUT` is, or refer to one, as a `VAR_OUTPUT` does.
    /// Any other variable is not lowered as a parameter yet.
    fn scalar_variable(&mut self, frame: &BodyFrame, name: &Id, string: bool) -> Lowered<VarId> {
        let place = self.named_place(frame, name, &name.span())?;
        let types = self.program.types();
        let whole = matches!(place.path(), [] | [Projection::Deref]);
        let scalar = whole && types.operation_type(place.ty()).is_some();
        let of_string = whole && matches!(types.kind(place.ty()), Some(TypeKind::String(_)));
        if !(scalar || (string && of_string)) {
            return Err(Unlowered::NotYet);
        }
        Ok(place.root())
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
    pub(crate) fn temporary(
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
