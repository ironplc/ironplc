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
use std::collections::HashMap;

use crate::callee_resolution::FunctionBlocks;
use crate::enumerated_value_type::{self, Context, EnumeratedValueType, OwnedContext};
use crate::function_environment::FunctionEnvironment;
use crate::intermediates::arithmetic_overload::{
    resolve_arithmetic_fold, resolve_arithmetic_overload, Overload,
};
use crate::intermediates::common_operand::common_operand_of;
use crate::intermediates::inherited_fields::collect_inherited_fields;
use crate::intermediates::numeric_operation::literal_default_type;
use crate::intermediates::operator_function_form::{operator_function_form, FormOf};
use crate::intrinsic::{InputsOfOneType, Intrinsic, OneTypeResult};
use crate::semantic_type::SemanticType;
use crate::symbol_environment::{ScopeTracker, SymbolEnvironment};
use crate::type_environment::TypeEnvironment;
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
    let inherited_fields = collect_inherited_fields(&lib);
    let method_return_types = collect_method_return_types(&lib);
    let function_block_inputs =
        enumerated_value_type::function_block_inputs(&lib, &inherited_fields);
    let named_enumerations = enumerated_value_type::named_enumerations(type_environment);
    let mut resolver = ExprTypeResolver {
        symbols,
        scope: ScopeTracker::default(),
        method_return_types,
        type_environment,
        function_environment,
        options: *options,
        named_enumerations,
        function_block_inputs,
        ambiguous: Vec::new(),
    };

    let library = resolver.fold_library(lib).map_err(|e| vec![e])?;
    let diagnostics = resolver
        .ambiguous
        .iter()
        .map(enumerated_value_type::ambiguous)
        .collect();
    Ok((library, diagnostics))
}

/// The return type of every method callable on every function block, by
/// function block type and method name: the block's own methods, then those
/// it inherits through `EXTENDS` (ADR-0041 Phase 1 static dispatch). `None`
/// for a method without a return type.
///
/// Built before the fold, which consumes the library, so the resolver can
/// type a method call without holding a reference into it.
fn collect_method_return_types(lib: &Library) -> HashMap<TypeName, HashMap<Id, Option<TypeName>>> {
    let function_blocks = FunctionBlocks::from_library(lib);
    let method_names: Vec<&Id> = lib
        .elements
        .iter()
        .filter_map(|element| match element {
            LibraryElementKind::FunctionBlockDeclaration(fb) => Some(fb),
            _ => None,
        })
        .flat_map(|fb| fb.methods.iter().map(|m| &m.name))
        .collect();

    let mut result = HashMap::new();
    for element in &lib.elements {
        let LibraryElementKind::FunctionBlockDeclaration(fb) = element else {
            continue;
        };
        let callable: HashMap<Id, Option<TypeName>> = method_names
            .iter()
            .filter_map(|name| {
                let (_, method) = function_blocks.resolve_method(&fb.name, name)?;
                Some((
                    method.name.clone(),
                    method.return_type.as_ref().map(|rt| rt.to_type_name()),
                ))
            })
            .collect();
        result.insert(fb.name.clone(), callable);
    }
    result
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

/// The inputs of `f` when every argument is a positional input, as the
/// named-argument pass leaves a call it accepted.
fn positional_inputs(f: &Function) -> Option<Vec<&Expr>> {
    f.param_assignment
        .iter()
        .map(|p| match p {
            ParamAssignmentKind::PositionalInput(input) => Some(&input.expr),
            ParamAssignmentKind::NamedInput(_) | ParamAssignmentKind::Output(_) => None,
        })
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

/// Maps an [`SemanticType`] to its canonical elementary [`TypeName`].
///
/// Delegates to [`TypeEnvironment::elementary_type_name_for`] for the simple
/// cases. That helper does a strict equality lookup against the elementary
/// types table, which only contains `String { max_len: None }`. A struct
/// field declared `STRING[n]` resolves to `String { max_len: Some(n) }` and
/// would otherwise return `None`, so we handle strings explicitly.
fn semantic_type_to_elementary_type_name(
    env: &TypeEnvironment,
    it: &SemanticType,
) -> Option<TypeName> {
    if let Some(tn) = env.elementary_type_name_for(it.operated_as()) {
        return Some(tn);
    }
    match it {
        SemanticType::String { .. } => Some(TypeName::from("STRING")),
        _ => None,
    }
}

struct ExprTypeResolver<'a> {
    /// The variables in scope, by the scope the traversal is in.
    symbols: &'a SymbolEnvironment,
    scope: ScopeTracker,
    /// See [`collect_method_return_types`].
    method_return_types: HashMap<TypeName, HashMap<Id, Option<TypeName>>>,
    type_environment: &'a mut TypeEnvironment,
    function_environment: &'a FunctionEnvironment,
    /// The compiler options, which decide whether a bit-string operand of
    /// an arithmetic operator is judged as an unsigned integer (ADR-0053).
    options: CompilerOptions,
    /// See [`enumerated_value_type::named_enumerations`].
    named_enumerations: Vec<TypeId>,
    /// See [`enumerated_value_type::function_block_inputs`].
    function_block_inputs: HashMap<TypeName, HashMap<Id, TypeId>>,
    /// The unqualified enumerated values left ambiguous so far. A value
    /// leaves the list when the place it is used in gives it a type.
    ambiguous: Vec<EnumeratedValue>,
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
            ExprKind::Compare(compare) => match compare.op {
                // AND, OR and XOR have the type both operands widen to, as
                // their function forms do: `w OR lw` on a `WORD` and an
                // `LWORD` is an `LWORD`, and `d AND 16#FF` on a `DWORD` is a
                // `DWORD`.
                CompareOp::And | CompareOp::Or | CompareOp::Xor => self.inputs_of_one_type_result(
                    &[&compare.left, &compare.right],
                    InputsOfOneType {
                        first: 0,
                        result: OneTypeResult::Common,
                    },
                ),
                // Only `BOOL`s.
                CompareOp::AndThen | CompareOp::OrElse => {
                    prefer_concrete(&compare.left.expr_type, &compare.right.expr_type)
                }
                CompareOp::Eq
                | CompareOp::Ne
                | CompareOp::Lt
                | CompareOp::Gt
                | CompareOp::LtEq
                | CompareOp::GtEq => self.expr_type_named(TypeName::from("BOOL")),
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
                match &sig.intrinsic {
                    // An integer of whichever type its context stores it
                    // at, which the literal pass gives it (ADR-0028).
                    Some(intrinsic) if intrinsic.result_is_integer_of_context() => {
                        return Some(ExprType::Literal(GenericTypeName::AnyInt));
                    }
                    Some(Intrinsic::IntToBcd) => return self.bcd_result(f),
                    _ => {}
                }
                if let Some(shape) = sig
                    .intrinsic
                    .as_ref()
                    .and_then(Intrinsic::inputs_of_one_type)
                {
                    if let Some(inputs) = positional_inputs(f) {
                        return self.inputs_of_one_type_result(&inputs, shape);
                    }
                }
                // Generic return type: infer concrete type from the first argument
                // whose parameter declaration type matches the generic return type.
                // This correctly skips selector parameters whose type differs from
                // the return type (e.g., BOOL for SEL, ANY_INT for MUX).
                let mut positional_index = 0usize;
                f.param_assignment.iter().find_map(|p| match p {
                    ParamAssignmentKind::PositionalInput(pos) => {
                        let idx = positional_index;
                        positional_index += 1;
                        match sig.parameters.get(idx) {
                            Some(param) if param.param_type == return_type => {
                                pos.expr.expr_type.clone()
                            }
                            _ => None,
                        }
                    }
                    ParamAssignmentKind::NamedInput(named) => {
                        let param = sig.parameters.iter().find(|p| p.name == named.name);
                        match param {
                            Some(param) if param.param_type == return_type => {
                                named.expr.expr_type.clone()
                            }
                            _ => None,
                        }
                    }
                    ParamAssignmentKind::Output(_) => None,
                })
            }
            // The method's return type. A call to a method without a return
            // type has no type here; the method call rule reports it.
            ExprKind::MethodCall(call) => {
                let fb_type = match &call.receiver {
                    MethodReceiver::Instance(instance) => self
                        .type_environment
                        .name_of(self.declared_type_id(instance)?)?
                        .clone(),
                    MethodReceiver::SelfRef(self_ref) => self
                        .symbols
                        .self_type(&self.scope.current(), self_ref.kind)?,
                };
                let return_type = self
                    .method_return_types
                    .get(&fb_type)?
                    .get(&call.method)?
                    .clone()?;
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
        }
    }

    /// The type of the unqualified enumerated value `ev` used where a value
    /// of type `context` is expected (see [`enumerated_value_type`]). An
    /// ambiguous value is recorded until a context resolves it.
    fn unqualified_enumerated_value_type(
        &mut self,
        ev: &EnumeratedValue,
        context: Option<Context>,
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
    fn type_from_context(&mut self, expr: &mut Expr, context: Option<Context>) {
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

    /// What a value compared with or assigned to `variable` is expected to
    /// be: its type, or for a structure field of enumeration type, which has
    /// no type id here, its members.
    fn variable_context(&self, variable: &Variable) -> Option<Context<'_>> {
        if let Some(id) = self.variable_type_id(variable) {
            return Some(Context::Type(id));
        }
        let Variable::Symbolic(SymbolicVariableKind::Structured(sv)) = variable else {
            return None;
        };
        let parent = self.resolve_parent_struct_type(sv.record.as_ref())?;
        let field = parent
            .member_fields()?
            .iter()
            .find(|f| f.name == sv.field)?;
        field.field_type.enumeration_members().map(Context::Members)
    }

    /// What a value compared with `expr` is expected to be.
    fn expr_context(&self, expr: &Expr) -> Option<Context<'_>> {
        match (&expr.expr_type, &expr.kind) {
            (Some(ExprType::Concrete(id)), _) => Some(Context::Type(*id)),
            (None, ExprKind::Variable(variable)) => self.variable_context(variable),
            _ => None,
        }
    }

    /// The type a value the input `input` of the function block instance
    /// `instance` receives is expected to have.
    fn function_block_input_type(&self, instance: &Id, input: &Id) -> Option<TypeId> {
        let fb_type = self
            .type_environment
            .name_of(self.declared_type_id(instance)?)?;
        self.function_block_inputs.get(fb_type)?.get(input).copied()
    }

    /// The type `type_name` names: a literal of the category a generic name
    /// names, else the named type.
    fn expr_type_named(&self, type_name: TypeName) -> Option<ExprType> {
        if let Ok(generic) = GenericTypeName::try_from(&type_name.name) {
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
    /// Returns `None` for any other function, for a call with a named input
    /// left (one the named-argument pass diagnosed), or when no overload is
    /// judged or applies; the caller then types the call from its signature.
    fn resolve_overloaded_call(&self, f: &Function) -> Option<TypeName> {
        let form = operator_function_form(&f.name.to_string())?;
        let FormOf::Arithmetic(op) = &form.operator else {
            return None;
        };
        if form.typed_overloads().is_empty() {
            return None;
        }
        let names: Vec<Option<TypeName>> = f
            .param_assignment
            .iter()
            .map(|p| match p {
                ParamAssignmentKind::PositionalInput(input) => Some(self.operand_name(&input.expr)),
                ParamAssignmentKind::NamedInput(_) | ParamAssignmentKind::Output(_) => None,
            })
            .collect::<Option<_>>()?;
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
        let input = *positional_inputs(f)?.first()?;
        let name = match &input.expr_type {
            Some(ExprType::Literal(generic)) => literal_default_type(generic)?.into(),
            _ => self.operand_name(input)?,
        };
        let bit_string = match ElementaryTypeName::try_from(&name.name).ok()? {
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
    /// for `EXPT` its first input's (see [`Intrinsic::inputs_of_one_type`]).
    /// `MAX(i, l)` on an `INT` and an `LINT` is an `LINT`.
    ///
    /// When no input's type accepts every other one -- a `DINT` and a `UDINT`
    /// -- the call has the type of its first concrete input, else of its
    /// first, as a comparison of such a pair compares at its concrete left
    /// operand's (#1931).
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
                common_operand_of(&names, &self.options)
                    .or_else(|| {
                        inputs.iter().position(|input| {
                            matches!(
                                input.expr_type,
                                Some(ExprType::Concrete(_) | ExprType::Inferred(_))
                            )
                        })
                    })
                    .unwrap_or(0)
            }
        };
        inputs.get(index)?.expr_type.clone()
    }

    /// The id of the type the variable `name` names from the current scope
    /// was declared with.
    fn declared_type_id(&self, name: &Id) -> Option<TypeId> {
        self.symbols.find(name, &self.scope.current())?.type_id
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

    /// Resolves the type of a member access expression (e.g., `setup.FLAG` or
    /// `timer.Q`).
    ///
    /// Walks the member chain to find the root variable, looks up its type
    /// definition, then finds the leaf member's type.
    fn resolve_structured_variable_type(&self, sv: &StructuredVariable) -> Option<TypeName> {
        self.type_environment
            .elementary_type_name_for(self.field_type(sv)?.operated_as())
    }

    /// The type of the member `sv` accesses, as its representation: a field's
    /// type is known by its representation only.
    fn field_type<'b>(&'b self, sv: &StructuredVariable) -> Option<&'b SemanticType> {
        // A member of what `THIS^` or `SUPER^` names.
        if let SymbolicVariableKind::SelfRef(self_ref) = sv.record.as_ref() {
            let id = self.self_member_type_id(self_ref.kind, &sv.field)?;
            return Some(&self.type_environment.get_by_id(id)?.representation);
        }
        let parent_type = self.resolve_parent_struct_type(sv.record.as_ref())?;
        parent_type
            .member_fields()?
            .iter()
            .find(|f| f.name == sv.field)
            .map(|f| &f.field_type)
    }

    /// Resolves a `SymbolicVariableKind` to the `SemanticType` whose
    /// members it exposes.
    ///
    /// For `Structured`, recursively resolves the parent and finds the nested
    /// member type, and for an element of an array that is a field
    /// (`h.items[1]`), the array's element type. For anything else -- a
    /// variable, an element of an array variable, a dereferenced reference --
    /// takes the type its id names, when that is a structure or function
    /// block.
    fn resolve_parent_struct_type<'b>(
        &'b self,
        kind: &SymbolicVariableKind,
    ) -> Option<&'b SemanticType> {
        let representation = match kind {
            SymbolicVariableKind::Structured(sv) => self.field_type(sv)?,
            SymbolicVariableKind::Array(array) => match array.subscripted_variable.as_ref() {
                SymbolicVariableKind::Structured(sv) => self.field_element_type(sv)?,
                _ => self.representation_of(kind)?,
            },
            _ => self.representation_of(kind)?,
        };
        representation.has_members().then_some(representation)
    }

    /// The type the id of the symbolic variable `kind` names, as its
    /// representation.
    fn representation_of(&self, kind: &SymbolicVariableKind) -> Option<&SemanticType> {
        let id = self.symbolic_type_id(kind)?;
        Some(&self.type_environment.get_by_id(id)?.representation)
    }

    /// The element type of the array the member access `sv` names
    /// (`DATA.DIRS`), as its representation.
    fn field_element_type<'b>(&'b self, sv: &StructuredVariable) -> Option<&'b SemanticType> {
        match self.field_type(sv)? {
            SemanticType::Array { element_type, .. } => Some(element_type),
            _ => None,
        }
    }

    /// The id of the declared type of `field` on what `THIS^` (the enclosing
    /// function block and the blocks it `EXTENDS`) or `SUPER^` (only the
    /// blocks its base `EXTENDS`, so inherited fields alone) names. The
    /// lookup starts from the block's own scope, not the method's, so a
    /// method parameter that hides the field does not answer for it. `None`
    /// outside a function block, for `SUPER^` without a base, and for a
    /// field the block doesn't have.
    fn self_member_type_id(&self, kind: SelfRefKind, field: &Id) -> Option<TypeId> {
        variable_type::self_member(kind, field, self.symbols, &self.scope.current())?.type_id
    }

    /// Resolves the element type of an array that lives inside a struct field.
    ///
    /// For an expression like `DATA.DIRS[i, j]`, `sv` is the `DATA.DIRS`
    /// struct field access. Walks the struct chain to find `DIRS`'s
    /// `SemanticType::Array`, then returns the element type's
    /// canonical `TypeName`.
    fn resolve_struct_field_array_element_type(&self, sv: &StructuredVariable) -> Option<TypeName> {
        semantic_type_to_elementary_type_name(self.type_environment, self.field_element_type(sv)?)
    }

    /// The id of the type of the value the symbolic variable `kind` names:
    /// the declared type of a variable, the element type of a subscript,
    /// the referenced type of a dereference and the field type of a member
    /// access.
    fn symbolic_type_id(&self, kind: &SymbolicVariableKind) -> Option<TypeId> {
        match kind {
            SymbolicVariableKind::Named(nv) => self.declared_type_id(&nv.name),
            SymbolicVariableKind::Array(arr_var) => {
                // Array subscript on a struct field (e.g. `DATA.DIRS[i, j]`).
                // A field's type is known by its representation only, so
                // the element type is resolved through the struct chain.
                if let SymbolicVariableKind::Structured(sv) = arr_var.subscripted_variable.as_ref()
                {
                    return self
                        .type_environment
                        .id_of(&self.resolve_struct_field_array_element_type(sv)?);
                }
                let array = self.symbolic_type_id(&arr_var.subscripted_variable)?;
                self.type_environment.element_type(array)
            }
            SymbolicVariableKind::Structured(sv) => self
                .type_environment
                .id_of(&self.resolve_structured_variable_type(sv)?),
            SymbolicVariableKind::BitAccess(_) => {
                self.type_environment.id_of(&TypeName::from("BOOL"))
            }
            SymbolicVariableKind::PartialAccess(pa) => {
                let type_name = match pa.size {
                    PartialAccessSize::Byte => "BYTE",
                    PartialAccessSize::Word => "WORD",
                    PartialAccessSize::DWord => "DWORD",
                    PartialAccessSize::LWord => "LWORD",
                };
                self.type_environment.id_of(&TypeName::from(type_name))
            }
            SymbolicVariableKind::Deref(deref_var) => {
                let reference = self.symbolic_type_id(&deref_var.variable)?;
                self.type_environment.referenced_type(reference)
            }
            SymbolicVariableKind::SelfRef(_) => {
                // A bare THIS^/SUPER^ is a function block instance with no
                // value type of its own, and `rule_unsupported_extension`
                // reports it. Its members are typed through
                // `field_type`.
                None
            }
        }
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
            if !matches!(
                compare.op,
                CompareOp::And
                    | CompareOp::Or
                    | CompareOp::Xor
                    | CompareOp::AndThen
                    | CompareOp::OrElse
            ) {
                let left = self.expr_context(&compare.left).map(OwnedContext::from);
                let right = self.expr_context(&compare.right).map(OwnedContext::from);
                self.type_from_context(&mut compare.right, left.as_ref().map(OwnedContext::get));
                self.type_from_context(&mut compare.left, right.as_ref().map(OwnedContext::get));
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
        let target = self.variable_context(&node.target).map(OwnedContext::from);
        self.type_from_context(&mut node.value, target.as_ref().map(OwnedContext::get));
        Ok(node)
    }

    fn fold_fb_call(&mut self, node: FbCall) -> Result<FbCall, Diagnostic> {
        let mut node = node.recurse_fold(self)?;
        // An unqualified enumerated value passed to an input has its type.
        for param in &mut node.params {
            if let ParamAssignmentKind::NamedInput(input) = param {
                let input_type = self
                    .function_block_input_type(&node.var_name, &input.name)
                    .map(Context::Type);
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
