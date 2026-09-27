//! Literals as IR constants, in the units of the bytecode target.

use ironplc_dsl::common::{Boolean, ConstantKind};
use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_wasm_ir::{Const, Scalar};

/// The value of a scalar literal in the scalar `sc`. Integers keep the low
/// bits of their two's complement pattern, as the bytecode's truncation does;
/// durations and times of day are milliseconds and dates seconds since
/// 1970-01-01 (ADR-0021, ADR-0025).
pub(crate) fn constant(
    c: &ConstantKind,
    sc: Scalar,
    span: &SourceSpan,
) -> Result<Const, Diagnostic> {
    let int = |v: i128| -> Result<Const, Diagnostic> {
        Ok(match sc {
            Scalar::Bool => Const::Bool(v != 0),
            Scalar::Real { .. } => real(v as f64, sc),
            _ => Const::Int(wrap(v, sc)),
        })
    };
    match c {
        ConstantKind::IntegerLiteral(lit) => {
            let v = lit.value.value.value as i128;
            int(if lit.value.is_neg { -v } else { v })
        }
        ConstantKind::BitStringLiteral(lit) => int(lit.value.value as i128),
        ConstantKind::Boolean(b) => int(matches!(b.value, Boolean::True) as i128),
        ConstantKind::Duration(d) => int(d.interval.whole_milliseconds()),
        ConstantKind::TimeOfDay(t) => int(t.whole_milliseconds() as i128),
        ConstantKind::Date(d) => int(d.seconds_since_epoch() as i128),
        ConstantKind::DateAndTime(d) => int(d.seconds_since_epoch() as i128),
        ConstantKind::RealLiteral(r) => match sc {
            Scalar::Real { .. } => Ok(real(r.value, sc)),
            _ => Err(Diagnostic::not_implemented(Label::span(
                span.clone(),
                "A real literal in an integer context in the WebAssembly target",
            ))),
        },
        ConstantKind::CharacterString(_) => Err(Diagnostic::not_implemented(Label::span(
            span.clone(),
            "A string literal as a scalar in the WebAssembly target",
        ))),
    }
}

fn real(v: f64, sc: Scalar) -> Const {
    match sc {
        Scalar::Real { bits: 32 } => Const::Real(v as f32 as f64),
        _ => Const::Real(v),
    }
}

/// `v` reduced to the range of the integer scalar `sc`.
pub(crate) fn wrap(v: i128, sc: Scalar) -> i128 {
    let (bits, signed) = match sc {
        Scalar::Int { bits, signed } => (bits as u32, signed),
        Scalar::Bits { bits } => (bits as u32, false),
        Scalar::Duration { .. } => (64, true),
        _ => return v,
    };
    let m = v & ((1i128 << bits) - 1);
    if signed && m >= 1i128 << (bits - 1) {
        m - (1i128 << bits)
    } else {
        m
    }
}

#[cfg(test)]
mod tests {
    use super::wrap;
    use crate::types::{I32, U32};

    #[test]
    fn wrap_when_value_above_signed_range_then_negative() {
        assert_eq!(wrap(0xFFFF_FFFF, I32), -1);
        assert_eq!(wrap(-1, U32), 0xFFFF_FFFF);
    }
}
