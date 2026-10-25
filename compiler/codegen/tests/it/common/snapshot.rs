//! What a test can see of a program after one scan.

use ironplc_analyzer::SemanticContext;
use ironplc_dsl::common::Library;
use ironplc_parser::options::CompilerOptions;

use super::backend::{
    observe, run, Backend, Declared, Fault, Observed, Program, Reference, Running,
};
use super::run::{compile_to_run, parse_and_compile_to_run};
use super::value::{convert, FromValue, Value};

/// A program's variables after one scan on the reference backend, read by
/// name.
pub struct Snapshot {
    variables: Vec<(String, Value, Declared)>,
}

impl Snapshot {
    /// Parses, compiles and runs `source` for one scan.
    pub fn run(source: &str, options: &CompilerOptions) -> Self {
        Self::run_compiled(parse_and_compile_to_run(source, options))
    }

    /// Parses, compiles and runs `source` for one scan that may trap. Returns
    /// the variables as the scan left them, and the trap if there was one, so
    /// a test can see what a trap leaves behind.
    pub fn try_run(source: &str, options: &CompilerOptions) -> (Self, Option<Fault<Reference>>) {
        let program = parse_and_compile_to_run(source, options);
        Reference::start(&program, |running, fault| {
            let fault = match fault {
                Some(fault) => Some(fault),
                None => running.scan(0).err(),
            };
            (Self::of(running), fault)
        })
    }

    /// Compiles the analyzed `library` and runs it for one scan. Use this when
    /// the program is analyzed together with another library, such as a
    /// bundled one, so source text alone is not enough.
    pub fn run_analyzed(
        library: &Library,
        context: &SemanticContext,
        options: &CompilerOptions,
    ) -> Self {
        Self::run_compiled(compile_to_run(library, context, options).unwrap())
    }

    /// Runs the compiled `program` for one scan.
    pub fn run_compiled(program: Program<Reference>) -> Self {
        run::<Reference, _>(&program, |running| {
            running.scan(0).unwrap();
            Self::of(running)
        })
        .unwrap()
    }

    /// What `running` holds now.
    fn of(running: &dyn Running<Fault = Fault<Reference>>) -> Self {
        let observed: Observed = observe(running);
        let variables = observed
            .into_iter()
            .map(|(name, value)| {
                let declared = running.declared(&name);
                (name, value, declared)
            })
            .collect();
        Snapshot { variables }
    }

    /// The program or global variable `name`, matched without regard to case
    /// as IEC identifiers are.
    fn variable(&self, name: &str) -> (&Value, &Declared) {
        self.variables
            .iter()
            .find(|(declared, _, _)| declared.eq_ignore_ascii_case(name))
            .map(|(_, value, declared)| (value, declared))
            .unwrap_or_else(|| {
                let declared: Vec<&str> = self
                    .variables
                    .iter()
                    .map(|(name, _, _)| name.as_str())
                    .collect();
                panic!(
                    "the program declares no variable `{name}`; it declares {}",
                    declared.join(", ")
                )
            })
    }

    /// The value of the program or global variable `name`.
    pub fn read(&self, name: &str) -> Value {
        self.variable(name).0.clone()
    }

    /// The value of `name` as a `T`. Fails the test when the value cannot be
    /// converted to a `T` without loss, naming where the variable is kept.
    pub fn read_as<T: FromValue>(&self, name: &str) -> T {
        let (value, declared) = self.variable(name);
        convert(name, declared, value)
    }
}
