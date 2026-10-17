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
use super::expr::check_user_call;
use super::ids::{PouId, Stamp, VarId};
use super::in_place::InPlaceCall;
use super::invariant::broken;
use super::place::Place;
use super::stmt::Stmt;
use super::types::{Encoding, StringShape, TypeKind, TypeTable};
use super::variables::{PouTable, VariableTable};

/// What a string expression is.
#[derive(Clone, Debug)]
pub enum StrExprKind {
    /// A literal's characters.
    Literal(Vec<char>),
    /// The string at a place.
    Read(Place),
    /// The string a standard string function yields (REQ-LOW-lowering-052).
    Call { callee: Intrinsic, args: Vec<Arg> },
    /// The string a call of a function whose result is a string writes to
    /// `result`, the temporary it passes for its result parameter, run where
    /// the expression is evaluated: the call, then the stores of its outputs
    /// (REQ-LOW-ir-182, REQ-LOW-ir-184).
    InPlaceCall { call: InPlaceCall, result: VarId },
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

    /// The string the function `pou` that `pous` declares, whose result is a
    /// string, writes to the temporary it is passed for its result, called
    /// in place with `args`, which [`check_user_call`] describes, then
    /// `stores`, which [`InPlaceCall`] describes. The argument for the result
    /// parameter is a `Ref` to the whole of the temporary, and the string has
    /// the shape of the temporary's type (REQ-LOW-ir-184).
    pub(crate) fn call_in_place(
        types: &TypeTable,
        variables: &VariableTable,
        pous: &PouTable,
        pou: PouId,
        args: Vec<Arg>,
        stores: Vec<Stmt>,
        span: SourceSpan,
    ) -> Result<Self, Diagnostic> {
        const CONSTRUCTOR: &str = "ProgramBuilder::str_call_in_place";
        check_user_call(CONSTRUCTOR, types, variables, pous, pou, &args, &span)?;
        let returns_through = pous
            .function(pou)
            .and_then(|signature| signature.result_parameter())
            .is_some();
        let result = match args.first() {
            Some(Arg::Ref(place)) if returns_through && place.path().is_empty() => place,
            Some(Arg::Ref(_) | Arg::Value(_) | Arg::Str(_) | Arg::Copy(_)) | None => {
                return Err(broken(
                    CONSTRUCTOR,
                    &span,
                    format!("{pou} is not passed the whole of a temporary for a string result"),
                ))
            }
        };
        let Some(TypeKind::String(shape)) = types.kind(result.ty()) else {
            return Err(broken(
                CONSTRUCTOR,
                &span,
                format!("{} is not a string", result.ty()),
            ));
        };
        let (shape, result) = (*shape, result.root());
        let call = InPlaceCall::new(CONSTRUCTOR, pou, args, stores, &span)?;
        let program = call.program();
        Ok(Self {
            kind: StrExprKind::InPlaceCall { call, result },
            shape,
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
