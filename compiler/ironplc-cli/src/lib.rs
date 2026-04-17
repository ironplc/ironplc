// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

extern crate ironplc_dsl;
extern crate ironplc_parser;

pub mod cli;
pub mod logger;
pub mod lsp;
pub mod lsp_project;
pub mod lsp_runner;

#[cfg(test)]
mod test_helpers;

#[cfg(test)]
#[ctor::ctor]
fn init_test_logger() {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Trace)
        .try_init();
}
