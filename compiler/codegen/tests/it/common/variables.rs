//! Resolving a variable by name, and reading or writing its value.
//!
//! The VM backend finds a variable here for [`Snapshot`](super::Snapshot)
//! and [`Session`](super::Session), so a name means the same variable to
//! every test.

use std::collections::HashMap;

use ironplc_container::debug_section::{iec_type_tag, DebugSection, VarNameEntry};
use ironplc_container::{Container, VarIndex};
use ironplc_vm::Slot;

use super::backend::Declared;
use super::slot_value;
use super::value::Value;

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
        debug.program_variable(name).unwrap_or_else(|| {
            let declared: Vec<&str> = debug
                .program_variables()
                .map(|entry| entry.name.as_str())
                .collect();
            panic!(
                "the program declares no variable `{name}`; it declares {}",
                declared.join(", ")
            )
        })
    }

    /// The name of the variable at `index` of the variable table, for a
    /// message about it; `None` when the debug section names none there.
    pub(super) fn name_at(&self, index: usize) -> Option<&'a str> {
        self.debug
            .var_names
            .iter()
            .find(|entry| usize::from(entry.var_index.raw()) == index)
            .map(|entry| entry.name.as_str())
    }

    /// Each variable this container names, beside the variable of the same
    /// owning function and name, matched without regard to case, that
    /// `other` names. `Err` with the name of a variable only one of them
    /// names.
    pub(super) fn pair_with(
        &self,
        other: &Variables<'a>,
    ) -> Result<Vec<(&'a VarNameEntry, &'a VarNameEntry)>, String> {
        let key = |entry: &VarNameEntry| (entry.function_id, entry.name.to_lowercase());
        let mut theirs: HashMap<_, Vec<&'a VarNameEntry>> = HashMap::new();
        for entry in other.debug.var_names.iter().rev() {
            theirs.entry(key(entry)).or_default().push(entry);
        }
        let mut pairs = Vec::new();
        for entry in &self.debug.var_names {
            let matched = theirs.get_mut(&key(entry)).and_then(Vec::pop);
            pairs.push((entry, matched.ok_or_else(|| entry.name.clone())?));
        }
        match theirs.values().flatten().next() {
            Some(unmatched) => Err(unmatched.name.clone()),
            None => Ok(pairs),
        }
    }

    /// Whether the debug section says where `entry`'s string lives. It does
    /// for a program's or a global's string, not for a function's.
    pub(super) fn has_string_layout(&self, entry: &VarNameEntry) -> bool {
        self.debug
            .string_layouts
            .iter()
            .any(|layout| layout.var_index == entry.var_index)
    }

    /// The names of the program and global variables a test reads by name,
    /// in the order the debug section lists them: every one but a structure,
    /// an array or a function block instance, which a test reads through a
    /// path into it.
    pub(super) fn names(&self) -> Vec<String> {
        self.debug
            .program_variables()
            .filter(|entry| {
                !matches!(
                    entry.iec_type_tag,
                    iec_type_tag::STRUCT | iec_type_tag::ARRAY | iec_type_tag::FB_INSTANCE
                )
            })
            .map(|entry| entry.name.clone())
            .collect()
    }

    /// The declared type of the program or global variable `name`.
    pub(super) fn declared(&self, name: &str) -> Declared {
        let entry = self.entry(name);
        Declared {
            tag: entry.iec_type_tag,
            described: describe(entry),
        }
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
