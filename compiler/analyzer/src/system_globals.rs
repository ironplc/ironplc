//! The globals the compiler provides when `allow_system_uptime_global` is on.
//!
//! The `__` prefix marks a name only the compiler can provide (see
//! `specs/design/compatibility-libraries.md`, *Non-Goals*). Every place that
//! seeds, types, lays out or guards these names reads this one table, so a
//! new reserved global is added here and nowhere else.

/// A compiler-provided global: what it is in the execution model, its name
/// and the name of its type.
pub struct SystemGlobal {
    pub kind: ironplc_ir::execution::SystemGlobal,
    pub name: &'static str,
    pub type_name: &'static str,
}

/// The implicit uptime globals, in variable-table order: the VM writes
/// slot 0 and slot 1 with these, so codegen lays them out first.
pub const SYSTEM_UPTIME_GLOBALS: [SystemGlobal; 2] = [
    SystemGlobal {
        kind: ironplc_ir::execution::SystemGlobal::UpTime,
        name: "__SYSTEM_UP_TIME",
        type_name: "TIME",
    },
    SystemGlobal {
        kind: ironplc_ir::execution::SystemGlobal::UpLTime,
        name: "__SYSTEM_UP_LTIME",
        type_name: "LTIME",
    },
];
