//! Ignored tests that help investigate the gate:
//!
//! - `PROGRAM_ID=codegen/end_to_end_len.rs#15 cargo test -p ironplc-wasm --test it
//!   tools::show -- --ignored --nocapture` prints a program of the corpus;
//! - `SOURCE=main.st cargo test ... tools::ir -- --ignored --nocapture`
//!   prints the IR of a file and the variables of both targets after one cycle.

use crate::corpus::{codegen_programs, repository_programs};
use crate::harness::{compile, option_sets, run_vm, run_wasm};

#[test]
#[ignore = "investigation tool"]
fn show() {
    let id = std::env::var("PROGRAM_ID").unwrap_or_default();
    for p in codegen_programs().into_iter().chain(repository_programs()) {
        if p.id == id {
            println!("{}", p.source);
        }
    }
}

#[test]
#[ignore = "investigation tool"]
fn ir() {
    let Ok(path) = std::env::var("SOURCE") else {
        return;
    };
    let source = std::fs::read_to_string(path).unwrap_or_default();
    let file = ironplc_dsl::core::FileId::from_string("main.st");
    for options in option_sets() {
        let mut project = ironplc_project::MemoryBackedProject::new(options);
        project.add_source(file.clone(), source.clone());
        if let Ok((l, c)) = ironplc_project::analyze(&mut project, vec![]) {
            match ironplc_wasm::lower(l, c, &Default::default(), vec![]) {
                Ok((m, _)) => println!("{}", m.dump()),
                Err(d) => println!("{d:?}"),
            }
            break;
        }
    }
    if let Ok(c) = compile(&source) {
        println!("VM:       {:?}", run_vm(&c, 1));
        println!(
            "wasmtime: {:?}",
            run_wasm(&c, ironplc_wasm_runner::Engine::Wasmtime, 1)
        );
    }
}
