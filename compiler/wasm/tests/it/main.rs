//! Integration tests of the WebAssembly target: conformance to
//! `specs/design/wasm-target.md` and the differential gate against the VM.

mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}

mod corpus;
mod gate;
mod harness;
mod spec;
mod tools;
