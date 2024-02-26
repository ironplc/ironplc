use ironplc_dsl::{common::Library, diagnostic::Diagnostic};
use renderer::apply;

mod renderer;

pub fn write(lib: &Library) -> Result<String, Vec<Diagnostic>> {
    apply(lib)
}
