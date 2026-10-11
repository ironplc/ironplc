//! The lowered program's type table.
//!
//! The table describes types as the language defines them: the width of an
//! `INT` is part of the language, while where a field sits in memory, how
//! many slots a value takes and how wide a reference is are each backend's
//! layout. So an entry holds no size, offset or slot count, and refers to
//! every other type by its [`TypeId`] (REQ-LOW-lowering-027).

use ironplc_dsl::core::SourceSpan;

use super::ids::{PouId, TypeId};
use super::scalar::{ScalarType, Signedness};
use crate::execution::DebugName;

/// The section a field of a function block or program is declared in.
/// A field lowering synthesizes is `Internal`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Input,
    Output,
    InOut,
    Local,
    Internal,
}

/// Whether a reference can hold `NULL`.
///
/// A `REF_TO` can; a `VAR_IN_OUT` parameter and the instance parameter of a
/// body cannot, since their call binds them to a place that exists
/// (REQ-LOW-lowering-035).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nullability {
    Nullable,
    NonNull,
}

/// Which time or date a temporal type counts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TemporalKind {
    Time,
    Date,
    TimeOfDay,
    DateAndTime,
}

/// The encoding of a string (ADR-0034).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// `STRING`: one byte per character.
    Narrow,
    /// `WSTRING`: two bytes per character.
    Wide,
}

/// The encoding and capacity of a string, in characters. The capacity is
/// always stated (REQ-LOW-lowering-054).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StringShape {
    pub encoding: Encoding,
    pub capacity: u32,
}

/// One field of a structure, function block or program.
#[derive(Clone, Debug)]
pub struct Field {
    /// The field's name and the span of its declaration, for debug
    /// information and diagnostics only.
    pub name: DebugName,
    /// The section of a function block or program field; `None` for a field
    /// of a structure.
    pub section: Option<Section>,
    pub ty: TypeId,
}

/// What a type is.
#[derive(Clone, Debug)]
pub enum TypeKind {
    Bool,
    Integer {
        bits: u8,
        signed: bool,
    },
    BitString {
        bits: u8,
    },
    Real {
        bits: u8,
    },
    Temporal {
        kind: TemporalKind,
        bits: u8,
    },
    /// An enumeration. Every enumeration is operated at the operation type of
    /// a `DINT`, as the bytecode backend does today (REQ-EN-codegen-003).
    Enumeration {
        values: Vec<DebugName>,
    },
    Subrange {
        base: TypeId,
        min: i128,
        max: i128,
    },
    String(StringShape),
    Array {
        element: TypeId,
        dimensions: Vec<(i32, i32)>,
    },
    Structure {
        fields: Vec<Field>,
    },
    FunctionBlock {
        fields: Vec<Field>,
        body: PouId,
    },
    Program {
        fields: Vec<Field>,
        body: PouId,
    },
    Reference {
        target: TypeId,
        nullability: Nullability,
    },
}

/// One type of the table: its declared name, if it has one, and what it is.
#[derive(Clone, Debug)]
pub struct TypeEntry {
    pub name: Option<DebugName>,
    pub kind: TypeKind,
}

/// Every type of a lowered program, by [`TypeId`].
#[derive(Clone, Debug, Default)]
pub struct TypeTable {
    entries: Vec<TypeEntry>,
}

impl TypeTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `entry` and returns the id that names it.
    pub fn push(&mut self, entry: TypeEntry) -> TypeId {
        let id = TypeId::new(self.entries.len() as u32);
        self.entries.push(entry);
        id
    }

    /// Replaces the entry of `id`, for a type that refers to itself through
    /// a field, such as a program whose body is allocated after its type.
    /// Returns `false`, changing nothing, if the table has no entry `id`.
    pub fn replace(&mut self, id: TypeId, entry: TypeEntry) -> bool {
        match self.entries.get_mut(id.index() as usize) {
            Some(slot) => {
                *slot = entry;
                true
            }
            None => false,
        }
    }

    /// The entry of `id`, or `None` if the table has none.
    pub fn get(&self, id: TypeId) -> Option<&TypeEntry> {
        self.entries.get(id.index() as usize)
    }

    /// What `id` is, or `None` if the table has no entry `id`.
    pub fn kind(&self, id: TypeId) -> Option<&TypeKind> {
        self.get(id).map(|entry| &entry.kind)
    }

    /// Every type, with its id, in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = (TypeId, &TypeEntry)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (TypeId::new(index as u32), entry))
    }

    /// The type a value of `id` is operated at; `None` for a type that is
    /// not operated on as one value (a string or an aggregate), or for an id
    /// the table does not hold.
    pub fn operation_type(&self, id: TypeId) -> Option<ScalarType> {
        match self.kind(id)? {
            TypeKind::Bool | TypeKind::Enumeration { .. } => Some(ScalarType::BOOL),
            TypeKind::Integer { bits, signed } => Some(integer(*bits, signedness(*signed))),
            TypeKind::BitString { bits } => Some(integer(*bits, Signedness::Unsigned)),
            TypeKind::Real { bits } if *bits <= 32 => Some(ScalarType::F32),
            TypeKind::Real { .. } => Some(ScalarType::F64),
            // A duration is signed; a date or a time of day counts forward
            // from an epoch.
            TypeKind::Temporal {
                kind: TemporalKind::Time,
                bits,
            } => Some(integer(*bits, Signedness::Signed)),
            TypeKind::Temporal {
                kind: TemporalKind::Date | TemporalKind::TimeOfDay | TemporalKind::DateAndTime,
                bits,
            } => Some(integer(*bits, Signedness::Unsigned)),
            TypeKind::Subrange { base, .. } => self.operation_type(*base),
            TypeKind::Reference { .. } => Some(ScalarType::Ref),
            TypeKind::String(_)
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. } => None,
        }
    }

    /// The width in bits a value of `id` is stored at, where it is narrower
    /// than the type's operation width and a store must narrow it; `None`
    /// for a type a store never narrows.
    ///
    /// A `BOOL` occupies one bit of value, but is never narrowed: every
    /// operation that yields a `BOOL` yields 0 or 1.
    pub fn narrowed_storage_bits(&self, id: TypeId) -> Option<u8> {
        let bits = match self.kind(id)? {
            TypeKind::Integer { bits, .. } | TypeKind::BitString { bits } => *bits,
            TypeKind::Subrange { base, .. } => return self.narrowed_storage_bits(*base),
            TypeKind::Bool
            | TypeKind::Enumeration { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::String(_)
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. }
            | TypeKind::Reference { .. } => return None,
        };
        let width = self.operation_type(id)?.bits()?;
        (bits < width).then_some(bits)
    }

    /// The smallest and largest value of an integer-like type `id`: an
    /// integer, a bit string, a subrange or a `BOOL`. `None` for any other.
    pub fn value_range(&self, id: TypeId) -> Option<(i128, i128)> {
        match self.kind(id)? {
            TypeKind::Bool => Some((0, 1)),
            TypeKind::Integer { bits, signed: true } => {
                let half = 1i128 << (bits - 1);
                Some((-half, half - 1))
            }
            TypeKind::Integer {
                bits,
                signed: false,
            }
            | TypeKind::BitString { bits } => Some((0, (1i128 << bits) - 1)),
            TypeKind::Subrange { min, max, .. } => Some((*min, *max)),
            TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::String(_)
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. }
            | TypeKind::Reference { .. } => None,
        }
    }

    /// The fields of a structure, function block or program type; `None`
    /// for any other.
    pub fn fields(&self, id: TypeId) -> Option<&[Field]> {
        match self.kind(id)? {
            TypeKind::Structure { fields }
            | TypeKind::FunctionBlock { fields, .. }
            | TypeKind::Program { fields, .. } => Some(fields),
            TypeKind::Bool
            | TypeKind::Integer { .. }
            | TypeKind::BitString { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Subrange { .. }
            | TypeKind::String(_)
            | TypeKind::Array { .. }
            | TypeKind::Reference { .. } => None,
        }
    }

    /// The target and nullability of a reference type; `None` for any other.
    pub fn reference(&self, id: TypeId) -> Option<(TypeId, Nullability)> {
        match self.kind(id)? {
            TypeKind::Reference {
                target,
                nullability,
            } => Some((*target, *nullability)),
            TypeKind::Bool
            | TypeKind::Integer { .. }
            | TypeKind::BitString { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Subrange { .. }
            | TypeKind::String(_)
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. } => None,
        }
    }
}

fn signedness(signed: bool) -> Signedness {
    if signed {
        Signedness::Signed
    } else {
        Signedness::Unsigned
    }
}

/// The integer operation type a value of `bits` bits is operated at.
fn integer(bits: u8, signedness: Signedness) -> ScalarType {
    if bits <= 32 {
        ScalarType::I32(signedness)
    } else {
        ScalarType::I64(signedness)
    }
}

impl Field {
    /// A field named `name`, declared at `span`.
    pub fn new(name: &str, span: SourceSpan, section: Option<Section>, ty: TypeId) -> Self {
        Self {
            name: DebugName::new(name, span),
            section,
            ty,
        }
    }
}
