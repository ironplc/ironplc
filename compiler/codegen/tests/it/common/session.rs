//! A program driven across several scans, read and written by name.

use ironplc_container::Container;
use ironplc_vm::{FaultContext, Slot, VmRunning};

use super::run::assert_stack_balanced;
use super::value::{FromValue, Value};
use super::variables::Variables;

/// A loaded program driven across several scans.
///
/// Variables are read and written by name, as with
/// [`Snapshot`](super::Snapshot); values persist from one scan to the next.
pub struct Session<'s, 'vm> {
    container: &'s Container,
    vm: &'s mut VmRunning<'vm>,
}

impl<'s, 'vm> Session<'s, 'vm> {
    pub(super) fn new(container: &'s Container, vm: &'s mut VmRunning<'vm>) -> Self {
        Session { container, vm }
    }

    /// Runs one scan at absolute VM time `time_us` (microseconds), then checks
    /// that the scan left the operand stack as it found it.
    pub fn scan(&mut self, time_us: u64) -> Result<(), FaultContext> {
        self.vm.run_round(time_us)?;
        assert_stack_balanced(self.vm, "after scan round");
        Ok(())
    }

    /// The value of the program or global variable `name`.
    pub fn read(&self, name: &str) -> Value {
        let variables = Variables::of(self.container);
        let entry = variables.entry(name);
        let raw = self.vm.read_variable_raw(entry.var_index).unwrap();
        variables.decode(entry, Slot::from_u64(raw), self.vm.data_region())
    }

    /// The value of `name` as a `T`. Fails the test when the value cannot be
    /// converted to a `T` without loss.
    pub fn read_as<T: FromValue>(&self, name: &str) -> T {
        let entry = Variables::of(self.container).entry(name);
        Variables::convert(name, entry, &self.read(name))
    }

    /// Writes `value` to the variable `name`; the next scan reads it. Fails
    /// the test when the variable's declared type cannot hold `value`.
    pub fn write(&mut self, name: &str, value: impl Into<Value>) {
        let entry = Variables::of(self.container).entry(name);
        let slot = Variables::encode(name, entry, &value.into());
        self.vm
            .write_variable_raw(entry.var_index, slot.as_u64())
            .unwrap();
    }
}
