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
use ironplc_dsl::textual::MethodReceiver;

use crate::function_environment::FunctionSignature;
use crate::semantic_type::FunctionBlockVarType;
use crate::symbol_environment::{Resolution, ScopeKind, SymbolEnvironment};
use crate::type_environment::{BlockField, TypeEnvironment};

/// The function blocks a library declares, by name, and the blocks they
/// `EXTENDS` as the type environment records them.
pub(crate) struct FunctionBlocks<'a> {
    by_name: HashMap<TypeName, &'a FunctionBlockDeclaration>,
    types: &'a TypeEnvironment,
}

impl<'a> FunctionBlocks<'a> {
    pub(crate) fn from_library(lib: &'a Library, types: &'a TypeEnvironment) -> Self {
        let by_name = lib
            .elements
            .iter()
            .filter_map(|element| match element {
                LibraryElementKind::FunctionBlockDeclaration(fb) => Some((fb.name.clone(), fb)),
                _ => None,
            })
            .collect();
        FunctionBlocks { by_name, types }
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
    /// on, each base the one the type environment records
    /// ([`TypeEnvironment::base_of`]), so a name that is not a block the
    /// library declares ends the chain. Stops at a block seen before: a
    /// cycle is independently invalid (and rejected elsewhere), this is just
    /// a safety net.
    fn chain(&self, fb_name: &TypeName) -> impl Iterator<Item = &'a FunctionBlockDeclaration> + '_ {
        let mut current = self.get(fb_name);
        let mut visited: HashSet<TypeName> = HashSet::new();
        std::iter::from_fn(move || {
            let fb = current?;
            if !visited.insert(fb.name.clone()) {
                return None;
            }
            current = self.base_of(fb);
            Some(fb)
        })
    }

    /// The declaration of the block `fb` `EXTENDS`, as the type environment
    /// records it.
    fn base_of(&self, fb: &FunctionBlockDeclaration) -> Option<&'a FunctionBlockDeclaration> {
        let base = self.types.base_of(self.types.id_of(&fb.name)?)?;
        self.get(self.types.name_of(base)?)
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

/// The block a method call on `receiver` starts its search from, as a body in
/// `scope` names it: the declared block of an instance, the enclosing block
/// for `THIS^`, and the block that one `EXTENDS` for `SUPER^`.
///
/// `None` when the instance is not a function block instance the scope can
/// use, for `THIS^` outside a function block, and for `SUPER^` in a block
/// that extends nothing.
pub(crate) fn receiver_block(
    receiver: &MethodReceiver,
    scope: &ScopeKind,
    symbols: &SymbolEnvironment,
    types: &TypeEnvironment,
) -> Option<TypeName> {
    match receiver {
        MethodReceiver::Instance(instance) => instance_type(instance, scope, symbols, types),
        MethodReceiver::SelfRef(self_ref) => symbols.self_type(scope, self_ref.kind),
    }
}

/// Which block declares each method that can be called on each function
/// block a library declares: the block's own method of that name, or else
/// the nearest one in its `EXTENDS` chain (ADR-0041 case 1).
///
/// These are the answers of [`FunctionBlocks::resolve_method`], kept without
/// borrowing the library, for a pass that consumes the library as it folds
/// it and for code generation, which call a method by the block that
/// declares it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MethodOwners {
    declaring: HashMap<(TypeName, Id), TypeName>,
}

impl MethodOwners {
    /// The owner of every method callable on every function block `lib`
    /// declares, whose bases `types` records.
    pub fn of_library(lib: &Library, types: &TypeEnvironment) -> Self {
        let blocks = FunctionBlocks::from_library(lib, types);
        let mut declaring = HashMap::new();
        for block in blocks.by_name.values() {
            for ancestor in blocks.chain(&block.name) {
                for method in &ancestor.methods {
                    if let Some((owner, _)) = blocks.resolve_method(&block.name, &method.name) {
                        declaring
                            .entry((block.name.clone(), method.name.clone()))
                            .or_insert_with(|| owner.name.clone());
                    }
                }
            }
        }
        MethodOwners { declaring }
    }

    /// The block that declares the method `method` called on an instance of
    /// `block`: `block` itself or a block it extends. `None` when no block in
    /// the chain declares one, or `block` is not a block the library
    /// declares.
    pub fn declaring_block(&self, block: &TypeName, method: &Id) -> Option<&TypeName> {
        self.declaring.get(&(block.clone(), method.clone()))
    }
}

/// The parameters of every method a call can reach on a function block the
/// library declares, from the declaration of the block that declares it. A
/// block's own parameters are its fields, which the type environment records
/// ([`CallParameters::of_block`]).
///
/// Owned, so that a pass that consumes the library as it folds it can ask.
#[derive(Clone, Debug, Default)]
pub(crate) struct MethodParameters {
    /// The parameters of each method, by the block that declares it and the
    /// method's name.
    methods: HashMap<(TypeName, Id), CallParameters>,
    owners: MethodOwners,
}

impl MethodParameters {
    /// The parameters of the methods of every function block `lib`
    /// declares.
    pub(crate) fn of_library(lib: &Library, types: &TypeEnvironment) -> Self {
        let blocks = FunctionBlocks::from_library(lib, types);
        let methods = blocks
            .by_name
            .values()
            .flat_map(|block| {
                block.methods.iter().map(|method| {
                    (
                        (block.name.clone(), method.name.clone()),
                        CallParameters::of_declaration(method),
                    )
                })
            })
            .collect();
        MethodParameters {
            methods,
            owners: MethodOwners::of_library(lib, types),
        }
    }

    /// The parameters of the method `method` called on an instance of
    /// `block`: the method `block` declares, or else the one the nearest
    /// block it `EXTENDS` declares ([`MethodOwners`]).
    pub(crate) fn method(&self, block: &TypeName, method: &Id) -> Option<&CallParameters> {
        let owner = self.owners.declaring_block(block, method)?;
        self.methods.get(&(owner.clone(), method.clone()))
    }
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

/// One parameter of a function, function block or method, as a call sees it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CallParameter {
    /// The name, as the callee declares it.
    pub(crate) name: Id,
    pub(crate) section: ParameterSection,
}

/// The parameters of a function, function block or method, in declaration
/// order.
///
/// Every analysis that binds a call's arguments asks this list which
/// parameters there are, so they agree on what counts as an input and on the
/// order positional arguments take.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct CallParameters {
    parameters: Vec<CallParameter>,
    /// Whether a positional argument binds to a `VAR_IN_OUT` as well as to a
    /// `VAR_INPUT`: true for a function, false for a function block or method.
    positional_in_outs: bool,
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
        CallParameters {
            parameters,
            positional_in_outs: false,
        }
    }

    /// The parameters of the function block `block`, a block the library
    /// declares or a standard block: the inputs, in-outs and outputs of an
    /// instance of it that the type environment records, those it inherits
    /// through `EXTENDS` first, in order. `None` for any other type.
    pub(crate) fn of_block(block: &TypeName, types: &TypeEnvironment) -> Option<Self> {
        Some(Self::of_block_fields(
            types.instance_fields(types.id_of(block)?)?,
        ))
    }

    /// The parameters among the fields `fields` of a function block.
    fn of_block_fields(fields: Vec<&BlockField>) -> Self {
        let parameters = fields
            .iter()
            .filter_map(|field| {
                let section = match field.var_type {
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
        CallParameters {
            parameters,
            positional_in_outs: false,
        }
    }

    /// The parameters of a call to the function `signature` with `inputs`
    /// input arguments: its `VAR_INPUT` and `VAR_IN_OUT` parameters, then its
    /// `VAR_OUTPUT` parameters, each in declaration order.
    ///
    /// An extensible function takes inputs beyond its declared ones, named by
    /// counting on from the last declared one
    /// ([`FunctionSignature::input_parameters`]). The list has as many inputs
    /// as the call supplies, when that is more than the function declares and
    /// within its limit, so a further input numbered past a gap names no
    /// parameter.
    pub(crate) fn of_function(signature: &FunctionSignature, inputs: usize) -> Self {
        let count = inputs.max(signature.input_parameter_count());
        let inputs = signature
            .input_parameters()
            .take(count)
            .map(|parameter| CallParameter {
                section: if parameter.is_inout {
                    ParameterSection::InOut
                } else {
                    ParameterSection::Input
                },
                name: parameter.name,
            });
        let outputs = signature
            .parameters
            .iter()
            .filter(|parameter| parameter.is_output)
            .map(|parameter| CallParameter {
                name: parameter.name.clone(),
                section: ParameterSection::Output,
            });
        CallParameters {
            parameters: inputs.chain(outputs).collect(),
            positional_in_outs: true,
        }
    }

    /// Every parameter, in declaration order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &CallParameter> {
        self.parameters.iter()
    }

    /// The parameters a positional argument binds to, in declaration order:
    /// the `VAR_INPUT` parameters of a function block or method, and the
    /// `VAR_INPUT` and `VAR_IN_OUT` parameters of a function.
    pub(crate) fn positional(&self) -> impl Iterator<Item = &CallParameter> {
        let in_outs = self.positional_in_outs;
        self.parameters.iter().filter(move |parameter| {
            parameter.section == ParameterSection::Input
                || (in_outs && parameter.section == ParameterSection::InOut)
        })
    }

    /// The parameter a named input `name := ...` binds to: a `VAR_INPUT` or
    /// `VAR_IN_OUT` of that name.
    pub(crate) fn named_input(&self, name: &Id) -> Option<&CallParameter> {
        self.parameters.iter().find(|parameter| {
            parameter.name == *name && parameter.section != ParameterSection::Output
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_context::SemanticContext;
    use crate::semantic_type::SemanticType;
    use crate::symbol_environment::ScopePath;
    use crate::test_helpers::parse_and_resolve_types_with_options;
    use crate::type_attributes::TypeAttributes;
    use crate::type_environment::TypeEnvironmentBuilder;
    use ironplc_dsl::core::FileId;
    use ironplc_dsl::textual::{SelfRefKind, SelfRefVariable};
    use ironplc_parser::{options::CompilerOptions, parse_program};
    use spec_test_macro::spec_test;

    fn oop_options() -> CompilerOptions {
        CompilerOptions {
            allow_fb_inheritance: true,
            ..CompilerOptions::default()
        }
    }

    /// Parses only, for the lookups that read declarations as the parser
    /// leaves them.
    fn parse(program: &str) -> Library {
        parse_program(program, &FileId::default(), &oop_options()).unwrap()
    }

    /// Parses and resolves types, for the lookups that follow `EXTENDS`:
    /// they follow the bases the type environment records.
    fn resolve(program: &str) -> (Library, SemanticContext) {
        parse_and_resolve_types_with_options(program, &oop_options())
    }

    /// A type environment holding a declared function block for each name,
    /// with the base each `bases` entry gives it. Analysis rejects a cycle
    /// in `EXTENDS` before it records one, so a test of the walks' safety
    /// net records it by hand.
    fn recording_bases(bases: &[(&str, &str)]) -> TypeEnvironment {
        let mut types = TypeEnvironmentBuilder::new()
            .with_elementary_types()
            .build()
            .unwrap();
        for (block, _) in bases {
            types.insert_type(
                &TypeName::from(block),
                TypeAttributes::new(SemanticType::FunctionBlock {
                    name: block.to_string(),
                }),
            );
        }
        for (block, base) in bases {
            let id = |name: &str| types.id_of(&TypeName::from(name));
            let (block, base) = (id(block).unwrap(), id(base));
            types.record_block(block, vec![], vec![], vec![], base);
        }
        types
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
        let (lib, context) = resolve(HIERARCHY);
        let fbs = FunctionBlocks::from_library(&lib, context.types());
        assert!(fbs.contains(&TypeName::from("FB_Base")));
        assert!(fbs.get(&TypeName::from("FB_Derived")).is_some());
        assert!(fbs.get(&TypeName::from("FB_Missing")).is_none());
    }

    #[test]
    fn resolve_method_when_declared_on_own_block_then_returns_own_block() {
        let (lib, context) = resolve(HIERARCHY);
        let fbs = FunctionBlocks::from_library(&lib, context.types());
        let (owner, method) = fbs
            .resolve_method(&TypeName::from("FB_Derived"), &Id::from("Stop"))
            .unwrap();
        assert_eq!(TypeName::from("FB_Derived"), owner.name);
        assert_eq!(Id::from("Stop"), method.name);
    }

    #[test]
    fn resolve_method_when_declared_on_base_then_returns_base_block() {
        let (lib, context) = resolve(HIERARCHY);
        let fbs = FunctionBlocks::from_library(&lib, context.types());
        let (owner, method) = fbs
            .resolve_method(&TypeName::from("FB_Derived"), &Id::from("Start"))
            .unwrap();
        assert_eq!(TypeName::from("FB_Base"), owner.name);
        assert_eq!(Id::from("Start"), method.name);
    }

    #[test]
    fn resolve_method_when_not_declared_anywhere_then_none() {
        let (lib, context) = resolve(HIERARCHY);
        let fbs = FunctionBlocks::from_library(&lib, context.types());
        assert!(fbs
            .resolve_method(&TypeName::from("FB_Derived"), &Id::from("Reset"))
            .is_none());
    }

    #[test]
    fn declaring_block_when_field_inherited_then_returns_base_block() {
        let (lib, context) = resolve(
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
        let fbs = FunctionBlocks::from_library(&lib, context.types());
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
        let types = recording_bases(&[("FB_A", "FB_B"), ("FB_B", "FB_A")]);
        let fbs = FunctionBlocks::from_library(&lib, &types);
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
        lib.elements
            .iter()
            .find_map(|element| match element {
                LibraryElementKind::FunctionBlockDeclaration(fb)
                    if fb.name == TypeName::from(name) =>
                {
                    Some(fb)
                }
                _ => None,
            })
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
        assert_eq!(vec!["first", "second"], names(parameters.positional()));
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

    /// The section of the parameter `name`, if `parameters` has one.
    fn section_of(parameters: &CallParameters, name: &str) -> Option<ParameterSection> {
        parameters
            .iter()
            .find(|p| p.name == Id::from(name))
            .map(|p| p.section)
    }

    #[test]
    fn call_parameters_when_declaration_then_every_parameter_in_declaration_order() {
        let lib = parse(PARAMETERS);
        let parameters = CallParameters::of_declaration(block(&lib, "FB_Params"));
        assert_eq!(
            vec!["done", "first", "total", "second"],
            names(parameters.iter())
        );
        assert_eq!(
            Some(ParameterSection::Output),
            section_of(&parameters, "done")
        );
        assert_eq!(
            Some(ParameterSection::InOut),
            section_of(&parameters, "total")
        );
        assert_eq!(None, section_of(&parameters, "hidden"));
    }

    /// A block the library declares has the parameters its declaration
    /// lists, as the type environment records them.
    #[test]
    fn call_parameters_when_declared_block_then_same_as_its_declaration() {
        let (lib, context) = parse_and_resolve_types_with_options(PARAMETERS, &oop_options());
        let declared = CallParameters::of_declaration(block(&lib, "FB_Params"));

        let recorded = CallParameters::of_block(&TypeName::from("FB_Params"), context.types());

        assert_eq!(recorded, Some(declared));
    }

    #[test]
    fn call_parameters_when_standard_block_then_inputs_and_outputs_it_records() {
        let (_, context) = parse_and_resolve_types_with_options(PARAMETERS, &oop_options());
        let parameters = CallParameters::of_block(&TypeName::from("CTU"), context.types()).unwrap();
        assert_eq!(vec!["CU", "R", "PV"], names(parameters.positional()));
        assert_eq!(
            Some(ParameterSection::Output),
            section_of(&parameters, "CV")
        );
        assert!(parameters.named_input(&Id::from("CV")).is_none());
    }

    const RECEIVERS: &str = "
FUNCTION_BLOCK FB_Base
METHOD Get : INT
    Get := 1;
END_METHOD
METHOD Reset
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
METHOD Get : INT
    Get := 2;
END_METHOD
METHOD Run
END_METHOD
END_FUNCTION_BLOCK
PROGRAM main
VAR
    d : FB_Derived;
END_VAR
END_PROGRAM";

    /// The block a call on `receiver` starts from, in a body at `path`.
    fn receiver_block_in(path: &[&str], receiver: MethodReceiver) -> Option<TypeName> {
        let (_, context) = parse_and_resolve_types_with_options(RECEIVERS, &oop_options());
        receiver_block(&receiver, &scope(path), context.symbols(), context.types())
    }

    fn self_ref(kind: SelfRefKind) -> MethodReceiver {
        MethodReceiver::SelfRef(SelfRefVariable {
            kind,
            position: ironplc_dsl::core::SourceSpan::default(),
        })
    }

    #[spec_test(REQ_CB_analyzer_003)]
    fn receiver_block_when_instance_then_its_declared_block() {
        assert_eq!(
            Some(TypeName::from("FB_Derived")),
            receiver_block_in(&["main"], MethodReceiver::Instance(Id::from("d")))
        );
    }

    #[spec_test(REQ_CB_analyzer_005)]
    fn receiver_block_when_this_in_method_then_enclosing_block() {
        assert_eq!(
            Some(TypeName::from("FB_Derived")),
            receiver_block_in(&["FB_Derived", "Run"], self_ref(SelfRefKind::This))
        );
    }

    #[spec_test(REQ_CB_analyzer_005)]
    fn receiver_block_when_super_in_method_then_block_it_extends() {
        assert_eq!(
            Some(TypeName::from("FB_Base")),
            receiver_block_in(&["FB_Derived", "Run"], self_ref(SelfRefKind::Super))
        );
    }

    #[spec_test(REQ_CB_analyzer_006)]
    fn receiver_block_when_this_outside_function_block_then_none() {
        assert_eq!(
            None,
            receiver_block_in(&["main"], self_ref(SelfRefKind::This))
        );
    }

    #[spec_test(REQ_CB_analyzer_006)]
    fn receiver_block_when_super_in_block_that_extends_nothing_then_none() {
        assert_eq!(
            None,
            receiver_block_in(&["FB_Base", "Reset"], self_ref(SelfRefKind::Super))
        );
    }

    /// The block that declares `method` called on `block`, in `RECEIVERS`.
    fn owner(block: &str, method: &str) -> Option<TypeName> {
        let (lib, context) = resolve(RECEIVERS);
        MethodOwners::of_library(&lib, context.types())
            .declaring_block(&TypeName::from(block), &Id::from(method))
            .cloned()
    }

    #[spec_test(REQ_CB_analyzer_003)]
    fn method_owners_when_declared_on_own_block_then_own_block() {
        assert_eq!(
            Some(TypeName::from("FB_Derived")),
            owner("FB_Derived", "Run")
        );
    }

    #[spec_test(REQ_CB_analyzer_003)]
    fn method_owners_when_inherited_then_base_block() {
        assert_eq!(
            Some(TypeName::from("FB_Base")),
            owner("FB_Derived", "Reset")
        );
    }

    #[spec_test(REQ_CB_analyzer_004)]
    fn method_owners_when_overridden_then_derived_block() {
        assert_eq!(
            Some(TypeName::from("FB_Derived")),
            owner("FB_Derived", "Get")
        );
        assert_eq!(Some(TypeName::from("FB_Base")), owner("FB_Base", "Get"));
    }

    #[test]
    fn method_owners_when_declared_only_on_derived_then_base_has_none() {
        assert_eq!(None, owner("FB_Base", "Run"));
    }

    #[test]
    fn method_owners_when_not_declared_anywhere_then_none() {
        assert_eq!(None, owner("FB_Derived", "Missing"));
    }

    #[test]
    fn method_owners_when_method_name_differs_in_case_then_found() {
        assert_eq!(
            Some(TypeName::from("FB_Base")),
            owner("fb_derived", "RESET")
        );
    }

    #[test]
    fn method_owners_when_extends_cycle_then_terminates() {
        let lib = parse(
            "
FUNCTION_BLOCK FB_A EXTENDS FB_B
METHOD M
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_B EXTENDS FB_A
END_FUNCTION_BLOCK",
        );
        let types = recording_bases(&[("FB_A", "FB_B"), ("FB_B", "FB_A")]);
        let owners = MethodOwners::of_library(&lib, &types);
        assert_eq!(
            Some(&TypeName::from("FB_A")),
            owners.declaring_block(&TypeName::from("FB_B"), &Id::from("M"))
        );
    }

    /// The names of the parameters of `method` called on `block`, in
    /// `program`.
    fn method_parameter_names(program: &str, block: &str, method: &str) -> Option<Vec<String>> {
        let (lib, context) = resolve(program);
        let parameters = MethodParameters::of_library(&lib, context.types());
        parameters
            .method(&TypeName::from(block), &Id::from(method))
            .map(|parameters| names(parameters.iter()))
    }

    const OVERRIDES: &str = "
FUNCTION_BLOCK FB_Base
METHOD Get : INT
VAR_INPUT a : INT; END_VAR
END_METHOD
METHOD Put
VAR_INPUT v : INT; END_VAR
VAR_IN_OUT t : INT; END_VAR
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
METHOD Get : INT
VAR_INPUT b : INT; END_VAR
END_METHOD
END_FUNCTION_BLOCK";

    #[test]
    fn block_parameters_method_when_inherited_then_parameters_of_base_method() {
        assert_eq!(
            Some(vec!["v".to_owned(), "t".to_owned()]),
            method_parameter_names(OVERRIDES, "FB_Derived", "Put")
        );
    }

    #[test]
    fn block_parameters_method_when_overridden_then_parameters_of_own_method() {
        assert_eq!(
            Some(vec!["b".to_owned()]),
            method_parameter_names(OVERRIDES, "FB_Derived", "Get")
        );
        assert_eq!(
            Some(vec!["a".to_owned()]),
            method_parameter_names(OVERRIDES, "FB_Base", "Get")
        );
    }

    #[test]
    fn block_parameters_method_when_not_declared_then_none() {
        assert_eq!(
            None,
            method_parameter_names(OVERRIDES, "FB_Derived", "Missing")
        );
    }
}
