// Allow large errors because this is a compiler - we expect large errors.
#![allow(clippy::result_large_err)]

pub mod disassemble;
pub mod project;
pub mod tokenizer;

pub use project::{FileBackedProject, MemoryBackedProject, Project};

#[cfg(test)]
#[ctor::ctor]
fn init_test_logger() {
    let _ = env_logger::builder()
        .is_test(true)
        .filter_level(log::LevelFilter::Trace)
        .try_init();
}
