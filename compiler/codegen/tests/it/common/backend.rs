//! The backends the end-to-end suite runs a program on
//! (`specs/design/end-to-end-test-observation.md`, "Backends").
//!
//! A test compiles a program once on every backend and reads and drives the
//! program of the [`Reference`] backend. Every other backend must compile the
//! program too, or fail to as well, and a program the test runs must leave
//! every variable with the same value on every backend, after its
//! initialization and after one scan.

use std::fmt::Debug;

use ironplc_analyzer::SemanticContext;
use ironplc_dsl::common::Library;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::CompilerOptions;

use super::value::Value;
use super::vm::Vm;

/// The backend whose program a test reads and drives.
pub type Reference = Vm;

/// What a backend compiles a library to.
pub type Program<B> = <B as Backend>::Program;

/// What stops a backend's program: a trap, with where it happened.
pub type Fault<B> = <B as Backend>::Fault;

/// A backend the end-to-end suite runs a program on.
pub trait Backend {
    /// The backend, as a message about a disagreement names it.
    const NAME: &'static str;

    /// What the backend compiles a library to.
    type Program;

    /// What stops a program: a trap, with where it happened.
    type Fault: Debug;

    /// Compiles an analyzed library.
    fn compile(
        library: &Library,
        context: &SemanticContext,
        options: &CompilerOptions,
    ) -> Result<Self::Program, Diagnostic>;

    /// Loads `program`, which runs its initialization, and hands `f` the
    /// program as initialization left it, with the fault that stopped
    /// initialization, if one did. A program whose initialization stopped
    /// cannot scan or be written.
    fn start<T>(
        program: &Self::Program,
        f: impl FnOnce(&mut dyn Running<Fault = Self::Fault>, Option<Self::Fault>) -> T,
    ) -> T;
}

/// A program a backend loaded, driven across scans, its program and global
/// variables read and written by name.
pub trait Running {
    /// What stops the program.
    type Fault;

    /// Runs one scan at absolute time `time_us` (microseconds).
    fn scan(&mut self, time_us: u64) -> Result<(), Self::Fault>;

    /// The names of the program and global variables, in the order the
    /// program declares them.
    fn names(&self) -> Vec<String>;

    /// The value of the program or global variable `name`, matched without
    /// regard to case as IEC identifiers are. Fails the test when the program
    /// declares no such variable, listing the ones it declares.
    fn read(&self, name: &str) -> Value;

    /// The declared type of the variable `name`.
    fn declared(&self, name: &str) -> Declared;

    /// Writes `value` to the variable `name`; the next scan reads it. Fails
    /// the test when the variable's declared type cannot hold `value`.
    fn write(&mut self, name: &str, value: Value);
}

/// The declared type of a variable, as a value read from it is converted by.
pub struct Declared {
    /// The type's `iec_type_tag`.
    pub tag: u8,
    /// The variable as a message about it names it: where the backend keeps
    /// it, and its type.
    pub described: String,
}

/// Loads and initializes `program`, then hands `f` the program to drive.
/// Returns the fault that stopped initialization, if one did.
pub fn run<B: Backend, T>(
    program: &Program<B>,
    f: impl FnOnce(&mut dyn Running<Fault = Fault<B>>) -> T,
) -> Result<T, Fault<B>> {
    B::start(program, |running, fault| match fault {
        Some(fault) => Err(fault),
        None => Ok(f(running)),
    })
}

/// Compiles an analyzed library on every backend, failing the test unless
/// every backend compiles it or every backend fails to. When `run` is set,
/// one scan of each must also leave every variable with the same value as
/// on the reference, or every backend trap. Returns the reference's program.
pub fn compile_on_every_backend(
    library: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
    run: bool,
) -> Result<Program<Reference>, Diagnostic> {
    // Each backend other than the reference is checked against it here, with
    // `agree_with_reference`. The bytecode backend is the only one yet.
    let _ = run;
    Reference::compile(library, context, options)
}

/// Compiles the library on `B`, and checks it against the reference's
/// `reference`, as [`compile_on_every_backend`] does for each backend other
/// than the reference. Returns `B`'s program when both compiled it.
pub fn agree_with_reference<B: Backend>(
    reference: &Result<Program<Reference>, Diagnostic>,
    library: &Library,
    context: &SemanticContext,
    options: &CompilerOptions,
    run: bool,
) -> Option<Program<B>> {
    match (reference, B::compile(library, context, options)) {
        (Ok(reference), Ok(other)) => {
            if run {
                assert_backends_agree::<Reference, B>(reference, &other);
            }
            Some(other)
        }
        (Err(_), Err(_)) => None,
        (Ok(_), Err(diagnostic)) => panic!("only {} failed to compile: {diagnostic:?}", B::NAME),
        (Err(diagnostic), Ok(_)) => {
            panic!("only {} failed to compile: {diagnostic:?}", Reference::NAME)
        }
    }
}

/// Fails the test unless initialization alone, and then one scan, leave
/// every variable of `a` on `A` and of `b` on `B` with the same value, or
/// both trap, in initialization or in the scan.
///
/// A variable is matched by its name (REQ-LOW-codegen-131). A real equals a
/// real of the same bits, so a `NaN` equals itself.
pub fn assert_backends_agree<A: Backend, B: Backend>(a: &Program<A>, b: &Program<B>) {
    match (init_then_one_scan::<A>(a), init_then_one_scan::<B>(b)) {
        (Ok((a_init, a_scan)), Ok((b_init, b_scan))) => {
            assert_observed_agree::<A, B>("init", &a_init, &b_init);
            match (a_scan, b_scan) {
                (Ok(a_scan), Ok(b_scan)) => {
                    assert_observed_agree::<A, B>("one scan", &a_scan, &b_scan)
                }
                (Err(_), Err(_)) => {}
                (Ok(_), Err(fault)) => panic!("only {} traps in a scan: {fault:?}", B::NAME),
                (Err(fault), Ok(_)) => panic!("only {} traps in a scan: {fault:?}", A::NAME),
            }
        }
        (Err(_), Err(_)) => {}
        (Ok(_), Err(fault)) => panic!("only {} traps in init: {fault:?}", B::NAME),
        (Err(fault), Ok(_)) => panic!("only {} traps in init: {fault:?}", A::NAME),
    }
}

/// Every program and global variable of a program, and its value.
pub(super) type Observed = Vec<(String, Value)>;

/// The value of every program and global variable of `running`.
pub(super) fn observe<F>(running: &dyn Running<Fault = F>) -> Observed {
    running
        .names()
        .into_iter()
        .map(|name| {
            let value = running.read(&name);
            (name, value)
        })
        .collect()
}

type InitThenScan<B> = Result<(Observed, Result<Observed, Fault<B>>), Fault<B>>;

/// What initialization left, and what one scan after it left or the fault
/// that stopped the scan; or the fault that stopped initialization.
fn init_then_one_scan<B: Backend>(program: &Program<B>) -> InitThenScan<B> {
    run::<B, _>(program, |running| {
        let init = observe(running);
        let scan = running.scan(0).map(|()| observe(running));
        (init, scan)
    })
}

/// Fails the test unless `a` and `b` name the same variables, without regard
/// to case, and give each the same value.
fn assert_observed_agree<A: Backend, B: Backend>(when: &str, a: &Observed, b: &Observed) {
    for (name, a_value) in a {
        let b_value = b
            .iter()
            .find(|(other, _)| other.eq_ignore_ascii_case(name))
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("after {when}, only {} names the variable {name}", A::NAME));
        assert!(
            same(a_value, b_value),
            "after {when}, variable {name} differs: {} leaves {a_value:?}, {} leaves {b_value:?}",
            A::NAME,
            B::NAME
        );
    }
    if let Some((name, _)) = b
        .iter()
        .find(|(name, _)| !a.iter().any(|(other, _)| other.eq_ignore_ascii_case(name)))
    {
        panic!("after {when}, only {} names the variable {name}", B::NAME);
    }
}

/// Whether two values read from variables are the same: a real of the same
/// bits, so a `NaN` equals itself, and any other value equal.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Real(a), Value::Real(b)) => a.to_bits() == b.to_bits(),
        _ => a == b,
    }
}
