//! String expressions.
//!
//! A string is never an [`Expr`](super::Expr): its value lives in storage of
//! a capacity of its own, so a [`StrExpr`] states the encoding and capacity
//! of the value it makes, its [`StringShape`] (REQ-LOW-lowering-054). How a
//! backend holds the value while it computes it, such as the bytecode VM's
//! temporary buffers (ADR-0052), is not part of the lowered program.

use ironplc_dsl::core::SourceSpan;
use ironplc_dsl::diagnostic::Diagnostic;

use super::call::{program_of, Arg, Intrinsic, Param};
use super::ids::Stamp;
use super::invariant::broken;
use super::place::Place;
use super::types::{Encoding, StringShape, TypeKind, TypeTable};

/// What a string expression is.
#[derive(Clone, Debug)]
pub enum StrExprKind {
    /// A literal's characters.
    Literal(Vec<char>),
    /// The string at a place.
    Read(Place),
    /// The string a standard string function yields (REQ-LOW-lowering-052).
    Call { callee: Intrinsic, args: Vec<Arg> },
}

/// A string value of the encoding and capacity of its shape.
#[derive(Clone, Debug)]
pub struct StrExpr {
    kind: StrExprKind,
    shape: StringShape,
    span: SourceSpan,
    /// The program whose places the expression reads; `None` when it reads
    /// none.
    program: Option<Stamp>,
}

impl StrExpr {
    /// The literal `chars` in `encoding`, whose capacity is its length. Each
    /// character fits the encoding (REQ-LOW-lowering-051): a `STRING` holds
    /// one byte for each character, and a `WSTRING` one 16-bit code unit
    /// (ADR-0016).
    pub fn literal(
        chars: Vec<char>,
        encoding: Encoding,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        const CONSTRUCTOR: &str = "StrExpr::literal";
        let largest = match encoding {
            Encoding::Narrow => 0xFF,
            Encoding::Wide => 0xFFFF,
        };
        if let Some(ch) = chars.iter().find(|ch| u32::from(**ch) > largest) {
            return Err(broken(
                CONSTRUCTOR,
                &span,
                format!("{ch:?} does not fit the {encoding:?} encoding"),
            ));
        }
        let Ok(capacity) = u32::try_from(chars.len()) else {
            return Err(broken(CONSTRUCTOR, &span, "the literal is too long"));
        };
        Ok(Self {
            kind: StrExprKind::Literal(chars),
            shape: StringShape { encoding, capacity },
            span,
            program: None,
        })
    }

    /// The string `callee` yields from `args`, with room for `capacity` code
    /// units, the capacity lowering gives the result. There is one argument
    /// for each of its parameters, of what the parameter takes
    /// (REQ-LOW-ir-145), and the arguments read places of one program.
    pub fn call(
        callee: Intrinsic,
        args: Vec<Arg>,
        capacity: u32,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        const CONSTRUCTOR: &str = "StrExpr::call";
        let Param::Str(encoding) = callee.signature().result() else {
            return Err(broken(
                CONSTRUCTOR,
                &span,
                format!("{} yields a value, not a string", callee.name()),
            ));
        };
        callee.check_args(CONSTRUCTOR, &args, &span)?;
        let program = program_of(CONSTRUCTOR, &args, &span)?;
        Ok(Self {
            kind: StrExprKind::Call { callee, args },
            shape: StringShape { encoding, capacity },
            span,
            program,
        })
    }

    /// The string at `place`, of the shape of its type.
    pub(crate) fn read(
        types: &TypeTable,
        place: Place,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        let Some(TypeKind::String(shape)) = types.kind(place.ty()) else {
            return Err(broken(
                "ProgramBuilder::read_str",
                &span,
                format!("{} is not a string", place.ty()),
            ));
        };
        let shape = *shape;
        let program = Some(place.stamp());
        Ok(Self {
            kind: StrExprKind::Read(place),
            shape,
            span,
            program,
        })
    }

    pub fn kind(&self) -> &StrExprKind {
        &self.kind
    }

    pub fn shape(&self) -> StringShape {
        self.shape
    }

    pub fn span(&self) -> &SourceSpan {
        &self.span
    }

    /// The program the places the expression reads belong to.
    pub(crate) fn program(&self) -> Option<Stamp> {
        self.program
    }
}
