//! Recording the type of an untyped literal from the context it is used in.
//!
//! An untyped literal such as `1` has a generic type (`ANY_INT`) until a
//! context gives it one (ADR-0028). The code generator gives it one top-down:
//! each statement passes the type it stores or tests at into its expression,
//! and every construct on the way to the literal either passes that type on
//! (parentheses), computes at a type of its own (an arithmetic operation, a
//! negation, `ABS` or `MAX` of a concrete type, a comparison, a call to a
//! user-defined function), or computes at a fixed type (a subscript, a
//! string position, a shift count). This module records the type each
//! literal reaches, so that a backend reads it from the literal rather than
//! carrying it down.
//!
//! A literal stored in a function block field, passed to a method parameter
//! or assigned through a dereference takes the declared type of the variable
//! it is stored in. Only a numeric literal is typed: a string literal is
//! compiled in its own encoding, and a bit-string or time literal already
//! has a type.

use ironplc_dsl::common::{ConstantKind, ElementaryTypeName, GenericTypeName, TypeName};
use ironplc_dsl::core::Id;
use ironplc_dsl::textual::{
    Case, CompareExpr, CompareOp, Expr, ExprKind, ExprType, FbCall, For, Function, If, MethodCall,
    MethodReceiver, ParamAssignmentKind, Repeat, SymbolicVariableKind, UnaryOp, Variable, While,
};
use ironplc_dsl::type_id::TypeId;

use super::declared::elementary_of;
use super::ImplicitConversions;
use crate::intermediates::conversion_target::{concrete, wrap};
use crate::intermediates::numeric_operation::{literal_default_type, OperationWidth};
use crate::intermediates::operator_function_form::FormOf;
use crate::intrinsic::Intrinsic;
use crate::semantic_type::{FunctionBlockVarType, SemanticType};
use crate::variable_type;

impl ImplicitConversions<'_> {
    /// Gives the untyped numeric literals of `expr` the type they are
    /// compiled at, when `expr` is compiled at `context`.
    pub(super) fn type_literals(&self, expr: &mut Expr, context: Option<TypeId>) {
        match (&expr.expr_type, context) {
            (Some(ExprType::Literal(generic)), Some(context)) if is_numeric_category(generic) => {
                if self.is_integer_result_in_other_context(expr, context) {
                    self.settle_integer_result(expr, context);
                } else {
                    expr.expr_type = Some(ExprType::Inferred(context));
                }
            }
            // A typed numeric literal (`DINT#5`) keeps its type and is
            // converted to its context's, which codegen compiled it at.
            (Some(ExprType::Concrete(own) | ExprType::Inferred(own)), Some(context))
                if is_numeric_literal(expr) && self.converts_numeric(*own, context) =>
            {
                wrap(expr, context);
                return;
            }
            _ => {}
        }
        let own = concrete(expr);
        // An operation of a numeric result type computes at that type.
        let numeric = self.width_of(expr).and(own);
        let numeric_pair =
            matches!(&expr.kind, ExprKind::Function(func) if self.is_numeric_pair(func, expr));
        match &mut expr.kind {
            ExprKind::Const(_)
            | ExprKind::LateBound(_)
            | ExprKind::EnumeratedValue(_)
            | ExprKind::Ref(_)
            | ExprKind::Null(_) => {}
            // A negation or `NOT` computes at its own numeric type, as an
            // arithmetic operation does.
            ExprKind::UnaryOp(unary) => {
                self.type_literals(&mut unary.term, numeric.or(context).or(own))
            }
            ExprKind::Expression(inner) => self.type_literals(inner, context.or(own)),
            ExprKind::BinaryOp(binary) => {
                if self.has_typed_overload(&binary.op, &binary.left, &binary.right) {
                    self.type_own(&mut binary.left);
                    self.type_own(&mut binary.right);
                } else {
                    // An operation of a numeric type computes at it; any
                    // other at the type of its context.
                    let at = numeric.or(context);
                    self.type_literals(&mut binary.left, at);
                    self.type_literals(&mut binary.right, at);
                }
            }
            ExprKind::Compare(compare) => self.type_compare_literals(compare, context),
            ExprKind::Function(func) => {
                // A function of several inputs of one type converts its
                // inputs to its own type, which a context may have settled
                // only after the call was folded: `l := MAX(1, 2)`.
                if let Some(own) = own {
                    self.record_one_type_call(func, own);
                }
                self.type_call_literals(func, own, numeric, context, numeric_pair)
            }
            ExprKind::MethodCall(call) => self.type_method_call_literals(call),
            ExprKind::Variable(variable) => self.type_variable_literals(variable),
            ExprKind::Deref(inner) | ExprKind::ImplicitConversion(inner) => self.type_own(inner),
        }
    }

    /// Types the literals of `expr` at its own type, or a literal at its
    /// default type: how a construct compiles an operand it does not give a
    /// type.
    fn type_own(&self, expr: &mut Expr) {
        let own = self.own_or_default(expr);
        self.type_literals(expr, own);
    }

    /// The type `expr` is compiled at by a construct that does not give it
    /// one: its own, or the default type of an untyped literal (ADR-0028).
    fn own_or_default(&self, expr: &Expr) -> Option<TypeId> {
        match &expr.expr_type {
            Some(ExprType::Concrete(id) | ExprType::Inferred(id)) => Some(*id),
            Some(ExprType::Literal(generic)) => {
                let default: TypeName = literal_default_type(generic)?.into();
                self.context.types().id_of(&default)
            }
            Some(ExprType::Null) | None => None,
        }
    }

    fn type_compare_literals(&self, compare: &mut CompareExpr, context: Option<TypeId>) {
        let at = if compare.op.is_comparison() {
            // A comparison compares at the type of its left operand, else of
            // its right one; the comparison pass gave both that type.
            self.own_or_default(&compare.left)
                .or_else(|| self.own_or_default(&compare.right))
                .or(context)
        } else {
            // A logical or bitwise operator operates at the type of a
            // concrete operand, else at the left one's default type.
            concrete(&compare.left)
                .or_else(|| concrete(&compare.right))
                .or_else(|| self.own_or_default(&compare.left))
                .or(context)
        };
        self.type_literals(&mut compare.left, at);
        self.type_literals(&mut compare.right, at);
    }

    /// Types the literals of the inputs of `func`, a call whose own type is
    /// `own`, and `numeric` when that is a numeric operation type, compiled
    /// at `context`.
    fn type_call_literals(
        &self,
        func: &mut Function,
        own: Option<TypeId>,
        numeric: Option<TypeId>,
        context: Option<TypeId>,
        numeric_pair: bool,
    ) {
        let intrinsic = self.intrinsic_of(func);
        let one_type = intrinsic.as_ref().and_then(Intrinsic::inputs_of_one_type);
        let typed_pair = matches!(
            func.param_assignment.as_slice(),
            [ParamAssignmentKind::PositionalInput(left), ParamAssignmentKind::PositionalInput(right), ..]
                if self.is_typed_operator_pair(func, &left.expr, &right.expr)
        );
        let mut inputs: Vec<&mut Expr> = func
            .param_assignment
            .iter_mut()
            .filter_map(|input| match input {
                ParamAssignmentKind::PositionalInput(input) => Some(&mut input.expr),
                ParamAssignmentKind::NamedInput(_) | ParamAssignmentKind::Output(_) => None,
            })
            .collect();
        let dint = self.dint();
        // An operation on one value computes at its own numeric type, and any
        // other at the type of its context.
        let operated = if intrinsic
            .as_ref()
            .is_some_and(Intrinsic::computes_at_own_type)
        {
            numeric.or(context)
        } else {
            context
        };
        match intrinsic {
            // A user-defined function, or one the analyzer has no signature
            // for: each argument at its own type, which the argument pass
            // gave it.
            None => inputs.into_iter().for_each(|arg| self.type_own(arg)),
            Some(intrinsic) => match intrinsic {
                Intrinsic::Operator(FormOf::Arithmetic(_)) => {
                    let at = if typed_pair {
                        None
                    } else if numeric_pair {
                        numeric
                    } else {
                        context
                    };
                    for arg in inputs {
                        match at {
                            Some(at) => self.type_literals(arg, Some(at)),
                            None => self.type_own(arg),
                        }
                    }
                }
                Intrinsic::Operator(FormOf::Compare(op)) if op.is_comparison() => {
                    if let [left, right] = inputs.as_mut_slice() {
                        let at = self
                            .own_or_default(left)
                            .or_else(|| self.own_or_default(right))
                            .or(context);
                        self.type_literals(left, at);
                        self.type_literals(right, at);
                    }
                }
                Intrinsic::Operator(FormOf::Compare(_) | FormOf::Not) | Intrinsic::Move => inputs
                    .into_iter()
                    .for_each(|arg| self.type_literals(arg, operated)),
                Intrinsic::Numeric(_) | Intrinsic::Mux => match one_type {
                    // A function of several inputs of one type computes at
                    // its own type. A selector is a BOOL or an integer,
                    // whatever the type of the inputs it selects between.
                    Some(shape) => {
                        for (index, arg) in inputs.into_iter().enumerate() {
                            let at = if index < shape.first {
                                dint
                            } else {
                                own.or(context)
                            };
                            self.type_literals(arg, at);
                        }
                    }
                    None => inputs
                        .into_iter()
                        .for_each(|arg| self.type_literals(arg, operated)),
                },
                Intrinsic::BitShift(_) => {
                    let wide =
                        operated.and_then(|id| self.width_of_type(id)) == Some(OperationWidth::W64);
                    let count = if wide { self.lint() } else { dint };
                    for (index, arg) in inputs.into_iter().enumerate() {
                        self.type_literals(arg, if index == 0 { operated } else { count });
                    }
                }
                // A position or a length is a `DINT`; a string is compiled in
                // its own encoding.
                Intrinsic::String(_) => inputs.into_iter().for_each(|arg| {
                    if self.conversions.is_string(arg) {
                        self.type_own(arg);
                    } else {
                        self.type_literals(arg, dint);
                    }
                }),
                Intrinsic::Conversion { source, .. } => {
                    let source: TypeName = source.into();
                    let at = self.context.types().id_of(&source);
                    inputs
                        .into_iter()
                        .for_each(|arg| self.type_literals(arg, at));
                }
                Intrinsic::Trunc
                | Intrinsic::BcdToInt
                | Intrinsic::IntToBcd
                | Intrinsic::Time { .. }
                | Intrinsic::DtToDate
                | Intrinsic::DtToTod => inputs.into_iter().for_each(|arg| self.type_own(arg)),
                Intrinsic::Sizeof => {}
            },
        }
    }

    /// Returns `true` when `func` is the function form of an arithmetic
    /// operator whose first two inputs have a typed overload.
    fn is_typed_operator_pair(&self, func: &Function, left: &Expr, right: &Expr) -> bool {
        super::arithmetic::arithmetic_operator(func)
            .is_some_and(|op| self.has_typed_overload(&op, left, right))
    }

    /// Types the literals of the arguments of a method call by the method's
    /// parameters.
    fn type_method_call_literals(&self, call: &mut MethodCall) {
        for (arg, at) in self.method_arguments(call) {
            match at {
                Some(at) => self.type_literals(arg, Some(at)),
                None => self.type_own(arg),
            }
        }
    }

    /// The input arguments of the method call `call`, each with the type its
    /// parameter is passed as: positional arguments fill the parameters in
    /// order, and a named one the parameter it names.
    pub(super) fn method_arguments<'c>(
        &self,
        call: &'c mut MethodCall,
    ) -> Vec<(&'c mut Expr, Option<TypeId>)> {
        let parameters = match &call.receiver {
            MethodReceiver::Instance(instance) => {
                variable_type::declared(instance, self.context, &self.scope.current()).and_then(
                    |representation| match representation {
                        SemanticType::FunctionBlock { name, .. } => self
                            .declarations
                            .method(&Id::from(name.as_str()), &call.method),
                        _ => None,
                    },
                )
            }
            MethodReceiver::SelfRef(_) => None,
        };
        let mut position = 0;
        let mut arguments = Vec::with_capacity(call.params.len());
        for param in &mut call.params {
            match param {
                ParamAssignmentKind::PositionalInput(input) => {
                    let at = parameters
                        .and_then(|parameters| parameters.get(position))
                        .and_then(|(_, at)| *at);
                    position += 1;
                    arguments.push((&mut input.expr, at));
                }
                ParamAssignmentKind::NamedInput(input) => {
                    let at = parameters
                        .and_then(|parameters| parameters.iter().find(|(p, _)| *p == input.name))
                        .and_then(|(_, at)| *at);
                    arguments.push((&mut input.expr, at));
                }
                ParamAssignmentKind::Output(_) => {}
            }
        }
        arguments
    }

    /// Types the literals of the subscripts of `variable`, which compile at
    /// `DINT`.
    fn type_variable_literals(&self, variable: &mut Variable) {
        if let Variable::Symbolic(kind) = variable {
            self.type_symbolic_literals(kind);
        }
    }

    fn type_symbolic_literals(&self, kind: &mut SymbolicVariableKind) {
        match kind {
            SymbolicVariableKind::Array(array) => {
                self.type_symbolic_literals(&mut array.subscripted_variable);
                for subscript in &mut array.subscripts {
                    self.type_literals(subscript, self.dint());
                }
            }
            SymbolicVariableKind::Structured(structured) => {
                self.type_symbolic_literals(&mut structured.record)
            }
            SymbolicVariableKind::BitAccess(access) => {
                self.type_symbolic_literals(&mut access.variable)
            }
            SymbolicVariableKind::PartialAccess(access) => {
                self.type_symbolic_literals(&mut access.variable)
            }
            SymbolicVariableKind::Deref(deref) => self.type_symbolic_literals(&mut deref.variable),
            SymbolicVariableKind::Named(_) | SymbolicVariableKind::SelfRef(_) => {}
        }
    }

    /// Types the literals of an assignment's value by its target, and those
    /// of the target's subscripts.
    pub(super) fn type_assignment_literals(
        &self,
        target: &mut Variable,
        deref: bool,
        value: &mut Expr,
    ) {
        let at = self
            .assigned_at(target, deref)
            .or_else(|| self.own_or_default(value));
        self.type_literals(value, at);
        self.type_variable_literals(target);
    }

    /// The type the value assigned to `target` is compiled at, or `None` for
    /// a target that is not a single numeric value (a string, an aggregate)
    /// or a directly represented variable, whose value compiles at its own.
    pub(super) fn assigned_at(&self, target: &Variable, deref: bool) -> Option<TypeId> {
        let Variable::Symbolic(kind) = target else {
            return None;
        };
        // A dereference stores into the variable the reference refers to.
        if deref {
            let SemanticType::Reference { target_type } = self.type_of(kind)? else {
                return None;
            };
            return elementary_of(self.context.types(), &target_type);
        }
        if let SymbolicVariableKind::Structured(structured) = kind {
            if let SymbolicVariableKind::Named(named) = structured.record.as_ref() {
                if let Some(at) = self.function_block_field_at(&named.name, &structured.field) {
                    return Some(at);
                }
            }
        }
        self.stored_as(target).map(|(id, _)| id)
    }

    /// The declared type of the field `field` of the function block
    /// instance `instance`, as the elementary type it is operated as. `None`
    /// when `instance` is not a function block instance, or the field is not
    /// a single numeric value.
    pub(super) fn function_block_field_at(&self, instance: &Id, field: &Id) -> Option<TypeId> {
        let representation =
            variable_type::declared(instance, self.context, &self.scope.current())?;
        let SemanticType::FunctionBlock { name, fields } = representation else {
            return None;
        };
        // The type of a user-defined block lists only the fields declared
        // with a simple type, so its fields come from its declaration. A
        // standard block's type lists every field.
        match self.declarations.field(&Id::from(name.as_str()), field) {
            Some(at) => at,
            None => fields
                .iter()
                .find(|candidate| candidate.name == *field)
                .and_then(|candidate| elementary_of(self.context.types(), &candidate.field_type)),
        }
    }

    /// Types the literals of the bounds and step of `node` by its control
    /// variable.
    pub(super) fn type_for_literals(&self, node: &mut For) {
        let control = self.control_at(node);
        self.type_literals(&mut node.from, control);
        self.type_literals(&mut node.to, control);
        if let Some(step) = &mut node.step {
            self.type_literals(step, control);
        }
    }

    /// The type the control variable of `node` is stored and operated at:
    /// its elementary type, or `DINT` when it has none.
    pub(super) fn control_at(&self, node: &For) -> Option<TypeId> {
        variable_type::declared(&node.control, self.context, &self.scope.current())
            .and_then(|representation| elementary_of(self.context.types(), representation))
            .or_else(|| self.dint())
    }

    /// Types the literals of the inputs of `node` by the fields they are
    /// stored in.
    pub(super) fn type_fb_call_literals(&self, node: &mut FbCall) {
        for (input, at) in self.fb_call_inputs(node) {
            match at {
                Some(at) => self.type_literals(input, Some(at)),
                None => self.type_own(input),
            }
        }
    }

    /// The inputs of the function block call `node`, each with the type of
    /// the field it is stored in: a named input the field it names, and the
    /// n-th positional input the n-th input the block declares, which is how
    /// IEC 61131-3 binds a non-formal call.
    ///
    /// The code generator drops a positional input today (#1855), so its
    /// conversion and the type of its literals have no effect yet.
    pub(super) fn fb_call_inputs<'c>(
        &self,
        node: &'c mut FbCall,
    ) -> Vec<(&'c mut Expr, Option<TypeId>)> {
        let instance = &node.var_name;
        let mut position = 0;
        let mut inputs = Vec::with_capacity(node.params.len());
        for param in &mut node.params {
            match param {
                ParamAssignmentKind::NamedInput(input) => {
                    let at = self.function_block_field_at(instance, &input.name);
                    inputs.push((&mut input.expr, at));
                }
                ParamAssignmentKind::PositionalInput(input) => {
                    let at = self
                        .function_block_input(instance, position)
                        .and_then(|name| self.function_block_field_at(instance, &name));
                    position += 1;
                    inputs.push((&mut input.expr, at));
                }
                // An output is stored into a variable by the call, not
                // computed into the block (#2125).
                ParamAssignmentKind::Output(_) => {}
            }
        }
        inputs
    }

    /// The name of the input at `position` among the inputs of the function
    /// block instance `instance`, in the order the block declares them.
    fn function_block_input(&self, instance: &Id, position: usize) -> Option<Id> {
        let representation =
            variable_type::declared(instance, self.context, &self.scope.current())?;
        let SemanticType::FunctionBlock { name, fields } = representation else {
            return None;
        };
        // The type of a user-defined block lists only the fields declared
        // with a simple type, so its inputs come from its declaration. A
        // standard block's type lists every field.
        match self.declarations.inputs(&Id::from(name.as_str())) {
            Some(inputs) => inputs.get(position).cloned(),
            None => fields
                .iter()
                .filter(|field| field.var_type == Some(FunctionBlockVarType::Input))
                .nth(position)
                .map(|field| field.name.clone()),
        }
    }

    pub(super) fn type_if_literals(&self, node: &mut If) {
        self.type_condition_literals(&mut node.expr);
        for else_if in &mut node.else_ifs {
            self.type_condition_literals(&mut else_if.expr);
        }
    }

    pub(super) fn type_while_literals(&self, node: &mut While) {
        self.type_condition_literals(&mut node.condition);
    }

    pub(super) fn type_repeat_literals(&self, node: &mut Repeat) {
        self.type_condition_literals(&mut node.until);
    }

    pub(super) fn type_case_literals(&self, node: &mut Case) {
        self.type_own(&mut node.selector);
    }

    pub(super) fn type_method_call_statement_literals(&self, node: &mut MethodCall) {
        self.type_method_call_literals(node);
    }

    /// Types the literals of a condition by the type the code generator
    /// tests it at: a comparison at its left operand's, a logical operator
    /// at its left operand's condition type, and anything else at its own.
    fn type_condition_literals(&self, condition: &mut Expr) {
        let at = self.condition_type(condition);
        self.type_literals(condition, at);
    }

    fn condition_type(&self, condition: &Expr) -> Option<TypeId> {
        match &condition.kind {
            ExprKind::Compare(compare) => match compare.op {
                CompareOp::And
                | CompareOp::Or
                | CompareOp::Xor
                | CompareOp::AndThen
                | CompareOp::OrElse => self.condition_type(&compare.left),
                CompareOp::Eq
                | CompareOp::Ne
                | CompareOp::Lt
                | CompareOp::Gt
                | CompareOp::LtEq
                | CompareOp::GtEq => {
                    if self.conversions.is_string(&compare.left) {
                        self.dint()
                    } else {
                        self.own_or_default(&compare.left)
                    }
                }
            },
            ExprKind::UnaryOp(unary) => match unary.op {
                UnaryOp::Not => self.condition_type(&unary.term),
                UnaryOp::Neg => self.own_or_default(condition),
            },
            ExprKind::Expression(inner) => self.condition_type(inner),
            ExprKind::Const(_)
            | ExprKind::BinaryOp(_)
            | ExprKind::EnumeratedValue(_)
            | ExprKind::Variable(_)
            | ExprKind::Function(_)
            | ExprKind::MethodCall(_)
            | ExprKind::LateBound(_)
            | ExprKind::Ref(_)
            | ExprKind::Deref(_)
            | ExprKind::ImplicitConversion(_)
            | ExprKind::Null(_) => self.own_or_default(condition),
        }
    }

    /// Returns `true` when a numeric value of type `own` is converted to be
    /// operated on as the numeric type `context`.
    fn converts_numeric(&self, own: TypeId, context: TypeId) -> bool {
        self.width_of_type(own).is_some()
            && self.width_of_type(context).is_some()
            && self.conversions.needs_conversion(own, context)
    }

    /// The type `DINT`.
    pub(super) fn dint(&self) -> Option<TypeId> {
        let dint: TypeName = ElementaryTypeName::DINT.into();
        self.context.types().id_of(&dint)
    }

    fn lint(&self) -> Option<TypeId> {
        let lint: TypeName = ElementaryTypeName::LINT.into();
        self.context.types().id_of(&lint)
    }
}

/// Returns `true` when `expr` is an integer or real literal.
fn is_numeric_literal(expr: &Expr) -> bool {
    matches!(
        &expr.kind,
        ExprKind::Const(ConstantKind::IntegerLiteral(_) | ConstantKind::RealLiteral(_))
    )
}

/// Returns `true` for the category of an untyped numeric literal.
fn is_numeric_category(generic: &GenericTypeName) -> bool {
    literal_default_type(generic).is_some()
}
