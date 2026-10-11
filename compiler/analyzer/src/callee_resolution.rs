//! Shared resolution of an invocation's callee.
//!
//! A function-block invocation (`inst(...)`) and a method call
//! (`inst.M(...)`) start the same way: find the declared type of the instance
//! variable, find that function block's declaration, and for a method walk
//! up the `EXTENDS` chain to the block that declares it. Any later analysis
//! that needs to know which declared parameter a call argument binds to --
//! a type check, a write analysis -- needs the same lookups. The rules used
//! to keep private copies of them; this module is the one copy, so they
//! cannot drift.
//!
//! The instance a call names is found through the symbol environment, as any
//! other variable is ([`instance_type`]), and the parameters a callee
//! declares are listed once ([`CallParameters`]). Argument-to-parameter
//! binding for a resolved callee lives beside the parameter-assignment
//! checks in `call_assignment_check`.

use std::collections::{HashMap, HashSet};

use ironplc_dsl::common::{
    FunctionBlockDeclaration, HasVariables, Library, LibraryElementKind, MethodDeclaration,
    TypeName, VariableType,
};
use ironplc_dsl::core::Id;

use crate::semantic_type::{FunctionBlockVarType, SemanticStructField};
use crate::symbol_environment::{Resolution, ScopeKind, SymbolEnvironment};
use crate::type_environment::TypeEnvironment;

/// The function blocks a library declares, by name.
pub(crate) struct FunctionBlocks<'a> {
    by_name: HashMap<TypeName, &'a FunctionBlockDeclaration>,
}

impl<'a> FunctionBlocks<'a> {
    pub(crate) fn from_library(lib: &'a Library) -> Self {
        let by_name = lib
            .elements
            .iter()
            .filter_map(|element| match element {
                LibraryElementKind::FunctionBlockDeclaration(fb) => Some((fb.name.clone(), fb)),
                _ => None,
            })
            .collect();
        FunctionBlocks { by_name }
    }

    /// The declaration of the function block named `name`, if the library
    /// declares one. Standard-library function blocks are not declared in
    /// the library and so are never found here.
    pub(crate) fn get(&self, name: &TypeName) -> Option<&'a FunctionBlockDeclaration> {
        self.by_name.get(name).copied()
    }

    pub(crate) fn contains(&self, name: &TypeName) -> bool {
        self.by_name.contains_key(name)
    }

    /// `fb_name`, then its `EXTENDS` base, then that base's base, and so
    /// on. Stops at a block seen before: a cycle is independently invalid
    /// (and rejected elsewhere), this is just a safety net.
    fn chain(&self, fb_name: &TypeName) -> impl Iterator<Item = &'a FunctionBlockDeclaration> + '_ {
        let mut current = self.get(fb_name);
        let mut visited: HashSet<TypeName> = HashSet::new();
        std::iter::from_fn(move || {
            let fb = current?;
            if !visited.insert(fb.name.clone()) {
                return None;
            }
            current = fb
                .oop
                .as_ref()
                .and_then(|oop| oop.base.as_ref())
                .and_then(|base| self.get(base));
            Some(fb)
        })
    }

    /// Resolves `method_name` against `fb_name`'s own methods, then its
    /// `EXTENDS` base, then that base's base, and so on (ADR-0041 Phase 1
    /// static dispatch). Returns the function block that actually declares
    /// the method (which may be a base, not `fb_name` itself) together
    /// with the method declaration.
    pub(crate) fn resolve_method(
        &self,
        fb_name: &TypeName,
        method_name: &Id,
    ) -> Option<(&'a FunctionBlockDeclaration, &'a MethodDeclaration)> {
        self.chain(fb_name).find_map(|fb| {
            fb.methods
                .iter()
                .find(|m| &m.name == method_name)
                .map(|method| (fb, method))
        })
    }

    /// The block in `fb_name`'s `EXTENDS` chain -- `fb_name` itself first --
    /// that declares the variable `field`, if any does.
    pub(crate) fn declaring_block(
        &self,
        fb_name: &TypeName,
        field: &Id,
    ) -> Option<&'a FunctionBlockDeclaration> {
        self.chain(fb_name).find(|fb| {
            fb.variables
                .iter()
                .any(|decl| decl.identifier.symbolic_id() == Some(field))
        })
    }
}

/// The function block type of the variable `instance`, as a body in `scope`
/// names it: the type its declaration resolved to, user-defined or standard.
///
/// `None` when `instance` names no variable the scope can use, or a variable
/// that is not a function block instance. The variable is the one
/// [`SymbolEnvironment::resolve_variable`] finds, so a local hides a field of
/// the same name, a method sees its block's instances, and a block sees the
/// instances it inherits through `EXTENDS`.
pub(crate) fn instance_type(
    instance: &Id,
    scope: &ScopeKind,
    symbols: &SymbolEnvironment,
    types: &TypeEnvironment,
) -> Option<TypeName> {
    let Resolution::Variable { info, .. } = symbols.resolve_variable(instance, scope) else {
        return None;
    };
    let type_id = info.type_id?;
    if !types.get_by_id(type_id)?.representation.is_function_block() {
        return None;
    }
    types.name_of(type_id).cloned()
}

/// Which kind of parameter a call passes something to or reads from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ParameterSection {
    /// `VAR_INPUT`
    Input,
    /// `VAR_IN_OUT`
    InOut,
    /// `VAR_OUTPUT`
    Output,
}

/// One parameter of a function block or method, as a call sees it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CallParameter {
    /// The name, as the callee declares it.
    pub(crate) name: Id,
    pub(crate) section: ParameterSection,
}

/// The parameters of a function block or method, in declaration order.
///
/// Every analysis that binds a call's arguments asks this list which
/// parameters there are, so they agree on what counts as an input and on the
/// order positional arguments take.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CallParameters {
    parameters: Vec<CallParameter>,
}

impl CallParameters {
    /// The parameters a user-defined function block or method declares.
    pub(crate) fn of_declaration(owner: &dyn HasVariables) -> Self {
        let parameters = owner
            .variables()
            .iter()
            .filter_map(|decl| {
                let section = match decl.var_type {
                    VariableType::Input => ParameterSection::Input,
                    VariableType::InOut => ParameterSection::InOut,
                    VariableType::Output => ParameterSection::Output,
                    _ => return None,
                };
                Some(CallParameter {
                    name: decl.identifier.symbolic_id()?.clone(),
                    section,
                })
            })
            .collect();
        CallParameters { parameters }
    }

    /// The parameters of a standard function block, from the fields of its
    /// type. A standard block's type lists every field it has, which the
    /// type of a user-defined block does not.
    pub(crate) fn of_standard_block(fields: &[SemanticStructField]) -> Self {
        let parameters = fields
            .iter()
            .filter_map(|field| {
                let section = match field.var_type? {
                    FunctionBlockVarType::Input => ParameterSection::Input,
                    FunctionBlockVarType::InOut => ParameterSection::InOut,
                    FunctionBlockVarType::Output => ParameterSection::Output,
                    FunctionBlockVarType::Internal => return None,
                };
                Some(CallParameter {
                    name: field.name.clone(),
                    section,
                })
            })
            .collect();
        CallParameters { parameters }
    }

    /// The `VAR_INPUT` parameters, in declaration order: the parameters a
    /// positional argument of a function block or method call binds to.
    pub(crate) fn inputs(&self) -> impl Iterator<Item = &CallParameter> {
        self.in_section(ParameterSection::Input)
    }

    /// The parameter a named input `name := ...` binds to: a `VAR_INPUT` or
    /// `VAR_IN_OUT` of that name.
    pub(crate) fn named_input(&self, name: &Id) -> Option<&CallParameter> {
        self.parameters.iter().find(|parameter| {
            parameter.name == *name && parameter.section != ParameterSection::Output
        })
    }

    /// The parameter an output assignment `name => ...` reads: a
    /// `VAR_OUTPUT` of that name. A `VAR_IN_OUT` is assigned only through
    /// the input syntax, so it is not one.
    pub(crate) fn output(&self, name: &Id) -> Option<&CallParameter> {
        self.in_section(ParameterSection::Output)
            .find(|parameter| parameter.name == *name)
    }

    fn in_section(&self, section: ParameterSection) -> impl Iterator<Item = &CallParameter> {
        self.parameters
            .iter()
            .filter(move |parameter| parameter.section == section)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_type::SemanticType;
    use crate::symbol_environment::ScopePath;
    use crate::test_helpers::parse_and_resolve_types_with_options;
    use ironplc_dsl::core::FileId;
    use ironplc_parser::{options::CompilerOptions, parse_program};

    fn oop_options() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        }
    }

    /// Parses only: these lookups read declarations as the parser leaves
    /// them, so no type resolution is needed.
    fn parse(program: &str) -> Library {
        parse_program(program, &FileId::default(), &oop_options()).unwrap()
    }

    const HIERARCHY: &str = "
FUNCTION_BLOCK FB_Base
METHOD Start
    ;
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
METHOD Stop
    ;
END_METHOD
END_FUNCTION_BLOCK";

    #[test]
    fn from_library_when_function_blocks_declared_then_finds_each_by_name() {
        let lib = parse(HIERARCHY);
        let fbs = FunctionBlocks::from_library(&lib);
        assert!(fbs.contains(&TypeName::from("FB_Base")));
        assert!(fbs.get(&TypeName::from("FB_Derived")).is_some());
        assert!(fbs.get(&TypeName::from("FB_Missing")).is_none());
    }

    #[test]
    fn resolve_method_when_declared_on_own_block_then_returns_own_block() {
        let lib = parse(HIERARCHY);
        let fbs = FunctionBlocks::from_library(&lib);
        let (owner, method) = fbs
            .resolve_method(&TypeName::from("FB_Derived"), &Id::from("Stop"))
            .unwrap();
        assert_eq!(TypeName::from("FB_Derived"), owner.name);
        assert_eq!(Id::from("Stop"), method.name);
    }

    #[test]
    fn resolve_method_when_declared_on_base_then_returns_base_block() {
        let lib = parse(HIERARCHY);
        let fbs = FunctionBlocks::from_library(&lib);
        let (owner, method) = fbs
            .resolve_method(&TypeName::from("FB_Derived"), &Id::from("Start"))
            .unwrap();
        assert_eq!(TypeName::from("FB_Base"), owner.name);
        assert_eq!(Id::from("Start"), method.name);
    }

    #[test]
    fn resolve_method_when_not_declared_anywhere_then_none() {
        let lib = parse(HIERARCHY);
        let fbs = FunctionBlocks::from_library(&lib);
        assert!(fbs
            .resolve_method(&TypeName::from("FB_Derived"), &Id::from("Reset"))
            .is_none());
    }

    #[test]
    fn declaring_block_when_field_inherited_then_returns_base_block() {
        let lib = parse(
            "
FUNCTION_BLOCK FB_Base
VAR
    count : INT;
END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    own : INT;
END_VAR
END_FUNCTION_BLOCK",
        );
        let fbs = FunctionBlocks::from_library(&lib);
        let derived = TypeName::from("FB_Derived");
        assert_eq!(
            Some(TypeName::from("FB_Base")),
            fbs.declaring_block(&derived, &Id::from("count"))
                .map(|fb| fb.name.clone())
        );
        assert_eq!(
            Some(derived.clone()),
            fbs.declaring_block(&derived, &Id::from("own"))
                .map(|fb| fb.name.clone())
        );
        assert!(fbs
            .declaring_block(&derived, &Id::from("missing"))
            .is_none());
    }

    #[test]
    fn resolve_method_when_extends_cycle_then_none() {
        let lib = parse(
            "
FUNCTION_BLOCK FB_A EXTENDS FB_B
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_B EXTENDS FB_A
END_FUNCTION_BLOCK",
        );
        let fbs = FunctionBlocks::from_library(&lib);
        assert!(fbs
            .resolve_method(&TypeName::from("FB_A"), &Id::from("Reset"))
            .is_none());
    }

    /// The scope a body at `path` is in: a unit, or a method of it.
    fn scope(path: &[&str]) -> ScopeKind {
        ScopeKind::Named(ScopePath::new(
            path.iter().map(|name| Id::from(name)).collect(),
        ))
    }

    /// The type of `instance` as a body in `path` names it, in a library
    /// resolved from `program`.
    fn instance_type_in(program: &str, path: &[&str], instance: &str) -> Option<TypeName> {
        let (_, context) = parse_and_resolve_types_with_options(program, &oop_options());
        instance_type(
            &Id::from(instance),
            &scope(path),
            context.symbols(),
            context.types(),
        )
    }

    const INSTANCES: &str = "
FUNCTION_BLOCK FB_Base
VAR
    x : INT;
    inherited : TON;
END_VAR
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
METHOD Run
VAR
    local : FB_Base;
END_VAR
END_METHOD
END_FUNCTION_BLOCK
PROGRAM main
VAR
    inst : FB_Base;
    with_init : FB_Base := (x := 1);
    timer : TON;
    count : INT;
END_VAR
END_PROGRAM";

    #[test]
    fn instance_type_when_user_block_instance_then_its_block() {
        assert_eq!(
            Some(TypeName::from("FB_Base")),
            instance_type_in(INSTANCES, &["main"], "inst")
        );
    }

    #[test]
    fn instance_type_when_instance_has_member_initializer_then_its_block() {
        assert_eq!(
            Some(TypeName::from("FB_Base")),
            instance_type_in(INSTANCES, &["main"], "with_init")
        );
    }

    #[test]
    fn instance_type_when_standard_block_instance_then_its_block() {
        assert_eq!(
            Some(TypeName::from("TON")),
            instance_type_in(INSTANCES, &["main"], "timer")
        );
    }

    #[test]
    fn instance_type_when_not_an_instance_then_none() {
        assert_eq!(None, instance_type_in(INSTANCES, &["main"], "count"));
    }

    #[test]
    fn instance_type_when_undeclared_then_none() {
        assert_eq!(None, instance_type_in(INSTANCES, &["main"], "missing"));
    }

    #[test]
    fn instance_type_when_declared_in_another_unit_then_none() {
        assert_eq!(None, instance_type_in(INSTANCES, &["FB_Base"], "inst"));
    }

    #[test]
    fn instance_type_when_method_local_then_found_in_that_method_only() {
        assert_eq!(
            Some(TypeName::from("FB_Base")),
            instance_type_in(INSTANCES, &["FB_Derived", "Run"], "local")
        );
        assert_eq!(None, instance_type_in(INSTANCES, &["FB_Derived"], "local"));
    }

    #[test]
    fn instance_type_when_inherited_through_extends_then_its_block() {
        assert_eq!(
            Some(TypeName::from("TON")),
            instance_type_in(INSTANCES, &["FB_Derived", "Run"], "inherited")
        );
    }

    /// The function block `name` that `lib` declares.
    fn block<'a>(lib: &'a Library, name: &str) -> &'a FunctionBlockDeclaration {
        FunctionBlocks::from_library(lib)
            .get(&TypeName::from(name))
            .unwrap()
    }

    const PARAMETERS: &str = "
FUNCTION_BLOCK FB_Params
VAR_OUTPUT
    done : BOOL;
END_VAR
VAR_INPUT
    first : INT;
END_VAR
VAR_IN_OUT
    total : INT;
END_VAR
VAR
    hidden : INT;
END_VAR
VAR_INPUT
    second : INT;
END_VAR
END_FUNCTION_BLOCK";

    fn names<'a>(parameters: impl Iterator<Item = &'a CallParameter>) -> Vec<String> {
        parameters.map(|p| p.name.to_string()).collect()
    }

    #[test]
    fn call_parameters_when_declaration_then_inputs_in_declaration_order() {
        let lib = parse(PARAMETERS);
        let parameters = CallParameters::of_declaration(block(&lib, "FB_Params"));
        assert_eq!(vec!["first", "second"], names(parameters.inputs()));
    }

    #[test]
    fn call_parameters_when_named_input_then_finds_input_and_in_out_only() {
        let lib = parse(PARAMETERS);
        let parameters = CallParameters::of_declaration(block(&lib, "FB_Params"));
        assert_eq!(
            Some(ParameterSection::Input),
            parameters
                .named_input(&Id::from("FIRST"))
                .map(|p| p.section)
        );
        assert_eq!(
            Some(ParameterSection::InOut),
            parameters
                .named_input(&Id::from("total"))
                .map(|p| p.section)
        );
        assert!(parameters.named_input(&Id::from("done")).is_none());
        assert!(parameters.named_input(&Id::from("hidden")).is_none());
    }

    #[test]
    fn call_parameters_when_output_then_finds_output_only() {
        let lib = parse(PARAMETERS);
        let parameters = CallParameters::of_declaration(block(&lib, "FB_Params"));
        assert!(parameters.output(&Id::from("done")).is_some());
        assert!(parameters.output(&Id::from("total")).is_none());
        assert!(parameters.output(&Id::from("first")).is_none());
    }

    #[test]
    fn call_parameters_when_standard_block_then_inputs_and_outputs_of_its_type() {
        let (_, context) = parse_and_resolve_types_with_options(PARAMETERS, &oop_options());
        let SemanticType::FunctionBlock { fields, .. } = &context
            .types()
            .get(&TypeName::from("CTU"))
            .unwrap()
            .representation
        else {
            panic!("CTU is a function block");
        };
        let parameters = CallParameters::of_standard_block(fields);
        assert_eq!(vec!["CU", "R", "PV"], names(parameters.inputs()));
        assert!(parameters.output(&Id::from("CV")).is_some());
        assert!(parameters.named_input(&Id::from("CV")).is_none());
    }
}
