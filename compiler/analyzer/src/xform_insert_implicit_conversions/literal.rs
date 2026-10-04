//! Recording the type of an untyped literal from the context it is used in.
//!
//! An untyped literal such as `1` has a generic type (`ANY_INT`) until a
//! context gives it one (ADR-0028). The code generator gives it one top-down:
//! each statement passes the type it stores or tests at into its expression,
//! and every construct on the way to the literal either passes that type on
//! (a negation, parentheses, `ABS`, `MUX`), computes at a type of its own (an
//! arithmetic operation of a concrete type, a comparison, a call to a
//! user-defined function), or computes at a fixed type (a subscript, a
//! string position, a shift count). This module records the type each
//! literal reaches, so that a backend reads it from the literal rather than
//! carrying it down.
//!
//! It records what the code generator does, including where that is a
//! default rather than the declared type: a field of a standard function
//! block and a dereferenced target take `DINT`, the default slot type. Only
//! a numeric literal is typed: a string literal is compiled in its own
//! encoding, and a bit-string or time literal already has a type.

use std::collections::HashMap;

use ironplc_dsl::common::{
    ElementaryTypeName, GenericTypeName, InitialValueAssignmentKind, Library, LibraryElementKind,
    TypeName,
};
use ironplc_dsl::core::Id;
use ironplc_dsl::textual::{
    Case, CompareExpr, CompareOp, Expr, ExprKind, ExprType, FbCall, For, Function, If, MethodCall,
    MethodReceiver, ParamAssignmentKind, Repeat, SymbolicVariableKind, UnaryOp, Variable, While,
};
use ironplc_dsl::type_id::TypeId;

use super::ImplicitConversions;
use crate::intermediates::conversion_target::concrete;
use crate::intermediates::numeric_operation::{
    literal_default_type, numeric_operation_width, OperationWidth,
};
use crate::intermediates::operator_function_form::FormOf;
use crate::intermediates::stdlib_function_block::is_stdlib_function_block;
use crate::intrinsic::{Intrinsic, NumericFunction};
use crate::semantic_type::SemanticType;
use crate::type_environment::TypeEnvironment;
use crate::variable_type;

/// The input parameters of each method, by function block type (upper case)
/// and method (lower case): each parameter's name (lower case) and the type
/// a call passes it as.
pub(super) type MethodParameters = HashMap<(String, String), Vec<(String, Option<TypeId>)>>;

/// Collects the parameters of every method in `lib`, the way the code
/// generator passes them: a parameter declared with a simple type at that
/// type, and any other at `DINT`, the default slot type.
pub(super) fn method_parameters(lib: &Library, types: &TypeEnvironment) -> MethodParameters {
    let mut parameters = MethodParameters::new();
    for element in &lib.elements {
        let LibraryElementKind::FunctionBlockDeclaration(block) = element else {
            continue;
        };
        let block_name = block.name.name.to_string().to_uppercase();
        for method in &block.methods {
            let inputs = method
                .variables
                .iter()
                .filter(|decl| decl.var_type.is_input_compatible())
                .filter_map(|decl| {
                    let name = decl.identifier.symbolic_id()?.to_string().to_lowercase();
                    let passed = match &decl.initializer {
                        InitialValueAssignmentKind::Simple(_) => decl
                            .type_id
                            .and_then(|id| types.get_by_id(id))
                            .and_then(|attributes| {
                                elementary_of(types, &attributes.representation)
                            }),
                        _ => None,
                    }
                    .or_else(|| dint(types));
                    Some((name, passed))
                })
                .collect();
            parameters.insert(
                (block_name.clone(), method.name.to_string().to_lowercase()),
                inputs,
            );
        }
    }
    parameters
}

impl ImplicitConversions<'_> {
    /// Gives the untyped numeric literals of `expr` the type they are
    /// compiled at, when `expr` is compiled at `context`.
    pub(super) fn type_literals(&self, expr: &mut Expr, context: Option<TypeId>) {
        if let (Some(ExprType::Literal(generic)), Some(context)) = (&expr.expr_type, context) {
            if is_numeric_category(generic) {
                expr.expr_type = Some(ExprType::Concrete(context));
            }
        }
        let own = concrete(expr);
        let numeric_pair =
            matches!(&expr.kind, ExprKind::Function(func) if self.is_numeric_pair(func, expr));
        let numeric_result = self.width_of(expr).is_some();
        match &mut expr.kind {
            ExprKind::Const(_)
            | ExprKind::LateBound(_)
            | ExprKind::EnumeratedValue(_)
            | ExprKind::Ref(_)
            | ExprKind::Null(_) => {}
            ExprKind::UnaryOp(unary) => self.type_literals(&mut unary.term, context.or(own)),
            ExprKind::Expression(inner) => self.type_literals(inner, context.or(own)),
            ExprKind::BinaryOp(binary) => {
                if self.has_typed_overload(&binary.op, &binary.left, &binary.right) {
                    self.type_own(&mut binary.left);
                    self.type_own(&mut binary.right);
                } else {
                    // An operation of a numeric type computes at it; any
                    // other at the type of its context.
                    let at = if numeric_result { own } else { context };
                    self.type_literals(&mut binary.left, at);
                    self.type_literals(&mut binary.right, at);
                }
            }
            ExprKind::Compare(compare) => self.type_compare_literals(compare, context),
            ExprKind::Function(func) => self.type_call_literals(func, own, context, numeric_pair),
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
            Some(ExprType::Concrete(id)) => Some(*id),
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
    /// `own`, compiled at `context`.
    fn type_call_literals(
        &self,
        func: &mut Function,
        own: Option<TypeId>,
        context: Option<TypeId>,
        numeric_pair: bool,
    ) {
        let intrinsic = self
            .context
            .functions()
            .get(&func.name)
            .map(|signature| signature.intrinsic.clone());
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
        match intrinsic {
            // A user-defined function, or one the analyzer has no signature
            // for: each argument at its own type, which the argument pass
            // gave it.
            None | Some(None) => inputs.into_iter().for_each(|arg| self.type_own(arg)),
            Some(Some(intrinsic)) => match intrinsic {
                Intrinsic::Operator(FormOf::Arithmetic(_)) => {
                    let at = if typed_pair {
                        None
                    } else if numeric_pair {
                        own
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
                    .for_each(|arg| self.type_literals(arg, context)),
                Intrinsic::Numeric(function) => {
                    for (index, arg) in inputs.into_iter().enumerate() {
                        // SEL's selector is a BOOL or an integer, whatever
                        // the type of its inputs.
                        let at = if function == NumericFunction::Sel && index == 0 {
                            dint
                        } else {
                            context
                        };
                        self.type_literals(arg, at);
                    }
                }
                Intrinsic::BitShift(_) => {
                    let wide =
                        context.and_then(|id| self.width_of_type(id)) == Some(OperationWidth::W64);
                    let count = if wide { self.lint() } else { dint };
                    for (index, arg) in inputs.into_iter().enumerate() {
                        self.type_literals(arg, if index == 0 { context } else { count });
                    }
                }
                Intrinsic::Mux => {
                    for (index, arg) in inputs.into_iter().enumerate() {
                        self.type_literals(arg, if index == 0 { dint } else { context });
                    }
                }
                // A position or a length is an integer at the default slot
                // type; a string is compiled in its own encoding.
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
    /// parameters: positional arguments fill the parameters in order, and a
    /// named one the parameter it names.
    fn type_method_call_literals(&self, call: &mut MethodCall) {
        let parameters = match &call.receiver {
            MethodReceiver::Instance(instance) => {
                variable_type::declared(instance, self.context, &self.scope.current()).and_then(
                    |representation| match representation {
                        SemanticType::FunctionBlock { name, .. } => self
                            .methods
                            .get(&(name.to_uppercase(), call.method.to_string().to_lowercase())),
                        _ => None,
                    },
                )
            }
            MethodReceiver::SelfRef(_) => None,
        };
        let mut position = 0;
        for param in &mut call.params {
            let (arg, at) = match param {
                ParamAssignmentKind::PositionalInput(input) => {
                    let at = parameters
                        .and_then(|parameters| parameters.get(position))
                        .and_then(|(_, at)| *at);
                    position += 1;
                    (&mut input.expr, at)
                }
                ParamAssignmentKind::NamedInput(input) => {
                    let name = input.name.to_string().to_lowercase();
                    let at = parameters
                        .and_then(|parameters| parameters.iter().find(|(p, _)| *p == name))
                        .and_then(|(_, at)| *at);
                    (&mut input.expr, at)
                }
                ParamAssignmentKind::Output(_) => continue,
            };
            match at {
                Some(at) => self.type_literals(arg, Some(at)),
                None => self.type_own(arg),
            }
        }
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
        // A dereference stores at the default slot type.
        if deref {
            return self.dint();
        }
        let Variable::Symbolic(kind) = target else {
            return None;
        };
        if let SymbolicVariableKind::Structured(structured) = kind {
            if let SymbolicVariableKind::Named(named) = structured.record.as_ref() {
                if let Some(at) = self.function_block_field_at(&named.name, &structured.field) {
                    return at;
                }
            }
        }
        self.stored_as(target).map(|(id, _)| id)
    }

    /// The type an input of the function block instance `instance` named
    /// `field` is stored at: a field of a user-defined block at its declared
    /// type, and one of a standard block at the default slot type. `None`
    /// when `instance` is not a function block instance.
    fn function_block_field_at(&self, instance: &Id, field: &Id) -> Option<Option<TypeId>> {
        let representation =
            variable_type::declared(instance, self.context, &self.scope.current())?;
        let SemanticType::FunctionBlock { name, fields } = representation else {
            return None;
        };
        if is_stdlib_function_block(&Id::from(name.as_str())) {
            return Some(self.dint());
        }
        let field_type = fields
            .iter()
            .find(|candidate| candidate.name == *field)
            .map(|candidate| &candidate.field_type);
        Some(field_type.and_then(|field_type| elementary_of(self.context.types(), field_type)))
    }

    /// Types the literals of the bounds and step of `node` by its control
    /// variable.
    pub(super) fn type_for_literals(&self, node: &mut For) {
        let control = variable_type::declared(&node.control, self.context, &self.scope.current())
            .and_then(|representation| elementary_of(self.context.types(), representation))
            .or_else(|| self.dint());
        self.type_literals(&mut node.from, control);
        self.type_literals(&mut node.to, control);
        if let Some(step) = &mut node.step {
            self.type_literals(step, control);
        }
    }

    /// Types the literals of the named inputs of `node` by the fields they
    /// are stored in.
    pub(super) fn type_fb_call_literals(&self, node: &mut FbCall) {
        for param in &mut node.params {
            if let ParamAssignmentKind::NamedInput(input) = param {
                match self.function_block_field_at(&node.var_name, &input.name) {
                    Some(Some(at)) => self.type_literals(&mut input.expr, Some(at)),
                    _ => self.type_own(&mut input.expr),
                }
            }
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
                _ if self.conversions.is_string(&compare.left) => self.dint(),
                _ => self.own_or_default(&compare.left),
            },
            ExprKind::UnaryOp(unary) if unary.op == UnaryOp::Not => {
                self.condition_type(&unary.term)
            }
            ExprKind::Expression(inner) => self.condition_type(inner),
            _ => self.own_or_default(condition),
        }
    }

    fn width_of_type(&self, id: TypeId) -> Option<OperationWidth> {
        numeric_operation_width(&self.conversions.name_of(id)?)
    }

    /// The default slot type, `DINT`.
    fn dint(&self) -> Option<TypeId> {
        dint(self.context.types())
    }

    fn lint(&self) -> Option<TypeId> {
        let lint: TypeName = ElementaryTypeName::LINT.into();
        self.context.types().id_of(&lint)
    }
}

/// Returns `true` for the category of an untyped numeric literal.
fn is_numeric_category(generic: &GenericTypeName) -> bool {
    literal_default_type(generic).is_some()
}

/// The elementary type a value of `representation` is operated as: its own,
/// or its base type for a subrange.
fn elementary_of(types: &TypeEnvironment, representation: &SemanticType) -> Option<TypeId> {
    let representation = match representation {
        SemanticType::Subrange { base_type, .. } => base_type,
        other => other,
    };
    types.id_of(&types.elementary_type_name_for(representation)?)
}

fn dint(types: &TypeEnvironment) -> Option<TypeId> {
    let dint: TypeName = ElementaryTypeName::DINT.into();
    types.id_of(&dint)
}
