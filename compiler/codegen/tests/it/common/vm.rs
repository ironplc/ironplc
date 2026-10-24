//! The bytecode backend, run on the VM, as the end-to-end suite runs it.

use ironplc_analyzer::SemanticContext;
use ironplc_container::Container;
use ironplc_dsl::common::Library;
use ironplc_dsl::diagnostic::Diagnostic;
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::test_support::{load_and_start, TestVm};
use ironplc_vm::{FaultContext, Slot, VmBuffers};

use super::backend::{Backend, Declared, Running};
use super::run::{assert_stack_balanced, compile_on_vm};
use super::value::Value;
use super::variables::Variables;

/// The bytecode backend.
pub struct Vm;

impl Backend for Vm {
    const NAME: &'static str = "the bytecode backend";
    type Program = Container;
    type Fault = FaultContext;

    fn compile(
        library: &Library,
        context: &SemanticContext,
        options: &CompilerOptions,
    ) -> Result<Container, Diagnostic> {
        compile_on_vm(library, context, options)
    }

    /// Loads `program` on the VM, which runs its `init`.
    fn start<T>(
        program: &Container,
        f: impl FnOnce(&mut dyn Running<Fault = FaultContext>, Option<FaultContext>) -> T,
    ) -> T {
        let mut bufs = VmBuffers::from_container(program);
        match load_and_start(program, &mut bufs) {
            Ok(mut vm) => {
                assert_stack_balanced(&vm, "after init");
                let state = Loaded::Started(&mut vm);
                f(
                    &mut VmRunning {
                        container: program,
                        state,
                    },
                    None,
                )
            }
            Err(fault) => {
                let state = Loaded::Stopped(&bufs);
                f(
                    &mut VmRunning {
                        container: program,
                        state,
                    },
                    Some(fault),
                )
            }
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
