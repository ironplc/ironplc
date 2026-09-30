//! Semantic rule that a function block provides every member of the
//! interfaces it `IMPLEMENTS` (OOP extension), including the members of
//! the interfaces those extend.
//!
//! - A method prototype needs a method of the same name on the function
//!   block or a base in its `EXTENDS` chain, with the same return type and
//!   the same `VAR_INPUT`/`VAR_OUTPUT`/`VAR_IN_OUT` parameters: same names,
//!   kinds and types, in the same order. Locals are not compared.
//! - A property prototype needs a property of the same name and type with
//!   every accessor the prototype declares. An extra accessor is allowed.
//!
//! A missing member is P4067, a member whose signature differs is P4068.
//! An interface that is not declared is left to the type checks.
//!
//! ## Passes
//!
//! ```ignore
//! INTERFACE I_Comm
//! METHOD Send : BOOL
//! VAR_INPUT data : INT; END_VAR
//! END_METHOD
//! END_INTERFACE
//!
//! FUNCTION_BLOCK FB_Serial IMPLEMENTS I_Comm
//! METHOD Send : BOOL
//! VAR_INPUT data : INT; END_VAR
//!     Send := TRUE;
//! END_METHOD
//! END_FUNCTION_BLOCK
//! ```
//!
//! ## Fails
//!
//! ```ignore
//! INTERFACE I_Comm
//! METHOD Send : BOOL
//! END_METHOD
//! END_INTERFACE
//!
//! FUNCTION_BLOCK FB_Serial IMPLEMENTS I_Comm
//! END_FUNCTION_BLOCK
//! ```
use std::collections::{HashMap, HashSet};

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

use crate::semantic_context::SemanticContext;
use ironplc_dsl::type_id::TypeId;

use crate::{
    callee_resolution::FunctionBlocks, result::SemanticResult, type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    let interfaces: HashMap<TypeName, &InterfaceDeclaration> = lib
        .elements
        .iter()
        .filter_map(|element| match element {
            LibraryElementKind::InterfaceDeclaration(itf) => {
                Some((TypeName::from_id(&itf.name), itf))
            }
            _ => None,
        })
        .collect();
    let checker = Conformance {
        interfaces: &interfaces,
        function_blocks: FunctionBlocks::from_library(lib),
        types: context.types(),
    };

    let mut diagnostics = Vec::new();
    for element in &lib.elements {
        if let LibraryElementKind::FunctionBlockDeclaration(fb) = element {
            if let Some(oop) = &fb.oop {
                for interface in &oop.implements {
                    checker.check(fb, interface, &mut diagnostics);
                }
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

struct Conformance<'a> {
    interfaces: &'a HashMap<TypeName, &'a InterfaceDeclaration>,
    function_blocks: FunctionBlocks<'a>,
    types: &'a TypeEnvironment,
}

impl<'a> Conformance<'a> {
    /// Reports each member of `interface` that `fb` does not provide, or
    /// provides with a different signature.
    fn check(
        &self,
        fb: &FunctionBlockDeclaration,
        interface: &TypeName,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for itf in self.with_extended(interface) {
            for prototype in &itf.methods {
                let found = self
                    .function_blocks
                    .resolve_method(&fb.name, &prototype.name);
                let problem = match found {
                    None => Some(Problem::InterfaceMemberMissing),
                    Some((_, method)) if !self.method_matches(method, prototype) => {
                        Some(Problem::InterfaceMemberMismatch)
                    }
                    Some(_) => None,
                };
                if let Some(problem) = problem {
                    diagnostics.push(Self::report(
                        problem,
                        fb,
                        interface,
                        itf,
                        &prototype.name,
                        prototype.span(),
                    ));
                }
            }
            for prototype in &itf.properties {
                let found = self
                    .function_blocks
                    .resolve_property(&fb.name, &prototype.name);
                let problem = match found {
                    None => Some(Problem::InterfaceMemberMissing),
                    Some(property) if !self.property_matches(property, prototype) => {
                        Some(Problem::InterfaceMemberMismatch)
                    }
                    Some(_) => None,
                };
                if let Some(problem) = problem {
                    diagnostics.push(Self::report(
                        problem,
                        fb,
                        interface,
                        itf,
                        &prototype.name,
                        prototype.span(),
                    ));
                }
            }
        }
    }

    /// `interface` and every interface it extends, directly or not, each
    /// once. Interfaces that are not declared are left out.
    fn with_extended(&self, interface: &TypeName) -> Vec<&'a InterfaceDeclaration> {
        let mut seen = HashSet::new();
        let mut pending = vec![interface.clone()];
        let mut found = Vec::new();
        while let Some(name) = pending.pop() {
            if !seen.insert(name.clone()) {
                continue;
            }
            if let Some(itf) = self.interfaces.get(&name) {
                pending.extend(itf.extends.iter().cloned());
                found.push(*itf);
            }
        }
        found
    }

    fn method_matches(&self, method: &MethodDeclaration, prototype: &MethodPrototype) -> bool {
        let same_return = match (&method.return_type, &prototype.return_type) {
            (None, None) => true,
            (Some(a), Some(b)) => self.return_type_key(a) == self.return_type_key(b),
            _ => false,
        };
        let parameters: Vec<&VarDecl> = method
            .variables
            .iter()
            .filter(|v| {
                matches!(
                    v.var_type,
                    VariableType::Input | VariableType::Output | VariableType::InOut
                )
            })
            .collect();
        same_return
            && parameters.len() == prototype.variables.len()
            && parameters
                .iter()
                .zip(&prototype.variables)
                .all(|(a, b)| same_parameter(a, b))
    }

    fn property_matches(
        &self,
        property: &PropertyDeclaration,
        prototype: &PropertyPrototype,
    ) -> bool {
        self.return_type_key(&property.property_type)
            == self.return_type_key(&prototype.property_type)
            && (prototype.get.is_none() || property.get.is_some())
            && (prototype.set.is_none() || property.set.is_some())
    }

    /// What two return or property types are compared by: the type's id
    /// when the name resolves, else the name; a string by its width and
    /// declared length.
    fn return_type_key(&self, return_type: &FunctionReturnType) -> TypeKey {
        match return_type {
            FunctionReturnType::Named(name) => match self.types.id_of(name) {
                Some(id) => TypeKey::Id(id),
                None => TypeKey::Name(name.clone()),
            },
            FunctionReturnType::String(spec) | FunctionReturnType::WString(spec) => {
                TypeKey::String {
                    wide: matches!(return_type, FunctionReturnType::WString(_)),
                    length: spec.length.as_ref().map(LengthKey::of),
                }
            }
        }
    }

    fn report(
        problem: Problem,
        fb: &FunctionBlockDeclaration,
        interface: &TypeName,
        declaring: &InterfaceDeclaration,
        member: &Id,
        member_span: ironplc_dsl::core::SourceSpan,
    ) -> Diagnostic {
        let (label, secondary) = match problem {
            Problem::InterfaceMemberMissing => ("Interface implemented", "Member not provided"),
            _ => ("Interface implemented", "Member declared differently"),
        };
        Diagnostic::problem(problem, Label::span(interface.span(), label))
            .with_secondary(Label::span(member_span, secondary))
            .with_context_id("function block", &fb.name.name)
            .with_context_id("interface", &declaring.name)
            .with_context_id("member", member)
    }
}

/// Two parameters match when they have the same name, the same kind and
/// the same type. The type is compared by resolved id when both have one,
/// else by the name each declaration states.
fn same_parameter(a: &VarDecl, b: &VarDecl) -> bool {
    let same_type = match (a.type_id, b.type_id) {
        (Some(x), Some(y)) => x == y,
        _ => a.initializer.type_reference() == b.initializer.type_reference(),
    };
    a.identifier.symbolic_id() == b.identifier.symbolic_id()
        && a.var_type == b.var_type
        && same_type
}

#[derive(PartialEq)]
enum TypeKey {
    Id(TypeId),
    Name(TypeName),
    String {
        wide: bool,
        length: Option<LengthKey>,
    },
}

#[derive(PartialEq)]
enum LengthKey {
    Literal(u128),
    Constant(Id),
}

impl LengthKey {
    fn of(length: &IntegerRef) -> Self {
        match length {
            IntegerRef::Literal(integer) => LengthKey::Literal(integer.value),
            IntegerRef::Constant(id) => LengthKey::Constant(id.clone()),
        }
    }
}

#[cfg(test)]
mod tests;
