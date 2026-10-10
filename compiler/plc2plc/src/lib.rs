// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl as dsl;

use ironplc_dsl::{common::Library, diagnostic::Diagnostic};
use renderer::apply;

pub use type_comment::TypeNamer;

mod renderer;
#[cfg(test)]
mod tests;
mod type_comment;
mod written;

// Spec conformance testing infrastructure (test-only).
#[cfg(test)]
mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}
#[cfg(test)]
mod spec_conformance;
#[cfg(test)]
mod spec_conformance_pointer_to;
#[cfg(test)]
mod spec_conformance_string_literals;

pub fn write_to_string(lib: &Library) -> Result<String, Vec<Diagnostic>> {
    apply(lib, None)
}

/// Renders `lib` with a comment after each expression giving the type the
/// analyzer recorded for it, such as `count (* INT *)` or
/// `total (* DINT -> LINT *)` for an implicit conversion. `type_name` names
/// a type by its id. The output is still Structured Text and re-parses to
/// the library [`write_to_string`] renders.
///
/// See "Inspecting the annotation" in
/// `specs/design/expression-type-resolution.md`.
pub fn write_to_string_with_types(
    lib: &Library,
    type_name: TypeNamer,
) -> Result<String, Vec<Diagnostic>> {
    apply(lib, Some(type_name))
}
