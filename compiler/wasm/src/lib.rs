#![allow(clippy::result_large_err)]
//! WebAssembly target for IronPLC.
//!
//! Lowers the analysed program (the same [`Library`] and
//! [`SemanticContext`] the bytecode code generator takes) to the
//! intermediate representation of its WebAssembly back end, and generates
//! with it a WebAssembly module of the logic module ABI 1.1 and its
//! symbol map. See `specs/design/wasm-target.md`.
//!
//! ```ignore
//! let (library, context) = ironplc_project::analyze(&mut project, vec![])?;
//! let out = ironplc_wasm::compile(library, context, &WasmOptions::default(), &sources)?;
//! std::fs::write("main.wasm", &out.wasm)?;
//! ```

mod bits;
mod body;
mod call;
mod config;
mod constant;
mod enums;
mod expr;
mod layout;
mod leaves;
mod lower;
mod pou;
mod stdfb;
mod stdlib;
mod strings;
mod time;
mod types;

use ironplc_analyzer::SemanticContext;
use ironplc_dsl::common::Library;
use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_wasm_codegen::{generate, CodegenError, CodegenOptions};

pub use config::DEFAULT_TASK;
pub use ironplc_wasm_ir::Module;
pub use ironplc_wasm_symbols::SymbolMap;

/// Instrumentation of the generated module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WasmOptions {
    /// Fuel counter decremented by the code (ABI-070).
    pub fuel: bool,
    /// A call of `plc_rt.debug_hook` before each statement (ABI-032).
    pub debug_hooks: bool,
    /// Array bounds checks (ABI-082).
    pub bounds_checks: bool,
}

impl Default for WasmOptions {
    fn default() -> Self {
        WasmOptions {
            fuel: false,
            debug_hooks: false,
            bounds_checks: true,
        }
    }
}

/// A generated logic module.
#[derive(Clone, Debug)]
pub struct WasmOutput {
    /// The validated module; its `plc.meta` section holds the symbol map.
    pub wasm: Vec<u8>,
    /// The symbol map.
    pub symbols: SymbolMap,
}

/// Limit of the static data of a module.
const STATIC_LIMIT: u32 = 64 << 20;

/// Lowers an analysed library to the IR, and checks the result with the IR
/// validator (REQ-WT-wasm-043).
pub fn lower(
    library: &Library,
    context: &SemanticContext,
    options: &WasmOptions,
    files: Vec<FileId>,
) -> Result<(Module, Vec<FileId>), Diagnostic> {
    let mut l = lower::Lowerer::new(library, context, files);
    l.bounds_checks = options.bounds_checks;
    l.debug_hooks = options.debug_hooks;
    config::lower_project(&mut l)?;
    if l.m.sites.is_empty() {
        l.m.sites.push(Default::default());
    }
    let files = l.files().to_vec();
    Ok((validated(l.m)?, files))
}

/// The module when the IR validator accepts it; an internal error
/// otherwise, since the adapter built it (REQ-WT-wasm-043).
fn validated(m: Module) -> Result<Module, Diagnostic> {
    ironplc_wasm_ir::validate(&m).map_err(|e| {
        Diagnostic::internal_error_at(Label::file(
            FileId::default(),
            format!("The WebAssembly target built an invalid IR module: {e}"),
        ))
    })?;
    Ok(m)
}

/// Compiles an analysed library to a WebAssembly logic module.
///
/// `sources` gives the text of each file, hashed into the `plc.build`
/// section and listed in the symbol map in this order.
pub fn compile(
    library: &Library,
    context: &SemanticContext,
    options: &WasmOptions,
    sources: &[(FileId, String)],
) -> Result<WasmOutput, Diagnostic> {
    let files = sources.iter().map(|(f, _)| f.clone()).collect();
    let (module, files) = lower(library, context, options, files)?;
    let files = files
        .iter()
        .map(|f| {
            let text = sources
                .iter()
                .find(|(id, _)| id == f)
                .map(|(_, t)| t.clone())
                .unwrap_or_default();
            (f.to_string(), text)
        })
        .collect();
    let out = generate(
        &module,
        &CodegenOptions {
            fuel: options.fuel,
            debug_hooks: options.debug_hooks,
            bounds_checks: options.bounds_checks,
            static_limit: STATIC_LIMIT,
            compiler: format!("ironplcc {}", env!("CARGO_PKG_VERSION")),
            files,
        },
    )
    .map_err(|e| match e {
        CodegenError::StaticLimit { size, limit } => Diagnostic::not_implemented(Label::file(
            FileId::default(),
            format!("Static data of {size} bytes, more than the {limit} bytes of the WebAssembly target"),
        )),
        CodegenError::Internal(m) => Diagnostic::internal_error_at(Label::file(
            FileId::default(),
            format!("The WebAssembly code generator failed: {m}"),
        )),
    })?;
    Ok(WasmOutput {
        wasm: out.wasm,
        symbols: out.symbols,
    })
}

// Spec conformance testing infrastructure (test-only)
#[cfg(test)]
mod spec_requirements {
    include!(concat!(env!("OUT_DIR"), "/spec_requirements.rs"));
}

#[cfg(test)]
mod tests {
    use ironplc_wasm_ir::{FuncKind, Function};
    use spec_test_macro::spec_test;

    use super::*;

    #[test]
    fn all_spec_requirements_have_tests() {
        assert!(
            crate::spec_requirements::UNTESTED.is_empty(),
            "Requirements in spec with no conformance test: {:?}",
            crate::spec_requirements::UNTESTED
        );
    }

    #[spec_test(REQ_WT_wasm_043)]
    fn validated_when_task_runs_a_function_block_then_internal_error() {
        let mut m = Module::default();
        m.functions.push(Function {
            name: "FB".into(),
            kind: FuncKind::FunctionBlock,
            frame: None,
            temps: vec![],
            body: vec![],
            span: Default::default(),
        });
        m.tasks.push(ironplc_wasm_ir::Task {
            name: "T".into(),
            interval_ns: 0,
            priority: 0,
            programs: vec![(0, "FB".into())],
        });
        let d = validated(m).unwrap_err();
        assert_eq!(d.code, "P9998");
    }
}
