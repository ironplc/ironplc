//! What a test can see of a program after one scan.

use ironplc_analyzer::SemanticContext;
use ironplc_container::Container;
use ironplc_dsl::common::Library;
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::test_support::load_and_start;
use ironplc_vm::{FaultContext, VmBuffers};

use super::run::{compile_to_run, parse_and_compile_to_run, parse_and_try_run, run_one_scan};
use super::value::{FromValue, Value};
use super::variables::Variables;

/// A program's variables after one scan, read by name.
pub struct Snapshot {
    container: Container,
    bufs: VmBuffers,
}

impl Snapshot {
    /// Parses, compiles and runs `source` for one scan.
    pub fn run(source: &str, options: &CompilerOptions) -> Self {
        let (container, bufs) = parse_and_try_run(source, options).unwrap();
        Snapshot { container, bufs }
    }

    /// Parses, compiles and runs `source` for one scan that may trap. Returns
    /// the variables as the scan left them, and the trap if there was one, so
    /// a test can see what a trap leaves behind.
    pub fn try_run(source: &str, options: &CompilerOptions) -> (Self, Option<FaultContext>) {
        let container = parse_and_compile_to_run(source, options);
        let mut bufs = VmBuffers::from_container(&container);
        let fault = load_and_start(&container, &mut bufs)
            .and_then(|mut vm| vm.run_round(0))
            .err();
        (Snapshot { container, bufs }, fault)
    }

    /// Compiles the analyzed `library` and runs it for one scan. Use this when
    /// the program is analyzed together with another library, such as a
    /// bundled one, so source text alone is not enough.
    pub fn run_analyzed(
        library: &Library,
        context: &SemanticContext,
        options: &CompilerOptions,
    ) -> Self {
        let container = compile_to_run(library, context, options).unwrap();
        let bufs = run_one_scan(&container).unwrap();
        Snapshot { container, bufs }
    }

    /// Runs the compiled `container` for one scan.
    pub fn run_compiled(container: Container) -> Self {
        let bufs = run_one_scan(&container).unwrap();
        Snapshot { container, bufs }
    }

    /// The value of the program or global variable `name`.
    pub fn read(&self, name: &str) -> Value {
        let variables = Variables::of(&self.container);
        let entry = variables.entry(name);
        let slot = self.bufs.vars[usize::from(entry.var_index.raw())];
        variables.decode(entry, slot, &self.bufs.data_region)
    }

    /// The value of `name` as a `T`. Fails the test when the value cannot be
    /// converted to a `T` without loss, naming the slot the name resolved to.
    pub fn read_as<T: FromValue>(&self, name: &str) -> T {
        let entry = Variables::of(&self.container).entry(name);
        Variables::convert(name, entry, &self.read(name))
    }
}
