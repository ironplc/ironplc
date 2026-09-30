//! Interface values (OOP extension) in code generation.
//!
//! A variable of an interface type refers to a function block instance
//! that implements the interface. How that reference is laid out at run
//! time is not decided yet (ADR-0041 Phase 2), so a program that declares
//! one is refused before any code is generated, rather than compiled with
//! the variable left out.

use ironplc_dsl::common::{InterfaceInitializer, Library};
use ironplc_dsl::core::Located;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_dsl::visitor::Visitor;

/// Returns a not-implemented diagnostic for the first variable of an
/// interface type in `library`, if any.
pub(crate) fn reject_interface_variables(library: &Library) -> Result<(), Diagnostic> {
    InterfaceVariableFinder.walk(library)
}

struct InterfaceVariableFinder;

impl Visitor<Diagnostic> for InterfaceVariableFinder {
    type Value = ();

    fn visit_interface_initializer(
        &mut self,
        node: &InterfaceInitializer,
    ) -> Result<Self::Value, Diagnostic> {
        Err(Diagnostic::not_implemented(Label::span(
            node.type_name.span(),
            "A variable of an interface type is recognized but not yet compiled by IronPLC",
        )))
    }
}
