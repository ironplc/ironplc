//! Transformation pass that resolves expression types.
//!
//! This pass populates the `expr_type` field on `Expr` nodes: the type of
//! each expression's value by identity (ADR-0055). Later rules and codegen
//! read an expression's type from it and nowhere else. Where a relation
//! compares types by name, it derives the name from the id through
//! `value_type::operand_type_name`.
use ironplc_dsl::common::*;
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::textual::*;
use ironplc_dsl::type_id::TypeId;

use crate::builtin_function::{BuiltinFunction, InputsOfOneType, OneTypeResult};
use crate::enumerated_value_type::{self, EnumeratedValueType};
use crate::function_environment::FunctionEnvironment;
use crate::intermediates::arithmetic_overload::{
    resolve_arithmetic_fold, resolve_arithmetic_overload, Overload,
};
use crate::intermediates::common_operand::{common_operand_of, CommonOperand};
use crate::intermediates::numeric_operation::literal_default_type;
use crate::intermediates::operator_function_form::{operator_function_form, FormOf};
use crate::semantic_type::ParameterType;
use crate::symbol_environment::{ScopeTracker, SymbolEnvironment};
use crate::type_environment::{MethodSignature, TypeEnvironment};
use crate::value_type::operand_type_name;
use crate::variable_type;
use ironplc_parser::options::CompilerOptions;

/// Returns the library with every expression's type, and the unqualified
/// enumerated values whose type is ambiguous (`P2043`).
pub fn apply(
    lib: Library,
    symbols: &SymbolEnvironment,
    type_environment: &mut TypeEnvironment,
    function_environment: &FunctionEnvironment,
    options: &CompilerOptions,
) -> Result<(Library, Vec<Diagnostic>), Vec<Diagnostic>> {
    let named_enumerations = enumerated_value_type::named_enumerations(type_environment);
    let mut resolver = ExprTypeResolver {
        symbols,
        scope: ScopeTracker::default(),
        type_environment,
        function_environment,
        options: *options,
        named_enumerations,
        ambiguous: Vec::new(),
        calls: Vec::new(),
    };

    let library = resolver.fold_library(lib).map_err(|e| vec![e])?;
    let diagnostics = resolver
        .ambiguous
        .iter()
        .map(enumerated_value_type::ambiguous)
        .collect();
    Ok((library, diagnostics))
}

/// Returns true if the type name is an IEC 61131-3 generic type category.
///
/// These are abstract types used in stdlib function signatures that must be
/// resolved to concrete types based on the actual arguments.
fn is_generic_type(tn: &TypeName) -> bool {
    const GENERIC_TYPES: &[&str] = &[
        "ANY",
        "ANY_NUM",
        "ANY_REAL",
        "ANY_INT",
        "ANY_BIT",
        "ANY_STRING",
    ];
    GENERIC_TYPES.iter().any(|name| TypeName::from(name) == *tn)
}

/// The inputs of `f` when every argument is an input, in the order the call
/// lists them: the order of the parameters, for a call the binding pass wrote
/// in normal form.
fn all_inputs(f: &Function) -> Option<Vec<&Expr>> {
    f.param_assignment
        .iter()
        .map(ParamAssignmentKind::input_expr)
        .collect()
}

/// The type of an operation on two operands that keeps their type: the
/// concrete operand's when the other is an untyped literal (an untyped
/// literal and a `DWORD` give a `DWORD`), else the left operand's, else the
/// right's.
fn prefer_concrete(left: &Option<ExprType>, right: &Option<ExprType>) -> Option<ExprType> {
    match (left, right) {
        (Some(ExprType::Literal(_)), Some(concrete @ ExprType::Concrete(_))) => {
            Some(concrete.clone())
        }
        (Some(left), _) => Some(left.clone()),
        (None, right) => right.clone(),
    }
}

struct ExprTypeResolver<'a> {
    /// The variables in scope, by the scope the traversal is in.
    symbols: &'a SymbolEnvironment,
    scope: ScopeTracker,
    type_environment: &'a mut TypeEnvironment,
    function_environment: &'a FunctionEnvironment,
    /// The compiler options, which decide whether a bit-string operand of
    /// an arithmetic operator is judged as an unsigned integer (ADR-0053).
    options: CompilerOptions,
    /// See [`enumerated_value_type::named_enumerations`].
    named_enumerations: Vec<TypeId>,
    /// The unqualified enumerated values left ambiguous so far. A value
    /// leaves the list when the place it is used in gives it a type.
    ambiguous: Vec<EnumeratedValue>,
    /// The calls the fold is inside, innermost last: an
    /// [`ExprKind::CallOutput`] reads an output of the innermost.
    calls: Vec<CallFrame>,
}

/// A call whose arguments the fold is inside, for the type of the outputs a
/// [`ExprKind::CallOutput`] reads from it.
enum CallFrame {
    /// A call of the function block instance of this name: its output `q`
    /// has the type `instance.q` has.
    Instance(Id),
    /// A call of the function of this name: an output has the type its
    /// signature declares.
    Function(Id),
    /// A call of a method on a receiver: an output has the type the method
    /// declares it with. A receiver other than an instance has none.
    Method {
        receiver: MethodReceiver,
        method: Id,
    },
}

impl ExprTypeResolver<'_> {
    /// Determines the type of the value of an expression of `kind`, once its
    /// operands' types are known.
    fn resolve_type(&mut self, kind: &ExprKind) -> Option<ExprType> {
        match kind {
            ExprKind::Const(constant) => self.expr_type_named(self.resolve_const_type(constant)?),
            // A whole variable takes the type its declaration declares,
            // which is the only answer for an anonymous type and the
            // precise one for an alias (`x : MyByte` is a `MyByte`).
            ExprKind::Variable(var) => self.variable_type_id(var).map(ExprType::Concrete),
            ExprKind::BinaryOp(op) => {
                let left = self.operand_name(&op.left);
                let right = self.operand_name(&op.right);
                // The type of the overload that applies (see
                // `intermediates::arithmetic_overload`). Where none is
                // judged or none applies, the left operand's type, so later
                // passes still see a type and the operator rule reports it.
                match resolve_arithmetic_overload(
                    &op.op,
                    left.as_ref(),
                    right.as_ref(),
                    &self.options,
                ) {
                    Some(Overload::Numeric { result } | Overload::Typed { result, .. }) => {
                        return self.expr_type_named(result);
                    }
                    Some(Overload::Unchecked { .. }) | None => {}
                }
                prefer_concrete(&op.left.expr_type, &op.right.expr_type)
            }
            ExprKind::UnaryOp(op) => op.term.expr_type.clone(),
            ExprKind::Compare(compare) => match compare.op.kind() {
                // AND, OR and XOR, and their short-circuit forms AND_THEN
                // and OR_ELSE, have the type both operands widen to, as the
                // function forms do: `w OR lw` on a `WORD` and an `LWORD` is
                // an `LWORD`, and `d AND 16#FF` on a `DWORD` is a `DWORD`.
                CompareOpKind::Bitwise => self.inputs_of_one_type_result(
                    &[&compare.left, &compare.right],
                    InputsOfOneType {
                        first: 0,
                        result: OneTypeResult::Common,
                    },
                ),
                CompareOpKind::Equality | CompareOpKind::Ordering => {
                    self.expr_type_named(TypeName::from("BOOL"))
                }
            },
            ExprKind::Function(f) => {
                if let Some(result) = self.resolve_overloaded_call(f) {
                    return self.expr_type_named(result);
                }
                let sig = self.function_environment.get(&f.name)?;
                let return_type = sig.return_type.as_ref()?.to_type_name();
                if !is_generic_type(&return_type) {
                    return self.expr_type_named(return_type);
                }
                match &sig.builtin_function {
                    // An integer of whichever type its context stores it
                    // at, which the literal pass gives it (ADR-0028).
                    Some(builtin) if builtin.result_is_integer_of_context() => {
                        return Some(ExprType::Literal(GenericTypeName::AnyInt));
                    }
                    Some(BuiltinFunction::IntToBcd) => return self.bcd_result(f),
                    _ => {}
                }
                if let Some(shape) = sig
                    .builtin_function
                    .as_ref()
                    .and_then(BuiltinFunction::inputs_of_one_type)
                {
                    if let Some(inputs) = all_inputs(f) {
                        return self.inputs_of_one_type_result(&inputs, shape);
                    }
                }
                // Generic return type: infer concrete type from the first argument
                // whose parameter declaration type matches the generic return type.
                // This correctly skips selector parameters whose type differs from
                // the return type (e.g., BOOL for SEL, ANY_INT for MUX).
                let generic =
                    ParameterType::Generic(GenericTypeName::try_from(&return_type.id).ok()?);
                sig.inputs_of(&f.param_assignment)
                    .find_map(|(param, expr)| {
                        if param.param_type == generic {
                            expr.expr_type.clone()
                        } else {
                            None
                        }
                    })
            }
            // The method's return type. A call to a method without a return
            // type has no type here; the method call rule reports it.
            ExprKind::MethodCall(call) => {
                let block = match &call.receiver {
                    MethodReceiver::Instance(instance) => self.declared_type_id(instance)?,
                    MethodReceiver::SelfRef(self_ref) => {
                        let block = self
                            .symbols
                            .self_type(&self.scope.current(), self_ref.kind)?;
                        self.type_environment.id_of(&block)?
                    }
                };
                // The result's type named again, as a function call's
                // is, so a `STRING[10]` result types the call as `STRING`.
                let result = self.method_of(block, &call.method)?.result_type()?;
                let return_type = self.type_environment.name_of(result)?.clone();
                self.expr_type_named(return_type)
            }
            ExprKind::EnumeratedValue(ev) => match &ev.type_name {
                Some(type_name) => self.expr_type_named(type_name.clone()),
                None => self.unqualified_enumerated_value_type(ev, None),
            },
            ExprKind::Expression(inner) => inner.expr_type.clone(),
            ExprKind::LateBound(_) => None,
            // `REF(x)` is a reference to `x`'s type.
            ExprKind::Ref(var) => {
                let target = self.variable_type_id(var)?;
                self.type_environment
                    .reference_to(target)
                    .map(ExprType::Concrete)
            }
            // Dereferencing a reference gives the type it references.
            ExprKind::Deref(inner) => match &inner.expr_type {
                Some(ExprType::Concrete(reference) | ExprType::Inferred(reference)) => self
                    .type_environment
                    .referenced_type(*reference)
                    .map(ExprType::Concrete),
                Some(ExprType::Literal(_) | ExprType::Null) | None => None,
            },
            ExprKind::Null(_) => Some(ExprType::Null),
            // The type a conversion converts to is not a property of what it
            // converts, so it is recorded on the node, which `fold_expr`
            // keeps.
            ExprKind::ImplicitConversion(_) => None,
            ExprKind::CallOutput(output) => self.call_output_type(output).map(ExprType::Concrete),
        }
    }

    /// The type of the output `output` of the innermost call the fold is
    /// inside: for a function block, the type the read of that output of the
    /// instance has; for a function or method, the type it declares the
    /// output with.
    fn call_output_type(&mut self, output: &Id) -> Option<TypeId> {
        match self.calls.last()? {
            CallFrame::Instance(instance) => {
                self.symbolic_type_id(&SymbolicVariableKind::Structured(StructuredVariable {
                    record: Box::new(SymbolicVariableKind::Named(NamedVariable {
                        name: instance.clone(),
                    })),
                    field: output.clone(),
                }))
            }
            CallFrame::Function(function) => {
                let parameter = self
                    .function_environment
                    .get(function)?
                    .parameters
                    .iter()
                    .find(|parameter| parameter.is_output && parameter.name == *output)?;
                match parameter.param_type {
                    ParameterType::Type(declared) => Some(declared),
                    ParameterType::Generic(_) | ParameterType::Unresolved(_) => None,
                }
            }
            CallFrame::Method {
                receiver: MethodReceiver::Instance(instance),
                method,
            } => {
                let block = self.declared_type_id(instance)?;
                self.method_of(block, method)?.output_type(output)
            }
            CallFrame::Method { .. } => None,
        }
    }

    /// The signature of the method `method` called on the function block
    /// `block`: the one it declares or inherits.
    fn method_of(&self, block: TypeId, method: &Id) -> Option<&MethodSignature> {
        self.type_environment
            .method(block, method)
            .map(|(_, signature)| signature)
    }

    /// The type of the unqualified enumerated value `ev` used where a value
    /// of type `context` is expected (see [`enumerated_value_type`]). An
    /// ambiguous value is recorded until a context resolves it.
    fn unqualified_enumerated_value_type(
        &mut self,
        ev: &EnumeratedValue,
        context: Option<TypeId>,
    ) -> Option<ExprType> {
        // The anonymous enumerations of the variables in scope, each
        // declared in place (`e : (A, B)`).
        let mut in_scope: Vec<TypeId> = self
            .symbols
            .visible_variables(&self.scope.current())
            .into_iter()
            .filter_map(|(_, info)| info.type_id)
            .filter(|id| {
                self.type_environment.name_of(*id).is_none()
                    && self
                        .type_environment
                        .get_by_id(*id)
                        .is_some_and(|attributes| attributes.representation.is_enumeration())
            })
            .collect();
        in_scope.sort();
        let candidates = self.named_enumerations.iter().copied().chain(in_scope);
        let span = ev.span();
        self.ambiguous.retain(|pending| pending.span() != span);
        match enumerated_value_type::resolve(self.type_environment, &ev.value, context, candidates)
        {
            EnumeratedValueType::Resolved(id) => Some(ExprType::Concrete(id)),
            EnumeratedValueType::Ambiguous => {
                self.ambiguous.push(ev.clone());
                None
            }
            EnumeratedValueType::Undeclared => None,
        }
    }

    /// Gives `expr` the type `context` expects when `expr` is an
    /// unqualified enumerated value that `context`, an enumeration, declares.
    fn type_from_context(&mut self, expr: &mut Expr, context: Option<TypeId>) {
        if context.is_none() {
            return;
        }
        match &mut expr.kind {
            ExprKind::Expression(inner) => {
                self.type_from_context(inner, context);
                expr.expr_type = inner.expr_type.clone();
            }
            ExprKind::EnumeratedValue(ev) if ev.type_name.is_none() => {
                let ev = ev.clone();
                expr.expr_type = self.unqualified_enumerated_value_type(&ev, context);
            }
            _ => {}
        }
    }

    /// The type a value compared with `expr` is expected to have.
    fn expr_context(&self, expr: &Expr) -> Option<TypeId> {
        match (&expr.expr_type, &expr.kind) {
            (Some(ExprType::Concrete(id)), _) => Some(*id),
            (None, ExprKind::Variable(variable)) => self.variable_type_id(variable),
            _ => None,
        }
    }

    /// The type a value the input `input` of the function block instance
    /// `instance` receives is expected to have: the declared type of the
    /// field, own or inherited.
    fn function_block_input_type(&self, instance: &Id, input: &Id) -> Option<TypeId> {
        self.type_environment
            .field_type(self.declared_type_id(instance)?, input)
    }

    /// The type `type_name` names: a literal of the category a generic name
    /// names, else the named type.
    fn expr_type_named(&self, type_name: TypeName) -> Option<ExprType> {
        if let Ok(generic) = GenericTypeName::try_from(&type_name.id) {
            return Some(ExprType::Literal(generic));
        }
        self.type_environment
            .id_of(&type_name)
            .map(ExprType::Concrete)
    }

    /// The name the name-based relations know `expr`'s value by.
    fn operand_name(&self, expr: &Expr) -> Option<TypeName> {
        operand_type_name(self.type_environment, expr.expr_type.as_ref()?)
    }

    /// Returns the result type of a call to `ADD`, `SUB`, `MUL` or `DIV`, the
    /// arithmetic functions with typed overloads, from the overload that
    /// applies to its inputs folded from the left: `SUB(d1, d2)` on `DATE`
    /// is `TIME`, and `ADD(i, d)` on `INT` and `DINT` is `DINT`.
    ///
    /// Returns `None` for any other function, for a call with an output
    /// assignment, or when no overload is judged or applies; the caller then
    /// types the call from its signature.
    fn resolve_overloaded_call(&self, f: &Function) -> Option<TypeName> {
        let form = operator_function_form(&f.name.to_string())?;
        let FormOf::Arithmetic(op) = &form.operator else {
            return None;
        };
        if form.typed_overloads().is_empty() {
            return None;
        }
        let names: Vec<Option<TypeName>> = all_inputs(f)?
            .into_iter()
            .map(|input| self.operand_name(input))
            .collect();
        let inputs: Vec<Option<&TypeName>> = names.iter().map(Option::as_ref).collect();
        match resolve_arithmetic_fold(op, &inputs, &self.options) {
            Ok(Overload::Numeric { result } | Overload::Typed { result, .. }) => Some(result),
            Ok(Overload::Unchecked { .. }) | Err(_) => None,
        }
    }

    /// The type of a call to `INT_TO_BCD`: the bit string as wide as its
    /// input, which it encodes digit by digit. `INT_TO_BCD(i)` on an `INT` is
    /// a `WORD`, and an untyped literal input is a `DINT` (ADR-0028), so
    /// `INT_TO_BCD(42)` is a `DWORD`.
    fn bcd_result(&self, f: &Function) -> Option<ExprType> {
        let input = *all_inputs(f)?.first()?;
        let name = match &input.expr_type {
            Some(ExprType::Literal(generic)) => literal_default_type(generic)?.into(),
            _ => self.operand_name(input)?,
        };
        let bit_string = match ElementaryTypeName::try_from(&name.id).ok()? {
            ElementaryTypeName::SINT | ElementaryTypeName::USINT => ElementaryTypeName::BYTE,
            ElementaryTypeName::INT | ElementaryTypeName::UINT => ElementaryTypeName::WORD,
            ElementaryTypeName::DINT | ElementaryTypeName::UDINT => ElementaryTypeName::DWORD,
            ElementaryTypeName::LINT | ElementaryTypeName::ULINT => ElementaryTypeName::LWORD,
            _ => return None,
        };
        self.expr_type_named(bit_string.into())
    }

    /// The type of a call to a function of several inputs of one type, whose
    /// inputs are `inputs`: the type every input of that type widens to, or
    /// for `EXPT` its first input's (see [`BuiltinFunction::inputs_of_one_type`]).
    /// `MAX(i, l)` on an `INT` and an `LINT` is an `LINT`, and `MIN(i, 1.5)`
    /// on an `INT` is a `REAL`, the type the literal defaults to, as `i * 1.5`
    /// is (see `intermediates::common_operand`).
    ///
    /// When no input's type accepts every other one -- a `DINT` and a `UDINT`
    /// -- the call has the type of its first concrete input, else of its
    /// first, so later passes see a type, as a comparison of such a pair
    /// compares at its concrete left operand's;
    /// `rule_function_inputs_common_type` reports the call, as the
    /// comparison rule reports the comparison.
    fn inputs_of_one_type_result(
        &self,
        inputs: &[&Expr],
        shape: InputsOfOneType,
    ) -> Option<ExprType> {
        let inputs = inputs.get(shape.first..)?;
        let index = match shape.result {
            OneTypeResult::First => 0,
            OneTypeResult::Common => {
                let names: Vec<Option<TypeName>> = inputs
                    .iter()
                    .map(|input| self.operand_name(input))
                    .collect();
                let names: Vec<Option<&TypeName>> = names.iter().map(Option::as_ref).collect();
                match common_operand_of(&names, &self.options) {
                    Some(CommonOperand::Input(index)) => index,
                    Some(CommonOperand::Type(name)) => return self.expr_type_named(name),
                    None => inputs
                        .iter()
                        .position(|input| {
                            matches!(
                                input.expr_type,
                                Some(ExprType::Concrete(_) | ExprType::Inferred(_))
                            )
                        })
                        .unwrap_or(0),
                }
            }
        };
        inputs.get(index)?.expr_type.clone()
    }

    /// The id of the type the variable `name` names from the current scope
    /// was declared with.
    fn declared_type_id(&self, name: &Id) -> Option<TypeId> {
        variable_type::declared_type_id(name, self.symbols, &self.scope.current())
    }

    /// The id of the type of the value `var` names.
    fn variable_type_id(&self, var: &Variable) -> Option<TypeId> {
        match var {
            Variable::Symbolic(kind) => self.symbolic_type_id(kind),
            Variable::Direct(_) => None,
        }
    }

    fn resolve_const_type(&self, constant: &ConstantKind) -> Option<TypeName> {
        match constant {
            ConstantKind::IntegerLiteral(lit) => Some(
                lit.data_type
                    .as_ref()
                    .map(|itn| {
                        let elem: ElementaryTypeName = itn.clone().into();
                        let tn: TypeName = elem.into();
                        tn
                    })
                    .unwrap_or_else(|| TypeName::from("ANY_INT")),
            ),
            ConstantKind::RealLiteral(lit) => Some(
                lit.data_type
                    .as_ref()
                    .map(|rtn| {
                        let elem: ElementaryTypeName = rtn.clone().into();
                        let tn: TypeName = elem.into();
                        tn
                    })
                    .unwrap_or_else(|| TypeName::from("ANY_REAL")),
            ),
            ConstantKind::BitStringLiteral(lit) => lit.data_type.as_ref().map(|bstn| {
                let elem: ElementaryTypeName = bstn.clone().into();
                elem.into()
            }),
            ConstantKind::Boolean(_) => Some(TypeName::from("BOOL")),
            // The delimiter is the type: `'abc'` is a STRING and `"abc"` a
            // WSTRING (IEC 61131-3 Table 5). Typing every literal STRING made
            // `w := "abc"` a P4035 and `f("abc")` a P4026 -- the analyzer
            // never learned what the quotes already said.
            ConstantKind::CharacterString(lit) => Some(TypeName::from(lit.width.keyword())),
            // The prefix is the type, as the delimiter is for a string:
            // `LDATE#2024-01-01` is an LDATE and `DATE#2024-01-01` a DATE.
            // Typing every temporal literal as the 32-bit member held a
            // 64-bit literal to a 32-bit range (issue #1560).
            ConstantKind::Duration(lit) => Some(TypeName::from_id(&lit.type_name().into())),
            ConstantKind::TimeOfDay(lit) => Some(TypeName::from_id(&lit.type_name().into())),
            ConstantKind::Date(lit) => Some(TypeName::from_id(&lit.type_name().into())),
            ConstantKind::DateAndTime(lit) => Some(TypeName::from_id(&lit.type_name().into())),
        }
    }

    /// The id of the type of the value the symbolic variable `kind` names
    /// (see [`variable_type::type_id`]).
    fn symbolic_type_id(&self, kind: &SymbolicVariableKind) -> Option<TypeId> {
        variable_type::type_id(
            kind,
            self.symbols,
            self.type_environment,
            &self.scope.current(),
        )
    }
}

impl Fold<Diagnostic> for ExprTypeResolver<'_> {
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Diagnostic> {
        self.scope.enter(&node);
        Ok(())
    }

    fn exit_scope(&mut self) {
        self.scope.exit();
    }

    fn fold_self_ref_variable(
        &mut self,
        node: SelfRefVariable,
    ) -> Result<SelfRefVariable, Diagnostic> {
        // `THIS^`/`SUPER^` with nothing to refer to (outside a function
        // block, or `SUPER^` without a base) is reported by
        // `rule_self_reference_context`; its members then have no type.
        Ok(node)
    }

    fn fold_expr(&mut self, node: Expr) -> Result<Expr, Diagnostic> {
        // First, recurse to fold children (bottom-up)
        let mut expr = node.recurse_fold(self)?;

        // An unqualified enumerated value compared with a value of an
        // enumeration type has that type.
        if let ExprKind::Compare(compare) = &mut expr.kind {
            if compare.op.is_comparison() {
                let left = self.expr_context(&compare.left);
                let right = self.expr_context(&compare.right);
                self.type_from_context(&mut compare.right, left);
                self.type_from_context(&mut compare.left, right);
            }
        }

        // Then determine type based on the (now-folded) kind
        if !matches!(expr.kind, ExprKind::ImplicitConversion(_)) {
            expr.expr_type = self.resolve_type(&expr.kind);
        }
        Ok(expr)
    }

    fn fold_assignment(&mut self, node: Assignment) -> Result<Assignment, Diagnostic> {
        let mut node = node.recurse_fold(self)?;
        // An unqualified enumerated value assigned has the target's type.
        let target = self.variable_type_id(&node.target);
        self.type_from_context(&mut node.value, target);
        Ok(node)
    }

    fn fold_function(&mut self, node: Function) -> Result<Function, Diagnostic> {
        self.calls.push(CallFrame::Function(node.name.clone()));
        let folded = node.recurse_fold(self);
        self.calls.pop();
        folded
    }

    fn fold_method_call(&mut self, node: MethodCall) -> Result<MethodCall, Diagnostic> {
        self.calls.push(CallFrame::Method {
            receiver: node.receiver.clone(),
            method: node.method.clone(),
        });
        let folded = node.recurse_fold(self);
        self.calls.pop();
        folded
    }

    fn fold_fb_call(&mut self, node: FbCall) -> Result<FbCall, Diagnostic> {
        self.calls.push(CallFrame::Instance(node.var_name.clone()));
        let folded = node.recurse_fold(self);
        self.calls.pop();
        let mut node = folded?;
        // An unqualified enumerated value passed to an input has its type.
        for param in &mut node.params {
            if let ParamAssignmentKind::NamedInput(input) = param {
                let input_type = self.function_block_input_type(&node.var_name, &input.name);
                self.type_from_context(&mut input.expr, input_type);
            }
        }
        Ok(node)
    }

    fn fold_initial_value_assignment_kind(
        &mut self,
        node: InitialValueAssignmentKind,
    ) -> Result<InitialValueAssignmentKind, Diagnostic> {
        if let InitialValueAssignmentKind::Simple(simple) = &node {
            if let Some(resolved) = self
                .type_environment
                .resolve_elementary_type_name(&simple.type_name)
            {
                return Ok(InitialValueAssignmentKind::Simple(SimpleInitializer {
                    type_name: resolved,
                    initial_value: simple.initial_value.clone(),
                }));
            }
        }
        node.recurse_fold(self)
    }
}

#[cfg(test)]
mod tests;
