//! Resolving a variable by name, and reading or writing its value.
//!
//! [`Snapshot`](super::Snapshot) and [`Session`](super::Session) both find a
//! variable here, so a name means the same variable to every test.

use ironplc_container::debug_section::{function_id, DebugSection, VarNameEntry};
use ironplc_container::{Container, VarIndex};
use ironplc_vm::Slot;

use super::slot_value;
use super::value::{FromValue, Value};

/// The program and global variables of a compiled container.
///
/// A name resolves through the container's debug section: the `VAR_NAME`
/// entries of the program and its globals, the table the debugger and
/// `--dump-vars` also read. No test code holds a copy of the variable layout.
pub(super) struct Variables<'a> {
    debug: &'a DebugSection,
}

impl<'a> Variables<'a> {
    pub(super) fn of(container: &'a Container) -> Self {
        Variables {
            debug: container
                .debug_section
                .as_ref()
                .expect("the compiled container has a debug section"),
        }
    }

    /// The program or global variable called `name`, matched without regard
    /// to case as IEC identifiers are. A function's locals are not candidates.
    pub(super) fn entry(&self, name: &str) -> &'a VarNameEntry {
        let debug = self.debug;
        let candidates = || {
            debug
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

    /// The value `entry`'s variable holds in `slot`, with its string content,
    /// if any, in `data_region`.
    pub(super) fn decode(&self, entry: &VarNameEntry, slot: Slot, data_region: &[u8]) -> Value {
        let string_offset = self
            .debug
            .string_layouts
            .iter()
            .find(|layout| layout.var_index == entry.var_index)
            .map(|layout| layout.data_offset);
        slot_value::decode(slot, entry.iec_type_tag, data_region, string_offset)
    }

    /// The slot `entry`'s variable holds `value` in. Fails the test when the
    /// declared type cannot hold `value`.
    pub(super) fn encode(name: &str, entry: &VarNameEntry, value: &Value) -> Slot {
        slot_value::encode(value, entry.iec_type_tag)
            .unwrap_or_else(|reason| panic!("writing `{name}` {}: {reason}", describe(entry)))
    }

    /// `value` as a `T`. Fails the test when it cannot be converted without
    /// loss, naming the slot the name resolved to.
    pub(super) fn convert<T: FromValue>(name: &str, entry: &VarNameEntry, value: &Value) -> T {
        T::from_value(value, entry.iec_type_tag)
            .unwrap_or_else(|reason| panic!("`{name}` {}: {reason}", describe(entry)))
    }
}

fn describe(entry: &VarNameEntry) -> String {
    format!("(var[{}], {})", entry.var_index.raw(), entry.type_name)
}

/// The slot the VM holds the program or global variable `name` in, for the
/// tests whose subject is the VM or the layout. Every other test reads by
/// name and never sees a slot.
pub fn vm_var_index(container: &Container, name: &str) -> VarIndex {
    Variables::of(container).entry(name).var_index
}
