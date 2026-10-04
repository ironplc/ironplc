//! What a test can see of a program after one scan.

use ironplc_container::Container;
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::VmBuffers;

use super::run::parse_and_run;
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
        let (container, bufs) = parse_and_run(source, options);
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
