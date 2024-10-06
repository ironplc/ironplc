//! Provides definitions of objects from IEC 61131-3 common elements.
//!
//! See section 2.
use core::str::FromStr;
use lazy_static::lazy_static;
use logos::Source;
use regex::Regex;
use std::fmt::{self, Display};
use std::hash::{Hash, Hasher};
use std::num::TryFromIntError;
use std::ops::{Add, Deref};
use std::panic::Location;
use time::convert::{Day, Hour, Minute, Second};
use time::{Date, Duration, Month, PrimitiveDateTime, Time};

use dsl_macro_derive::Recurse;

use crate::configuration::{ConfigurationDeclaration, Direction};
use crate::core::{Id, Located, SourceSpan};
use crate::fold::Fold;
use crate::sfc::{Network, Sfc};
use crate::textual::*;
use crate::time::*;
use crate::visitor::Visitor;

/// Container for elementary constants.
///
/// See section 2.2.
#[derive(PartialEq, Clone, Debug, Recurse)]
pub enum ConstantKind {
    IntegerLiteral(IntegerLiteral),
    RealLiteral(RealLiteral),
    Boolean(BooleanLiteral),
    CharacterString(CharacterStringLiteral),
    Duration(DurationLiteral),
    TimeOfDay(TimeOfDayLiteral),
    Date(DateLiteral),
    DateAndTime(DateAndTimeLiteral),
    BitStringLiteral(BitStringLiteral),
}

impl ConstantKind {
    pub fn integer_literal(value: &str) -> Result<Self, &'static str> {
        Ok(Self::IntegerLiteral(IntegerLiteral {
            value: SignedInteger::new(value, SourceSpan::default())?,
            data_type: None,
        }))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Boolean {
    True,
    False,
}

/// Numeric liberals declared by 2.2.1. Numeric literals define
/// how data is expressed and are distinct from but associated with
/// data types.

/// Integer liberal. The representation is of the largest possible integer
/// and later bound to smaller types depend on context.
#[derive(Debug, Clone, PartialEq, Recurse)]
pub struct Integer {
    pub span: SourceSpan,
    /// The value in the maximum possible size. An integer is inherently
    /// an unsigned value.
    #[recurse(ignore)]
    pub value: u128,
}

impl Located for Integer {
    fn span(&self) -> SourceSpan {
        self.span.clone()
    }
}

#[derive(Debug)]
pub struct TryFromIntegerError();

impl TryFrom<Integer> for u8 {
    type Error = TryFromIntegerError;
    fn try_from(value: Integer) -> Result<u8, Self::Error> {
        value.value.try_into().map_err(|e| TryFromIntegerError {})
    }
}

impl TryFrom<Integer> for u32 {
    type Error = TryFromIntegerError;
    fn try_from(value: Integer) -> Result<u32, Self::Error> {
        value.value.try_into().map_err(|e| TryFromIntegerError {})
    }
}

impl TryFrom<Integer> for i128 {
    type Error = TryFromIntegerError;
    fn try_from(value: Integer) -> Result<i128, Self::Error> {
        value.value.try_into().map_err(|e| TryFromIntegerError {})
    }
}

impl TryFrom<Integer> for f64 {
    type Error = TryFromIntegerError;
    fn try_from(value: Integer) -> Result<f64, Self::Error> {
        let res: Result<u32, _> = value.value.try_into();
        let val = res.map_err(|e| TryFromIntegerError {})?;

        let res: f64 = val.into();
        Ok(res)
    }
}

impl From<Integer> for f32 {
    fn from(value: Integer) -> f32 {
        value.value as f32
    }
}

impl Integer {
    pub fn new(a: &str, span: SourceSpan) -> Result<Self, &'static str> {
        let without_underscore: String = a.chars().filter(|c| c.is_ascii_digit()).collect();
        without_underscore
            .as_str()
            .parse::<u128>()
            .map(|value| Integer { span, value })
            .map_err(|e| "dec")
    }

    pub fn try_hex(a: &str) -> Result<Self, &'static str> {
        if !a.starts_with("16#") {
            return Err("Non-hex start");
        }

        let (hex, remainder): (Vec<_>, Vec<_>) = a
            .chars()
            .skip(3)
            .filter(|c| *c != '_')
            .partition(|c| c.is_ascii_hexdigit());
        if !remainder.is_empty() {
            return Err("Non-hex characters");
        }
        let hex: String = hex.into_iter().collect();
        u128::from_str_radix(hex.as_str(), 16)
            .map(|value| Integer {
                span: SourceSpan::default(),
                value,
            })
            .map_err(|e| "hex")
    }

    pub fn hex(a: &str, span: SourceSpan) -> Result<Self, &'static str> {
        let without_underscore: String = a.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        u128::from_str_radix(without_underscore.as_str(), 16)
            .map(|value| Integer { span, value })
            .map_err(|e| "hex")
    }

    pub fn try_octal(a: &str) -> Result<Self, &'static str> {
        if !a.starts_with("8#") {
            return Err("Non-octal start");
        }

        let (oct, remainder): (Vec<_>, Vec<_>) = a
            .chars()
            .skip(2)
            .filter(|c| *c != '_')
            .partition(|c| matches!(c, '0'..='7'));
        if !remainder.is_empty() {
            return Err("Non-octal characters");
        }
        let oct: String = oct.into_iter().collect();
        u128::from_str_radix(oct.as_str(), 8)
            .map(|value| Integer {
                span: SourceSpan::default(),
                value,
            })
            .map_err(|e| "octal")
    }

    pub fn octal(a: &str, span: SourceSpan) -> Result<Self, &'static str> {
        let without_underscore: String = a.chars().filter(|c| matches!(c, '0'..='7')).collect();
        u128::from_str_radix(without_underscore.as_str(), 8)
            .map(|value| Integer { span, value })
            .map_err(|e| "octal")
    }

    pub fn try_binary(a: &str) -> Result<Self, &'static str> {
        if !a.starts_with("2#") {
            return Err("Non-binary start");
        }

        let (bin, remainder): (Vec<_>, Vec<_>) = a
            .chars()
            .skip(2)
            .filter(|c| *c != '_')
            .partition(|c| matches!(c, '0'..='1'));
        if !remainder.is_empty() {
            return Err("Non-binary characters");
        }
        let bin: String = bin.into_iter().collect();
        u128::from_str_radix(bin.as_str(), 2)
            .map(|value| Integer {
                span: SourceSpan::default(),
                value,
            })
            .map_err(|e| "binary")
    }

    pub fn binary(a: &str, span: SourceSpan) -> Result<Self, &'static str> {
        let without_underscore: String = a.chars().filter(|c| matches!(c, '0'..='1')).collect();
        u128::from_str_radix(without_underscore.as_str(), 2)
            .map(|value| Integer { span, value })
            .map_err(|e| "binary")
    }
}

impl fmt::Display for Integer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}", self.value))
    }
}

#[derive(Debug, Clone, PartialEq, Recurse)]
pub struct SignedInteger {
    pub value: Integer,
    #[recurse(ignore)]
    pub is_neg: bool,
}

impl SignedInteger {
    pub fn new(a: &str, span: SourceSpan) -> Result<Self, &'static str> {
        match a.chars().next() {
            Some('+') => {
                let whole = a.get(1..).ok_or("int")?;
                Ok(Self {
                    value: Integer::new(whole, span)?,
                    is_neg: false,
                })
            }
            Some('-') => {
                let whole = a.get(1..).ok_or("int")?;
                Ok(Self {
                    value: Integer::new(whole, span)?,
                    is_neg: true,
                })
            }
            _ => Ok(Self {
                value: Integer::new(a, span)?,
                is_neg: false,
            }),
        }
    }

    pub fn positive(a: &str) -> Result<Self, &'static str> {
        Ok(Self {
            value: Integer::new(a, SourceSpan::default())?,
            is_neg: false,
        })
    }

    pub fn negative(a: &str) -> Result<Self, &'static str> {
        Ok(Self {
            value: Integer::new(a, SourceSpan::default())?,
            is_neg: true,
        })
    }
}

impl From<Integer> for SignedInteger {
    fn from(value: Integer) -> SignedInteger {
        SignedInteger {
            value,
            is_neg: false,
        }
    }
}

impl TryFrom<SignedInteger> for u8 {
    type Error = TryFromIntegerError;
    fn try_from(value: SignedInteger) -> Result<u8, Self::Error> {
        value.value.try_into().map_err(|e| TryFromIntegerError {})
    }
}

impl TryFrom<SignedInteger> for u32 {
    type Error = TryFromIntegerError;
    fn try_from(value: SignedInteger) -> Result<u32, Self::Error> {
        value.value.try_into().map_err(|e| TryFromIntegerError {})
    }
}

impl TryFrom<SignedInteger> for i128 {
    type Error = TryFromIntegerError;
    fn try_from(value: SignedInteger) -> Result<i128, Self::Error> {
        let mut primitive = value.value.try_into().map_err(|e| TryFromIntegerError {})?;
        if value.is_neg {
            primitive *= -1;
        }
        Ok(primitive)
    }
}

impl fmt::Display for SignedInteger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_neg {
            f.write_fmt(format_args!("-{}", self.value))
        } else {
            f.write_fmt(format_args!("{}", self.value))
        }
    }
}

impl From<SignedInteger> for String {
    fn from(value: SignedInteger) -> Self {
        if value.is_neg {
            format!("-{}", value)
        } else {
            format!("{}", value)
        }
    }
}

/// A signed integer literal with a optional type name.
///
/// See section 2.2.1.
#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct IntegerLiteral {
    pub value: SignedInteger,
    // TODO restrict to valid integer type names
    #[recurse(ignore)]
    pub data_type: Option<ElementaryTypeName>,
}

/// The fixed point structure represents a fixed point number.
///
/// The structure keeps the whole and decimal parts as integers so that
/// we do not lose precision with floating point rounding.
#[derive(Debug, PartialEq, Clone)]
pub struct FixedPoint {
    pub span: SourceSpan,
    pub whole: u64,
    pub femptos: u64,
}

impl FixedPoint {
    pub const FRACTIONAL_UNITS: u64 = 1_000_000_000_000_000;

    pub fn parse(input: &str) -> Result<FixedPoint, &'static str> {
        // IEC 61131 allows underscores in numbers so remove those before we try to parse.
        let value: String = input
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect();

        // We want to handle left and right of the decimal as integers, so start by splitting into
        // the two parts if there is a period character.
        match value.split_once('.') {
            Some((whole, decimal)) => {
                let whole = whole
                    .parse::<u64>()
                    .map_err(|e| "floating point whole not valid")?;

                // The maximum value for a u64 integer is:
                //   18446744073709551615
                // This has up to 20 digits of precision, but we allow for only 15 to avoid wrapping
                // when parsing and so that there is a nice name. That means the decimal part can be at most
                //    999999999999999
                // To parse this, we need the add post-fix zeros as necessary so that we have the right
                // precision and then we can parse this as an u64 directly.

                // Check that we have not already exceeded the precision
                if decimal.len() > 15 {
                    return Err("floating point decimal excessive precision");
                }

                let mut decimal = decimal.to_owned();

                // Add post-fix zeros as necessary
                let number_of_zeros_to_add = 15 - decimal.len();
                let post_fix_zeros = "0".to_owned().repeat(number_of_zeros_to_add);
                decimal.push_str(post_fix_zeros.as_str());

                // Now parse this value as a u64
                let decimal = decimal
                    .parse::<u64>()
                    .map_err(|e| "floating point decimal not valid")?;

                Ok(FixedPoint {
                    span: SourceSpan::default(),
                    whole,
                    femptos: decimal,
                })
            }
            None => {
                // There is no decimal point so this is essentially a whole number
                Ok(FixedPoint {
                    span: SourceSpan::default(),
                    whole: value.parse::<u64>().map_err(|e| "u64")?,
                    femptos: 0,
                })
            }
        }
    }
}

impl From<Integer> for FixedPoint {
    fn from(value: Integer) -> Self {
        FixedPoint {
            span: value.span,
            whole: value.value as u64,
            femptos: 0,
        }
    }
}

/// See section 2.2.1.
#[derive(Debug, PartialEq, Clone)]
pub struct RealLiteral {
    pub value: f64,
    // TODO restrict to valid float type names
    pub data_type: Option<ElementaryTypeName>,
}

impl RealLiteral {
    pub fn try_parse(a: &str, tn: Option<ElementaryTypeName>) -> Result<Self, &'static str> {
        let (r, remainder): (Vec<_>, Vec<_>) = a
            .chars()
            .filter(|c| *c != '_')
            .partition(|c| c.is_ascii_digit() || *c == '.' || *c == 'E' || *c == 'e' || *c == '-');
        if !remainder.is_empty() {
            return Err("Non-real characters");
        }
        let r: String = r.into_iter().collect();
        f64::from_str(r.as_str())
            .map(|value| RealLiteral {
                value,
                data_type: tn,
            })
            .map_err(|e| "real")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BooleanLiteral {
    pub value: Boolean,
}

impl BooleanLiteral {
    pub fn new(value: Boolean) -> Self {
        Self { value }
    }
}

// See section 2.2.2
#[derive(Debug, PartialEq, Clone)]
pub struct CharacterStringLiteral {
    pub value: Vec<char>,
}

impl CharacterStringLiteral {
    pub fn new(value: Vec<char>) -> Self {
        Self { value }
    }
}

#[derive(Debug, PartialEq, Clone, Recurse)]
pub struct BitStringLiteral {
    pub value: Integer,
    // TODO restrict to valid float type names
    #[recurse(ignore)]
    pub data_type: Option<ElementaryTypeName>,
}

/// Implements a type identifier.
///
/// Types are all identifiers but we use a separate structure
/// because it is convenient to treat types and other identifiers
/// separately.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct Type {
    pub name: Id,
}

impl Type {
    /// Converts a `&str` into an `Identifier`.
    pub fn from(str: &str) -> Self {
        Self {
            name: Id::from(str),
        }
    }

    pub fn from_id(name: &Id) -> Self {
        Self { name: name.clone() }
    }
}

impl Eq for Type {}

impl Hash for Type {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Located for Type {
    fn span(&self) -> SourceSpan {
        self.name.span()
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}", &self.name))
    }
}

/// Elementary type names.
///
/// See section 2.3.1.
#[derive(Debug, PartialEq, Clone)]
pub enum ElementaryTypeName {
    BOOL,
    SINT,
    INT,
    DINT,
    LINT,
    USINT,
    UINT,
    UDINT,
    ULINT,
    REAL,
    LREAL,
    TIME,
    DATE,
    TimeOfDay,
    DateAndTime,
    STRING,
    BYTE,
    WORD,
    DWORD,
    LWORD,
    WSTRING,
}

impl ElementaryTypeName {
    pub fn as_id(&self) -> Id {
        match self {
            ElementaryTypeName::BOOL => Id::from("BOOL"),
            ElementaryTypeName::SINT => Id::from("SINT"),
            ElementaryTypeName::INT => Id::from("INT"),
            ElementaryTypeName::DINT => Id::from("DINT"),
            ElementaryTypeName::LINT => Id::from("LINT"),
            ElementaryTypeName::USINT => Id::from("USINT"),
            ElementaryTypeName::UINT => Id::from("UINT"),
            ElementaryTypeName::UDINT => Id::from("UDINT"),
            ElementaryTypeName::ULINT => Id::from("ULINT"),
            ElementaryTypeName::REAL => Id::from("REAL"),
            ElementaryTypeName::LREAL => Id::from("LREAL"),
            ElementaryTypeName::TIME => Id::from("TIME"),
            ElementaryTypeName::DATE => Id::from("DATE"),
            ElementaryTypeName::TimeOfDay => Id::from("TIME_OF_DAY"),
            ElementaryTypeName::DateAndTime => Id::from("DATE_AND_TIME"),
            ElementaryTypeName::STRING => Id::from("STRING"),
            ElementaryTypeName::BYTE => Id::from("BYTE"),
            ElementaryTypeName::WORD => Id::from("WORD"),
            ElementaryTypeName::DWORD => Id::from("DWORD"),
            ElementaryTypeName::LWORD => Id::from("LWORD"),
            ElementaryTypeName::WSTRING => Id::from("WSTRING"),
        }
    }
}

impl From<ElementaryTypeName> for Id {
    fn from(value: ElementaryTypeName) -> Id {
        match value {
            ElementaryTypeName::BOOL => Id::from("BOOL"),
            ElementaryTypeName::SINT => Id::from("SINT"),
            ElementaryTypeName::INT => Id::from("INT"),
            ElementaryTypeName::DINT => Id::from("DINT"),
            ElementaryTypeName::LINT => Id::from("LINT"),
            ElementaryTypeName::USINT => Id::from("USINT"),
            ElementaryTypeName::UINT => Id::from("UINT"),
            ElementaryTypeName::UDINT => Id::from("UDINT"),
            ElementaryTypeName::ULINT => Id::from("ULINT"),
            ElementaryTypeName::REAL => Id::from("REAL"),
            ElementaryTypeName::LREAL => Id::from("LREAL"),
            ElementaryTypeName::TIME => Id::from("TIME"),
            ElementaryTypeName::DATE => Id::from("DATE"),
            ElementaryTypeName::TimeOfDay => Id::from("TIME_OF_DAY"),
            ElementaryTypeName::DateAndTime => Id::from("DATE_AND_TIME"),
            ElementaryTypeName::STRING => Id::from("STRING"),
            ElementaryTypeName::BYTE => Id::from("BYTE"),
            ElementaryTypeName::WORD => Id::from("WORD"),
            ElementaryTypeName::DWORD => Id::from("DWORD"),
            ElementaryTypeName::LWORD => Id::from("LWORD"),
            ElementaryTypeName::WSTRING => Id::from("WSTRING"),
        }
    }
}

impl From<ElementaryTypeName> for Type {
    fn from(value: ElementaryTypeName) -> Type {
        match value {
            ElementaryTypeName::BOOL => Type::from("BOOL"),
            ElementaryTypeName::SINT => Type::from("SINT"),
            ElementaryTypeName::INT => Type::from("INT"),
            ElementaryTypeName::DINT => Type::from("DINT"),
            ElementaryTypeName::LINT => Type::from("LINT"),
            ElementaryTypeName::USINT => Type::from("USINT"),
            ElementaryTypeName::UINT => Type::from("UINT"),
            ElementaryTypeName::UDINT => Type::from("UDINT"),
            ElementaryTypeName::ULINT => Type::from("ULINT"),
            ElementaryTypeName::REAL => Type::from("REAL"),
            ElementaryTypeName::LREAL => Type::from("LREAL"),
            ElementaryTypeName::TIME => Type::from("TIME"),
            ElementaryTypeName::DATE => Type::from("DATE"),
            ElementaryTypeName::TimeOfDay => Type::from("TIME_OF_DAY"),
            ElementaryTypeName::DateAndTime => Type::from("DATE_AND_TIME"),
            ElementaryTypeName::STRING => Type::from("STRING"),
            ElementaryTypeName::BYTE => Type::from("BYTE"),
            ElementaryTypeName::WORD => Type::from("WORD"),
            ElementaryTypeName::DWORD => Type::from("DWORD"),
            ElementaryTypeName::LWORD => Type::from("LWORD"),
            ElementaryTypeName::WSTRING => Type::from("WSTRING"),
        }
    }
}

/// Kinds of derived data types.
///
/// See section 2.3.3.1
#[derive(Clone, Debug, PartialEq, Recurse)]
#[allow(clippy::large_enum_variant)]
pub enum DataTypeDeclarationKind {
    /// Derived data type the restricts permitted values from a set of identifiers.
    Enumeration(EnumerationDeclaration),
    /// Derived data type that restricts permitted values to a smaller range
    /// of the parent data type.
    Subrange(SubrangeDeclaration),
    Simple(SimpleDeclaration),
    /// Derived data type that specifies required storage space for each instance.
    Array(ArrayDeclaration),
    Structure(StructureDeclaration),
    StructureInitialization(StructureInitializationDeclaration),
    String(StringDeclaration),
    /// Data declaration that is ambiguous at parse time and must be
    /// resolved to a data type declaration after parsing all types.
    LateBound(LateBoundDeclaration),
}

/// Type declarations that are indistinguishable as parsing time.
/// These are one of the following without an initial value:
/// * enumeration
/// * structure
/// * simple
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct LateBoundDeclaration {
    /// The type name of this declaration. Other library elements
    /// refer to this this type with this name.
    pub data_type_name: Type,
    /// The referenced type name.
    ///
    /// For example, if this is an alias then this is the underlying
    /// type.
    pub base_type_name: Type,
}

/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct EnumerationDeclaration {
    pub type_name: Type,
    // TODO need to understand when the context name matters in the definition
    pub spec_init: EnumeratedSpecificationInit,
}

/// The specification of an enumeration with a possible default value.
///
/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct EnumeratedSpecificationInit {
    pub spec: EnumeratedSpecificationKind,
    pub default: Option<EnumeratedValue>,
}

impl EnumeratedSpecificationInit {
    pub fn values_and_default(values: Vec<&str>, default: &str) -> Self {
        EnumeratedSpecificationInit {
            spec: EnumeratedSpecificationKind::Values(EnumeratedSpecificationValues {
                values: values.into_iter().map(EnumeratedValue::new).collect(),
            }),
            default: Some(EnumeratedValue::new(default)),
        }
    }
}

/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub enum EnumeratedSpecificationKind {
    /// Enumeration declaration that renames another enumeration.
    TypeName(Type),
    /// Enumeration declaration that provides a list of values.
    ///
    /// Order of the values is important because the order declares the
    /// default value if no default is specified directly.
    Values(EnumeratedSpecificationValues),
}

impl EnumeratedSpecificationKind {
    pub fn from_values(values: Vec<&'static str>) -> EnumeratedSpecificationKind {
        let values = values
            .iter()
            .map(|v| EnumeratedValue {
                type_name: None,
                value: Id::from(v),
            })
            .collect();
        EnumeratedSpecificationKind::Values(EnumeratedSpecificationValues { values })
    }

    pub fn values(values: Vec<EnumeratedValue>) -> EnumeratedSpecificationKind {
        EnumeratedSpecificationKind::Values(EnumeratedSpecificationValues { values })
    }
}

/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct EnumeratedSpecificationValues {
    pub values: Vec<EnumeratedValue>,
}

/// A particular value in a enumeration.
///
/// May include a type name (especially where the enumeration would be
/// ambiguous.)
///
/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct EnumeratedValue {
    pub type_name: Option<Type>,
    pub value: Id,
}

impl EnumeratedValue {
    pub fn new(value: &str) -> Self {
        EnumeratedValue {
            type_name: None,
            value: Id::from(value),
        }
    }
}

impl Located for EnumeratedValue {
    fn span(&self) -> SourceSpan {
        match &self.type_name {
            Some(name) => SourceSpan::join2(name, &self.value),
            None => self.value.span.clone(),
        }
    }
}

/// Subrange declaration narrows a type definition to the values in a smaller
/// range. Permitted values are the inclusive range minimum through maximum
/// specified values, that is, `[minimum, maximum]`.
///
/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct SubrangeDeclaration {
    pub type_name: Type,
    pub spec: SubrangeSpecificationKind,
    pub default: Option<SignedInteger>,
}

/// Subranges can be specified by either providing a direct specification
/// or by specializing another type.
///
/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub enum SubrangeSpecificationKind {
    Specification(SubrangeSpecification),
    Type(Type),
}

/// The specification for a subrange. The specification restricts an integer
/// type to a subset of the integer range.
///
/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct SubrangeSpecification {
    /// The parent type that is being restricted.
    /// TODO how can this be restricted to integer type names?
    #[recurse(ignore)]
    pub type_name: ElementaryTypeName,
    pub subrange: Subrange,
}

/// The specification for a simple declared type.
///
/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct SimpleDeclaration {
    pub type_name: Type,
    pub spec_and_init: InitialValueAssignmentKind,
}

/// Derived data type that
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct ArrayDeclaration {
    pub type_name: Type,
    pub spec: ArraySpecificationKind,
    pub init: Vec<ArrayInitialElementKind>,
}

#[derive(Clone, Debug, PartialEq, Recurse)]
pub enum ArrayInitialElementKind {
    Constant(ConstantKind),
    EnumValue(EnumeratedValue),
    Repeated(Repeated),
}

impl ArrayInitialElementKind {
    pub fn repeated(size: Integer, init: Option<ArrayInitialElementKind>) -> Self {
        ArrayInitialElementKind::Repeated(Repeated {
            size,
            init: Box::new(init),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Repeated {
    pub size: Integer,
    pub init: Box<Option<ArrayInitialElementKind>>,
}

impl Repeated {
    pub fn recurse_visit<V: Visitor<E> + ?Sized, E>(&self, v: &mut V) -> Result<V::Value, E> {
        v.visit_integer(&self.size)?;
        self.init.as_ref().as_ref().map_or_else(
            || Ok(V::Value::default()),
            |val| v.visit_array_initial_element_kind(val),
        )
    }
}

impl Repeated {
    pub fn recurse_fold<F: Fold<E> + ?Sized, E>(self, f: &mut F) -> Result<Repeated, E> {
        let init = self
            .init
            .as_ref()
            .as_ref()
            .map(|x| f.fold_array_initial_element_kind(x.clone()))
            .transpose()?;

        Ok(Repeated {
            size: f.fold_integer(self.size)?,
            init: Box::new(init),
        })
    }
}

/// Structure declaration creates a combination of multiple elements (each having
/// a specific type) as a single unit. Components are accessed by a name. Structures
/// may be nested but must not contain an instance of itself.
///
/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct StructureDeclaration {
    /// The name of the structure.
    pub type_name: Type,
    /// The elements (components) of the structure declaration.
    pub elements: Vec<StructureElementDeclaration>,
}

/// Declares an element contained within a structure.
///
/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct StructureElementDeclaration {
    pub name: Id,
    pub init: InitialValueAssignmentKind,
}

/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct StructureInitializationDeclaration {
    pub type_name: Type,
    pub elements_init: Vec<StructureElementInit>,
}

/// Initializes a particular element in a structured type.
///
/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct StructureElementInit {
    /// The name of the element in the structure to initialize.
    pub name: Id,
    pub init: StructInitialValueAssignmentKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StringType {
    /// String of single-byte characters
    String,
    /// String of double-byte characters
    WString,
}

/// Declares a string type with restricted length.
///
/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct StringDeclaration {
    pub type_name: Type,
    pub length: Integer,
    /// The size of a single 'character'
    #[recurse(ignore)]
    pub width: StringType,
    #[recurse(ignore)]
    pub init: Option<String>,
}

/// Location prefix for directly represented variables.
///
/// See section 2.4.1.1.
#[derive(Clone, Debug, PartialEq)]
pub enum LocationPrefix {
    /// Input location
    I,
    /// Output location
    Q,
    /// Memory location
    M,
}

impl TryFrom<Option<char>> for LocationPrefix {
    type Error = &'static str;

    fn try_from(value: Option<char>) -> Result<Self, Self::Error> {
        match value {
            Some('I') => Ok(LocationPrefix::I),
            Some('Q') => Ok(LocationPrefix::Q),
            Some('M') => Ok(LocationPrefix::M),
            _ => Err("Value must be one of I, Q, M"),
        }
    }
}

impl TryFrom<&str> for LocationPrefix {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let c = value.chars().nth(0);
        LocationPrefix::try_from(c)
    }
}

/// Size prefix for directly represented variables. Defines how many bits
/// are associated with the variable.
///
/// See section 2.4.1.1.
#[derive(Clone, Debug, PartialEq)]
pub enum SizePrefix {
    /// Unspecified (indicated by asterisk)
    Unspecified,
    /// Single bit size
    Nil,
    /// Single bit size
    X,
    /// 8-bit size
    B,
    /// 16-bit size
    W,
    /// 32-bit size
    D,
    /// 64-bit size
    L,
}

impl TryFrom<Option<char>> for SizePrefix {
    type Error = &'static str;

    fn try_from(value: Option<char>) -> Result<Self, Self::Error> {
        match value {
            Some('*') => Ok(SizePrefix::Unspecified),
            Some('X') => Ok(SizePrefix::X),
            Some('B') => Ok(SizePrefix::B),
            Some('W') => Ok(SizePrefix::W),
            Some('D') => Ok(SizePrefix::D),
            Some('L') => Ok(SizePrefix::L),
            None => Ok(SizePrefix::Nil),
            _ => Err("Value must be one of *, X, B, W, D, L, NIL"),
        }
    }
}

impl TryFrom<&str> for SizePrefix {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let c = value.chars().nth(0);
        SizePrefix::try_from(c)
    }
}

/// Array specification defines a size/shape of an array.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub enum ArraySpecificationKind {
    Type(Type),
    Subranges(ArraySubranges),
}

#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct ArraySubranges {
    pub ranges: Vec<Subrange>,
    pub type_name: Type,
}

/// Subrange of an array.
///
/// See section 2.4.2.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct Subrange {
    pub start: SignedInteger,
    pub end: SignedInteger,
}

/// Container for structures that have variables.
///
/// Several different structures own variables and implementing this trait
/// allows a common handling of those items.
pub trait HasVariables {
    fn variables(&self) -> &Vec<VarDecl>;
}

#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct ProgramAccessDecl {
    pub access_name: Id,
    pub symbolic_variable: SymbolicVariableKind,
    pub type_name: Type,
    #[recurse(ignore)]
    pub direction: Option<Direction>,
}

/// Variable declaration.
///
/// See section 2.4.3.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct VarDecl {
    // Not all variable types have a "name", so the name is part of the type.
    pub identifier: VariableIdentifier,
    #[recurse(ignore)]
    pub var_type: VariableType,
    #[recurse(ignore)]
    pub qualifier: DeclarationQualifier,
    pub initializer: InitialValueAssignmentKind,
}

impl Located for VarDecl {
    fn span(&self) -> SourceSpan {
        self.identifier.span()
    }
}

impl VarDecl {
    /// Creates a variable declaration for simple type and no initialization.
    /// The declaration has type `VAR` and no qualifier.
    pub fn simple(name: &str, type_name: &str) -> Self {
        Self {
            identifier: VariableIdentifier::new_symbol(name),
            var_type: VariableType::Var,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignmentKind::simple_uninitialized(Type::from(type_name)),
        }
    }

    pub fn string(name: &str, var_type: VariableType, qualifier: DeclarationQualifier) -> Self {
        Self {
            identifier: VariableIdentifier::new_symbol(name),
            var_type,
            qualifier,
            initializer: InitialValueAssignmentKind::String(StringInitializer {
                length: None,
                width: StringType::String,
                initial_value: None,
                keyword_span: SourceSpan::default(),
            }),
        }
    }

    /// Creates a variable declaration for enumeration without having an initial value.
    /// The declaration has type `VAR` and no qualifier.
    pub fn uninitialized_enumerated(name: &str, type_name: &str) -> Self {
        VarDecl {
            identifier: VariableIdentifier::new_symbol(name),
            var_type: VariableType::Var,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignmentKind::EnumeratedType(
                EnumeratedInitialValueAssignment {
                    type_name: Type::from(type_name),
                    initial_value: None,
                },
            ),
        }
    }

    /// Creates a variable declaration for enumeration having an initial value.
    /// The declaration has type `VAR` and no qualifier.
    pub fn enumerated(name: &str, type_name: &str, initial_value: &str) -> Self {
        VarDecl {
            identifier: VariableIdentifier::new_symbol(name),
            var_type: VariableType::Var,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignmentKind::EnumeratedType(
                EnumeratedInitialValueAssignment {
                    type_name: Type::from(type_name),
                    initial_value: Some(EnumeratedValue {
                        type_name: None,
                        value: Id::from(initial_value),
                    }),
                },
            ),
        }
    }

    /// Creates a variable declaration for a function block.
    /// The declaration has type `VAR` and no qualifier.
    pub fn function_block(name: &str, type_name: &str) -> Self {
        VarDecl {
            identifier: VariableIdentifier::new_symbol(name),
            var_type: VariableType::Var,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignmentKind::FunctionBlock(
                FunctionBlockInitialValueAssignment {
                    type_name: Type::from(type_name),
                    init: vec![],
                },
            ),
        }
    }

    /// Creates a variable declaration for a structure.
    pub fn structure(name: &str, type_name: &str) -> Self {
        VarDecl {
            identifier: VariableIdentifier::new_symbol(name),
            var_type: VariableType::Var,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignmentKind::Structure(
                StructureInitializationDeclaration {
                    type_name: Type::from(type_name),
                    elements_init: vec![],
                },
            ),
        }
    }

    /// Creates a variable declaration that is ambiguous on the type.
    /// The declaration has type `VAR` and no qualifier.
    ///
    /// The language has some ambiguity for types. The late bound represents
    /// a placeholder that is later resolved once all types are known.
    pub fn late_bound(name: &str, type_name: &str) -> Self {
        VarDecl {
            identifier: VariableIdentifier::new_symbol(name),
            var_type: VariableType::Var,
            qualifier: DeclarationQualifier::Unspecified,
            initializer: InitialValueAssignmentKind::LateResolvedType(Type::from(type_name)),
        }
    }

    /// Assigns the type of the variable declaration.
    pub fn with_type(mut self, var_type: VariableType) -> Self {
        self.var_type = var_type;
        self
    }

    /// Assigns the qualifier of the variable declaration.
    pub fn with_qualifier(mut self, qualifier: DeclarationQualifier) -> Self {
        self.qualifier = qualifier;
        self
    }
}

/// Keywords for declarations.
///
/// IEC 61131-3 defines groups that can contain multiple variables. These
/// groups introduce complexity in parsing and in iterating. This
/// implementation treats the groups as labels on individual variables; in
/// effect, there are no groups.
///
/// The variable type owns the name of a variable because the type
/// defines whether a name is required.
///
/// See section 2.4.3.
#[derive(Clone, Debug, PartialEq)]
pub enum VariableType {
    /// Local to a POU.
    Var,
    /// Local to a POU. Does not need to be maintained
    /// between calls to a POU.
    VarTemp,
    /// Variable that is visible to a calling POU as an input.
    Input,
    /// Variable that is visible to calling POU and can only
    /// be ready from the calling POU. It can be written to
    /// by the POU that defines the variable.
    Output,
    /// Variable that is visible to calling POU and is readable
    /// writeable by the calling POU.
    InOut,
    /// Enables a POU to read and (possibly) write to a global
    /// variable.
    External,
    /// A variable that may be read and written by multiple
    /// POUs that also declare the variable as external.
    Global,
    /// Configurations for communication channels.
    Access,
}

/// Declaration (that does not permit a location).
///
/// See section 2.4.3.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct EdgeVarDecl {
    pub identifier: Id,
    #[recurse(ignore)]
    pub direction: EdgeDirection,
    #[recurse(ignore)]
    pub qualifier: DeclarationQualifier,
}

/// Ways of identifying variable data objects. These are used
/// in declarations (as opposed to in statements), hence this
/// does not have multi-element information (arrays and structures).
///
/// See section 2.4.1.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub enum VariableIdentifier {
    /// A variable data object that is referenced by a symbol. This is
    /// typical reference type common in most programming languages.
    Symbol(Id),

    /// A variable data object that is "directly" mapped to a hardware
    /// address. These variables are typically used for I/O.
    ///
    /// Directly represented variables have an address and an optional
    /// symbolic identifier.
    Direct(DirectVariableIdentifier),
}

impl VariableIdentifier {
    /// Create a new symbolic variable identifier.
    pub fn new_symbol(name: &str) -> Self {
        VariableIdentifier::Symbol(Id::from(name))
    }

    /// Create a new direct variable identifier.
    pub fn new_direct(name: Option<Id>, location: AddressAssignment) -> Self {
        VariableIdentifier::Direct(DirectVariableIdentifier {
            name,
            address_assignment: location,
            span: SourceSpan::default(),
        })
    }

    /// Return the symbolic identifier if there is one.
    ///
    /// Direct representations have an optional symbolic identifier.
    pub fn symbolic_id(&self) -> Option<&Id> {
        match self {
            VariableIdentifier::Symbol(name) => Option::Some(name),
            VariableIdentifier::Direct(direct) => match &direct.name {
                Some(n) => Some(n),
                None => None,
            },
        }
    }
}

impl Located for VariableIdentifier {
    fn span(&self) -> SourceSpan {
        match self {
            VariableIdentifier::Symbol(name) => name.span(),
            VariableIdentifier::Direct(direct) => direct.span(),
        }
    }
}

impl Display for VariableIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VariableIdentifier::Symbol(name) => name.fmt(f),
            VariableIdentifier::Direct(direct) => {
                if let Some(n) = &direct.name {
                    n.fmt(f)
                } else {
                    direct.address_assignment.fmt(f)
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct DirectVariableIdentifier {
    pub name: Option<Id>,
    pub address_assignment: AddressAssignment,
    pub span: SourceSpan,
}

impl Located for DirectVariableIdentifier {
    fn span(&self) -> SourceSpan {
        self.span.clone()
    }
}

/// Qualifier types for definitions.
///
/// IEC 61131-3 defines groups that share common qualifiers. These
/// groups introduce complexity in parsing and in iterating. This
/// implementation treats the groups as labels on individual variables; in
/// effect, there are no groups.
///
/// See section 2.4.3.
#[derive(Clone, Debug, PartialEq)]
pub enum DeclarationQualifier {
    // TODO Some of these are not valid for some contexts - should there be multiple
    // qualifier classes, indicate some how, or fail?
    Unspecified,
    Constant,
    /// Stored so that the value is retained through power loss.
    Retain,
    /// Stored so that the value is NOT retained through power loss.
    NonRetain,
}

/// Location assignment for a variable.
///
/// See section 2.4.3.1.
#[derive(Clone, PartialEq, Recurse)]
pub struct AddressAssignment {
    #[recurse(ignore)]
    pub location: LocationPrefix,
    #[recurse(ignore)]
    pub size: SizePrefix,
    #[recurse(ignore)]
    pub address: Vec<u32>,
    pub position: SourceSpan,
}

lazy_static! {
    static ref DIRECT_ADDRESS_UNASSIGNED: Regex = Regex::new(r"%([IQM])\*").unwrap();
    static ref DIRECT_ADDRESS: Regex = Regex::new(r"%([IQM])([XBWDL])?(\d(\.\d)*)").unwrap();
}

impl TryFrom<&str> for AddressAssignment {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if let Some(cap) = DIRECT_ADDRESS_UNASSIGNED.captures(value) {
            let location_prefix = LocationPrefix::try_from(&cap[1])?;
            return Ok(AddressAssignment {
                location: location_prefix,
                size: SizePrefix::Unspecified,
                address: vec![],
                position: SourceSpan::default(),
            });
        }

        if let Some(cap) = DIRECT_ADDRESS.captures(value) {
            let location_prefix = LocationPrefix::try_from(&cap[1])?;
            let size_prefix = SizePrefix::try_from(&cap[2])?;
            let pos: Vec<u32> = cap[3]
                .split('.')
                .map(|v| v.parse::<u32>().unwrap())
                .collect();

            return Ok(AddressAssignment {
                location: location_prefix,
                size: size_prefix,
                address: pos,
                position: SourceSpan::default(),
            });
        }

        Err("Value not convertible to direct variable")
    }
}

impl fmt::Debug for AddressAssignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AddressAssignment")
            .field("location", &self.location)
            .field("size", &self.size)
            .finish()
    }
}

impl fmt::Display for AddressAssignment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AddressAssignment")
            .field("location", &self.location)
            .field("size", &self.size)
            .finish()
    }
}

/// Container for variable specifications.

/// Container for initial value assignments. The initial value specifies a
/// "coarse grained assignment",
///
/// Declarations of variables can be associated with an initial value. The
/// initial value assignment is not necessarily compatible with the associated
/// variable.
///
/// See section 2.4.3.2.
#[derive(Clone, PartialEq, Debug, Recurse)]
pub enum InitialValueAssignmentKind {
    /// Represents no type initializer.
    ///
    /// Some types allow no initializer and this avoids nesting of the
    /// enumeration with an Option enumeration.
    None(SourceSpan),
    Simple(SimpleInitializer),
    String(StringInitializer),
    EnumeratedValues(EnumeratedValuesInitializer),
    EnumeratedType(EnumeratedInitialValueAssignment),
    FunctionBlock(FunctionBlockInitialValueAssignment),
    Subrange(SubrangeSpecificationKind),
    Structure(StructureInitializationDeclaration),
    Array(ArrayInitialValueAssignment),
    /// Type that is ambiguous until have discovered type
    /// definitions. Value is the name of the type.
    LateResolvedType(Type),
}

impl InitialValueAssignmentKind {
    /// Creates an initial value with
    pub fn simple_uninitialized(type_name: Type) -> Self {
        InitialValueAssignmentKind::Simple(SimpleInitializer {
            type_name,
            initial_value: None,
        })
    }

    /// Creates an initial value from the initializer.
    pub fn simple(type_name: &str, value: ConstantKind) -> Self {
        InitialValueAssignmentKind::Simple(SimpleInitializer {
            type_name: Type::from(type_name),
            initial_value: Some(value),
        })
    }

    /// Creates an initial value consisting of an enumeration definition and
    /// possible initial value for the enumeration.
    pub fn enumerated_values(
        values: Vec<EnumeratedValue>,
        initial_value: Option<EnumeratedValue>,
    ) -> Self {
        InitialValueAssignmentKind::EnumeratedValues(EnumeratedValuesInitializer {
            values,
            initial_value,
        })
    }
}

/// Container for initial value assignments in structures.
///
/// Initial value assignments in structures are similar to initial value
/// assignments outside of structures except that they cannot have a
/// specification (the specification is with the structure) and that the
/// initialization is required.
///
/// See section 2.4.3.2.
#[derive(Clone, PartialEq, Debug, Recurse)]
pub enum StructInitialValueAssignmentKind {
    Constant(ConstantKind),
    EnumeratedValue(EnumeratedValue),
    Array(Vec<ArrayInitialElementKind>),
    Structure(Vec<StructureElementInit>),
}

#[derive(Clone, PartialEq, Debug, Recurse)]
pub struct EnumeratedInitialValueAssignment {
    pub type_name: Type,
    pub initial_value: Option<EnumeratedValue>,
}

#[derive(Clone, PartialEq, Debug, Recurse)]
pub struct SimpleInitializer {
    pub type_name: Type,
    pub initial_value: Option<ConstantKind>,
}

/// Provides the initialization of a string variable declaration.
///
/// See sections 2.4.3.1 and 2.4.3.2.
#[derive(Clone, PartialEq, Debug, Recurse)]
pub struct StringInitializer {
    /// Maximum length of the string.
    pub length: Option<Integer>,
    /// The size of a single 'character'
    #[recurse(ignore)]
    pub width: StringType,
    /// Default value of the string. If not specified, then
    /// the default value is the empty string.
    #[recurse(ignore)]
    pub initial_value: Option<Vec<char>>,

    pub keyword_span: SourceSpan,
}

#[derive(Clone, PartialEq, Debug, Recurse)]
pub struct EnumeratedValuesInitializer {
    pub values: Vec<EnumeratedValue>,
    pub initial_value: Option<EnumeratedValue>,
}

#[derive(Clone, PartialEq, Debug, Recurse)]
pub struct FunctionBlockInitialValueAssignment {
    // In this context, the name is referring to a type, much like a function pointer
    // in other languages, so the correct representation here is a type and not
    // an identifier.
    pub type_name: Type,
    // The initializer may be empty
    pub init: Vec<StructureElementInit>,
}

/// See section 2.4.3.2. #6
#[derive(Clone, PartialEq, Debug, Recurse)]
pub struct ArrayInitialValueAssignment {
    pub spec: ArraySpecificationKind,
    pub initial_values: Vec<ArrayInitialElementKind>,
}

#[derive(Clone, PartialEq, Debug, Recurse)]
pub enum VariableSpecificationKind {
    Simple(Type),
    Subrange(SubrangeSpecificationKind),
    Enumerated(EnumeratedSpecificationKind),
    Array(ArraySpecificationKind),
    Struct(StructureDeclaration),
    String(StringSpecification),
    WString(StringSpecification),
    // Represents simple, subrange or structure
    Ambiguous(Type),
}

#[derive(Clone, PartialEq, Debug, Recurse)]
pub struct StringSpecification {
    #[recurse(ignore)]
    pub width: StringType,
    pub length: Option<Integer>,
    pub keyword_span: SourceSpan,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EdgeDirection {
    Rising,
    Falling,
}

/// Container for top-level elements that are valid top-level declarations in
/// a library.
///
/// The library element flattens data type declaration blocks so that each
/// enumeration is for a single data type declaration.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub enum LibraryElementKind {
    DataTypeDeclaration(DataTypeDeclarationKind),
    FunctionDeclaration(FunctionDeclaration),
    FunctionBlockDeclaration(FunctionBlockDeclaration),
    ProgramDeclaration(ProgramDeclaration),
    ConfigurationDeclaration(ConfigurationDeclaration),
}

///Function Program Organization Unit Declaration
///
/// A function is stateless and has no "memory". Functions
/// consists of a series of statements that provide outputs through the
/// return value and bound variables.
///
/// See section 2.5.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct FunctionDeclaration {
    pub name: Id,
    pub return_type: Type,
    pub variables: Vec<VarDecl>,
    pub edge_variables: Vec<EdgeVarDecl>,
    pub body: Vec<StmtKind>,
}

impl HasVariables for FunctionDeclaration {
    fn variables(&self) -> &Vec<VarDecl> {
        &self.variables
    }
}

/// Function Block Program Organization Unit Declaration
///
/// A function block declaration (as distinct from a particular
/// instance of a function block). The Function block instance is stateful
/// and variables retain values between invocations.
///
/// See section 2.5.2.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct FunctionBlockDeclaration {
    // It would be possible to declare this as a Type (instead of as an Identifier).
    // In this context though, the name acts more as an identifier so we use the
    // identifier.
    pub name: Id,
    pub variables: Vec<VarDecl>,
    pub edge_variables: Vec<EdgeVarDecl>,
    pub body: FunctionBlockBodyKind,
    pub span: SourceSpan,
}

impl HasVariables for FunctionBlockDeclaration {
    fn variables(&self) -> &Vec<VarDecl> {
        &self.variables
    }
}

impl Located for FunctionBlockDeclaration {
    fn span(&self) -> SourceSpan {
        self.span.clone()
    }
}

/// "Program" Program Organization Unit Declaration Declaration
///
/// Programs assembled the units into a whole that embodies a measurement
/// or control objective.
///
/// See section 2.5.3.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct ProgramDeclaration {
    pub name: Id,
    pub variables: Vec<VarDecl>,
    pub access_variables: Vec<ProgramAccessDecl>,
    pub body: FunctionBlockBodyKind,
}

impl HasVariables for ProgramDeclaration {
    fn variables(&self) -> &Vec<VarDecl> {
        &self.variables
    }
}

/// Container for type types of elements that can compose the body of a
/// function block.
///
/// See section 2.5.2.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub enum FunctionBlockBodyKind {
    Sfc(Sfc),
    Statements(Statements),
    /// A function block that has no body (and is therefore no known type).
    ///
    /// This type is not strictly valid, but highly useful and can be detected
    /// with a semantic rule.
    #[recurse(ignore)]
    Empty,
}

impl FunctionBlockBodyKind {
    /// Creates a function body that is composed of statements.
    pub fn stmts(stmts: Vec<StmtKind>) -> FunctionBlockBodyKind {
        FunctionBlockBodyKind::Statements(Statements { body: stmts })
    }

    /// Creates a function body that is composed of a sequential function block.
    pub fn sfc(networks: Vec<Network>) -> FunctionBlockBodyKind {
        FunctionBlockBodyKind::Sfc(Sfc { networks })
    }

    /// Creates an empty function body.
    pub fn empty() -> FunctionBlockBodyKind {
        FunctionBlockBodyKind::Empty
    }
}

/// Container for a library that contains top-level elements. Libraries are
/// typically represented as a file resource.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct Library {
    pub elements: Vec<LibraryElementKind>,
}

impl Default for Library {
    fn default() -> Self {
        Library::new()
    }
}

impl Library {
    // Constructs a new empty library.
    pub fn new() -> Self {
        Library {
            elements: Vec::new(),
        }
    }

    /// Extends a library with the contents of another library.
    pub fn extend(mut self, other: Library) -> Self {
        self.elements.extend(other.elements);
        self
    }
}
