//! Provides definitions of objects from IEC 61131-3 common elements.
//!
//! See section 2.
use core::str::FromStr;
use lazy_static::lazy_static;
use regex::Regex;
use std::fmt::{self, Display};
use std::hash::{Hash, Hasher};

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

// Numeric liberals declared by 2.2.1. Numeric literals define
// how data is expressed and are distinct from but associated with
// data types.

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
        value.value.try_into().map_err(|_e| TryFromIntegerError {})
    }
}

impl TryFrom<Integer> for u32 {
    type Error = TryFromIntegerError;
    fn try_from(value: Integer) -> Result<u32, Self::Error> {
        value.value.try_into().map_err(|_e| TryFromIntegerError {})
    }
}

impl TryFrom<Integer> for i128 {
    type Error = TryFromIntegerError;
    fn try_from(value: Integer) -> Result<i128, Self::Error> {
        value.value.try_into().map_err(|_e| TryFromIntegerError {})
    }
}

impl TryFrom<Integer> for f64 {
    type Error = TryFromIntegerError;
    fn try_from(value: Integer) -> Result<f64, Self::Error> {
        let res: Result<u32, _> = value.value.try_into();
        let val = res.map_err(|_e| TryFromIntegerError {})?;

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
            .map_err(|_e| "dec")
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
            .map_err(|_e| "hex")
    }

    pub fn hex(a: &str, span: SourceSpan) -> Result<Self, &'static str> {
        let without_underscore: String = a.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        u128::from_str_radix(without_underscore.as_str(), 16)
            .map(|value| Integer { span, value })
            .map_err(|_e| "hex")
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
            .map_err(|_e| "octal")
    }

    pub fn octal(a: &str, span: SourceSpan) -> Result<Self, &'static str> {
        let without_underscore: String = a.chars().filter(|c| matches!(c, '0'..='7')).collect();
        u128::from_str_radix(without_underscore.as_str(), 8)
            .map(|value| Integer { span, value })
            .map_err(|_e| "octal")
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
            .map_err(|_e| "binary")
    }

    pub fn binary(a: &str, span: SourceSpan) -> Result<Self, &'static str> {
        let without_underscore: String = a.chars().filter(|c| matches!(c, '0'..='1')).collect();
        u128::from_str_radix(without_underscore.as_str(), 2)
            .map(|value| Integer { span, value })
            .map_err(|_e| "binary")
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
        value.value.try_into().map_err(|_e| TryFromIntegerError {})
    }
}

impl TryFrom<SignedInteger> for u32 {
    type Error = TryFromIntegerError;
    fn try_from(value: SignedInteger) -> Result<u32, Self::Error> {
        value.value.try_into().map_err(|_e| TryFromIntegerError {})
    }
}

impl TryFrom<SignedInteger> for i128 {
    type Error = TryFromIntegerError;
    fn try_from(value: SignedInteger) -> Result<i128, Self::Error> {
        let mut primitive = value
            .value
            .try_into()
            .map_err(|_e| TryFromIntegerError {})?;
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
            format!("-{value}")
        } else {
            format!("{value}")
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
                    .map_err(|_e| "floating point whole not valid")?;

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
                    .map_err(|_e| "floating point decimal not valid")?;

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
                    whole: value.parse::<u64>().map_err(|_e| "u64")?,
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
            .map_err(|_e| "real")
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
pub struct TypeName {
    pub name: Id,
}

impl TypeName {
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

impl Eq for TypeName {}

impl Hash for TypeName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Located for TypeName {
    fn span(&self) -> SourceSpan {
        self.name.span()
    }
}

impl fmt::Display for TypeName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{}", &self.name))
    }
}

/// Represents the type of a variable declaration.
///
/// This enum distinguishes between named types (which can be looked up in a
/// type environment) and inline/anonymous type definitions (which have no
/// named type to look up).
#[derive(Clone, Debug, PartialEq)]
pub enum TypeReference {
    /// A reference to a named type that can be resolved in the type environment.
    Named(TypeName),
    /// An inline/anonymous type definition with no type name to look up.
    /// Examples: `ARRAY[1..10] OF INT`, `INT(0..100)`, `(RED, GREEN, BLUE)`.
    Inline,
    /// No type specification was provided.
    Unspecified,
}

/// Generic specification kind that distinguishes between a named type reference
/// and an inline type definition.
///
/// This pattern is used for enumerations, subranges, and arrays where a
/// declaration can either reference an existing named type or define the
/// type inline.
///
/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq)]
pub enum SpecificationKind<T> {
    /// A reference to a named type.
    Named(TypeName),
    /// An inline type definition.
    Inline(T),
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

impl From<ElementaryTypeName> for TypeName {
    fn from(value: ElementaryTypeName) -> TypeName {
        match value {
            ElementaryTypeName::BOOL => TypeName::from("BOOL"),
            ElementaryTypeName::SINT => TypeName::from("SINT"),
            ElementaryTypeName::INT => TypeName::from("INT"),
            ElementaryTypeName::DINT => TypeName::from("DINT"),
            ElementaryTypeName::LINT => TypeName::from("LINT"),
            ElementaryTypeName::USINT => TypeName::from("USINT"),
            ElementaryTypeName::UINT => TypeName::from("UINT"),
            ElementaryTypeName::UDINT => TypeName::from("UDINT"),
            ElementaryTypeName::ULINT => TypeName::from("ULINT"),
            ElementaryTypeName::REAL => TypeName::from("REAL"),
            ElementaryTypeName::LREAL => TypeName::from("LREAL"),
            ElementaryTypeName::TIME => TypeName::from("TIME"),
            ElementaryTypeName::DATE => TypeName::from("DATE"),
            ElementaryTypeName::TimeOfDay => TypeName::from("TIME_OF_DAY"),
            ElementaryTypeName::DateAndTime => TypeName::from("DATE_AND_TIME"),
            ElementaryTypeName::STRING => TypeName::from("STRING"),
            ElementaryTypeName::BYTE => TypeName::from("BYTE"),
            ElementaryTypeName::WORD => TypeName::from("WORD"),
            ElementaryTypeName::DWORD => TypeName::from("DWORD"),
            ElementaryTypeName::LWORD => TypeName::from("LWORD"),
            ElementaryTypeName::WSTRING => TypeName::from("WSTRING"),
        }
    }
}

/// Generic type names used for polymorphic function signatures.
///
/// These types form a hierarchy where each generic type matches
/// a set of concrete types. Used primarily in standard library
/// function signatures to indicate polymorphism.
///
/// See section B.1.3.2 of IEC 61131-3.
#[derive(Debug, PartialEq, Clone)]
pub enum GenericTypeName {
    /// Matches any type
    Any,
    /// Matches any user-defined type (structures, enumerations, function blocks)
    AnyDerived,
    /// Matches any elementary type
    AnyElementary,
    /// Matches TIME and ANY_NUM types
    AnyMagnitude,
    /// Matches ANY_REAL and ANY_INT types
    AnyNum,
    /// Matches REAL and LREAL
    AnyReal,
    /// Matches all integer types (SINT, INT, DINT, LINT, USINT, UINT, UDINT, ULINT)
    AnyInt,
    /// Matches bit string types (BOOL, BYTE, WORD, DWORD, LWORD)
    AnyBit,
    /// Matches string types (STRING, WSTRING)
    AnyString,
    /// Matches date/time types (DATE, TIME_OF_DAY, DATE_AND_TIME)
    AnyDate,
}

impl GenericTypeName {
    /// Returns the string representation of the generic type name.
    pub fn as_str(&self) -> &'static str {
        match self {
            GenericTypeName::Any => "ANY",
            GenericTypeName::AnyDerived => "ANY_DERIVED",
            GenericTypeName::AnyElementary => "ANY_ELEMENTARY",
            GenericTypeName::AnyMagnitude => "ANY_MAGNITUDE",
            GenericTypeName::AnyNum => "ANY_NUM",
            GenericTypeName::AnyReal => "ANY_REAL",
            GenericTypeName::AnyInt => "ANY_INT",
            GenericTypeName::AnyBit => "ANY_BIT",
            GenericTypeName::AnyString => "ANY_STRING",
            GenericTypeName::AnyDate => "ANY_DATE",
        }
    }

    /// Checks if a concrete elementary type is compatible with this generic type.
    ///
    /// Returns true if the elementary type can be used where this generic type
    /// is expected (e.g., INT is compatible with ANY_INT, ANY_NUM, ANY_MAGNITUDE,
    /// ANY_ELEMENTARY, and ANY).
    pub fn is_compatible_with(&self, elementary: &ElementaryTypeName) -> bool {
        match self {
            GenericTypeName::Any => true,
            GenericTypeName::AnyDerived => false, // Elementary types are not derived
            GenericTypeName::AnyElementary => true,
            GenericTypeName::AnyMagnitude => {
                matches!(
                    elementary,
                    ElementaryTypeName::TIME
                        | ElementaryTypeName::SINT
                        | ElementaryTypeName::INT
                        | ElementaryTypeName::DINT
                        | ElementaryTypeName::LINT
                        | ElementaryTypeName::USINT
                        | ElementaryTypeName::UINT
                        | ElementaryTypeName::UDINT
                        | ElementaryTypeName::ULINT
                        | ElementaryTypeName::REAL
                        | ElementaryTypeName::LREAL
                )
            }
            GenericTypeName::AnyNum => {
                matches!(
                    elementary,
                    ElementaryTypeName::SINT
                        | ElementaryTypeName::INT
                        | ElementaryTypeName::DINT
                        | ElementaryTypeName::LINT
                        | ElementaryTypeName::USINT
                        | ElementaryTypeName::UINT
                        | ElementaryTypeName::UDINT
                        | ElementaryTypeName::ULINT
                        | ElementaryTypeName::REAL
                        | ElementaryTypeName::LREAL
                )
            }
            GenericTypeName::AnyReal => {
                matches!(
                    elementary,
                    ElementaryTypeName::REAL | ElementaryTypeName::LREAL
                )
            }
            GenericTypeName::AnyInt => {
                matches!(
                    elementary,
                    ElementaryTypeName::SINT
                        | ElementaryTypeName::INT
                        | ElementaryTypeName::DINT
                        | ElementaryTypeName::LINT
                        | ElementaryTypeName::USINT
                        | ElementaryTypeName::UINT
                        | ElementaryTypeName::UDINT
                        | ElementaryTypeName::ULINT
                )
            }
            GenericTypeName::AnyBit => {
                matches!(
                    elementary,
                    ElementaryTypeName::BOOL
                        | ElementaryTypeName::BYTE
                        | ElementaryTypeName::WORD
                        | ElementaryTypeName::DWORD
                        | ElementaryTypeName::LWORD
                )
            }
            GenericTypeName::AnyString => {
                matches!(
                    elementary,
                    ElementaryTypeName::STRING | ElementaryTypeName::WSTRING
                )
            }
            GenericTypeName::AnyDate => {
                matches!(
                    elementary,
                    ElementaryTypeName::DATE
                        | ElementaryTypeName::TimeOfDay
                        | ElementaryTypeName::DateAndTime
                )
            }
        }
    }
}

impl From<GenericTypeName> for Id {
    fn from(value: GenericTypeName) -> Id {
        Id::from(value.as_str())
    }
}

impl From<GenericTypeName> for TypeName {
    fn from(value: GenericTypeName) -> TypeName {
        TypeName::from(value.as_str())
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
    pub data_type_name: TypeName,
    /// The referenced type name.
    ///
    /// For example, if this is an alias then this is the underlying
    /// type.
    pub base_type_name: TypeName,
}

impl Located for LateBoundDeclaration {
    fn span(&self) -> SourceSpan {
        SourceSpan::join2(&self.data_type_name, &self.base_type_name)
    }
}

/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct EnumerationDeclaration {
    pub type_name: TypeName,
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
            spec: SpecificationKind::Inline(EnumeratedSpecificationValues {
                values: values.into_iter().map(EnumeratedValue::new).collect(),
            }),
            default: Some(EnumeratedValue::new(default)),
        }
    }
}

/// Enumeration specification: either a reference to a named enumeration type
/// or an inline list of enumeration values.
///
/// See section 2.3.3.1.
pub type EnumeratedSpecificationKind = SpecificationKind<EnumeratedSpecificationValues>;

impl EnumeratedSpecificationKind {
    pub fn from_values(values: Vec<&'static str>) -> EnumeratedSpecificationKind {
        let values = values
            .iter()
            .map(|v| EnumeratedValue {
                type_name: None,
                value: Id::from(v),
            })
            .collect();
        SpecificationKind::Inline(EnumeratedSpecificationValues { values })
    }

    pub fn values(values: Vec<EnumeratedValue>) -> EnumeratedSpecificationKind {
        SpecificationKind::Inline(EnumeratedSpecificationValues { values })
    }

    pub fn recurse_visit<V: Visitor<E> + ?Sized, E>(&self, v: &mut V) -> Result<V::Value, E> {
        match self {
            SpecificationKind::Named(node) => v.visit_type_name(node),
            SpecificationKind::Inline(node) => v.visit_enumerated_specification_values(node),
        }
    }

    pub fn recurse_fold<F: Fold<E> + ?Sized, E>(self, f: &mut F) -> Result<Self, E> {
        match self {
            SpecificationKind::Named(node) => Ok(SpecificationKind::Named(f.fold_type_name(node)?)),
            SpecificationKind::Inline(node) => Ok(SpecificationKind::Inline(
                f.fold_enumerated_specification_values(node)?,
            )),
        }
    }
}

/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct EnumeratedSpecificationValues {
    pub values: Vec<EnumeratedValue>,
}

pub trait HasEnumeratedValues {
    fn values(&self) -> &Vec<EnumeratedValue>;
    fn values_span(&self) -> SourceSpan;
}

impl HasEnumeratedValues for EnumeratedSpecificationValues {
    fn values(&self) -> &Vec<EnumeratedValue> {
        &self.values
    }
    fn values_span(&self) -> SourceSpan {
        // TODO
        match self.values.first() {
            Some(first) => first.span(),
            None => SourceSpan::default(),
        }
    }
}

/// A particular value in a enumeration.
///
/// May include a type name (especially where the enumeration would be
/// ambiguous.)
///
/// See section 2.3.3.1.
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct EnumeratedValue {
    pub type_name: Option<TypeName>,
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
    pub type_name: TypeName,
    pub spec: SubrangeSpecificationKind,
    pub default: Option<SignedInteger>,
}

/// Subrange specification: either a reference to a named subrange type
/// or an inline subrange definition.
///
/// See section 2.3.3.1.
pub type SubrangeSpecificationKind = SpecificationKind<SubrangeSpecification>;

impl SubrangeSpecificationKind {
    pub fn recurse_visit<V: Visitor<E> + ?Sized, E>(&self, v: &mut V) -> Result<V::Value, E> {
        match self {
            SpecificationKind::Named(node) => v.visit_type_name(node),
            SpecificationKind::Inline(node) => v.visit_subrange_specification(node),
        }
    }

    pub fn recurse_fold<F: Fold<E> + ?Sized, E>(self, f: &mut F) -> Result<Self, E> {
        match self {
            SpecificationKind::Named(node) => Ok(SpecificationKind::Named(f.fold_type_name(node)?)),
            SpecificationKind::Inline(node) => Ok(SpecificationKind::Inline(
                f.fold_subrange_specification(node)?,
            )),
        }
    }
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
    pub type_name: TypeName,
    pub spec_and_init: InitialValueAssignmentKind,
}

/// Derived data type that
#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct ArrayDeclaration {
    pub type_name: TypeName,
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
    pub type_name: TypeName,
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
    pub type_name: TypeName,
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
    pub type_name: TypeName,
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

/// Array specification: either a reference to a named array type
/// or an inline array definition with explicit subranges.
///
/// See section 2.3.3.1.
pub type ArraySpecificationKind = SpecificationKind<ArraySubranges>;

impl ArraySpecificationKind {
    pub fn recurse_visit<V: Visitor<E> + ?Sized, E>(&self, v: &mut V) -> Result<V::Value, E> {
        match self {
            SpecificationKind::Named(node) => v.visit_type_name(node),
            SpecificationKind::Inline(node) => v.visit_array_subranges(node),
        }
    }

    pub fn recurse_fold<F: Fold<E> + ?Sized, E>(self, f: &mut F) -> Result<Self, E> {
        match self {
            SpecificationKind::Named(node) => Ok(SpecificationKind::Named(f.fold_type_name(node)?)),
            SpecificationKind::Inline(node) => {
                Ok(SpecificationKind::Inline(f.fold_array_subranges(node)?))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Recurse)]
pub struct ArraySubranges {
    pub ranges: Vec<Subrange>,
    pub type_name: TypeName,
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
    pub type_name: TypeName,
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
            initializer: InitialValueAssignmentKind::simple_uninitialized(TypeName::from(
                type_name,
            )),
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
                    type_name: TypeName::from(type_name),
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
                    type_name: TypeName::from(type_name),
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
                    type_name: TypeName::from(type_name),
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
                    type_name: TypeName::from(type_name),
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
            initializer: InitialValueAssignmentKind::LateResolvedType(TypeName::from(type_name)),
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

    pub fn type_name(&self) -> TypeReference {
        match &self.initializer {
            InitialValueAssignmentKind::None(_source_span) => TypeReference::Unspecified,
            InitialValueAssignmentKind::Simple(simple_initializer) => {
                TypeReference::Named(simple_initializer.type_name.clone())
            }
            InitialValueAssignmentKind::String(string_initializer) => {
                TypeReference::Named(string_initializer.type_name())
            }
            InitialValueAssignmentKind::EnumeratedValues(_enumerated_values_initializer) => {
                TypeReference::Inline
            }
            InitialValueAssignmentKind::EnumeratedType(enumerated_initial_value_assignment) => {
                TypeReference::Named(enumerated_initial_value_assignment.type_name.clone())
            }
            InitialValueAssignmentKind::FunctionBlock(function_block_initial_value_assignment) => {
                TypeReference::Named(function_block_initial_value_assignment.type_name.clone())
            }
            InitialValueAssignmentKind::Subrange(subrange_specification_kind) => {
                match subrange_specification_kind {
                    SpecificationKind::Inline(_subrange_specification) => TypeReference::Inline,
                    SpecificationKind::Named(type_name) => TypeReference::Named(type_name.clone()),
                }
            }
            InitialValueAssignmentKind::Structure(structure_initialization_declaration) => {
                TypeReference::Named(structure_initialization_declaration.type_name.clone())
            }
            InitialValueAssignmentKind::Array(array_initial_value_assignment) => {
                match &array_initial_value_assignment.spec {
                    SpecificationKind::Named(type_name) => TypeReference::Named(type_name.clone()),
                    SpecificationKind::Inline(_) => TypeReference::Inline,
                }
            }
            InitialValueAssignmentKind::LateResolvedType(type_name) => {
                TypeReference::Named(type_name.clone())
            }
        }
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

// Container for variable specifications.

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
    /// Represents an initializer that is a constant.
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
    LateResolvedType(TypeName),
}

impl InitialValueAssignmentKind {
    /// Creates an initial value with
    pub fn simple_uninitialized(type_name: TypeName) -> Self {
        InitialValueAssignmentKind::Simple(SimpleInitializer {
            type_name,
            initial_value: None,
        })
    }

    /// Creates an initial value from the initializer.
    pub fn simple(type_name: &str, value: ConstantKind) -> Self {
        InitialValueAssignmentKind::Simple(SimpleInitializer {
            type_name: TypeName::from(type_name),
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
    pub type_name: TypeName,
    pub initial_value: Option<EnumeratedValue>,
}

#[derive(Clone, PartialEq, Debug, Recurse)]
pub struct SimpleInitializer {
    pub type_name: TypeName,
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

impl Located for StringInitializer {
    fn span(&self) -> SourceSpan {
        self.keyword_span.clone()
    }
}

impl StringInitializer {
    pub fn type_name(&self) -> TypeName {
        match self.width {
            StringType::String => TypeName::from("string"),
            StringType::WString => TypeName::from("wstring"),
        }
    }
}

#[derive(Clone, PartialEq, Debug, Recurse)]
pub struct EnumeratedValuesInitializer {
    pub values: Vec<EnumeratedValue>,
    pub initial_value: Option<EnumeratedValue>,
}

impl Located for EnumeratedValuesInitializer {
    fn span(&self) -> SourceSpan {
        let first = self.values.first();
        let last = self.values.last();

        if let Some(f) = first {
            if let Some(l) = last {
                return SourceSpan::join2(f, l);
            }
            return f.span();
        }
        SourceSpan::default()
    }
}

impl HasEnumeratedValues for EnumeratedValuesInitializer {
    fn values(&self) -> &Vec<EnumeratedValue> {
        &self.values
    }
    fn values_span(&self) -> SourceSpan {
        self.span()
    }
}

#[derive(Clone, PartialEq, Debug, Recurse)]
pub struct FunctionBlockInitialValueAssignment {
    // In this context, the name is referring to a type, much like a function pointer
    // in other languages, so the correct representation here is a type and not
    // an identifier.
    pub type_name: TypeName,
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
    Simple(TypeName),
    Subrange(SubrangeSpecificationKind),
    Enumerated(EnumeratedSpecificationKind),
    Array(ArraySpecificationKind),
    Struct(StructureDeclaration),
    String(StringSpecification),
    WString(StringSpecification),
    // Represents simple, subrange or structure
    Ambiguous(TypeName),
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
    pub return_type: TypeName,
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
    pub name: TypeName,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_kind_partial_eq_and_clone() {
        let int1 = ConstantKind::IntegerLiteral(IntegerLiteral {
            value: SignedInteger::new("42", SourceSpan::default()).unwrap(),
            data_type: None,
        });
        let int2 = int1.clone();
        assert_eq!(int1, int2);
        let int3 = ConstantKind::IntegerLiteral(IntegerLiteral {
            value: SignedInteger::new("43", SourceSpan::default()).unwrap(),
            data_type: None,
        });
        assert_ne!(int1, int3);
    }

    #[test]
    fn test_integer_partial_eq_and_clone() {
        let i1 = Integer::new("123", SourceSpan::default()).unwrap();
        let i2 = i1.clone();
        assert_eq!(i1, i2);
        let i3 = Integer::new("124", SourceSpan::default()).unwrap();
        assert_ne!(i1, i3);
    }

    #[test]
    fn test_signed_integer_partial_eq_and_clone() {
        let s1 = SignedInteger::new("-5", SourceSpan::default()).unwrap();
        let s2 = s1.clone();
        assert_eq!(s1, s2);
        let s3 = SignedInteger::new("5", SourceSpan::default()).unwrap();
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_boolean_partial_eq_and_clone() {
        let b1 = Boolean::True;
        let b2 = b1.clone();
        assert_eq!(b1, b2);
        let b3 = Boolean::False;
        assert_ne!(b1, b3);
    }

    #[test]
    fn test_integer_literal_partial_eq_and_clone() {
        let il1 = IntegerLiteral {
            value: SignedInteger::new("7", SourceSpan::default()).unwrap(),
            data_type: None,
        };
        let il2 = il1.clone();
        assert_eq!(il1, il2);
        let il3 = IntegerLiteral {
            value: SignedInteger::new("8", SourceSpan::default()).unwrap(),
            data_type: None,
        };
        assert_ne!(il1, il3);
    }

    #[test]
    fn test_real_literal_partial_eq_and_clone() {
        let rl1 = RealLiteral {
            value: 1.23,
            data_type: None,
        };
        let rl2 = rl1.clone();
        assert_eq!(rl1, rl2);
        let rl3 = RealLiteral {
            value: 2.34,
            data_type: None,
        };
        assert_ne!(rl1, rl3);
    }

    #[test]
    fn test_boolean_literal_partial_eq_and_clone() {
        let bl1 = BooleanLiteral {
            value: Boolean::True,
        };
        let bl2 = bl1.clone();
        assert_eq!(bl1, bl2);
        let bl3 = BooleanLiteral {
            value: Boolean::False,
        };
        assert_ne!(bl1, bl3);
    }

    #[test]
    fn test_character_string_literal_partial_eq_and_clone() {
        let csl1 = CharacterStringLiteral {
            value: vec!['a', 'b', 'c'],
        };
        let csl2 = csl1.clone();
        assert_eq!(csl1, csl2);
        let csl3 = CharacterStringLiteral {
            value: vec!['x', 'y', 'z'],
        };
        assert_ne!(csl1, csl3);
    }

    #[test]
    fn test_bit_string_literal_partial_eq_and_clone() {
        let int = Integer::new("255", SourceSpan::default()).unwrap();
        let bsl1 = BitStringLiteral {
            value: int.clone(),
            data_type: None,
        };
        let bsl2 = bsl1.clone();
        assert_eq!(bsl1, bsl2);
        let bsl3 = BitStringLiteral {
            value: Integer::new("0", SourceSpan::default()).unwrap(),
            data_type: None,
        };
        assert_ne!(bsl1, bsl3);
    }

    #[test]
    fn test_type_partial_eq_and_clone() {
        let t1 = TypeName::from("MYTYPE");
        let t2 = t1.clone();
        assert_eq!(t1, t2);
        let t3 = TypeName::from("OTHERTYPE");
        assert_ne!(t1, t3);
    }

    #[test]
    fn test_elementary_type_name_partial_eq_and_clone() {
        let e1 = ElementaryTypeName::BOOL;
        let e2 = e1.clone();
        assert_eq!(e1, e2);
        let e3 = ElementaryTypeName::INT;
        assert_ne!(e1, e3);
    }

    #[test]
    fn test_data_type_declaration_kind_partial_eq_and_clone() {
        let enum_decl = EnumerationDeclaration {
            type_name: TypeName::from("ENUM"),
            spec_init: EnumeratedSpecificationInit::values_and_default(vec!["A", "B"], "A"),
        };
        let d1 = DataTypeDeclarationKind::Enumeration(enum_decl.clone());
        let d2 = d1.clone();
        assert_eq!(d1, d2);
        let d3 = DataTypeDeclarationKind::Enumeration(EnumerationDeclaration {
            type_name: TypeName::from("ENUM"),
            spec_init: EnumeratedSpecificationInit::values_and_default(vec!["A", "B"], "B"),
        });
        assert_ne!(d1, d3);
    }

    #[test]
    fn test_library_partial_eq_and_clone() {
        let lib1 = Library { elements: vec![] };
        let lib2 = lib1.clone();
        assert_eq!(lib1, lib2);
        let lib3 = Library {
            elements: vec![LibraryElementKind::DataTypeDeclaration(
                DataTypeDeclarationKind::Enumeration(EnumerationDeclaration {
                    type_name: TypeName::from("ENUM"),
                    spec_init: EnumeratedSpecificationInit::values_and_default(vec!["A"], "A"),
                }),
            )],
        };
        assert_ne!(lib1, lib3);
    }

    #[test]
    fn generic_type_name_as_str_when_called_then_returns_expected_string() {
        assert_eq!(GenericTypeName::Any.as_str(), "ANY");
        assert_eq!(GenericTypeName::AnyDerived.as_str(), "ANY_DERIVED");
        assert_eq!(GenericTypeName::AnyElementary.as_str(), "ANY_ELEMENTARY");
        assert_eq!(GenericTypeName::AnyMagnitude.as_str(), "ANY_MAGNITUDE");
        assert_eq!(GenericTypeName::AnyNum.as_str(), "ANY_NUM");
        assert_eq!(GenericTypeName::AnyReal.as_str(), "ANY_REAL");
        assert_eq!(GenericTypeName::AnyInt.as_str(), "ANY_INT");
        assert_eq!(GenericTypeName::AnyBit.as_str(), "ANY_BIT");
        assert_eq!(GenericTypeName::AnyString.as_str(), "ANY_STRING");
        assert_eq!(GenericTypeName::AnyDate.as_str(), "ANY_DATE");
    }

    #[test]
    fn generic_type_name_is_compatible_with_when_any_then_accepts_all() {
        assert!(GenericTypeName::Any.is_compatible_with(&ElementaryTypeName::BOOL));
        assert!(GenericTypeName::Any.is_compatible_with(&ElementaryTypeName::INT));
        assert!(GenericTypeName::Any.is_compatible_with(&ElementaryTypeName::REAL));
        assert!(GenericTypeName::Any.is_compatible_with(&ElementaryTypeName::STRING));
        assert!(GenericTypeName::Any.is_compatible_with(&ElementaryTypeName::TIME));
        assert!(GenericTypeName::Any.is_compatible_with(&ElementaryTypeName::DATE));
    }

    #[test]
    fn generic_type_name_is_compatible_with_when_any_int_then_accepts_integers_only() {
        // Should accept all integer types
        assert!(GenericTypeName::AnyInt.is_compatible_with(&ElementaryTypeName::SINT));
        assert!(GenericTypeName::AnyInt.is_compatible_with(&ElementaryTypeName::INT));
        assert!(GenericTypeName::AnyInt.is_compatible_with(&ElementaryTypeName::DINT));
        assert!(GenericTypeName::AnyInt.is_compatible_with(&ElementaryTypeName::LINT));
        assert!(GenericTypeName::AnyInt.is_compatible_with(&ElementaryTypeName::USINT));
        assert!(GenericTypeName::AnyInt.is_compatible_with(&ElementaryTypeName::UINT));
        assert!(GenericTypeName::AnyInt.is_compatible_with(&ElementaryTypeName::UDINT));
        assert!(GenericTypeName::AnyInt.is_compatible_with(&ElementaryTypeName::ULINT));

        // Should reject non-integer types
        assert!(!GenericTypeName::AnyInt.is_compatible_with(&ElementaryTypeName::REAL));
        assert!(!GenericTypeName::AnyInt.is_compatible_with(&ElementaryTypeName::BOOL));
        assert!(!GenericTypeName::AnyInt.is_compatible_with(&ElementaryTypeName::STRING));
        assert!(!GenericTypeName::AnyInt.is_compatible_with(&ElementaryTypeName::TIME));
    }

    #[test]
    fn generic_type_name_is_compatible_with_when_any_real_then_accepts_reals_only() {
        assert!(GenericTypeName::AnyReal.is_compatible_with(&ElementaryTypeName::REAL));
        assert!(GenericTypeName::AnyReal.is_compatible_with(&ElementaryTypeName::LREAL));
        assert!(!GenericTypeName::AnyReal.is_compatible_with(&ElementaryTypeName::INT));
        assert!(!GenericTypeName::AnyReal.is_compatible_with(&ElementaryTypeName::BOOL));
    }

    #[test]
    fn generic_type_name_is_compatible_with_when_any_num_then_accepts_all_numerics() {
        // Should accept integers and reals
        assert!(GenericTypeName::AnyNum.is_compatible_with(&ElementaryTypeName::INT));
        assert!(GenericTypeName::AnyNum.is_compatible_with(&ElementaryTypeName::REAL));
        assert!(GenericTypeName::AnyNum.is_compatible_with(&ElementaryTypeName::LINT));
        assert!(GenericTypeName::AnyNum.is_compatible_with(&ElementaryTypeName::LREAL));

        // Should reject non-numeric types
        assert!(!GenericTypeName::AnyNum.is_compatible_with(&ElementaryTypeName::BOOL));
        assert!(!GenericTypeName::AnyNum.is_compatible_with(&ElementaryTypeName::TIME));
        assert!(!GenericTypeName::AnyNum.is_compatible_with(&ElementaryTypeName::STRING));
    }

    #[test]
    fn generic_type_name_is_compatible_with_when_any_bit_then_accepts_bit_types() {
        assert!(GenericTypeName::AnyBit.is_compatible_with(&ElementaryTypeName::BOOL));
        assert!(GenericTypeName::AnyBit.is_compatible_with(&ElementaryTypeName::BYTE));
        assert!(GenericTypeName::AnyBit.is_compatible_with(&ElementaryTypeName::WORD));
        assert!(GenericTypeName::AnyBit.is_compatible_with(&ElementaryTypeName::DWORD));
        assert!(GenericTypeName::AnyBit.is_compatible_with(&ElementaryTypeName::LWORD));
        assert!(!GenericTypeName::AnyBit.is_compatible_with(&ElementaryTypeName::INT));
    }

    #[test]
    fn generic_type_name_is_compatible_with_when_any_string_then_accepts_strings() {
        assert!(GenericTypeName::AnyString.is_compatible_with(&ElementaryTypeName::STRING));
        assert!(GenericTypeName::AnyString.is_compatible_with(&ElementaryTypeName::WSTRING));
        assert!(!GenericTypeName::AnyString.is_compatible_with(&ElementaryTypeName::INT));
    }

    #[test]
    fn generic_type_name_is_compatible_with_when_any_date_then_accepts_date_types() {
        assert!(GenericTypeName::AnyDate.is_compatible_with(&ElementaryTypeName::DATE));
        assert!(GenericTypeName::AnyDate.is_compatible_with(&ElementaryTypeName::TimeOfDay));
        assert!(GenericTypeName::AnyDate.is_compatible_with(&ElementaryTypeName::DateAndTime));
        assert!(!GenericTypeName::AnyDate.is_compatible_with(&ElementaryTypeName::TIME));
        assert!(!GenericTypeName::AnyDate.is_compatible_with(&ElementaryTypeName::INT));
    }

    #[test]
    fn generic_type_name_is_compatible_with_when_any_magnitude_then_accepts_time_and_numerics() {
        assert!(GenericTypeName::AnyMagnitude.is_compatible_with(&ElementaryTypeName::TIME));
        assert!(GenericTypeName::AnyMagnitude.is_compatible_with(&ElementaryTypeName::INT));
        assert!(GenericTypeName::AnyMagnitude.is_compatible_with(&ElementaryTypeName::REAL));
        assert!(!GenericTypeName::AnyMagnitude.is_compatible_with(&ElementaryTypeName::BOOL));
        assert!(!GenericTypeName::AnyMagnitude.is_compatible_with(&ElementaryTypeName::STRING));
    }

    #[test]
    fn generic_type_name_is_compatible_with_when_any_derived_then_rejects_all_elementary() {
        assert!(!GenericTypeName::AnyDerived.is_compatible_with(&ElementaryTypeName::INT));
        assert!(!GenericTypeName::AnyDerived.is_compatible_with(&ElementaryTypeName::BOOL));
        assert!(!GenericTypeName::AnyDerived.is_compatible_with(&ElementaryTypeName::STRING));
    }

    #[test]
    fn generic_type_name_partial_eq_and_clone() {
        let g1 = GenericTypeName::AnyInt;
        let g2 = g1.clone();
        assert_eq!(g1, g2);
        let g3 = GenericTypeName::AnyReal;
        assert_ne!(g1, g3);
    }
}
