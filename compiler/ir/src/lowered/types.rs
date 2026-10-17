//! The lowered program's type table.
//!
//! The table describes types as the language defines them: the width of an
//! `INT` is part of the language, while where a field sits in memory, how
//! many slots a value takes and how wide a reference is are each backend's
//! layout. So an entry holds no size, offset or slot count, and refers to
//! every other type by its [`TypeId`] (REQ-LOW-lowering-027).
//!
//! An entry is added only through [`TypeTable::push`], which checks it, and
//! is never changed afterwards, so a place built against a type stays right.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::ids::{PouId, Stamp, TypeId};
use super::invariant::broken;
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

/// The width of an integer or a bit string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntWidth {
    B8,
    B16,
    B32,
    B64,
}

impl IntWidth {
    pub fn bits(self) -> u8 {
        match self {
            IntWidth::B8 => 8,
            IntWidth::B16 => 16,
            IntWidth::B32 => 32,
            IntWidth::B64 => 64,
        }
    }

    /// The width a value of this width is narrowed to when stored, if it is
    /// narrower than the operation type it is operated at.
    fn narrowed(self) -> Option<NarrowWidth> {
        match self {
            IntWidth::B8 => Some(NarrowWidth::B8),
            IntWidth::B16 => Some(NarrowWidth::B16),
            IntWidth::B32 | IntWidth::B64 => None,
        }
    }
}

/// The width of a real, or of a time or date: 32 or 64 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WideWidth {
    B32,
    B64,
}

impl WideWidth {
    pub fn bits(self) -> u8 {
        match self {
            WideWidth::B32 => 32,
            WideWidth::B64 => 64,
        }
    }
}

/// A width a `Truncate` narrows an integer to: one narrower than the
/// operation type the value is computed at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NarrowWidth {
    B8,
    B16,
    B32,
}

impl NarrowWidth {
    pub fn bits(self) -> u8 {
        match self {
            NarrowWidth::B8 => 8,
            NarrowWidth::B16 => 16,
            NarrowWidth::B32 => 32,
        }
    }
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

/// One value of an enumeration.
#[derive(Clone, Debug)]
pub struct EnumerationValue {
    /// The value's name and the span of its declaration, for debug
    /// information and diagnostics only.
    pub name: DebugName,
    /// What the value is at runtime: the ordinal the analyzer decided for it,
    /// at the operation type every enumeration has.
    pub ordinal: i32,
}

/// What a type is.
#[derive(Clone, Debug)]
pub enum TypeKind {
    Bool,
    Integer {
        width: IntWidth,
        signed: bool,
    },
    BitString {
        width: IntWidth,
    },
    Real {
        width: WideWidth,
    },
    Temporal {
        kind: TemporalKind,
        width: WideWidth,
    },
    /// An enumeration, with its values in declaration order. Every
    /// enumeration is operated at the operation type of a `DINT`, as the
    /// bytecode backend does today (REQ-EN-codegen-003).
    Enumeration {
        values: Vec<EnumerationValue>,
    },
    /// The integers from `min` to `max` of the integer type `base`.
    Subrange {
        base: TypeId,
        min: i128,
        max: i128,
    },
    String(StringShape),
    /// An array of one or more dimensions, each from its lower bound to its
    /// upper bound.
    Array {
        element: TypeId,
        dimensions: Vec<(i32, i32)>,
    },
    /// A structure; its fields have no section.
    Structure {
        fields: Vec<Field>,
    },
    /// A function block; each field has a section. One that extends another
    /// function block type, `base`, begins with that type's fields
    /// (REQ-LOW-ir-176).
    FunctionBlock {
        fields: Vec<Field>,
        body: PouId,
        base: Option<TypeId>,
    },
    /// A program; each field has a section.
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

/// Every type of a lowered program, by [`TypeId`]. A
/// [`ProgramBuilder`](super::ProgramBuilder) owns it, and is the only way to
/// add to it.
#[derive(Clone, Debug)]
pub struct TypeTable {
    stamp: Stamp,
    entries: Vec<TypeEntry>,
}

impl TypeTable {
    pub(crate) fn new(stamp: Stamp) -> Self {
        Self {
            stamp,
            entries: Vec::new(),
        }
    }

    /// Adds `entry` and returns the id that names it.
    ///
    /// Every type the entry refers to is already in the table. A subrange's
    /// base is an integer type whose range holds `min` to `max`, an array has
    /// at least one dimension and each lower bound is at most its upper
    /// bound, a structure's fields have no section, a function block's or
    /// program's fields each have one, and a function block that extends
    /// another begins with that block's fields (REQ-LOW-ir-176). An entry
    /// that breaks one of these is a P9998 and is not added.
    pub(crate) fn push(
        &mut self,
        entry: TypeEntry,
        span: &SourceSpan,
    ) -> Result<TypeId, Diagnostic> {
        self.check(&entry.kind)
            .map_err(|what| broken("TypeTable::push", span, what))?;
        let id = TypeId::new(self.stamp, self.entries.len() as u32);
        self.entries.push(entry);
        Ok(id)
    }

    /// Why `kind` cannot be added to the table, if it cannot.
    fn check(&self, kind: &TypeKind) -> Result<(), String> {
        let known = |id: &TypeId| {
            if self.get(*id).is_some() {
                Ok(())
            } else {
                Err(format!("{id} is not in the table"))
            }
        };
        let sections = |fields: &[Field], with_section: bool| {
            fields.iter().try_for_each(|field| {
                known(&field.ty)?;
                if field.section.is_some() == with_section {
                    Ok(())
                } else {
                    Err(format!("field {} has the wrong section", field.name))
                }
            })
        };
        match kind {
            TypeKind::Bool
            | TypeKind::Integer { .. }
            | TypeKind::BitString { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::String(_) => Ok(()),
            TypeKind::Subrange { base, min, max } => {
                known(base)?;
                let Some(TypeKind::Integer { .. }) = self.kind(*base) else {
                    return Err(format!("subrange base {base} is not an integer"));
                };
                let (low, high) = self
                    .value_range(*base)
                    .ok_or_else(|| format!("subrange base {base} has no range"))?;
                if low <= *min && min <= max && *max <= high {
                    Ok(())
                } else {
                    Err(format!("subrange {min}..{max} is not within {base}"))
                }
            }
            TypeKind::Array {
                element,
                dimensions,
            } => {
                known(element)?;
                if !dimensions.is_empty() && dimensions.iter().all(|(low, high)| low <= high) {
                    Ok(())
                } else {
                    Err(format!(
                        "array dimensions {dimensions:?} are empty or inverted"
                    ))
                }
            }
            TypeKind::Structure { fields } => sections(fields, false),
            TypeKind::FunctionBlock { fields, base, .. } => {
                sections(fields, true)?;
                match base {
                    Some(base) => self.check_base(fields, *base),
                    None => Ok(()),
                }
            }
            TypeKind::Program { fields, .. } => sections(fields, true),
            TypeKind::Reference { target, .. } => known(target),
        }
    }

    /// Why a function block with `fields` cannot extend `base`, if it cannot:
    /// `base` is a function block type of the table, and `fields` begin with
    /// its fields, of the same names, sections and types (REQ-LOW-ir-176). So
    /// field `i` of the base is field `i` of every block that extends it, and
    /// a body that reaches field `i` of its instance reaches the same field
    /// whichever of the two its instance is.
    fn check_base(&self, fields: &[Field], base: TypeId) -> Result<(), String> {
        let Some(TypeKind::FunctionBlock {
            fields: inherited, ..
        }) = self.kind(base)
        else {
            return Err(format!("base {base} is not a function block of the table"));
        };
        let begins = inherited.len() <= fields.len()
            && inherited.iter().zip(fields).all(|(inherited, field)| {
                inherited.ty == field.ty
                    && inherited.section == field.section
                    && inherited
                        .name
                        .to_string()
                        .eq_ignore_ascii_case(&field.name.to_string())
            });
        if begins {
            Ok(())
        } else {
            Err(format!(
                "the fields do not begin with those of the base {base}"
            ))
        }
    }

    /// Whether a function block with `fields` can extend `base`: `base` is a
    /// function block type of the table, and `fields` begin with its fields,
    /// of the same names, sections and types (REQ-LOW-ir-176).
    pub fn can_extend(&self, fields: &[Field], base: TypeId) -> bool {
        self.check_base(fields, base).is_ok()
    }

    /// Whether the function block type `block` is `base`, or extends it
    /// directly or through the blocks it extends (REQ-LOW-ir-176). The walk
    /// ends, since a block's base is in the table before the block is.
    pub fn extends(&self, block: TypeId, base: TypeId) -> bool {
        let mut block = block;
        loop {
            if block == base {
                return true;
            }
            let Some(TypeKind::FunctionBlock {
                base: Some(next), ..
            }) = self.kind(block)
            else {
                return false;
            };
            block = *next;
        }
    }

    /// The entry of `id`, or `None` if the table has none, as for an id
    /// another program's table allocated.
    pub fn get(&self, id: TypeId) -> Option<&TypeEntry> {
        if id.stamp() != self.stamp {
            return None;
        }
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
            .map(|(index, entry)| (TypeId::new(self.stamp, index as u32), entry))
    }

    /// The type a value of `id` is operated at; `None` for a type that is
    /// not operated on as one value (a string or an aggregate), or for an id
    /// the table does not hold.
    pub fn operation_type(&self, id: TypeId) -> Option<ScalarType> {
        match self.kind(id)? {
            TypeKind::Bool => Some(ScalarType::Bool),
            TypeKind::Enumeration { .. } => Some(ScalarType::I32(Signedness::Signed)),
            TypeKind::Integer { width, signed } => Some(integer(*width, signedness(*signed))),
            TypeKind::BitString { width } => Some(integer(*width, Signedness::Unsigned)),
            TypeKind::Real {
                width: WideWidth::B32,
            } => Some(ScalarType::F32),
            TypeKind::Real {
                width: WideWidth::B64,
            } => Some(ScalarType::F64),
            // A duration is signed; a date or a time of day counts forward
            // from an epoch.
            TypeKind::Temporal {
                kind: TemporalKind::Time,
                width,
            } => Some(wide_integer(*width, Signedness::Signed)),
            TypeKind::Temporal {
                kind: TemporalKind::Date | TemporalKind::TimeOfDay | TemporalKind::DateAndTime,
                width,
            } => Some(wide_integer(*width, Signedness::Unsigned)),
            TypeKind::Subrange { base, .. } => self.operation_type(*base),
            TypeKind::Reference { .. } => Some(ScalarType::Ref),
            TypeKind::String(_)
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. } => None,
        }
    }

    /// How many bits a value of `id` holds that an `AssignBits` may name: the
    /// width of an integer or a bit string, through a subrange; `None` for any
    /// other type (REQ-LOW-lowering-065).
    pub fn bit_width(&self, id: TypeId) -> Option<u8> {
        match self.kind(id)? {
            TypeKind::Integer { width, .. } | TypeKind::BitString { width } => Some(width.bits()),
            TypeKind::Subrange { base, .. } => self.bit_width(*base),
            TypeKind::Bool
            | TypeKind::Enumeration { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::String(_)
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. }
            | TypeKind::Reference { .. } => None,
        }
    }

    /// The width a value of `id` is stored at, where it is narrower than the
    /// type's operation width and a store must narrow it; `None` for a type a
    /// store never narrows.
    ///
    /// A `BOOL` occupies one bit of value, but is never narrowed: every
    /// operation that yields a `BOOL` yields 0 or 1.
    pub fn narrowed_storage(&self, id: TypeId) -> Option<NarrowWidth> {
        match self.kind(id)? {
            TypeKind::Integer { width, .. } | TypeKind::BitString { width } => width.narrowed(),
            TypeKind::Subrange { base, .. } => self.narrowed_storage(*base),
            TypeKind::Bool
            | TypeKind::Enumeration { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::String(_)
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. }
            | TypeKind::Reference { .. } => None,
        }
    }

    /// The ordinal of the value at `position` of the enumeration `id`; `None`
    /// when `id` is not an enumeration or has no value there.
    pub fn enumeration_ordinal(&self, id: TypeId, position: usize) -> Option<i32> {
        match self.kind(id)? {
            TypeKind::Enumeration { values } => values.get(position).map(|value| value.ordinal),
            TypeKind::Bool
            | TypeKind::Integer { .. }
            | TypeKind::BitString { .. }
            | TypeKind::Real { .. }
            | TypeKind::Temporal { .. }
            | TypeKind::Subrange { .. }
            | TypeKind::String(_)
            | TypeKind::Array { .. }
            | TypeKind::Structure { .. }
            | TypeKind::FunctionBlock { .. }
            | TypeKind::Program { .. }
            | TypeKind::Reference { .. } => None,
        }
    }

    /// The smallest and largest value of an integer-like type `id`: an
    /// integer, a bit string, a subrange or a `BOOL`. `None` for any other.
    pub fn value_range(&self, id: TypeId) -> Option<(i128, i128)> {
        match self.kind(id)? {
            TypeKind::Bool => Some((0, 1)),
            TypeKind::Integer {
                width,
                signed: true,
            } => {
                let half = 1i128 << (width.bits() - 1);
                Some((-half, half - 1))
            }
            TypeKind::Integer {
                width,
                signed: false,
            }
            | TypeKind::BitString { width } => Some((0, (1i128 << width.bits()) - 1)),
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

    /// The field at `position` of the structure, function block or program
    /// type `id`; `None` if the type has no such field. The only way to name a
    /// field, so a [`FieldIdx`](super::FieldIdx) always names one.
    pub fn field_idx(&self, id: TypeId, position: usize) -> Option<super::FieldIdx> {
        let fields = self.fields(id)?;
        (position < fields.len()).then(|| super::FieldIdx::new(self.stamp, position as u32))
    }

    /// Whether a value of type `a` and one of type `b` have the same shape,
    /// as the analyzer compares the representations of two types: arrays of
    /// the same dimensions whose elements have the same shape, structures
    /// whose fields have the same names, in order, and the same shapes, and
    /// elementary types that are the same. Two enumerations have the same
    /// shape, as their representations compare equal in the analyzer, and two
    /// function blocks have when they are one block. The table keeps no
    /// field defaults, which the analyzer also compares, so this accepts a
    /// pair the analyzer may not.
    pub fn same_shape(&self, a: TypeId, b: TypeId) -> bool {
        if a == b {
            return true;
        }
        let (Some(a), Some(b)) = (self.kind(a), self.kind(b)) else {
            return false;
        };
        match (a, b) {
            (TypeKind::Bool, TypeKind::Bool) => true,
            (
                TypeKind::Integer { width, signed },
                TypeKind::Integer {
                    width: other_width,
                    signed: other_signed,
                },
            ) => width == other_width && signed == other_signed,
            (TypeKind::BitString { width }, TypeKind::BitString { width: other }) => width == other,
            (TypeKind::Real { width }, TypeKind::Real { width: other }) => width == other,
            (
                TypeKind::Temporal { kind, width },
                TypeKind::Temporal {
                    kind: other_kind,
                    width: other_width,
                },
            ) => kind == other_kind && width == other_width,
            (TypeKind::Enumeration { .. }, TypeKind::Enumeration { .. }) => true,
            (
                TypeKind::Subrange { base, min, max },
                TypeKind::Subrange {
                    base: other_base,
                    min: other_min,
                    max: other_max,
                },
            ) => min == other_min && max == other_max && self.same_shape(*base, *other_base),
            (TypeKind::String(shape), TypeKind::String(other)) => shape == other,
            (
                TypeKind::Array {
                    element,
                    dimensions,
                },
                TypeKind::Array {
                    element: other_element,
                    dimensions: other_dimensions,
                },
            ) => dimensions == other_dimensions && self.same_shape(*element, *other_element),
            (TypeKind::Structure { fields }, TypeKind::Structure { fields: other }) => {
                fields.len() == other.len()
                    && fields.iter().zip(other).all(|(field, other)| {
                        field.name.to_string().to_lowercase()
                            == other.name.to_string().to_lowercase()
                            && self.same_shape(field.ty, other.ty)
                    })
            }
            (
                TypeKind::FunctionBlock { body, .. },
                TypeKind::FunctionBlock {
                    body: other_body, ..
                },
            ) => body == other_body,
            (
                TypeKind::Reference { target, .. },
                TypeKind::Reference {
                    target: other_target,
                    ..
                },
            ) => self.same_shape(*target, *other_target),
            (
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
                | TypeKind::Program { .. }
                | TypeKind::Reference { .. },
                _,
            ) => false,
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

/// The integer operation type a value of `width` is operated at.
fn integer(width: IntWidth, signedness: Signedness) -> ScalarType {
    match width {
        IntWidth::B8 | IntWidth::B16 | IntWidth::B32 => ScalarType::I32(signedness),
        IntWidth::B64 => ScalarType::I64(signedness),
    }
}

fn wide_integer(width: WideWidth, signedness: Signedness) -> ScalarType {
    match width {
        WideWidth::B32 => ScalarType::I32(signedness),
        WideWidth::B64 => ScalarType::I64(signedness),
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
