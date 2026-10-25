//! Which standard function block a POU is.
//!
//! Lowering does not generate the standard function blocks yet (issue 2288),
//! so a call of one names a POU with no body, and the POU names the block it
//! stands for ([`super::PouTable::standard_block`]). A backend runs its own
//! implementation of that block. A standard function block is known by this
//! identity, not by its spelling: a user-defined block named like one is a
//! user-defined block.

/// The integer type a standard counter counts in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CounterWidth {
    /// `CTU`, `CTD`, `CTUD`: counts in `INT`.
    Int,
    /// `CTU_DINT`, `CTD_DINT`, `CTUD_DINT`
    Dint,
    /// `CTU_LINT`, `CTD_LINT`, `CTUD_LINT`
    Lint,
    /// `CTU_UDINT`, `CTD_UDINT`, `CTUD_UDINT`
    Udint,
    /// `CTU_ULINT`, `CTD_ULINT`, `CTUD_ULINT`
    Ulint,
}

/// A standard function block of IEC 61131-3.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StandardFunctionBlock {
    /// Set-dominant bistable.
    Sr,
    /// Reset-dominant bistable.
    Rs,
    /// Rising edge detection.
    RTrig,
    /// Falling edge detection.
    FTrig,
    /// On-delay timer.
    Ton,
    /// Off-delay timer.
    Tof,
    /// Pulse timer.
    Tp,
    /// Up counter.
    Ctu(CounterWidth),
    /// Down counter.
    Ctd(CounterWidth),
    /// Up-down counter.
    Ctud(CounterWidth),
}
