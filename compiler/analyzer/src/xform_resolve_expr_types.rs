//! Transformation pass that resolves expression types.
//!
//! This pass populates the `expr_type` field on `Expr` nodes: the type of
//! each expression's value by identity (ADR-0055). Later rules and codegen
//! read an expression's type from it and nowhere else. Where a relation
//! compares types by name, it derives the name from the id through
//! `value_type::operand_type_name`.
use ironplc_dsl::common::*;
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::textual::*;
use ironplc_dsl::type_id::TypeId;
use std::collections::HashMap;

use crate::callee_resolution::FunctionBlocks;
use crate::function_environment::FunctionEnvironment;
use crate::intermediates::arithmetic_overload::{
    resolve_arithmetic_fold, resolve_arithmetic_overload, Overload,
};
use crate::intermediates::inherited_fields::collect_inherited_fields;
use crate::intermediates::operator_function_form::{operator_function_form, FormOf};
use crate::selection_type::{ArrayDeclarations, Selections};
use crate::system_globals::SYSTEM_UPTIME_GLOBALS;
use crate::type_environment::TypeEnvironment;
use crate::value_type::operand_type_name;
use crate::variable_type::{Declarations, Declared};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: Library,
    type_environment: &mut TypeEnvironment,
    function_environment: &FunctionEnvironment,
    options: &CompilerOptions,
) -> Result<Library, Vec<Diagnostic>> {
    let inherited_fields = collect_inherited_fields(&lib);
    let method_return_types = collect_method_return_types(&lib);
    let arrays = ArrayDeclarations::from_library(&lib);
    let mut resolver = ExprTypeResolver {
        declarations: Declarations::new(),
        arrays,
        inherited_fields,
        method_return_types,
        type_environment,
        function_environment,
        options: *options,
    };

    // Implicit system globals live in the outermost scope, so every POU
    // body sees them and a POU-local of the same name shadows them.
    if options.allow_system_uptime_global {
        for global in &SYSTEM_UPTIME_GLOBALS {
            resolver.declarations.add(
                &Id::from(global.name),
                Declared::Typed(TypeName::from(global.type_name)),
            );
        }
    }

    resolver.fold_library(lib).map_err(|e| vec![e])
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

/// The type of an operation on two operands that keeps their type: the
/// concrete operand's when the other is an untyped literal (`d AND 16#FF` on
/// a `DWORD` is a `DWORD`), else the left operand's, else the right's.
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
    /// Declared type of every variable in scope.
    ///
    /// The outermost scope holds `VAR_GLOBAL` and the implicit system
    /// globals; each POU the traversal enters pushes a scope of its own.
    /// A method's scope nests inside its function block's, so a method
    /// body sees the instance's fields and a method local shadows a
    /// field of the same name.
    declarations: Declarations<'static>,
    /// The array type declarations of the library, which name the element
    /// type of a named array type.
    arrays: ArrayDeclarations,
    /// Fields inherited via `EXTENDS`, per function block -- see
    /// `intermediates::inherited_fields`. Seeded into `var_types` before a
    /// function block's own fields so unqualified references to a base
    /// class's fields type-check correctly.
    inherited_fields: HashMap<TypeName, Vec<VarDecl>>,
    /// See [`collect_method_return_types`].
    method_return_types: HashMap<TypeName, HashMap<Id, Option<TypeName>>>,
    type_environment: &'a mut TypeEnvironment,
    function_environment: &'a FunctionEnvironment,
    /// The compiler options, which decide whether a bit-string operand of
    /// an arithmetic operator is judged as an unsigned integer (ADR-0053).
    options: CompilerOptions,
}

impl ExprTypeResolver<'_> {
    /// Registers a declaration's own name as its result variable, so
    /// `Foo := ...` inside `FUNCTION Foo` (or a `METHOD Foo : T`)
    /// resolves to the declared return type.
    fn insert_result_variable(&mut self, name: &Id, return_type: &FunctionReturnType) {
        self.declarations
            .add(name, Declared::Typed(return_type.to_type_name()));
    }

    /// Records a variable declaration in the current scope.
    fn insert(&mut self, node: &VarDecl) {
        self.declarations
            .add_if(node.identifier.symbolic_id(), Declared::of(node));
    }

    /// Returns the type name a variable in scope was declared with.
    ///
    /// `None` for a variable that is not in scope and for one whose type
    /// has no name to give: an inline enumeration, an inline array, or a
    /// declaration without a type.
    fn declared_type_name(&self, id: &Id) -> Option<TypeName> {
        let init = match self.declarations.find(id)? {
            Declared::Variable { init, .. } => init,
            Declared::Typed(type_name) => return Some(type_name.clone()),
        };
        match init.as_ref() {
            InitialValueAssignmentKind::None(_) => None,
            InitialValueAssignmentKind::Simple(si) => Some(si.type_name.clone()),
            InitialValueAssignmentKind::String(si) => Some(si.type_name()),
            InitialValueAssignmentKind::EnumeratedValues(_) => None,
            InitialValueAssignmentKind::EnumeratedType(e) => Some(e.type_name.clone()),
            InitialValueAssignmentKind::FunctionBlock(fb) => Some(fb.type_name.clone()),
            InitialValueAssignmentKind::FunctionBlockCall(fbc) => Some(fbc.type_name.clone()),
            InitialValueAssignmentKind::Subrange(spec) => match spec {
                SpecificationKind::Named(tn) => Some(tn.clone()),
                SpecificationKind::Inline(sr) => Some(TypeName::from(&sr.type_name.to_string())),
            },
            InitialValueAssignmentKind::Structure(s) => Some(s.type_name.clone()),
            InitialValueAssignmentKind::Array(a) => match &a.spec {
                SpecificationKind::Named(tn) => Some(tn.clone()),
                SpecificationKind::Inline(_) => None,
            },
            // An inline array target has no single type name.
            InitialValueAssignmentKind::Reference(ref_init) => ref_init.target.type_name().cloned(),
            InitialValueAssignmentKind::LateResolvedType(LateResolvedInitializer {
                type_name: tn,
                ..
            }) => Some(tn.clone()),
            InitialValueAssignmentKind::SimpleExpr(se) => Some(se.type_name.clone()),
        }
    }

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
                // Bitwise/logical operators preserve operand type. When one
                // operand is an untyped literal and the other is concrete
                // (e.g. a DWORD variable), use the concrete type.
                CompareOp::And
                | CompareOp::Or
                | CompareOp::Xor
                | CompareOp::AndThen
                | CompareOp::OrElse => {
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
            // The method's return type. A call on `THIS^`/`SUPER^`, or to a
            // method without a return type, has no type here; the method
            // call rule reports both.
            ExprKind::MethodCall(call) => {
                let MethodReceiver::Instance(instance) = &call.receiver else {
                    return None;
                };
                let fb_type = self.declared_type_name(instance)?;
                let return_type = self
                    .method_return_types
                    .get(&fb_type)?
                    .get(&call.method)?
                    .clone()?;
                self.expr_type_named(return_type)
            }
            ExprKind::EnumeratedValue(ev) => self.expr_type_named(ev.type_name.clone()?),
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
                Some(ExprType::Concrete(reference)) => self
                    .type_environment
                    .referenced_type(*reference)
                    .map(ExprType::Concrete),
                Some(ExprType::Literal(_) | ExprType::Null) | None => None,
            },
            ExprKind::Null(_) => Some(ExprType::Null),
        }
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

    /// The id of the type the variable `name` in scope was declared with.
    fn declared_type_id(&self, name: &Id) -> Option<TypeId> {
        self.declarations
            .find(name)
            .and_then(|declared| declared.type_id(self.type_environment))
    }

    /// The id of the type of the value `var` names.
    fn variable_type_id(&self, var: &Variable) -> Option<TypeId> {
        if let Variable::Symbolic(SymbolicVariableKind::Named(nv)) = var {
            if let Some(id) = self.declared_type_id(&nv.name) {
                return Some(id);
            }
        }
        self.type_environment
            .id_of(&self.resolve_variable_type(var)?)
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

    fn resolve_variable_type(&self, var: &Variable) -> Option<TypeName> {
        match var {
            Variable::Symbolic(SymbolicVariableKind::Named(nv)) => {
                let declared = self.declared_type_name(&nv.name)?;
                // Try to resolve to an elementary type. If the type is complex
                // (enum, struct, etc.), keep the declared name.
                Some(
                    self.type_environment
                        .resolve_elementary_type_name(&declared)
                        .unwrap_or(declared),
                )
            }
            // A subscript, field or dereference names the type its
            // selection walks to (see `selection_type`).
            Variable::Symbolic(kind) => {
                let selections = Selections {
                    declarations: &self.declarations,
                    types: self.type_environment,
                    arrays: &self.arrays,
                };
                selections.type_name(&selections.view_of(kind)?)
            }
            Variable::Direct(_) => None,
        }
    }
}

impl Fold<Diagnostic> for ExprTypeResolver<'_> {
    fn fold_library(
        &mut self,
        node: ironplc_dsl::common::Library,
    ) -> Result<ironplc_dsl::common::Library, Diagnostic> {
        // Collect top-level VAR_GLOBAL types into the outermost scope,
        // where they stay visible to every POU body the fold enters.
        for element in &node.elements {
            if let LibraryElementKind::GlobalVarDeclarations(decls) = element {
                for decl in decls {
                    self.insert(decl);
                }
            }
        }
        node.recurse_fold(self)
    }

    /// Opens a declaration's scope and registers the types it declares.
    ///
    /// Replaces the per-POU `clear()` this pass used to do: clearing at a
    /// method boundary would discard the enclosing function block's
    /// fields, which a method body needs. A scope stack drops only what
    /// the declaration itself added.
    ///
    /// The match is exhaustive so a new kind of scope has to state what
    /// it contributes rather than silently contributing nothing -- which
    /// in this pass means silently skipping type checks, not failing.
    fn enter_scope(&mut self, node: ScopeNode<'_>) -> Result<(), Diagnostic> {
        self.declarations.enter();

        match node {
            ScopeNode::Function(node) => {
                node.variables.iter().for_each(|v| self.insert(v));
                self.insert_result_variable(&node.name, &node.return_type);
            }
            ScopeNode::FunctionBlock(node) => {
                // Inherited fields first so the function block's own
                // fields, inserted next into the same scope, win for a
                // name declared in both. A program that reaches code
                // generation never has such a name --
                // `rule_extends_field_duplicated` (`P4044`) rejects it --
                // but that rule runs after this transform, so this pass
                // still needs a defined answer.
                if let Some(fields) = self.inherited_fields.get(&node.name).cloned() {
                    fields.iter().for_each(|v| self.insert(v));
                }
                node.variables.iter().for_each(|v| self.insert(v));
            }
            ScopeNode::Program(node) => {
                node.variables.iter().for_each(|v| self.insert(v));
            }
            ScopeNode::Method(node) => {
                node.variables.iter().for_each(|v| self.insert(v));
                // Only a method that declares a return type has a result
                // variable; see `rule_use_declared_symbolic_var`, which
                // rejects the assignment for one that does not.
                if let Some(return_type) = &node.return_type {
                    self.insert_result_variable(&node.name, return_type);
                }
            }
        }

        Ok(())
    }

    fn exit_scope(&mut self) {
        self.declarations.exit();
    }

    fn fold_self_ref_variable(
        &mut self,
        node: SelfRefVariable,
    ) -> Result<SelfRefVariable, Diagnostic> {
        // Fail rather than resolve to "unknown": every downstream consumer
        // of this pass treats an unresolved type as a fact about the
        // program, and silently producing one here would let THIS^/SUPER^
        // through unnoticed once it is otherwise supported. See issue #1406.
        Err(Diagnostic::not_implemented(Label::span(
            node.span(),
            format!(
                "{} is recognized but its type cannot be resolved by IronPLC yet",
                node.kind.spelling()
            ),
        )))
    }

    fn fold_expr(&mut self, node: Expr) -> Result<Expr, Diagnostic> {
        // First, recurse to fold children (bottom-up)
        let mut expr = node.recurse_fold(self)?;

        // Then determine type based on the (now-folded) kind
        expr.expr_type = self.resolve_type(&expr.kind);
        Ok(expr)
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
