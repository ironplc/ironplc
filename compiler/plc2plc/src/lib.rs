extern crate ironplc_dsl as dsl;

use ironplc_dsl::{common::Library, diagnostic::Diagnostic};
use renderer::apply;

mod renderer;
mod tests;

pub fn write_to_string(lib: &Library) -> Result<String, Vec<Diagnostic>> {
    apply(lib)
}
