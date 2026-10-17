//! The standard function blocks the VM runs natively, by `FB_CALL` type id.
//!
//! [`Intrinsic::of`] is the one place that maps a type id to the VM's block
//! for it, so the `FB_CALL` dispatch matches an enum, not raw ids, and a
//! counter's width cannot be named in one place and missed in another.

use ironplc_container::opcode::fb_type;

use crate::intrinsic::CounterWidth;

/// A standard function block the VM runs natively.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intrinsic {
    Ton,
    Tof,
    Tp,
    Ctu(CounterWidth),
    Ctd(CounterWidth),
    Ctud(CounterWidth),
    Sr,
    Rs,
    RTrig,
    FTrig,
}

impl Intrinsic {
    /// The VM's block for `FB_CALL` type id `type_id`, or `None` for an id
    /// that names no standard block: a user-defined block's, or none.
    pub fn of(type_id: u16) -> Option<Intrinsic> {
        let intrinsic = match type_id {
            fb_type::TON => Intrinsic::Ton,
            fb_type::TOF => Intrinsic::Tof,
            fb_type::TP => Intrinsic::Tp,
            fb_type::CTU_DINT => Intrinsic::Ctu(CounterWidth::I32),
            fb_type::CTD_DINT => Intrinsic::Ctd(CounterWidth::I32),
            fb_type::CTUD_DINT => Intrinsic::Ctud(CounterWidth::I32),
            fb_type::CTU_UDINT => Intrinsic::Ctu(CounterWidth::U32),
            fb_type::CTD_UDINT => Intrinsic::Ctd(CounterWidth::U32),
            fb_type::CTUD_UDINT => Intrinsic::Ctud(CounterWidth::U32),
            fb_type::CTU_LINT => Intrinsic::Ctu(CounterWidth::I64),
            fb_type::CTD_LINT => Intrinsic::Ctd(CounterWidth::I64),
            fb_type::CTUD_LINT => Intrinsic::Ctud(CounterWidth::I64),
            fb_type::CTU_ULINT => Intrinsic::Ctu(CounterWidth::U64),
            fb_type::CTD_ULINT => Intrinsic::Ctd(CounterWidth::U64),
            fb_type::CTUD_ULINT => Intrinsic::Ctud(CounterWidth::U64),
            fb_type::SR => Intrinsic::Sr,
            fb_type::RS => Intrinsic::Rs,
            fb_type::R_TRIG => Intrinsic::RTrig,
            fb_type::F_TRIG => Intrinsic::FTrig,
            _ => return None,
        };
        Some(intrinsic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use spec_test_macro::spec_test;

    #[rstest]
    #[case::ton(fb_type::TON, Intrinsic::Ton)]
    #[case::tof(fb_type::TOF, Intrinsic::Tof)]
    #[case::tp(fb_type::TP, Intrinsic::Tp)]
    #[case::sr(fb_type::SR, Intrinsic::Sr)]
    #[case::rs(fb_type::RS, Intrinsic::Rs)]
    #[case::r_trig(fb_type::R_TRIG, Intrinsic::RTrig)]
    #[case::f_trig(fb_type::F_TRIG, Intrinsic::FTrig)]
    fn of_when_standard_block_type_then_its_block(
        #[case] type_id: u16,
        #[case] expected: Intrinsic,
    ) {
        assert_eq!(Intrinsic::of(type_id), Some(expected));
    }

    #[spec_test(REQ_CW_vm_001)]
    #[rstest]
    #[case::ctu_dint(fb_type::CTU_DINT, Intrinsic::Ctu(CounterWidth::I32))]
    #[case::ctd_dint(fb_type::CTD_DINT, Intrinsic::Ctd(CounterWidth::I32))]
    #[case::ctud_dint(fb_type::CTUD_DINT, Intrinsic::Ctud(CounterWidth::I32))]
    #[case::ctu_udint(fb_type::CTU_UDINT, Intrinsic::Ctu(CounterWidth::U32))]
    #[case::ctd_udint(fb_type::CTD_UDINT, Intrinsic::Ctd(CounterWidth::U32))]
    #[case::ctud_udint(fb_type::CTUD_UDINT, Intrinsic::Ctud(CounterWidth::U32))]
    #[case::ctu_lint(fb_type::CTU_LINT, Intrinsic::Ctu(CounterWidth::I64))]
    #[case::ctd_lint(fb_type::CTD_LINT, Intrinsic::Ctd(CounterWidth::I64))]
    #[case::ctud_lint(fb_type::CTUD_LINT, Intrinsic::Ctud(CounterWidth::I64))]
    #[case::ctu_ulint(fb_type::CTU_ULINT, Intrinsic::Ctu(CounterWidth::U64))]
    #[case::ctd_ulint(fb_type::CTD_ULINT, Intrinsic::Ctd(CounterWidth::U64))]
    #[case::ctud_ulint(fb_type::CTUD_ULINT, Intrinsic::Ctud(CounterWidth::U64))]
    fn of_when_counter_type_then_counter_at_its_width(
        #[case] type_id: u16,
        #[case] expected: Intrinsic,
    ) {
        assert_eq!(Intrinsic::of(type_id), Some(expected));
    }

    #[rstest]
    #[case::user_block(0x1000)]
    #[case::past_last_counter(0x002C)]
    #[case::zero(0x0000)]
    fn of_when_no_standard_block_type_then_none(#[case] type_id: u16) {
        assert_eq!(Intrinsic::of(type_id), None);
    }
}
