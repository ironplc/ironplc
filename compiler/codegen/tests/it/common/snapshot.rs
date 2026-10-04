//! What a test can see of a program after one scan.

use ironplc_container::debug_section::{function_id, DebugSection, VarNameEntry};
use ironplc_container::Container;
use ironplc_parser::options::CompilerOptions;
use ironplc_vm::VmBuffers;

use super::run::parse_and_run;
use super::slot_value;
use super::value::{FromValue, Value};

/// A program's variables after one scan, read by name.
///
/// A name resolves through the container's debug section: the `VAR_NAME`
/// entries of the program and its globals, the table the debugger and
/// `--dump-vars` also read. No test code holds a copy of the variable layout.
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
        self.value(self.entry(name))
    }

    /// The value of `name` as a `T`. Fails the test when the value cannot be
    /// converted to a `T` without loss, naming the slot the name resolved to.
    pub fn read_as<T: FromValue>(&self, name: &str) -> T {
        let entry = self.entry(name);
        T::from_value(&self.value(entry), entry.iec_type_tag).unwrap_or_else(|reason| {
            panic!(
                "`{name}` (var[{}], {}): {reason}",
                entry.var_index.raw(),
                entry.type_name
            )
        })
    }

    fn value(&self, entry: &VarNameEntry) -> Value {
        let string_offset = self
            .debug()
            .string_layouts
            .iter()
            .find(|layout| layout.var_index == entry.var_index)
            .map(|layout| layout.data_offset);
        slot_value::decode(
            self.bufs.vars[usize::from(entry.var_index.raw())],
            entry.iec_type_tag,
            &self.bufs.data_region,
            string_offset,
        )
    }

    /// The program or global variable called `name`, matched without regard
    /// to case as IEC identifiers are. A function's locals are not candidates.
    fn entry(&self, name: &str) -> &VarNameEntry {
        let candidates = || {
            self.debug()
                .var_names
                .iter()
                .filter(|entry| entry.function_id == function_id::GLOBAL_SCOPE)
        };
        candidates()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
            .unwrap_or_else(|| {
                let declared: Vec<&str> = candidates().map(|entry| entry.name.as_str()).collect();
                panic!(
                    "the program declares no variable `{name}`; it declares {}",
                    declared.join(", ")
                )
            })
    }

    fn debug(&self) -> &DebugSection {
        self.container
            .debug_section
            .as_ref()
            .expect("the compiled container has a debug section")
    }
}
