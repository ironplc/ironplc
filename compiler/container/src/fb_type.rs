//! Well-known function block type IDs for intrinsic dispatch.

/// TON (on-delay timer).
pub const TON: u16 = 0x0010;
/// TOF (off-delay timer).
pub const TOF: u16 = 0x0011;
/// TP (pulse timer).
pub const TP: u16 = 0x0012;
/// CTU (count up counter), counting in a signed 32-bit integer: `CTU`,
/// `CTU_INT` and `CTU_DINT`.
pub const CTU: u16 = 0x0020;
/// CTD (count down counter), counting in a signed 32-bit integer.
pub const CTD: u16 = 0x0021;
/// CTUD (count up/down counter), counting in a signed 32-bit integer.
pub const CTUD: u16 = 0x0022;
/// CTU_UDINT: CTU counting in an unsigned 32-bit integer.
pub const CTU_UDINT: u16 = 0x0023;
/// CTD_UDINT: CTD counting in an unsigned 32-bit integer.
pub const CTD_UDINT: u16 = 0x0024;
/// CTUD_UDINT: CTUD counting in an unsigned 32-bit integer.
pub const CTUD_UDINT: u16 = 0x0025;
/// CTU_LINT: CTU counting in a signed 64-bit integer.
pub const CTU_LINT: u16 = 0x0026;
/// CTD_LINT: CTD counting in a signed 64-bit integer.
pub const CTD_LINT: u16 = 0x0027;
/// CTUD_LINT: CTUD counting in a signed 64-bit integer.
pub const CTUD_LINT: u16 = 0x0028;
/// CTU_ULINT: CTU counting in an unsigned 64-bit integer.
pub const CTU_ULINT: u16 = 0x0029;
/// CTD_ULINT: CTD counting in an unsigned 64-bit integer.
pub const CTD_ULINT: u16 = 0x002A;
/// CTUD_ULINT: CTUD counting in an unsigned 64-bit integer.
pub const CTUD_ULINT: u16 = 0x002B;
/// SR (set-reset bistable, set dominant).
pub const SR: u16 = 0x0030;
/// RS (reset-set bistable, reset dominant).
pub const RS: u16 = 0x0031;
/// R_TRIG (rising edge detector).
pub const R_TRIG: u16 = 0x0040;
/// F_TRIG (falling edge detector).
pub const F_TRIG: u16 = 0x0041;
