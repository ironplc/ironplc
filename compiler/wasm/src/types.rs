//! Storage types of the WebAssembly target.
//!
//! A [`Ty`] says how a value of an IronPLC type is stored in the linear
//! memory: its size, its alignment and, for elementary types, the IR scalar
//! that loads and stores it. The representations follow the bytecode target
//! (ADR-0021 for durations, ADR-0025 for dates, enumerations as `DINT`), so
//! the two targets agree on every value.

use std::rc::Rc;

use ironplc_analyzer::intermediate_type::ByteSized;
use ironplc_analyzer::IntermediateType;
use ironplc_container::DEFAULT_STRING_MAX_LENGTH;
use ironplc_wasm_ir::Scalar;

/// How a value is stored.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Ty {
    /// An elementary value: its IR scalar and its IEC type name.
    Scalar { sc: Scalar, name: String },
    /// A zero-terminated string of at most `cap` characters (ABI-044).
    Str { cap: u32, wide: bool },
    /// An array.
    Array(Rc<ArrayTy>),
    /// A structure.
    Struct(Rc<StructTy>),
    /// An instance of the function block of that upper-case name.
    Fb(String),
    /// A reference (`REF_TO`, `POINTER TO`): the address of its target, 0
    /// for `NULL`.
    Ref(Box<Ty>),
}

/// An array: element type, dimensions and the distance between elements.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ArrayTy {
    pub elem: Ty,
    pub dims: Vec<(i64, i64)>,
    pub stride: u32,
}

impl ArrayTy {
    /// Number of elements.
    pub fn count(&self) -> u64 {
        self.dims
            .iter()
            .map(|(lo, hi)| (hi - lo + 1).max(0) as u64)
            .product()
    }
}

/// A structure with the offsets of its members.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct StructTy {
    pub name: String,
    pub fields: Vec<Field>,
    pub size: u32,
    pub align: u32,
}

/// A member of a structure or of a function block instance.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Field {
    pub name: String,
    pub offset: u32,
    pub ty: Ty,
}

impl StructTy {
    /// The member of that name, ignoring case.
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
    }
}

pub(crate) const BOOL: Scalar = Scalar::Bool;
pub(crate) const I32: Scalar = Scalar::Int {
    bits: 32,
    signed: true,
};
pub(crate) const U32: Scalar = Scalar::Int {
    bits: 32,
    signed: false,
};
pub(crate) const I64: Scalar = Scalar::Int {
    bits: 64,
    signed: true,
};
pub(crate) const U64: Scalar = Scalar::Int {
    bits: 64,
    signed: false,
};
pub(crate) const I16: Scalar = Scalar::Int {
    bits: 16,
    signed: true,
};
pub(crate) const F32: Scalar = Scalar::Real { bits: 32 };
pub(crate) const F64: Scalar = Scalar::Real { bits: 64 };

impl Ty {
    pub fn scalar(sc: Scalar, name: &str) -> Ty {
        Ty::Scalar {
            sc,
            name: name.to_string(),
        }
    }

    /// The IR scalar of an elementary type.
    pub fn sc(&self) -> Option<Scalar> {
        match self {
            Ty::Scalar { sc, .. } => Some(*sc),
            Ty::Ref(_) => Some(U32),
            _ => None,
        }
    }

    /// Name written in the symbol map.
    pub fn name(&self) -> String {
        match self {
            Ty::Scalar { name, .. } => name.clone(),
            Ty::Str { cap, wide } => {
                let base = if *wide { "WSTRING" } else { "STRING" };
                format!("{base}[{cap}]")
            }
            Ty::Array(a) => format!("ARRAY OF {}", a.elem.name()),
            Ty::Struct(s) => s.name.clone(),
            Ty::Fb(name) => name.clone(),
            Ty::Ref(t) => format!("REF_TO {}", t.name()),
        }
    }
}

/// Width in bits of a sized IronPLC type.
pub(crate) fn bits(size: &ByteSized) -> u8 {
    match size {
        ByteSized::B8 => 8,
        ByteSized::B16 => 16,
        ByteSized::B32 => 32,
        ByteSized::B64 => 64,
    }
}

/// The storage of an elementary IronPLC type, with its canonical name, or
/// `None` for a type that is not elementary.
pub(crate) fn elementary(it: &IntermediateType) -> Option<Ty> {
    let int = |b: u8, signed: bool| Scalar::Int { bits: b, signed };
    let ty = match it {
        IntermediateType::Bool => Ty::scalar(BOOL, "BOOL"),
        IntermediateType::Int { size } => {
            let b = bits(size);
            let name = ["SINT", "INT", "DINT", "LINT"][index(b)];
            Ty::scalar(int(b, true), name)
        }
        IntermediateType::UInt { size } => {
            let b = bits(size);
            let name = ["USINT", "UINT", "UDINT", "ULINT"][index(b)];
            Ty::scalar(int(b, false), name)
        }
        IntermediateType::Bytes { size } => {
            let b = bits(size);
            let name = ["BYTE", "WORD", "DWORD", "LWORD"][index(b)];
            Ty::scalar(int(b, false), name)
        }
        IntermediateType::Real { size } => match bits(size) {
            64 => Ty::scalar(F64, "LREAL"),
            _ => Ty::scalar(F32, "REAL"),
        },
        IntermediateType::Time { size } => match bits(size) {
            64 => Ty::scalar(I64, "LTIME"),
            _ => Ty::scalar(I32, "TIME"),
        },
        IntermediateType::Date { size } => wide_or_narrow(size, "DATE", "LDATE"),
        IntermediateType::TimeOfDay { size } => wide_or_narrow(size, "TIME_OF_DAY", "LTOD"),
        IntermediateType::DateAndTime { size } => wide_or_narrow(size, "DATE_AND_TIME", "LDT"),
        IntermediateType::String {
            max_len,
            char_width,
        } => Ty::Str {
            cap: max_len
                .map(|n| n as u32)
                .unwrap_or(DEFAULT_STRING_MAX_LENGTH as u32),
            wide: char_width.is_wide(),
        },
        IntermediateType::Subrange { base_type, .. } => return elementary(base_type),
        IntermediateType::Enumeration { .. } => Ty::scalar(I32, "DINT"),
        _ => return None,
    };
    Some(ty)
}

fn index(bits: u8) -> usize {
    match bits {
        8 => 0,
        16 => 1,
        32 => 2,
        _ => 3,
    }
}

fn wide_or_narrow(size: &ByteSized, narrow: &str, wide: &str) -> Ty {
    match bits(size) {
        64 => Ty::scalar(U64, wide),
        _ => Ty::scalar(U32, narrow),
    }
}

/// The scalar an expression of storage type `sc` is computed in: integers
/// narrower than 32 bits are computed at 32 bits (ADR-0001).
pub(crate) fn op_scalar(sc: Scalar) -> Scalar {
    match sc {
        Scalar::Int { bits, signed } if bits < 32 => Scalar::Int { bits: 32, signed },
        Scalar::Bits { bits } if bits < 32 => U32,
        Scalar::Bits { bits } => Scalar::Int {
            bits,
            signed: false,
        },
        Scalar::Duration { .. } => I64,
        other => other,
    }
}

/// Whether a scalar is an integer (bit strings included).
pub(crate) fn is_int(sc: Scalar) -> bool {
    matches!(sc, Scalar::Int { .. } | Scalar::Bits { .. })
}

/// Little-endian bytes of an integer constant in a scalar of that size.
pub(crate) fn int_bytes(value: i128, sc: Scalar) -> Vec<u8> {
    let bytes = (value as u64).to_le_bytes();
    bytes[..sc.size() as usize].to_vec()
}

/// Little-endian bytes of a real constant.
pub(crate) fn real_bytes(value: f64, sc: Scalar) -> Vec<u8> {
    match sc {
        Scalar::Real { bits: 32 } => (value as f32).to_le_bytes().to_vec(),
        _ => value.to_le_bytes().to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elementary_when_time_then_32_bit_signed_milliseconds() {
        let ty = elementary(&IntermediateType::Time {
            size: ByteSized::B32,
        })
        .unwrap();
        assert_eq!(ty.sc(), Some(I32));
        assert_eq!(ty.name(), "TIME");
    }

    #[test]
    fn elementary_when_date_then_unsigned() {
        let ty = elementary(&IntermediateType::Date {
            size: ByteSized::B32,
        })
        .unwrap();
        assert_eq!(ty.sc(), Some(U32));
    }

    #[test]
    fn op_scalar_when_sint_then_i32() {
        let sint = Scalar::Int {
            bits: 8,
            signed: true,
        };
        assert_eq!(op_scalar(sint), I32);
        assert_eq!(op_scalar(U64), U64);
    }

    #[test]
    fn int_bytes_when_negative_int_then_twos_complement() {
        assert_eq!(int_bytes(-2, I16), vec![0xfe, 0xff]);
    }

    #[test]
    fn array_count_when_two_dimensions_then_product() {
        let a = ArrayTy {
            elem: Ty::scalar(I16, "INT"),
            dims: vec![(1, 3), (0, 1)],
            stride: 2,
        };
        assert_eq!(a.count(), 6);
    }
}
