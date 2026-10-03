use indexmap::IndexMap;
use ironplc_dsl::common::{DeclarationQualifier, TypeName, VariableType};
use ironplc_dsl::core::{Id, Located};
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::scope::ScopeNode;
use ironplc_dsl::textual::SelfRefKind;
use ironplc_dsl::type_id::TypeId;
use ironplc_problems::Problem;

/// A scope's position in the nesting tree: the chain of declaration
/// names from the library root.
///
/// A function block is `⟨FB_Motor⟩`; a method declared on it is
/// `⟨FB_Motor, GetSpeed⟩`. Scopes are nameable, which is why this is a
/// path of names rather than an id assigned to an AST node.
///
/// Never empty: an empty path would be the global scope, which
/// [`ScopeKind::Global`] already represents.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScopePath(Vec<Id>);

impl ScopePath {
    pub fn new(segments: Vec<Id>) -> Self {
        debug_assert!(
            !segments.is_empty(),
            "a scope path is never empty; the empty scope is ScopeKind::Global"
        );
        Self(segments)
    }

    pub fn segments(&self) -> &[Id] {
        &self.0
    }
}

impl From<Id> for ScopePath {
    /// A scope directly inside the library, such as a function block.
    fn from(name: Id) -> Self {
        Self::new(vec![name])
    }
}

/// Tracks the scope a traversal is in, from the `enter_scope` and
/// `exit_scope` hooks, so that a pass can look names up in the
/// [`SymbolEnvironment`] from where it is.
#[derive(Debug, Default)]
pub(crate) struct ScopeTracker {
    /// The chain of declaration names the traversal is inside, outermost
    /// first.
    path: Vec<Id>,
}

impl ScopeTracker {
    /// The name `node`'s scope is known by in a [`ScopePath`]. Every pass
    /// that keys symbols by scope tracks scopes with this type, so they all
    /// name a scope the same way.
    fn name_of(node: &ScopeNode<'_>) -> Id {
        match node {
            ScopeNode::Function(node) => node.name.clone(),
            ScopeNode::FunctionBlock(node) => node.name.name.clone(),
            ScopeNode::Program(node) => node.name.clone(),
            ScopeNode::Method(node) => node.name.clone(),
        }
    }

    /// Enters the scope `node` opens.
    pub(crate) fn enter(&mut self, node: &ScopeNode<'_>) {
        self.path.push(Self::name_of(node));
    }

    /// Leaves the innermost scope.
    pub(crate) fn exit(&mut self) {
        self.path.pop();
    }

    /// The scope the traversal is in.
    pub(crate) fn current(&self) -> ScopeKind {
        if self.path.is_empty() {
            ScopeKind::Global
        } else {
            ScopeKind::Named(ScopePath::new(self.path.clone()))
        }
    }

    /// The scope `node` opens when the traversal enters it from the
    /// current scope, for a pass that declares into it before entering.
    pub(crate) fn scope_of(&self, node: &ScopeNode<'_>) -> ScopeKind {
        let mut path = self.path.clone();
        path.push(Self::name_of(node));
        ScopeKind::Named(ScopePath::new(path))
    }

    /// The outermost declaration the traversal is inside, such as the
    /// function block a method belongs to, or `None` outside every
    /// declaration.
    pub(crate) fn unit(&self) -> Option<&Id> {
        self.path.first()
    }
}

/// Represents the kind of scope a symbol belongs to
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    /// Global scope (library level)
    Global,
    /// Named scope (function, function block, program, method, etc.)
    Named(ScopePath),
}

/// Represents the kind of symbol
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    /// Variable declaration
    Variable,
    /// Function parameter (input)
    Parameter,
    /// Function parameter (output)
    OutputParameter,
    /// Function parameter (input/output)
    InOutParameter,
    /// Function block declaration
    FunctionBlock,
    /// Program declaration
    Program,
    /// Configuration declaration
    Configuration,
    /// Type declaration
    Type,
    /// Constant declaration
    #[allow(unused)]
    Constant,
    /// Structure element
    StructureElement,
    /// Edge variable (rising/falling edge)
    EdgeVariable,
    /// The implicit result variable of a function, or of a method that
    /// declares a return type: the declaration's own name, assigned by its
    /// body (`F := ...` inside `FUNCTION F`). A declared variable of the
    /// same name replaces it.
    ResultVariable,
}

/// Metadata associated with a symbol
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    /// The kind of symbol
    pub kind: SymbolKind,
    /// The scope where this symbol is declared
    pub scope: ScopeKind,
    /// The scope where this symbol is visible (for scoping rules)
    pub visibility_scope: ScopeKind,
    /// Whether this symbol is a reference to an external declaration
    pub is_external: bool,
    /// For variables, the id of the declared type in the type
    /// environment. `None` for symbols that are not variables and for a
    /// variable whose type the analyzer could not resolve.
    pub type_id: Option<TypeId>,
    /// For structure fields, the type name of the structure
    pub struct_type: Option<TypeName>,
    /// The variable type qualifier (VAR, VAR_INPUT, VAR_OUTPUT, etc.)
    pub variable_type: Option<VariableType>,
    /// The qualifier the declaration was written with (CONSTANT, RETAIN,
    /// etc.). Recorded before the compiler infers constants, so a variable
    /// that is never written but not declared `CONSTANT` is not constant here.
    pub qualifier: Option<DeclarationQualifier>,
    /// Formatted hardware address (e.g. "%IX0.0") for direct variables
    pub address: Option<String>,
    /// Source location information
    pub span: ironplc_dsl::core::SourceSpan,
    /// Declared by the compiler rather than by source, such as the implicit
    /// uptime globals. There is no source location to point at, and a user
    /// declaration of the name is reported as reserved.
    pub compiler_provided: bool,
    /// For a function block, whether it is declared `ABSTRACT`: it exists
    /// only to be extended and cannot be instantiated.
    pub is_abstract: bool,
    /// For a function block, the function block it `EXTENDS`, whose
    /// fields its own scope sees as well.
    pub extends: Option<TypeName>,
}

impl SymbolInfo {
    pub fn new(kind: SymbolKind, scope: ScopeKind, span: ironplc_dsl::core::SourceSpan) -> Self {
        Self {
            kind,
            scope: scope.clone(),
            visibility_scope: scope,
            is_external: false,
            type_id: None,
            struct_type: None,
            variable_type: None,
            qualifier: None,
            address: None,
            span,
            compiler_provided: false,
            is_abstract: false,
            extends: None,
        }
    }

    fn with_compiler_provided(mut self) -> Self {
        self.compiler_provided = true;
        self
    }

    /// Set the function block a function block `EXTENDS`
    pub fn with_extends(mut self, extends: Option<TypeName>) -> Self {
        self.extends = extends;
        self
    }

    /// Set whether a function block is declared `ABSTRACT`
    pub fn with_abstract(mut self, is_abstract: bool) -> Self {
        self.is_abstract = is_abstract;
        self
    }

    pub fn with_external(mut self, is_external: bool) -> Self {
        self.is_external = is_external;
        self
    }

    /// Set the structure type for structure field symbols
    pub fn with_struct_type(mut self, struct_type: TypeName) -> Self {
        self.struct_type = Some(struct_type);
        self
    }

    /// Set the id of the variable's declared type
    pub fn with_type_id(mut self, type_id: Option<TypeId>) -> Self {
        self.type_id = type_id;
        self
    }

    pub fn with_variable_type(mut self, vt: VariableType) -> Self {
        self.variable_type = Some(vt);
        self
    }

    pub fn with_address(mut self, addr: String) -> Self {
        self.address = Some(addr);
        self
    }

    pub fn with_qualifier(mut self, qualifier: DeclarationQualifier) -> Self {
        self.qualifier = Some(qualifier);
        self
    }

    /// Whether the variable was declared `CONSTANT`.
    pub fn is_constant(&self) -> bool {
        self.qualifier == Some(DeclarationQualifier::Constant)
    }
}

/// Whether `kind` is a variable: the kinds a `VAR*` block declares.
fn is_variable(kind: &SymbolKind) -> bool {
    matches!(
        kind,
        SymbolKind::Variable
            | SymbolKind::Parameter
            | SymbolKind::OutputParameter
            | SymbolKind::InOutParameter
            | SymbolKind::EdgeVariable
            | SymbolKind::Constant
    )
}

/// The problem a second declaration of a name in one scope is: `P4014` for
/// a variable repeating a variable, otherwise whatever a repeated global
/// declaration is.
fn repeated_symbol(existing: &SymbolKind, repeat: &SymbolKind) -> Option<Problem> {
    if is_variable(existing) && is_variable(repeat) {
        return Some(Problem::SymbolDeclDuplicated);
    }
    repeated_declaration(existing, repeat)
}

/// The diagnostic for a source declaration of a name the compiler provides.
fn reserved_name(name: &Id) -> Diagnostic {
    Diagnostic::problem(
        Problem::SymbolDeclDuplicated,
        Label::span(
            name.span(),
            "Variable name is reserved for a compiler-provided global",
        ),
    )
    .with_context_id("name", name)
    .with_help(
        "The compiler declares this global when --allow-system-uptime-global is on. \
         Remove the declaration to read the compiler's value, or rename the variable.",
    )
}

/// The problem a second global declaration of a name is, when `existing`
/// and `repeat` are both declarations and at least one is a program or a
/// configuration. A pair of types or function blocks is the type
/// environment's to report. The global scope also holds enumeration values,
/// structure elements and global variables, whose uniqueness other rules
/// own, so a pair involving one of those is `None`.
fn repeated_declaration(existing: &SymbolKind, repeat: &SymbolKind) -> Option<Problem> {
    let is_declaration = |kind: &SymbolKind| {
        matches!(
            kind,
            SymbolKind::Type
                | SymbolKind::FunctionBlock
                | SymbolKind::Program
                | SymbolKind::Configuration
        )
    };
    let is_unit =
        |kind: &SymbolKind| matches!(kind, SymbolKind::Program | SymbolKind::Configuration);
    if !is_declaration(existing) || !is_declaration(repeat) {
        return None;
    }
    if !is_unit(existing) && !is_unit(repeat) {
        return None;
    }
    Some(Problem::PouDeclNameDuplicated)
}

/// The diagnostic for `name` declared again at its own span when `first`
/// already declares it. Shared with the transform that feeds this
/// environment, which reports a function repeating a symbol the same way.
pub(crate) fn duplicate_declaration(
    problem: Problem,
    name: &Id,
    first: ironplc_dsl::core::SourceSpan,
) -> Diagnostic {
    Diagnostic::problem(
        problem,
        Label::span(name.span(), "Declaration repeats an earlier name"),
    )
    .with_context_id("name", name)
    .with_secondary(Label::span(first, "First declaration"))
}

/// The main symbol environment that tracks all symbols across the library.
///
/// Symbols are kept in insertion order so that every accessor that returns
/// a list reports symbols in declaration order, run after run.
pub struct SymbolEnvironment {
    /// Global symbols (types, functions, function blocks, programs)
    global_symbols: IndexMap<Id, SymbolInfo>,
    /// Scoped symbols (variables within functions, function blocks, etc.)
    scoped_symbols: IndexMap<ScopeKind, IndexMap<Id, SymbolInfo>>,
    /// The values of each enumeration type, in declaration order.
    ///
    /// Kept apart from the named symbols: an enumeration value is always
    /// read through its type (`Colors#Red`), and two enumerations, or an
    /// enumeration and a variable, may use the same name.
    enumeration_values: IndexMap<TypeName, Vec<Id>>,
}

impl SymbolEnvironment {
    pub fn new() -> Self {
        Self {
            global_symbols: IndexMap::new(),
            scoped_symbols: IndexMap::new(),
            enumeration_values: IndexMap::new(),
        }
    }

    /// Insert a symbol into the environment.
    ///
    /// A name declared twice in one scope is returned as a diagnostic and
    /// the first declaration is kept, so analysis continues on it: `P4014`
    /// for a variable repeating a variable in any scope, `P4013` for a
    /// program or configuration repeating a global declaration. The type
    /// environment owns the same check for the kinds it holds (data types,
    /// function blocks, interfaces), so a pair of those is not reported
    /// again here. Every other pair is recorded without a uniqueness check,
    /// the later declaration replacing the earlier one.
    pub fn insert(
        &mut self,
        name: &Id,
        kind: SymbolKind,
        scope: &ScopeKind,
    ) -> Result<(), Diagnostic> {
        self.insert_symbol(name, SymbolInfo::new(kind, scope.clone(), name.span()))
    }

    /// Insert a symbol the compiler declares, such as an implicit global.
    ///
    /// A later source declaration of the name is reported as reserved rather
    /// than as a repeat of a declaration the user could go and look at.
    pub fn insert_compiler_provided(
        &mut self,
        name: &Id,
        kind: SymbolKind,
        scope: &ScopeKind,
        type_id: Option<TypeId>,
    ) -> Result<(), Diagnostic> {
        self.insert_symbol(
            name,
            SymbolInfo::new(kind, scope.clone(), name.span())
                .with_type_id(type_id)
                .with_compiler_provided(),
        )
    }

    /// Insert a variable with direction, declaration qualifier, the id of
    /// its declared type and optional hardware address. The symbol's kind
    /// follows from the section the variable is declared in.
    ///
    /// A name already declared in the scope is returned as `P4014`, as for
    /// [`Self::insert`].
    pub fn insert_variable(
        &mut self,
        name: &Id,
        scope: &ScopeKind,
        variable_type: VariableType,
        qualifier: DeclarationQualifier,
        type_id: Option<TypeId>,
        address: Option<String>,
    ) -> Result<(), Diagnostic> {
        let kind = match variable_type {
            VariableType::Input => SymbolKind::Parameter,
            VariableType::Output => SymbolKind::OutputParameter,
            VariableType::InOut => SymbolKind::InOutParameter,
            _ => SymbolKind::Variable,
        };
        let mut symbol_info = SymbolInfo::new(kind, scope.clone(), name.span())
            .with_variable_type(variable_type.clone())
            .with_qualifier(qualifier)
            .with_type_id(type_id);
        if let Some(addr) = address {
            symbol_info = symbol_info.with_address(addr);
        }
        if variable_type == VariableType::External {
            symbol_info = symbol_info.with_external(true);
        }
        self.insert_symbol(name, symbol_info)
    }

    /// Insert a symbol described by `info`, in `info`'s scope.
    ///
    /// A repeated name is reported as for [`Self::insert`].
    pub fn insert_info(&mut self, name: &Id, info: SymbolInfo) -> Result<(), Diagnostic> {
        self.insert_symbol(name, info)
    }

    /// The one insertion path: checks the scope for a repeated name, then
    /// records the symbol in that scope.
    ///
    /// The global scope has its own map; [`Self::symbols_in`] is the read
    /// side of this choice.
    fn insert_symbol(&mut self, name: &Id, info: SymbolInfo) -> Result<(), Diagnostic> {
        let symbols = match &info.scope {
            ScopeKind::Global => &mut self.global_symbols,
            ScopeKind::Named(_) => self.scoped_symbols.entry(info.scope.clone()).or_default(),
        };
        if let Some(existing) = symbols.get(name) {
            if existing.compiler_provided && is_variable(&info.kind) {
                return Err(reserved_name(name));
            }
            if let Some(problem) = repeated_symbol(&existing.kind, &info.kind) {
                return Err(duplicate_declaration(problem, name, existing.span.clone()));
            }
        }
        symbols.insert(name.clone(), info);
        Ok(())
    }

    /// Records `name` as a value of the enumeration type `enum_type`.
    pub fn insert_enumeration_value(&mut self, name: &Id, enum_type: &TypeName) {
        self.enumeration_values
            .entry(enum_type.clone())
            .or_default()
            .push(name.clone());
    }

    /// Insert a structure field with its type information
    pub fn insert_structure_field(
        &mut self,
        name: &Id,
        struct_type: &TypeName,
        scope: &ScopeKind,
    ) -> Result<(), Diagnostic> {
        let symbol_info = SymbolInfo::new(SymbolKind::StructureElement, scope.clone(), name.span())
            .with_struct_type(struct_type.clone());

        match scope {
            ScopeKind::Global => {
                self.global_symbols.insert(name.clone(), symbol_info);
            }
            ScopeKind::Named(_) => {
                let scope_symbols = self.scoped_symbols.entry(scope.clone()).or_default();

                scope_symbols.insert(name.clone(), symbol_info);
            }
        }

        Ok(())
    }

    /// Duplicate enumeration values from one type to another (for aliases)
    pub fn duplicate_enumeration_values_for_alias(
        &mut self,
        source_type: &TypeName,
        alias_type: &TypeName,
    ) -> Result<(), Diagnostic> {
        if let Some(values) = self.enumeration_values.get(source_type).cloned() {
            self.enumeration_values
                .entry(alias_type.clone())
                .or_default()
                .extend(values);
        }
        Ok(())
    }

    /// Duplicate structure field symbols from one type to another (for aliases)
    pub fn duplicate_structure_fields_for_alias(
        &mut self,
        source_type: &TypeName,
        alias_type: &TypeName,
    ) -> Result<(), Diagnostic> {
        // Find all structure field symbols for the source type and collect them
        let source_fields: Vec<Id> = self
            .get_structure_fields_for_type(source_type)
            .iter()
            .map(|id| (*id).clone())
            .collect();

        // Duplicate each field with the alias type
        for field_name in source_fields {
            self.insert_structure_field(&field_name, alias_type, &ScopeKind::Global)?;
        }

        Ok(())
    }

    /// Duplicate array element type information from one type to another (for aliases)
    pub fn duplicate_array_elements_for_alias(
        &mut self,
        _source_type: &TypeName,
        _alias_type: &TypeName,
    ) -> Result<(), Diagnostic> {
        // For arrays, we don't need to duplicate symbols like we do for enumerations
        // and structures, since arrays don't have named elements that need to be
        // accessible through the alias. The array type itself is what gets aliased.
        // Array elements are accessed by index, not by name.
        Ok(())
    }

    /// Finds a symbol visible from the given scope.
    ///
    /// Walks outward through the enclosing scopes and then the global
    /// scope, so a method body sees its function block's fields and an
    /// inner declaration shadows an outer one of the same name. A
    /// function block's scope is followed by those of the function blocks
    /// it `EXTENDS`, nearest first, so a derived block and its methods see
    /// the fields it inherits.
    pub fn find(&self, name: &Id, scope: &ScopeKind) -> Option<&SymbolInfo> {
        self.visible_scopes(scope)
            .iter()
            .find_map(|scope| self.symbols_in(scope)?.get(name))
    }

    /// The function block `THIS^` or `SUPER^` names from `scope`.
    ///
    /// Inside a function block's body, its methods and its property
    /// accessors, `THIS^` names that block and `SUPER^` the block it
    /// `EXTENDS`. `None` outside a function block (in a program or a
    /// function), and for `SUPER^` in a block that extends nothing.
    pub fn self_type(&self, scope: &ScopeKind, kind: SelfRefKind) -> Option<TypeName> {
        let ScopeKind::Named(path) = scope else {
            return None;
        };
        let unit = &path.segments()[0];
        let block = self.function_block(unit)?;
        match kind {
            SelfRefKind::This => Some(TypeName::from_id(unit)),
            SelfRefKind::Super => block.extends.clone(),
        }
    }

    /// The symbol of the function block named `name`, or `None` when
    /// `name` is not a function block.
    fn function_block(&self, name: &Id) -> Option<&SymbolInfo> {
        self.global_symbols
            .get(name)
            .filter(|info| info.kind == SymbolKind::FunctionBlock)
    }

    /// The scopes a name is looked up in from `scope`, innermost first:
    /// each enclosing scope, the scopes of the function blocks the
    /// outermost one `EXTENDS`, then the global scope.
    fn visible_scopes(&self, scope: &ScopeKind) -> Vec<ScopeKind> {
        let mut scopes = Vec::new();
        if let ScopeKind::Named(path) = scope {
            let segments = path.segments();
            for depth in (1..=segments.len()).rev() {
                scopes.push(ScopeKind::Named(ScopePath::new(segments[..depth].to_vec())));
            }
            // The base chain. The analyzer rejects a cycle in it, but a
            // name seen twice still ends the walk rather than looping.
            let mut seen = vec![segments[0].clone()];
            let mut unit = &segments[0];
            while let Some(base) = self
                .function_block(unit)
                .and_then(|info| info.extends.as_ref())
            {
                if seen.contains(&base.name) {
                    break;
                }
                seen.push(base.name.clone());
                scopes.push(ScopeKind::Named(ScopePath::new(vec![base.name.clone()])));
                unit = &base.name;
            }
        }
        scopes.push(ScopeKind::Global);
        scopes
    }

    /// The variables visible from `scope`, as [`Self::find`] would see
    /// them: innermost scope first, each name listed once at its nearest
    /// declaration. Result variables count as variables.
    pub fn visible_variables(&self, scope: &ScopeKind) -> Vec<(&Id, &SymbolInfo)> {
        let mut visible: IndexMap<&Id, &SymbolInfo> = IndexMap::new();
        for scope in self.visible_scopes(scope) {
            let Some(symbols) = self.symbols_in(&scope) else {
                continue;
            };
            for (name, info) in symbols {
                if is_variable(&info.kind) || info.kind == SymbolKind::ResultVariable {
                    visible.entry(name).or_insert(info);
                }
            }
        }
        visible.into_iter().collect()
    }

    /// Get a symbol by name and scope (alias for find)
    #[allow(dead_code)]
    pub fn get(&self, name: &Id, scope: &ScopeKind) -> Option<&SymbolInfo> {
        self.find(name, scope)
    }

    /// Returns all program declarations from the global scope.
    pub fn get_programs(&self) -> Vec<(&Id, &SymbolInfo)> {
        self.global_symbols
            .iter()
            .filter(|(_, info)| info.kind == SymbolKind::Program)
            .collect()
    }

    /// Returns all function block declarations from the global scope.
    pub fn get_function_blocks(&self) -> Vec<(&Id, &SymbolInfo)> {
        self.global_symbols
            .iter()
            .filter(|(_, info)| info.kind == SymbolKind::FunctionBlock)
            .collect()
    }

    /// The symbols declared directly in `scope`, or `None` when nothing is.
    ///
    /// The global scope lives in its own map, as [`Self::insert_symbol`]
    /// stores it; every other scope is keyed in `scoped_symbols`.
    fn symbols_in(&self, scope: &ScopeKind) -> Option<&IndexMap<Id, SymbolInfo>> {
        match scope {
            ScopeKind::Global => Some(&self.global_symbols),
            ScopeKind::Named(_) => self.scoped_symbols.get(scope),
        }
    }

    /// Returns all variable-like symbols in the given scope (variables,
    /// parameters, output parameters, and in-out parameters). For
    /// [`ScopeKind::Global`] these are the global variables, including the
    /// ones the compiler provides.
    pub fn get_variables_in_scope(&self, scope: &ScopeKind) -> Vec<(&Id, &SymbolInfo)> {
        let Some(scope_symbols) = self.symbols_in(scope) else {
            return vec![];
        };
        scope_symbols
            .iter()
            .filter(|(_, info)| {
                matches!(
                    info.kind,
                    SymbolKind::Variable
                        | SymbolKind::Parameter
                        | SymbolKind::OutputParameter
                        | SymbolKind::InOutParameter
                )
            })
            .collect()
    }

    /// Iterate over every symbol in the environment: the global symbols
    /// first, followed by every scoped symbol across all named scopes.
    ///
    /// This is the shared traversal used by the read-only lookups that need
    /// to consider both global and scoped declarations.
    fn all_symbols(&self) -> impl Iterator<Item = (&Id, &SymbolInfo)> {
        self.global_symbols
            .iter()
            .chain(self.scoped_symbols.values().flat_map(|scope| scope.iter()))
    }

    /// Get all enumeration values for a specific enumeration type, in
    /// declaration order.
    pub fn get_enumeration_values_for_type(&self, enum_type: &TypeName) -> Vec<&Id> {
        self.enumeration_values
            .get(enum_type)
            .map(|values| values.iter().collect())
            .unwrap_or_default()
    }

    /// Get all structure fields for a specific structure type
    pub fn get_structure_fields_for_type(&self, struct_type: &TypeName) -> Vec<&Id> {
        self.all_symbols()
            .filter(|(_, symbol)| {
                matches!(symbol.kind, SymbolKind::StructureElement)
                    && symbol.struct_type.as_ref() == Some(struct_type)
            })
            .map(|(name, _)| name)
            .collect()
    }
}

impl Default for SymbolEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SymbolEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SymbolEnvironment")
            .field("global_symbols", &self.global_symbols)
            .field("scoped_symbols", &self.scoped_symbols)
            .finish()
    }
}

#[cfg(test)]
mod tests;
