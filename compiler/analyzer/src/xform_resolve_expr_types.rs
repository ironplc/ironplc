//! Transformation pass that resolves expression types.
//!
//! This pass populates the `resolved_type` field on `Expr` nodes. After this
//! pass, codegen can read types directly from expression nodes instead of
//! re-inferring them from variable names.
//!
//! The key problem this solves: codegen string-matches declared type names
//! (e.g., `"INT"`) against a hardcoded list. Type aliases like `"MyByte"`
//! don't match, causing incorrect opcode selection. By resolving aliases to
//! elementary types here, codegen gets clean type names.
use ironplc_dsl::common::*;
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::fold::Fold;
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::textual::*;
use std::collections::HashMap;

use crate::function_environment::FunctionEnvironment;
use crate::intermediate_type::IntermediateType;
use crate::intermediates::inherited_fields::collect_inherited_fields;
use crate::system_globals::SYSTEM_UPTIME_GLOBALS;
use crate::type_environment::TypeEnvironment;
use crate::variable_type::{Declarations, Declared};
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: Library,
    type_environment: &mut TypeEnvironment,
    function_environment: &FunctionEnvironment,
    options: &CompilerOptions,
) -> Result<Library, Vec<Diagnostic>> {
    let inherited_fields = collect_inherited_fields(&lib);
    let mut resolver = ExprTypeResolver {
        declarations: Declarations::new(),
        inherited_fields,
        type_environment,
        function_environment,
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

/// Maps an [`IntermediateType`] to its canonical elementary [`TypeName`].
///
/// Delegates to [`TypeEnvironment::elementary_type_name_for`] for the simple
/// cases. That helper does a strict equality lookup against the elementary
/// types table, which only contains `String { max_len: None }`. A struct
/// field declared `STRING[n]` resolves to `String { max_len: Some(n) }` and
/// would otherwise return `None`, so we handle strings explicitly.
fn intermediate_to_elementary_type_name(
    env: &TypeEnvironment,
    it: &IntermediateType,
) -> Option<TypeName> {
    if let Some(tn) = env.elementary_type_name_for(it) {
        return Some(tn);
    }
    match it {
        IntermediateType::String { .. } => Some(TypeName::from("STRING")),
        _ => None,
    }
}

/// Walks a nested [`SymbolicVariableKind`] chain to find the root named variable.
///
/// For example, `pt^[i]` is `Array { Deref { Named("pt") } }` — this returns `"pt"`.
fn find_base_variable_name(var: &SymbolicVariableKind) -> Option<&Id> {
    match var {
        SymbolicVariableKind::Named(nv) => Some(&nv.name),
        SymbolicVariableKind::Deref(dv) => find_base_variable_name(&dv.variable),
        SymbolicVariableKind::Array(av) => find_base_variable_name(&av.subscripted_variable),
        SymbolicVariableKind::BitAccess(ba) => find_base_variable_name(&ba.variable),
        SymbolicVariableKind::PartialAccess(pa) => find_base_variable_name(&pa.variable),
        _ => None,
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
    /// Fields inherited via `EXTENDS`, per function block -- see
    /// `intermediates::inherited_fields`. Seeded into `var_types` before a
    /// function block's own fields so unqualified references to a base
    /// class's fields type-check correctly.
    inherited_fields: HashMap<TypeName, Vec<VarDecl>>,
    type_environment: &'a TypeEnvironment,
    function_environment: &'a FunctionEnvironment,
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
            Declared::Variable(init) => init,
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

    /// Returns the element type name of a variable in scope declared as an
    /// array or a reference to one, so that `arr[i]` and `pt^[i]` resolve
    /// to the element type.
    ///
    /// For `arr : ARRAY[0..10] OF INT` this is `"int"`; for
    /// `pt : REF_TO ARRAY[1..255] OF BYTE` it is `"byte"`.
    fn declared_element_type_name(&self, id: &Id) -> Option<TypeName> {
        let Declared::Variable(init) = self.declarations.find(id)? else {
            // A result variable or system global is never subscripted.
            return None;
        };
        match init.as_ref() {
            // ARRAY[...] OF T (inline spec)
            InitialValueAssignmentKind::Array(a) => match &a.spec {
                SpecificationKind::Inline(inline) => {
                    Some(self.resolve_element_type_name(&inline.type_name))
                }
                SpecificationKind::Named(tn) => self.element_type_from_named_array(tn),
            },
            // REF_TO ARRAY[...] OF T or REF_TO <named_array_type>
            InitialValueAssignmentKind::Reference(ref_init) => match &ref_init.target {
                ReferenceTarget::Array(subranges) => {
                    Some(self.resolve_element_type_name(&subranges.type_name))
                }
                ReferenceTarget::Named(tn) => self.element_type_from_named_array(tn),
            },
            // Named type that may be an array alias (e.g., `arr : MyArr`
            // where `TYPE MyArr : ARRAY[0..10] OF INT; END_TYPE`)
            InitialValueAssignmentKind::Simple(si) => {
                self.element_type_from_named_array(&si.type_name)
            }
            // Late-resolved type that may be an array alias
            InitialValueAssignmentKind::LateResolvedType(LateResolvedInitializer {
                type_name: tn,
                ..
            }) => self.element_type_from_named_array(tn),
            _ => None,
        }
    }

    /// Resolves an [`ArrayElementType`] to a canonical elementary type name.
    fn resolve_element_type_name(&self, elem: &ArrayElementType) -> TypeName {
        let tn = elem.to_type_name();
        self.type_environment
            .resolve_elementary_type_name(&tn)
            .unwrap_or(tn)
    }

    /// Looks up a named type in the type environment; if it is an array,
    /// returns the element type name.
    fn element_type_from_named_array(&self, type_name: &TypeName) -> Option<TypeName> {
        let attrs = self.type_environment.get(type_name)?;
        match &attrs.representation {
            IntermediateType::Array { element_type, .. } => {
                self.type_environment.elementary_type_name_for(element_type)
            }
            _ => None,
        }
    }

    /// Determines the resolved type for the given expression kind.
    fn resolve_type(&self, kind: &ExprKind) -> Option<TypeName> {
        match kind {
            ExprKind::Const(constant) => self.resolve_const_type(constant),
            ExprKind::Variable(var) => self.resolve_variable_type(var),
            ExprKind::BinaryOp(op) => {
                match (&op.left.resolved_type, &op.right.resolved_type) {
                    // If left is generic and right is concrete, use the concrete type.
                    (Some(l), Some(r)) if is_generic_type(l) && !is_generic_type(r) => {
                        Some(r.clone())
                    }
                    (Some(l), _) => Some(l.clone()),
                    (_, r) => r.clone(),
                }
            }
            ExprKind::UnaryOp(op) => op.term.resolved_type.clone(),
            ExprKind::Compare(compare) => match compare.op {
                CompareOp::And
                | CompareOp::Or
                | CompareOp::Xor
                | CompareOp::AndThen
                | CompareOp::OrElse => {
                    // Bitwise/logical operators preserve operand type.
                    // When one operand is generic (e.g. ANY_INT literal)
                    // and the other is concrete (e.g. DWORD variable), use
                    // the concrete type.
                    match (&compare.left.resolved_type, &compare.right.resolved_type) {
                        (Some(l), Some(r)) if is_generic_type(l) && !is_generic_type(r) => {
                            Some(r.clone())
                        }
                        (Some(l), _) => Some(l.clone()),
                        (_, r) => r.clone(),
                    }
                }
                _ => Some(TypeName::from("BOOL")),
            },
            ExprKind::Function(f) => {
                let sig = self.function_environment.get(&f.name)?;
                let return_type = sig.return_type.as_ref()?.to_type_name();
                if is_generic_type(&return_type) {
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
                                    pos.expr.resolved_type.clone()
                                }
                                _ => None,
                            }
                        }
                        ParamAssignmentKind::NamedInput(named) => {
                            let param = sig.parameters.iter().find(|p| p.name == named.name);
                            match param {
                                Some(param) if param.param_type == return_type => {
                                    named.expr.resolved_type.clone()
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    })
                } else {
                    Some(return_type)
                }
            }
            ExprKind::EnumeratedValue(ev) => ev.type_name.clone(),
            ExprKind::Expression(inner) => inner.resolved_type.clone(),
            ExprKind::LateBound(_) => None,
            ExprKind::Ref(var) => {
                // REF(var) produces a reference — resolve the variable's type
                self.resolve_variable_type(var)
            }
            ExprKind::Deref(inner) => {
                // Dereference: the result type is the referenced variable's type.
                // The inner expression should be a reference whose resolved_type
                // is the referenced type name.
                inner.resolved_type.clone()
            }
            ExprKind::Null(_) => {
                // NULL has placeholder type BOOL (see design doc NULL Type Resolution Strategy).
                // Actual type compatibility is checked contextually by semantic rules.
                Some(TypeName::from("BOOL"))
            }
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
        let parent_type = self.resolve_parent_struct_type(sv.record.as_ref())?;
        let field = parent_type
            .member_fields()?
            .iter()
            .find(|f| f.name == sv.field)?;
        self.type_environment
            .elementary_type_name_for(&field.field_type)
    }

    /// Resolves a `SymbolicVariableKind` to the `IntermediateType` whose
    /// members it exposes.
    ///
    /// For `Named`, looks up the variable's declared type and resolves it as a
    /// structure or function block instance. For `Structured`, recursively
    /// resolves the parent and finds the nested member type.
    fn resolve_parent_struct_type<'b>(
        &'b self,
        kind: &SymbolicVariableKind,
    ) -> Option<&'b IntermediateType> {
        match kind {
            SymbolicVariableKind::Named(nv) => {
                let var_type = self.declared_type_name(&nv.name)?;
                self.type_environment.resolve_member_access_type(&var_type)
            }
            SymbolicVariableKind::Structured(sv) => {
                let parent_type = self.resolve_parent_struct_type(sv.record.as_ref())?;
                let field = parent_type
                    .member_fields()?
                    .iter()
                    .find(|f| f.name == sv.field)?;
                if field.field_type.has_members() {
                    Some(&field.field_type)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Resolves the element type of an array that lives inside a struct field.
    ///
    /// For an expression like `DATA.DIRS[i, j]`, `sv` is the `DATA.DIRS`
    /// struct field access. Walks the struct chain to find `DIRS`'s
    /// `IntermediateType::Array`, then returns the element type's
    /// canonical `TypeName`.
    fn resolve_struct_field_array_element_type(&self, sv: &StructuredVariable) -> Option<TypeName> {
        let parent_type = self.resolve_parent_struct_type(sv.record.as_ref())?;
        let field = parent_type
            .member_fields()?
            .iter()
            .find(|f| f.name == sv.field)?;
        let IntermediateType::Array { element_type, .. } = &field.field_type else {
            return None;
        };
        intermediate_to_elementary_type_name(self.type_environment, element_type)
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
            Variable::Symbolic(SymbolicVariableKind::Array(arr_var)) => {
                // Array subscript on a struct field (e.g. `DATA.DIRS[i, j]`).
                // The base variable is a struct, not the array itself, so we
                // resolve the field's type through the struct chain.
                if let SymbolicVariableKind::Structured(sv) = arr_var.subscripted_variable.as_ref()
                {
                    return self.resolve_struct_field_array_element_type(sv);
                }

                // Array subscript: walk to base variable, return element type.
                let base_name = find_base_variable_name(&arr_var.subscripted_variable)?;
                let elem_type = self.declared_element_type_name(base_name)?;
                Some(
                    self.type_environment
                        .resolve_elementary_type_name(&elem_type)
                        .unwrap_or(elem_type),
                )
            }
            Variable::Symbolic(SymbolicVariableKind::Structured(sv)) => {
                self.resolve_structured_variable_type(sv)
            }
            Variable::Symbolic(SymbolicVariableKind::BitAccess(_)) => Some(TypeName::from("BOOL")),
            Variable::Symbolic(SymbolicVariableKind::PartialAccess(pa)) => {
                let type_name = match pa.size {
                    PartialAccessSize::Byte => "BYTE",
                    PartialAccessSize::Word => "WORD",
                    PartialAccessSize::DWord => "DWORD",
                    PartialAccessSize::LWord => "LWORD",
                };
                Some(TypeName::from(type_name))
            }
            Variable::Symbolic(SymbolicVariableKind::Deref(deref_var)) => {
                // Dereference: resolve the target type of the reference.
                let base_name = find_base_variable_name(&deref_var.variable)?;
                let declared = self.declared_type_name(base_name)?;
                let attrs = self.type_environment.get(&declared)?;
                if let Some(target) = attrs.representation.referenced_type() {
                    self.type_environment.elementary_type_name_for(target)
                } else {
                    None
                }
            }
            Variable::Symbolic(SymbolicVariableKind::SelfRef(_)) => {
                // THIS^/SUPER^ has no resolvable type until function-block
                // member resolution exists. Unreachable in practice:
                // `fold_self_ref_variable` rejects the construct before any
                // type resolution runs. See issue #1406.
                None
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
        expr.resolved_type = self.resolve_type(&expr.kind);
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
