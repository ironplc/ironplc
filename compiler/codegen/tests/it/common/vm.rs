//! The bytecode backend, run on the VM, as the end-to-end suite runs it.
//!
//! While both routes exist, each is a backend of its own here: [`VmFromAst`]
//! compiles every statement from the AST, and [`Vm`] compiles the
//! statements lowering supports from the lowered program
//! (`specs/design/lowered-program.md`, "Delivery Constraints").

use ironplc_analyzer::SemanticContext;
use ironplc_codegen::Route;
use ironplc_container::debug_section::iec_type_tag;
use ironplc_container::{Container, VarIndex};
use ironplc_dsl::common::Library;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::test_support::{load_and_start, TestVm};
use ironplc_vm::{FaultContext, Slot, VmBuffers};

use super::backend::{Backend, Declared, Running};
use super::run::{assert_stack_balanced, compile_by_route, compile_recording_route_counts};
use super::value::Value;
use super::variables::Variables;

/// The bytecode backend, compiling every statement from the AST.
pub struct VmFromAst;

/// The bytecode backend, compiling each statement lowering supports from the
/// lowered program.
pub struct Vm;

impl Backend for VmFromAst {
    const NAME: &'static str = "the AST route";
    type Program = Container;
    type Fault = FaultContext;

    fn compile(
        library: &Library,
        context: &SemanticContext,
        options: &CompilerOptions,
    ) -> Result<Container, Diagnostic> {
        compile_by_route(library, context, options, Route::Ast)
    }

    fn start<T>(
        program: &Container,
        f: impl FnOnce(&mut dyn Running<Fault = FaultContext>, Option<FaultContext>) -> T,
    ) -> T {
        start(program, f)
    }
}

impl Backend for Vm {
    const NAME: &'static str = "the lowered route";
    type Program = Container;
    type Fault = FaultContext;

    fn compile(
        library: &Library,
        context: &SemanticContext,
        options: &CompilerOptions,
    ) -> Result<Container, Diagnostic> {
        compile_recording_route_counts(library, context, options, Route::PreferLowered)
    }

    fn start<T>(
        program: &Container,
        f: impl FnOnce(&mut dyn Running<Fault = FaultContext>, Option<FaultContext>) -> T,
    ) -> T {
        start(program, f)
    }
}

/// Loads `container` on the VM, which runs its `init`, and hands `f` the
/// program, with the trap that stopped `init`, if one did.
fn start<T>(
    container: &Container,
    f: impl FnOnce(&mut dyn Running<Fault = FaultContext>, Option<FaultContext>) -> T,
) -> T {
    let mut bufs = VmBuffers::from_container(container);
    match load_and_start(container, &mut bufs) {
        Ok(mut vm) => {
            assert_stack_balanced(&vm, "after init");
            let state = Loaded::Started(&mut vm);
            f(&mut VmRunning { container, state }, None)
        }
        Err(fault) => {
            let state = Loaded::Stopped(&bufs);
            f(&mut VmRunning { container, state }, Some(fault))
        }
    }
}

/// A program the VM loaded.
struct VmRunning<'r, 'vm> {
    container: &'r Container,
    state: Loaded<'r, 'vm>,
}

/// Whether the VM runs the program, or holds what its `init` left when a
/// trap stopped it.
enum Loaded<'r, 'vm> {
    Started(&'r mut TestVm<'vm>),
    Stopped(&'r VmBuffers),
}

impl<'vm> VmRunning<'_, 'vm> {
    fn started(&mut self) -> &mut TestVm<'vm> {
        match &mut self.state {
            Loaded::Started(vm) => vm,
            Loaded::Stopped(_) => panic!("the program's init trapped, so it does not run"),
        }
    }
}

impl Running for VmRunning<'_, '_> {
    type Fault = FaultContext;

    /// Runs one scan, then checks that the scan left the operand stack as it
    /// found it.
    fn scan(&mut self, time_us: u64) -> Result<(), FaultContext> {
        let vm = self.started();
        vm.run_round(time_us)?;
        assert_stack_balanced(vm, "after scan round");
        Ok(())
    }

    fn names(&self) -> Vec<String> {
        Variables::of(self.container).names()
    }

    fn read(&self, name: &str) -> Value {
        let variables = Variables::of(self.container);
        let entry = variables.entry(name);
        match &self.state {
            Loaded::Started(vm) => {
                let raw = vm.read_variable_raw(entry.var_index).unwrap();
                variables.decode(entry, Slot::from_u64(raw), vm.data_region())
            }
            Loaded::Stopped(bufs) => {
                let slot = bufs.vars[usize::from(entry.var_index.raw())];
                variables.decode(entry, slot, &bufs.data_region)
            }
        }
    }

    fn declared(&self, name: &str) -> Declared {
        Variables::of(self.container).declared(name)
    }

    fn write(&mut self, name: &str, value: Value) {
        let entry = Variables::of(self.container).entry(name);
        let slot = Variables::encode(name, entry, &value);
        self.started()
            .write_variable_raw(entry.var_index, slot.as_u64())
            .unwrap();
    }
}

/// Fails the test unless `init` alone, and then one scan, leave every
/// variable and the data region of `ast` and of `lowered` with the same
/// values.
///
/// While both routes exist they share one layout, so this compares what the
/// backend-neutral comparison cannot read by name: the variables inside a
/// structure, an array or a function block instance, a function's
/// variables, and the data region.
pub(super) fn assert_same_storage(ast: &Container, lowered: &Container) {
    // Whether each traps, and where, is the backend-neutral comparison's to
    // check; this compares only what both runs left.
    let (Ok((ast_init, ast_scan)), Ok((lowered_init, lowered_scan))) =
        (run_init_then_one_scan(ast), run_init_then_one_scan(lowered))
    else {
        return;
    };
    assert_states_agree("init", ast, lowered, &ast_init, &lowered_init);
    if let (Ok(ast_scan), Ok(lowered_scan)) = (ast_scan, lowered_scan) {
        assert_states_agree("one scan", ast, lowered, &ast_scan, &lowered_scan);
    }
}

/// What a run left in the variable table and the data region.
struct Storage {
    vars: Vec<Slot>,
    data_region: Vec<u8>,
}

impl Storage {
    fn of(vm: &TestVm<'_>) -> Self {
        let vars = (0..vm.num_variables())
            .map(|index| {
                vm.read_variable_raw(VarIndex::new(index))
                    .map(Slot::from_u64)
                    .expect("every slot below the variable count is readable")
            })
            .collect();
        Storage {
            vars,
            data_region: vm.data_region().to_vec(),
        }
    }
}

/// Loads `container`, which runs `init`, then runs one scan. Returns what
/// `init` left and what the scan left, or the trap `init` stopped at.
fn run_init_then_one_scan(
    container: &Container,
) -> Result<(Storage, Result<Storage, FaultContext>), FaultContext> {
    let mut bufs = VmBuffers::from_container(container);
    let mut vm = load_and_start(container, &mut bufs)?;
    assert_stack_balanced(&vm, "after init");
    let after_init = Storage::of(&vm);
    let after_scan = vm.run_round(0).map(|()| {
        assert_stack_balanced(&vm, "after scan round");
        Storage::of(&vm)
    });
    Ok((after_init, after_scan))
}

/// Fails the test unless the two routes left every variable with the same
/// value after `when` (REQ-LOW-codegen-131).
///
/// A variable is matched by its owning function and its name, not by its
/// slot. A string the debug section lays out is compared by its content, and
/// any other variable by its slot's bits, so a `NaN` equals itself; a
/// function's string has no layout entry, and its content is in the data
/// region. A slot the debug section names no variable for, such as a scratch
/// slot, is compared by its index, as the data region is compared whole: both
/// hold while the two routes share one layout.
fn assert_states_agree(
    when: &str,
    ast: &Container,
    lowered: &Container,
    ast_state: &Storage,
    lowered_state: &Storage,
) {
    let ast_vars = Variables::of(ast);
    let lowered_vars = Variables::of(lowered);
    let pairs = ast_vars
        .pair_with(&lowered_vars)
        .unwrap_or_else(|name| panic!("after {when}, only one route names the variable {name}"));
    for (ast_entry, lowered_entry) in pairs {
        let ast_slot = ast_state.vars[usize::from(ast_entry.var_index.raw())];
        let lowered_slot = lowered_state.vars[usize::from(lowered_entry.var_index.raw())];
        let is_string = matches!(
            ast_entry.iec_type_tag,
            iec_type_tag::STRING | iec_type_tag::WSTRING
        );
        let laid_out =
            ast_vars.has_string_layout(ast_entry) && lowered_vars.has_string_layout(lowered_entry);
        let agree = match is_string && laid_out {
            true => {
                ast_vars.decode(ast_entry, ast_slot, &ast_state.data_region)
                    == lowered_vars.decode(lowered_entry, lowered_slot, &lowered_state.data_region)
            }
            false => ast_slot == lowered_slot,
        };
        assert!(
            agree,
            "after {when}, the AST and lowered routes leave variable {} with different values",
            ast_entry.name
        );
    }
    assert_eq!(
        ast_state.vars.len(),
        lowered_state.vars.len(),
        "the AST and lowered routes have variable tables of different sizes"
    );
    for (index, (a, l)) in ast_state.vars.iter().zip(&lowered_state.vars).enumerate() {
        if ast_vars.name_at(index).is_none() {
            assert_eq!(
                a, l,
                "after {when}, the AST and lowered routes leave unnamed slot {index} with different values"
            );
        }
    }
    assert_eq!(
        ast_state.data_region, lowered_state.data_region,
        "after {when}, the AST and lowered routes leave the data region with different values"
    );
}
