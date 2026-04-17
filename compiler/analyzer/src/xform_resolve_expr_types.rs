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
use ironplc_dsl::core::Id;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_dsl::fold::Fold;
use ironplc_dsl::textual::*;
use std::collections::HashMap;

use crate::function_environment::FunctionEnvironment;
use crate::intermediate_type::IntermediateType;
use crate::type_environment::TypeEnvironment;
use ironplc_parser::options::CompilerOptions;

pub fn apply(
    lib: Library,
    type_environment: &mut TypeEnvironment,
    function_environment: &FunctionEnvironment,
    options: &CompilerOptions,
) -> Result<Library, Vec<Diagnostic>> {
    let mut resolver = ExprTypeResolver {
        var_types: HashMap::new(),
        global_var_types: HashMap::new(),
        array_element_types: HashMap::new(),
        type_environment,
        function_environment,
        options,
    };
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
    /// Maps variable names to their declared TypeName within the current POU scope.
    var_types: HashMap<Id, TypeName>,
    /// Maps global variable names to their declared TypeName, persists across POU folds.
    global_var_types: HashMap<Id, TypeName>,
    /// For variables declared as arrays or REF_TO arrays, stores the element type.
    ///
    /// For `arr : ARRAY[0..10] OF INT`, stores `"int"` keyed by `"arr"`.
    /// For `pt : REF_TO ARRAY[1..255] OF BYTE`, stores `"byte"` keyed by `"pt"`.
    /// This enables `resolve_variable_type` to return the correct element type
    /// when resolving `arr[i]` or `pt^[i]` expressions.
    array_element_types: HashMap<Id, TypeName>,
    type_environment: &'a TypeEnvironment,
    function_environment: &'a FunctionEnvironment,
    options: &'a CompilerOptions,
}

impl ExprTypeResolver<'_> {
    /// Registers implicit system globals and top-level VAR_GLOBAL variables
    /// so direct references resolve correctly within each POU scope.
    fn seed_implicit_globals(&mut self) {
        if self.options.allow_system_uptime_global {
            self.var_types
                .insert(Id::from("__SYSTEM_UP_TIME"), TypeName::from("TIME"));
            self.var_types
                .insert(Id::from("__SYSTEM_UP_LTIME"), TypeName::from("LTIME"));
        }
        for (id, type_name) in &self.global_var_types {
            self.var_types.insert(id.clone(), type_name.clone());
        }
    }

    /// Extracts the TypeName from a variable declaration and inserts it into the
    /// variable type map. Also populates `array_element_types` for array and
    /// REF_TO array variables so that subscript expressions can resolve their
    /// element type.
    fn insert(&mut self, node: &VarDecl) {
        self.insert_array_element_type(node);

        let type_name = match &node.initializer {
            InitialValueAssignmentKind::None(_) => return,
            InitialValueAssignmentKind::Simple(si) => si.type_name.clone(),
            InitialValueAssignmentKind::String(si) => si.type_name(),
            InitialValueAssignmentKind::EnumeratedValues(_) => return,
            InitialValueAssignmentKind::EnumeratedType(e) => e.type_name.clone(),
            InitialValueAssignmentKind::FunctionBlock(fb) => fb.type_name.clone(),
            InitialValueAssignmentKind::Subrange(spec) => match spec {
                SpecificationKind::Named(tn) => tn.clone(),
                SpecificationKind::Inline(sr) => TypeName::from(&sr.type_name.to_string()),
            },
            InitialValueAssignmentKind::Structure(s) => s.type_name.clone(),
            InitialValueAssignmentKind::Array(a) => match &a.spec {
                SpecificationKind::Named(tn) => tn.clone(),
                SpecificationKind::Inline(_) => return,
            },
            InitialValueAssignmentKind::Reference(ref_init) => match ref_init.target.type_name() {
                Some(tn) => tn.clone(),
                None => return, // Inline array targets don't have a single type name
            },
            InitialValueAssignmentKind::LateResolvedType(tn) => tn.clone(),
        };

        match &node.identifier {
            VariableIdentifier::Symbol(id) => {
                self.var_types.insert(id.clone(), type_name);
            }
            VariableIdentifier::Direct(direct) => {
                if let Some(name) = &direct.name {
                    self.var_types.insert(name.clone(), type_name);
                }
            }
        }
    }

    /// Extracts and stores the array element type for array and REF_TO array
    /// variable declarations.
    fn insert_array_element_type(&mut self, node: &VarDecl) {
        let id = match &node.identifier {
            VariableIdentifier::Symbol(id) => id.clone(),
            VariableIdentifier::Direct(direct) => match &direct.name {
                Some(name) => name.clone(),
                None => return,
            },
        };

        let elem_type_name = match &node.initializer {
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
            InitialValueAssignmentKind::LateResolvedType(tn) => {
                self.element_type_from_named_array(tn)
            }
            _ => None,
        };

        if let Some(elem_tn) = elem_type_name {
            self.array_element_types.insert(id, elem_tn);
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
                CompareOp::And | CompareOp::Or | CompareOp::Xor => {
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
            ConstantKind::CharacterString(_) => Some(TypeName::from("STRING")),
            ConstantKind::Duration(_) => Some(TypeName::from("TIME")),
            ConstantKind::TimeOfDay(_) => Some(TypeName::from("TIME_OF_DAY")),
            ConstantKind::Date(_) => Some(TypeName::from("DATE")),
            ConstantKind::DateAndTime(_) => Some(TypeName::from("DATE_AND_TIME")),
        }
    }

    /// Resolves the type of a struct field access expression (e.g., `setup.FLAG`).
    ///
    /// Walks the struct chain to find the root variable, looks up its struct
    /// type definition, then finds the leaf field's type.
    fn resolve_structured_variable_type(&self, sv: &StructuredVariable) -> Option<TypeName> {
        let parent_type = self.resolve_parent_struct_type(sv.record.as_ref())?;
        match parent_type {
            IntermediateType::Structure { fields } => {
                let field = fields.iter().find(|f| f.name == sv.field)?;
                self.type_environment
                    .elementary_type_name_for(&field.field_type)
            }
            _ => None,
        }
    }

    /// Resolves a `SymbolicVariableKind` to its struct `IntermediateType`.
    ///
    /// For `Named`, looks up the variable's declared type and resolves it as a
    /// struct. For `Structured`, recursively resolves the parent and finds the
    /// nested struct field type.
    fn resolve_parent_struct_type<'b>(
        &'b self,
        kind: &SymbolicVariableKind,
    ) -> Option<&'b IntermediateType> {
        match kind {
            SymbolicVariableKind::Named(nv) => {
                let var_type = self.var_types.get(&nv.name)?;
                self.type_environment.resolve_struct_type(var_type)
            }
            SymbolicVariableKind::Structured(sv) => {
                let parent_type = self.resolve_parent_struct_type(sv.record.as_ref())?;
                match parent_type {
                    IntermediateType::Structure { fields } => {
                        let field = fields.iter().find(|f| f.name == sv.field)?;
                        if field.field_type.is_structure() {
                            Some(&field.field_type)
                        } else {
                            None
                        }
                    }
                    _ => None,
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
        let IntermediateType::Structure { fields } = parent_type else {
            return None;
        };
        let field = fields.iter().find(|f| f.name == sv.field)?;
        let IntermediateType::Array { element_type, .. } = &field.field_type else {
            return None;
        };
        intermediate_to_elementary_type_name(self.type_environment, element_type)
    }

    fn resolve_variable_type(&self, var: &Variable) -> Option<TypeName> {
        match var {
            Variable::Symbolic(SymbolicVariableKind::Named(nv)) => {
                let declared = self.var_types.get(&nv.name)?;
                // Try to resolve to an elementary type. If the type is complex
                // (enum, struct, etc.), keep the declared name.
                Some(
                    self.type_environment
                        .resolve_elementary_type_name(declared)
                        .unwrap_or_else(|| declared.clone()),
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
                let elem_type = self.array_element_types.get(base_name)?;
                Some(
                    self.type_environment
                        .resolve_elementary_type_name(elem_type)
                        .unwrap_or_else(|| elem_type.clone()),
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
                let declared = self.var_types.get(base_name)?;
                let attrs = self.type_environment.get(declared)?;
                if let Some(target) = attrs.representation.referenced_type() {
                    self.type_environment.elementary_type_name_for(target)
                } else {
                    None
                }
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
        // Pre-collect top-level VAR_GLOBAL variable types so they are
        // available when folding FUNCTION/FB/PROGRAM bodies.
        for element in &node.elements {
            if let LibraryElementKind::GlobalVarDeclarations(decls) = element {
                for decl in decls {
                    self.insert(decl);
                }
            }
        }
        self.global_var_types = self.var_types.clone();
        self.var_types.clear();
        node.recurse_fold(self)
    }

    fn fold_function_declaration(
        &mut self,
        node: FunctionDeclaration,
    ) -> Result<FunctionDeclaration, Diagnostic> {
        node.variables.iter().for_each(|v| self.insert(v));
        self.seed_implicit_globals();
        // Register the implicit return variable (function name = return type)
        // so that references like `FOO := SHR(FOO, 1)` inside FUNCTION FOO
        // can resolve the type of the FOO variable.
        let return_type_name = node.return_type.to_type_name();
        let resolved_return_type = self
            .type_environment
            .resolve_elementary_type_name(&return_type_name)
            .unwrap_or(return_type_name);
        self.var_types
            .insert(node.name.clone(), resolved_return_type);
        let result = node.recurse_fold(self);
        self.var_types.clear();
        self.array_element_types.clear();
        result
    }

    fn fold_function_block_declaration(
        &mut self,
        node: FunctionBlockDeclaration,
    ) -> Result<FunctionBlockDeclaration, Diagnostic> {
        node.variables.iter().for_each(|v| self.insert(v));
        self.seed_implicit_globals();
        let result = node.recurse_fold(self);
        self.var_types.clear();
        self.array_element_types.clear();
        result
    }

    fn fold_program_declaration(
        &mut self,
        node: ProgramDeclaration,
    ) -> Result<ProgramDeclaration, Diagnostic> {
        node.variables.iter().for_each(|v| self.insert(v));
        self.seed_implicit_globals();
        let result = node.recurse_fold(self);
        self.var_types.clear();
        self.array_element_types.clear();
        result
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
mod tests {
    use super::apply;
    use crate::type_environment::TypeEnvironmentBuilder;
    use crate::xform_resolve_late_bound_expr_kind;
    use crate::xform_resolve_symbol_and_function_environment;
    use crate::xform_resolve_type_decl_environment;
    use crate::{
        function_environment::FunctionEnvironmentBuilder, symbol_environment::SymbolEnvironment,
    };
    use ironplc_dsl::common::Library;
    use ironplc_dsl::core::FileId;
    use ironplc_dsl::fold::Fold;
    use ironplc_dsl::textual::*;
    use ironplc_parser::options::{CompilerOptions, Dialect};
    use rstest::rstest;

    /// Runs the prerequisite passes and then the expression type resolution pass.
    fn run_pass(program: &str) -> Library {
        run_pass_with_options(program, &CompilerOptions::default())
    }

    /// Like [`run_pass`] but with explicit compiler options (needed for REF_TO tests).
    fn run_pass_with_options(program: &str, options: &CompilerOptions) -> Library {
        let library = ironplc_parser::parse_program(program, &FileId::default(), options).unwrap();
        let mut type_environment = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .with_stdlib_function_blocks()
            .build()
            .unwrap();
        let library =
            xform_resolve_type_decl_environment::apply(library, &mut type_environment).unwrap();
        let library =
            xform_resolve_late_bound_expr_kind::apply(library, &mut type_environment).unwrap();
        let mut function_environment = FunctionEnvironmentBuilder::new()
            .with_stdlib_functions()
            .build();
        let mut symbol_environment = SymbolEnvironment::new();
        let library = xform_resolve_symbol_and_function_environment::apply(
            library,
            &mut symbol_environment,
            &mut function_environment,
        )
        .unwrap();
        apply(
            library,
            &mut type_environment,
            &function_environment,
            options,
        )
        .unwrap()
    }

    /// Helper visitor to collect resolved types from assignment RHS expressions.
    struct ResolvedTypeCollector {
        types: Vec<Option<ironplc_dsl::common::TypeName>>,
    }

    impl ResolvedTypeCollector {
        fn new() -> Self {
            Self { types: vec![] }
        }
    }

    impl Fold<()> for ResolvedTypeCollector {
        fn fold_assignment(&mut self, node: Assignment) -> Result<Assignment, ()> {
            self.types.push(node.value.resolved_type.clone());
            node.recurse_fold(self)
        }
    }

    /// Collects the resolved_type from the top-level assignment expressions.
    fn collect_assignment_types(library: &Library) -> Vec<Option<ironplc_dsl::common::TypeName>> {
        let mut collector = ResolvedTypeCollector::new();
        let _ = collector.fold_library(library.clone());
        collector.types
    }

    /// Collects resolved_type from every Expr node in the tree.
    struct AllExprTypeCollector {
        types: Vec<Option<ironplc_dsl::common::TypeName>>,
    }

    impl AllExprTypeCollector {
        fn new() -> Self {
            Self { types: vec![] }
        }
    }

    impl Fold<()> for AllExprTypeCollector {
        fn fold_expr(&mut self, node: Expr) -> Result<Expr, ()> {
            self.types.push(node.resolved_type.clone());
            node.recurse_fold(self)
        }
    }

    fn collect_all_expr_types(library: &Library) -> Vec<Option<ironplc_dsl::common::TypeName>> {
        let mut collector = AllExprTypeCollector::new();
        let _ = collector.fold_library(library.clone());
        collector.types
    }

    /// Returns the resolved type name as an uppercase &str for comparison.
    /// TypeEnvironment stores elementary types in lowercase, but IEC 61131-3
    /// type names are case-insensitive, so we normalize to uppercase for assertions.
    fn type_name_upper(tn: &Option<ironplc_dsl::common::TypeName>) -> Option<String> {
        tn.as_ref().map(|t| t.name.original().to_uppercase())
    }

    fn assert_type_eq(tn: &Option<ironplc_dsl::common::TypeName>, expected: &str) {
        assert_eq!(
            type_name_upper(tn),
            Some(expected.to_string()),
            "Expected type {expected}"
        );
    }

    #[test]
    fn apply_when_nested_arithmetic_then_all_subexprs_resolve() {
        // (x + y) * z — the inner (x + y) and outer multiply should all resolve to INT
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
    z : INT;
    result : INT;
END_VAR
    result := (x + y) * z;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_all_expr_types(&result);
        // Every expression node in (x + y) * z should resolve to INT:
        // nodes: result:=(expr), (x+y)*z, (x+y), x, y, z — all INT
        for (i, t) in types.iter().enumerate() {
            assert!(
                t.is_some(),
                "Expression node {i} should have a resolved type, got None"
            );
            assert_eq!(
                type_name_upper(t),
                Some("INT".to_string()),
                "Expression node {i} should be INT"
            );
        }
    }

    #[test]
    fn apply_when_nested_comparison_with_arithmetic_then_resolves_correctly() {
        // (x + y) > z — inner (x + y) is INT, the comparison is BOOL
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
    z : INT;
    flag : BOOL;
END_VAR
    flag := (x + y) > z;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let top_types = collect_assignment_types(&result);
        assert_eq!(top_types.len(), 1);
        assert_type_eq(&top_types[0], "BOOL");

        // Also verify inner nodes
        let all_types = collect_all_expr_types(&result);
        // The top-level expr is BOOL (comparison), but inner operands should be INT
        let has_bool = all_types
            .iter()
            .any(|t| type_name_upper(t) == Some("BOOL".to_string()));
        let has_int = all_types
            .iter()
            .any(|t| type_name_upper(t) == Some("INT".to_string()));
        assert!(has_bool, "Should have BOOL from comparison");
        assert!(has_int, "Should have INT from arithmetic operands");
    }

    #[test]
    fn apply_when_negated_nested_expr_then_resolves_type() {
        // -(x + y) — unary negation of a parenthesized binary op
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
    result : INT;
END_VAR
    result := -(x + y);
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let top_types = collect_assignment_types(&result);
        assert_eq!(top_types.len(), 1);
        assert_type_eq(&top_types[0], "INT");

        let all_types = collect_all_expr_types(&result);
        // All nodes (unary, parenthesized, binary, x, y) should be INT
        for (i, t) in all_types.iter().enumerate() {
            assert!(
                t.is_some(),
                "Expression node {i} should have a resolved type"
            );
            assert_eq!(
                type_name_upper(t),
                Some("INT".to_string()),
                "Expression node {i} should be INT"
            );
        }
    }

    #[test]
    fn apply_when_deeply_nested_parens_then_resolves_type() {
        // ((x)) — multiple levels of parenthesization
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := ((x));
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let top_types = collect_assignment_types(&result);
        assert_eq!(top_types.len(), 1);
        assert_type_eq(&top_types[0], "INT");

        let all_types = collect_all_expr_types(&result);
        for (i, t) in all_types.iter().enumerate() {
            assert_eq!(
                type_name_upper(t),
                Some("INT".to_string()),
                "Expression node {i} should be INT"
            );
        }
    }

    #[test]
    fn apply_when_multiple_assignments_then_each_resolves_independently() {
        let program = "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : BOOL;
    a : INT;
    b : BOOL;
END_VAR
    a := x;
    b := y;
END_FUNCTION_BLOCK";

        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 2);
        assert_type_eq(&types[0], "INT");
        assert_type_eq(&types[1], "BOOL");
    }

    #[test]
    fn apply_when_function_return_var_used_in_builtin_then_resolves_type() {
        let program = "
FUNCTION FOO : INT
  VAR_INPUT
    A : INT;
  END_VAR
  FOO := 8;
  FOO := SHR(FOO, 1);
END_FUNCTION

PROGRAM main
  VAR
    result : INT;
  END_VAR
  result := FOO(A := 5);
END_PROGRAM";

        let result = run_pass(program);
        let all_types = collect_all_expr_types(&result);
        // Every expression node should have a resolved type (no None values)
        for (i, t) in all_types.iter().enumerate() {
            assert!(
                t.is_some(),
                "Expression node {i} should have a resolved type, got None"
            );
        }
    }

    #[test]
    fn apply_when_ref_to_inline_array_deref_subscript_then_resolves_element_type() {
        let options = CompilerOptions::from_dialect(Dialect::Rusty);
        let program = "
FUNCTION GET_CHAR_BYTE : BYTE
VAR_INPUT
    pt : REF_TO ARRAY[1..255] OF BYTE;
    pos : INT;
END_VAR
    GET_CHAR_BYTE := pt^[pos];
END_FUNCTION

PROGRAM main
VAR
    result : BYTE;
END_VAR
    result := GET_CHAR_BYTE(pt := NULL, pos := 1);
END_PROGRAM";

        let result = run_pass_with_options(program, &options);
        let types = collect_assignment_types(&result);
        // First assignment: GET_CHAR_BYTE := pt^[pos] — should resolve to BYTE
        assert_type_eq(&types[0], "BYTE");
    }

    /// Parameterized tests for the "single assignment, resolves to type T" shape.
    ///
    /// Each case is a complete IEC 61131-3 program containing exactly one
    /// top-level assignment; the test runs the expression-type-resolution
    /// pipeline and asserts the RHS resolves to the expected type name.
    /// This replaces 27 near-identical hand-written tests.
    #[rstest]
    #[case::simple_int_var(
        "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := x;
END_FUNCTION_BLOCK",
        "INT"
    )]
    #[case::type_alias_to_elementary(
        "
TYPE
    MyByte : BYTE := 0;
END_TYPE

FUNCTION_BLOCK FB_TEST
VAR
    x : MyByte;
    y : BYTE;
END_VAR
    y := x;
END_FUNCTION_BLOCK",
        "BYTE"
    )]
    #[case::bool_literal(
        "
FUNCTION_BLOCK FB_TEST
VAR
    y : BOOL;
END_VAR
    y := TRUE;
END_FUNCTION_BLOCK",
        "BOOL"
    )]
    #[case::typed_integer_literal(
        "
FUNCTION_BLOCK FB_TEST
VAR
    y : INT;
END_VAR
    y := INT#42;
END_FUNCTION_BLOCK",
        "INT"
    )]
    #[case::comparison_resolves_bool(
        "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : BOOL;
END_VAR
    y := x > 0;
END_FUNCTION_BLOCK",
        "BOOL"
    )]
    #[case::binary_op_inherits_operand_type(
        "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := x + x;
END_FUNCTION_BLOCK",
        "INT"
    )]
    #[case::unary_op_inherits_operand_type(
        "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := -x;
END_FUNCTION_BLOCK",
        "INT"
    )]
    // ABS has generic return type ANY_NUM; should resolve to concrete input type.
    #[case::function_call_resolves_return_type(
        "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : INT;
END_VAR
    y := ABS(x);
END_FUNCTION_BLOCK",
        "INT"
    )]
    // SHR has generic return type ANY_BIT; ABS(a) resolves to DINT,
    // so the outer SHR should also resolve to DINT.
    #[case::nested_function_call_resolves_concrete_type(
        "
PROGRAM test
  VAR
    a : DINT;
    result : DINT;
  END_VAR
    result := SHR(ABS(a), 1);
END_PROGRAM",
        "DINT"
    )]
    // The RHS expression type reflects the expression itself, not the target.
    // Here TRUE is BOOL even though the target y is INT.
    #[case::bool_assigned_to_int_var_rhs_resolves_bool(
        "
FUNCTION_BLOCK FB_TEST
VAR
    y : INT;
END_VAR
    y := TRUE;
END_FUNCTION_BLOCK",
        "BOOL"
    )]
    // The expression type is determined by the expression, not the target.
    #[case::int_assigned_to_bool_var_rhs_resolves_int(
        "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    y : BOOL;
END_VAR
    y := x;
END_FUNCTION_BLOCK",
        "INT"
    )]
    // In x + y where x is DINT and y is INT, the result inherits the left operand type.
    #[case::mixed_type_binary_op_inherits_left_operand_type(
        "
FUNCTION_BLOCK FB_TEST
VAR
    x : DINT;
    y : INT;
    result : DINT;
END_VAR
    result := x + y;
END_FUNCTION_BLOCK",
        "DINT"
    )]
    // In 5 + x where x is INT, the result should be INT (concrete wins over ANY_INT).
    #[case::binary_op_literal_plus_concrete(
        "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    result : INT;
END_VAR
    result := 5 + x;
END_FUNCTION_BLOCK",
        "INT"
    )]
    // In x + 5 where x is INT, the result should be INT (left is concrete).
    #[case::binary_op_concrete_plus_literal(
        "
FUNCTION_BLOCK FB_TEST
VAR
    x : INT;
    result : INT;
END_VAR
    result := x + 5;
END_FUNCTION_BLOCK",
        "INT"
    )]
    // In 5 + 10, both are ANY_INT, so the result is ANY_INT.
    #[case::binary_op_two_literals_resolves_any_int(
        "
FUNCTION_BLOCK FB_TEST
VAR
    result : DINT;
END_VAR
    result := 5 + 10;
END_FUNCTION_BLOCK",
        "ANY_INT"
    )]
    #[case::real_literal_without_type_resolves_any_real(
        "
FUNCTION_BLOCK FB_TEST
VAR
    y : REAL;
END_VAR
    y := 3.14;
END_FUNCTION_BLOCK",
        "ANY_REAL"
    )]
    // A subrange variable like INT(-100..100) should resolve to INT.
    #[case::subrange_var_resolves_base_type(
        "
FUNCTION_BLOCK FB_TEST
VAR_IN_OUT
    x : INT(-100..100);
END_VAR
VAR
    y : INT;
END_VAR
    y := x;
END_FUNCTION_BLOCK",
        "INT"
    )]
    #[case::string_literal(
        "
FUNCTION_BLOCK FB_TEST
VAR
    s : STRING;
END_VAR
    s := 'hello';
END_FUNCTION_BLOCK",
        "STRING"
    )]
    #[case::untyped_integer_literal_resolves_any_int(
        "
FUNCTION_BLOCK FB_TEST
VAR
    y : DINT;
END_VAR
    y := 42;
END_FUNCTION_BLOCK",
        "ANY_INT"
    )]
    #[case::time_literal(
        "
FUNCTION_BLOCK FB_TEST
VAR
    t : TIME;
END_VAR
    t := T#5s;
END_FUNCTION_BLOCK",
        "TIME"
    )]
    #[case::sel_resolves_value_type_not_selector(
        "
PROGRAM test
  VAR
    g : BOOL;
    a : INT;
    b : INT;
    result : INT;
  END_VAR
    result := SEL(g, a, b);
END_PROGRAM",
        "INT"
    )]
    #[case::mux_resolves_value_type_not_selector(
        "
PROGRAM test
  VAR
    k : INT;
    a : DINT;
    b : DINT;
    result : DINT;
  END_VAR
    result := MUX(k, a, b);
END_PROGRAM",
        "DINT"
    )]
    #[case::sel_nested_in_function(
        "
PROGRAM test
  VAR
    g : BOOL;
    a : INT;
    b : INT;
    result : INT;
  END_VAR
    result := ABS(SEL(g, a, b));
END_PROGRAM",
        "INT"
    )]
    #[case::named_array_subscript(
        "
TYPE MyArr : ARRAY[0..10] OF INT; END_TYPE

FUNCTION_BLOCK FB_TEST
VAR
    arr : MyArr;
    result : INT;
END_VAR
    result := arr[0];
END_FUNCTION_BLOCK",
        "INT"
    )]
    #[case::inline_array_subscript(
        "
FUNCTION_BLOCK FB_TEST
VAR
    arr : ARRAY[0..10] OF DINT;
    result : DINT;
END_VAR
    result := arr[0];
END_FUNCTION_BLOCK",
        "DINT"
    )]
    // Regression for the `compile_expr.rs#L32` TODO that fired when
    // `struct.field[i, j]` was used in a STRING comparison: the analyzer
    // previously left `resolved_type` unset for array subscripts whose
    // base was a struct field.
    #[case::struct_field_2d_string_array_subscript(
        "
TYPE MY_DATA : STRUCT
    DIRS : ARRAY[0..2, 0..15] OF STRING[3];
END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB_TEST
VAR
    data : MY_DATA;
    i : INT;
    j : INT;
    result : STRING[3];
END_VAR
    result := data.DIRS[i, j];
END_FUNCTION_BLOCK",
        "STRING"
    )]
    // Same fix must also cover numeric array fields, not just strings.
    #[case::struct_field_int_array_subscript(
        "
TYPE MY_DATA : STRUCT
    values : ARRAY[0..9] OF DINT;
END_STRUCT;
END_TYPE

FUNCTION_BLOCK FB_TEST
VAR
    data : MY_DATA;
    i : INT;
    result : DINT;
END_VAR
    result := data.values[i];
END_FUNCTION_BLOCK",
        "DINT"
    )]
    fn apply_when_single_assignment_then_resolves_expected_type(
        #[case] program: &str,
        #[case] expected: &str,
    ) {
        let result = run_pass(program);
        let types = collect_assignment_types(&result);
        assert_eq!(types.len(), 1);
        assert_type_eq(&types[0], expected);
    }
}
