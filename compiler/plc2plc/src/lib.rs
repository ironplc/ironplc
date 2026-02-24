// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl as dsl;

use ironplc_dsl::{common::Library, diagnostic::Diagnostic};
use renderer::apply;

mod renderer;
mod tests;

pub fn write_to_string(lib: &Library) -> Result<String, Vec<Diagnostic>> {
    apply(lib)
}
