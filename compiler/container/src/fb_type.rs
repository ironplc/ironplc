//! Well-known function block type IDs for intrinsic dispatch.

/// TON (on-delay timer).
pub const TON: u16 = 0x0010;
/// TOF (off-delay timer).
pub const TOF: u16 = 0x0011;
/// TP (pulse timer).
pub const TP: u16 = 0x0012;
/// CTU (count up counter).
pub const CTU: u16 = 0x0020;
/// CTD (count down counter).
pub const CTD: u16 = 0x0021;
/// CTUD (count up/down counter).
pub const CTUD: u16 = 0x0022;
/// SR (set-reset bistable, set dominant).
pub const SR: u16 = 0x0030;
/// RS (reset-set bistable, reset dominant).
pub const RS: u16 = 0x0031;
/// R_TRIG (rising edge detector).
pub const R_TRIG: u16 = 0x0040;
/// F_TRIG (falling edge detector).
pub const F_TRIG: u16 = 0x0041;
