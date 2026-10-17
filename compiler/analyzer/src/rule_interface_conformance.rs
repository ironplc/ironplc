//! Semantic rule that a function block provides every member of the
//! interfaces it implements (OOP extension).
//!
//! A block implements every interface it converts to
//! ([`crate::supertypes`]): those it lists in `IMPLEMENTS`, those they
//! extend, and those its bases implement. Each is checked once.
//!
//! - A method prototype needs a method of the same name on the function
//!   block or a base in its `EXTENDS` chain, with the same signature: the
//!   same return type and the same `VAR_INPUT`/`VAR_OUTPUT`/`VAR_IN_OUT`
//!   parameters (names, kinds and types, in order) and `R_EDGE`/`F_EDGE`
//!   inputs. Locals are not compared.
//! - A property prototype needs a property of the same name and type with
//!   every accessor the prototype declares. An extra accessor is allowed.
//!
//! The signatures compared are those the type environment records, and
//! types are compared by [`TypeEnvironment::same_type`].
//!
//! Each problem is reported at the block that introduces it. A block that
//! lists an interface, or an interface that extends it, must provide every
//! member: a missing member is P4067 and a different one P4068, both at the
//! `IMPLEMENTS` entry. A block that only inherits an interface from a base
//! is checked for the members it declares itself, its overrides: a
//! different one is P4068 at the member. A member it inherits unchanged was
//! checked with the base.
//!
//! A name in `IMPLEMENTS` that is not a declared interface is P4070.
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
use std::collections::HashMap;

use ironplc_dsl::{
    common::*,
    core::{Id, Located},
    diagnostic::{Diagnostic, Label},
    type_id::TypeId,
};
use ironplc_parser::options::CompilerOptions;
use ironplc_problems::Problem;

use crate::{
    result::SemanticResult, semantic_context::SemanticContext, type_environment::TypeEnvironment,
};

pub fn apply(
    lib: &Library,
    context: &SemanticContext,
    _options: &CompilerOptions,
) -> SemanticResult {
    // The first declaration of a name is the one the environment records.
    let mut interfaces: HashMap<TypeName, &InterfaceDeclaration> = HashMap::new();
    for element in &lib.elements {
        if let LibraryElementKind::InterfaceDeclaration(itf) = element {
            interfaces
                .entry(TypeName::from_id(&itf.name))
                .or_insert(itf);
        }
    }

    let mut diagnostics = Vec::new();
    for element in &lib.elements {
        if let LibraryElementKind::FunctionBlockDeclaration(fb) = element {
            let mut conformance = Conformance {
                types: context.types(),
                interfaces: &interfaces,
                fb,
                diagnostics: &mut diagnostics,
            };
            conformance.check();
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// The check of one function block.
struct Conformance<'a> {
    types: &'a TypeEnvironment,
    interfaces: &'a HashMap<TypeName, &'a InterfaceDeclaration>,
    fb: &'a FunctionBlockDeclaration,
    diagnostics: &'a mut Vec<Diagnostic>,
}

/// Where a problem with a member is reported.
enum Introduced<'a> {
    /// The block lists the interface, through this `IMPLEMENTS` entry.
    Listed(&'a TypeName),
    /// The block inherits the interface from a base.
    Inherited,
}

impl<'a> Conformance<'a> {
    /// Reports each name in `IMPLEMENTS` that is not a declared interface,
    /// and each member of the interfaces the block implements that it does
    /// not provide, or provides with a different signature.
    fn check(&mut self) {
        let Some(block) = self.types.id_of(&self.fb.name) else {
            return;
        };
        let listed = self.listed();
        for name in self.types.supertypes().all_of(&self.fb.name) {
            let (Some(itf), Some(itf_id)) = (self.interfaces.get(&name), self.types.id_of(&name))
            else {
                continue;
            };
            let introduced = match listed.get(&name) {
                Some(entry) => Introduced::Listed(entry),
                None => Introduced::Inherited,
            };
            self.check_methods(block, itf, itf_id, &introduced);
            self.check_properties(block, itf, itf_id, &introduced);
        }
    }

    /// The interfaces the block lists, directly or as an interface a listed
    /// one extends, each with the first `IMPLEMENTS` entry that reaches it.
    /// Reports each entry that is not a declared interface.
    fn listed(&mut self) -> HashMap<TypeName, &'a TypeName> {
        let mut listed = HashMap::new();
        let fb = self.fb;
        let entries = fb.oop.iter().flat_map(|oop| &oop.implements);
        for entry in entries {
            if !self.is_interface(entry) {
                self.report_not_interface(entry);
                continue;
            }
            listed.entry(entry.clone()).or_insert(entry);
            for extended in self.types.supertypes().all_of(entry) {
                listed.entry(extended).or_insert(entry);
            }
        }
        listed
    }

    fn is_interface(&self, name: &TypeName) -> bool {
        self.types
            .id_of(name)
            .and_then(|id| self.types.get_by_id(id))
            .is_some_and(|attributes| attributes.representation.is_interface())
    }

    fn check_methods(
        &mut self,
        block: TypeId,
        itf: &InterfaceDeclaration,
        itf_id: TypeId,
        introduced: &Introduced,
    ) {
        let types = self.types;
        let recorded = types.interface_methods(itf_id).unwrap_or_default();
        for prototype in &itf.methods {
            let Some(expected) = recorded.iter().find(|m| m.name == prototype.name) else {
                continue;
            };
            let found = types.method(block, &prototype.name);
            let differs = |(_, method)| !types.same_signature(method, expected);
            match introduced {
                Introduced::Listed(entry) => match found {
                    None => self.report_listed(
                        Problem::InterfaceMemberMissing,
                        entry,
                        itf,
                        &prototype.name,
                        prototype,
                    ),
                    Some(found) if differs(found) => self.report_listed(
                        Problem::InterfaceMemberMismatch,
                        entry,
                        itf,
                        &prototype.name,
                        prototype,
                    ),
                    Some(_) => {}
                },
                Introduced::Inherited => {
                    let own = self.fb.methods.iter().find(|m| m.name == prototype.name);
                    if let (Some(own), Some(found)) = (own, found) {
                        if differs(found) {
                            self.report_override(itf, &own.name, prototype);
                        }
                    }
                }
            }
        }
    }

    fn check_properties(
        &mut self,
        block: TypeId,
        itf: &InterfaceDeclaration,
        itf_id: TypeId,
        introduced: &Introduced,
    ) {
        let types = self.types;
        let recorded = types.interface_properties(itf_id).unwrap_or_default();
        for prototype in &itf.properties {
            let Some(expected) = recorded.iter().find(|p| p.name == prototype.name) else {
                continue;
            };
            let found = types.property(block, &prototype.name);
            let differs = |property| !types.provides_property(property, expected);
            match introduced {
                Introduced::Listed(entry) => match found {
                    None => self.report_listed(
                        Problem::InterfaceMemberMissing,
                        entry,
                        itf,
                        &prototype.name,
                        prototype,
                    ),
                    Some(found) if differs(found) => self.report_listed(
                        Problem::InterfaceMemberMismatch,
                        entry,
                        itf,
                        &prototype.name,
                        prototype,
                    ),
                    Some(_) => {}
                },
                Introduced::Inherited => {
                    let own = self.fb.properties.iter().find(|p| p.name == prototype.name);
                    if let (Some(own), Some(found)) = (own, found) {
                        if differs(found) {
                            self.report_override(itf, &own.name, prototype);
                        }
                    }
                }
            }
        }
    }

    /// Reports a member of an interface the block lists, at the
    /// `IMPLEMENTS` entry that reaches the interface.
    fn report_listed(
        &mut self,
        problem: Problem,
        entry: &TypeName,
        itf: &InterfaceDeclaration,
        member: &Id,
        prototype: &dyn Located,
    ) {
        let secondary = match problem {
            Problem::InterfaceMemberMissing => "Member not provided",
            _ => "Member declared differently",
        };
        let diagnostic =
            Diagnostic::problem(problem, Label::span(entry.span(), "Interface implemented"))
                .with_secondary(Label::span(prototype.span(), secondary));
        self.push(diagnostic, itf, member);
    }

    /// Reports a member the block declares itself that differs from the
    /// member of an interface it inherits from a base.
    fn report_override(
        &mut self,
        itf: &InterfaceDeclaration,
        member: &Id,
        prototype: &dyn Located,
    ) {
        let diagnostic = Diagnostic::problem(
            Problem::InterfaceMemberMismatch,
            Label::span(
                member.span(),
                "Member differs from the interface member it implements",
            ),
        )
        .with_secondary(Label::span(prototype.span(), "Interface member"));
        self.push(diagnostic, itf, member);
    }

    fn report_not_interface(&mut self, entry: &TypeName) {
        let label = match self.types.id_of(entry) {
            None => "Not declared",
            Some(_) => "Not an interface",
        };
        self.diagnostics.push(
            Diagnostic::problem(
                Problem::ImplementsNotInterface,
                Label::span(entry.span(), label),
            )
            .with_context_id("function block", &self.fb.name.name)
            .with_context_type("name", entry),
        );
    }

    fn push(&mut self, diagnostic: Diagnostic, itf: &InterfaceDeclaration, member: &Id) {
        self.diagnostics.push(
            diagnostic
                .with_context_id("function block", &self.fb.name.name)
                .with_context_id("interface", &itf.name)
                .with_context_id("member", member),
        );
    }
}

#[cfg(test)]
mod tests;
