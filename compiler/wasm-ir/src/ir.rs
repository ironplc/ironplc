//! Intermediate representation:
//! static objects with their initial images, functions with structured
//! statements, typed expressions without implicit conversions, and the
//! descriptions of tasks, symbol map leaves and source sites.
//!
//! The IR does not depend on Structured Text: places are addresses, types
//! are scalars, control flow is made of blocks, loops and labelled exits.

use std::fmt::Write;

/// Binary operators. `Pow` is not a binary operation of the IR (it is
/// [`Intrinsic::Pow`]); the validator rejects it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    /// Logical or bitwise or.
    Or,
    /// Logical or bitwise exclusive or.
    Xor,
    /// Logical or bitwise and.
    And,
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Less than.
    Lt,
    /// Greater than.
    Gt,
    /// Less or equal.
    Le,
    /// Greater or equal.
    Ge,
    /// Addition (wraps for integers).
    Add,
    /// Subtraction (wraps for integers).
    Sub,
    /// Multiplication (wraps for integers).
    Mul,
    /// Division (truncates for integers).
    Div,
    /// Remainder with the sign of the dividend.
    Mod,
    /// Power, see [`Intrinsic::Pow`].
    Pow,
}

impl BinaryOp {
    /// Spelling in Structured Text, for the text form.
    pub fn symbol(self) -> &'static str {
        match self {
            BinaryOp::Or => "OR",
            BinaryOp::Xor => "XOR",
            BinaryOp::And => "AND",
            BinaryOp::Eq => "=",
            BinaryOp::Ne => "<>",
            BinaryOp::Lt => "<",
            BinaryOp::Gt => ">",
            BinaryOp::Le => "<=",
            BinaryOp::Ge => ">=",
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::Mod => "MOD",
            BinaryOp::Pow => "**",
        }
    }
}

/// A constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Const {
    /// Boolean.
    Bool(bool),
    /// Integer or bit string, within the range of its scalar type.
    Int(i128),
    /// Real (the value of the `f32` for 32-bit reals).
    Real(f64),
    /// Duration, date or time of day, in nanoseconds.
    Duration(i64),
}

/// How a [`ExprKind::Convert`] computes its result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConvMode {
    /// A conversion that preserves the value.
    Widen,
    /// Keep the low bits of the two's complement pattern.
    Wrap,
    /// To `bool`: true when the value is not zero.
    NotZero,
    /// Real to integer: round to nearest, ties to even, saturate, NaN to 0.
    Round,
    /// Real to integer: truncate toward zero, saturate, NaN to 0.
    Truncate,
    /// Integer to real, or real to real: nearest representable value.
    Float,
    /// Nanoseconds to the day, rounded down.
    DatePart,
    /// Nanoseconds to the time since midnight, modulo one day.
    TimePart,
}

/// Index of an object in [`Module::objects`].
pub type ObjId = u32;
/// Index of a function in [`Module::functions`].
pub type FuncId = u32;
/// Index of a site in [`Module::sites`].
pub type SiteId = u32;
/// Index of a temporary in [`Function::temps`].
pub type TempId = u32;
/// Label of a block or loop.
pub type Label = u32;

/// A source span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    /// File index.
    pub file: u32,
    /// Start byte offset.
    pub start: u32,
    /// End byte offset (exclusive).
    pub end: u32,
}

/// Scalar types (section 3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scalar {
    /// `BOOL`, one byte holding 0 or 1.
    Bool,
    /// Two's complement or unsigned integer.
    Int {
        /// Width: 8, 16, 32 or 64.
        bits: u8,
        /// Signed.
        signed: bool,
    },
    /// Bit string.
    Bits {
        /// Width: 8, 16, 32 or 64.
        bits: u8,
    },
    /// IEEE 754 binary32 or binary64.
    Real {
        /// Width: 32 or 64.
        bits: u8,
    },
    /// Duration in signed 64-bit nanoseconds.
    Duration {
        /// `LTIME` rather than `TIME`.
        long: bool,
    },
}

impl Scalar {
    /// Size in bytes.
    pub fn size(self) -> u32 {
        match self {
            Scalar::Bool => 1,
            Scalar::Int { bits, .. } | Scalar::Bits { bits } | Scalar::Real { bits } => {
                bits as u32 / 8
            }
            Scalar::Duration { .. } => 8,
        }
    }

    /// Whether values are 64 bits wide on the machine (i64 or f64).
    pub fn is_wide(self) -> bool {
        self.size() == 8
    }

    /// Integer comparisons and divisions are signed.
    pub fn is_signed(self) -> bool {
        matches!(
            self,
            Scalar::Int { signed: true, .. } | Scalar::Duration { .. }
        )
    }

    /// Real type.
    pub fn is_real(self) -> bool {
        matches!(self, Scalar::Real { .. })
    }

    /// Type name.
    pub fn name(self) -> String {
        match self {
            Scalar::Bool => "bool".into(),
            Scalar::Int { bits, signed: true } => format!("i{bits}"),
            Scalar::Int {
                bits,
                signed: false,
            } => format!("u{bits}"),
            Scalar::Bits { bits } => format!("b{bits}"),
            Scalar::Real { bits } => format!("f{bits}"),
            Scalar::Duration { long: false } => "time".into(),
            Scalar::Duration { long: true } => "ltime".into(),
        }
    }
}

/// Memory region of an object (ABI sections 5 and 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Region {
    /// Volatile static data.
    Static,
    /// Retentive data.
    Retain,
    /// Located inputs.
    Input,
    /// Located outputs.
    Output,
    /// Located markers.
    Marker,
}

/// A static object: an instance part, a frame, globals, an image area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    /// Name, for debugging.
    pub name: String,
    /// Region.
    pub region: Region,
    /// Alignment (a power of two).
    pub align: u32,
    /// Initial content; its length is the size of the object.
    pub init: Vec<u8>,
}

/// Base of an address.
#[derive(Debug, Clone, PartialEq)]
pub enum Base {
    /// A static object.
    Object(ObjId),
    /// Part `k` (0 plain, 1 retentive, 2 non-retentive) of the instance of
    /// the current function block, received as parameter `k`.
    SelfPart(u8),
    /// The address stored at another address (a `VAR_IN_OUT` pointer).
    Deref(Box<Addr>),
    /// An address plus a byte offset computed at run time (an array
    /// element); the expression has type `u32`.
    Index(Box<Addr>, Box<Expr>),
}

/// An address: base plus constant offset.
#[derive(Debug, Clone, PartialEq)]
pub struct Addr {
    /// Base.
    pub base: Base,
    /// Offset in bytes.
    pub offset: u32,
}

impl Addr {
    /// Address of an object plus offset.
    pub fn object(o: ObjId, offset: u32) -> Addr {
        Addr {
            base: Base::Object(o),
            offset,
        }
    }

    /// Same base, offset increased.
    pub fn shifted(&self, by: u32) -> Addr {
        Addr {
            base: self.base.clone(),
            offset: self.offset + by,
        }
    }
}

/// A typed place.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    /// Address.
    pub addr: Addr,
    /// Type.
    pub ty: Scalar,
}

/// Unary operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnOp {
    /// Negation (wraps for integers).
    Neg,
    /// Logical or bitwise complement.
    Not,
}

/// Intrinsic operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Intrinsic {
    /// Absolute value (wraps for the minimum integer).
    Abs,
    /// Maximum of two or more values.
    Max,
    /// Minimum of two or more values.
    Min,
    /// `LIMIT(MN, IN, MX)`.
    Limit,
    /// Real base to a numeric exponent (LANG-073).
    Pow,
    /// `SHL(IN, N)`: bits shifted out are lost; `N` of any integer type, 0
    /// for `N` negative, all bits lost from the width on.
    Shl,
    /// `SHR(IN, N)`, zeros shifted in.
    Shr,
    /// `ROL(IN, N)`, `N` modulo the width.
    Rol,
    /// `ROR(IN, N)`, `N` modulo the width.
    Ror,
    /// `MUX(K, IN0, ...)`, `K` clamped to the inputs.
    Mux,
    /// Comparison between each operand and the next; `bool`.
    Compare(BinaryOp),
    /// Cycle time from `plc_rt.now_ns`.
    Now,
    /// Square root (IEEE 754, correctly rounded).
    Sqrt,
    /// Natural logarithm (computed in binary64).
    Ln,
    /// Logarithm in base 10 (computed in binary64).
    Log,
    /// Exponential (computed in binary64).
    Exp,
}

/// Expression kind.
#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    /// Constant.
    Const(Const),
    /// Value of a place.
    Load(Place),
    /// Value of a temporary.
    Temp(TempId),
    /// Address, as an `i32` (for `VAR_IN_OUT` and instance parts).
    AddrOf(Addr),
    /// Unary operation in the type of the expression.
    Unary(UnOp, Box<Expr>),
    /// Binary operation on operands of one type; comparisons give `bool`.
    /// Integer `/` and `MOD` carry the site of their division check
    /// (ABI section 10).
    Binary(BinaryOp, Box<Expr>, Box<Expr>, Option<SiteId>),
    /// Conversion of the operand to the type of the expression.
    Convert(ConvMode, Box<Expr>),
    /// `cond ? a : b`, both operands evaluated.
    Select(Box<Expr>, Box<Expr>, Box<Expr>),
    /// Intrinsic.
    Intrinsic(Intrinsic, Vec<Expr>),
    /// Call of a function with a result.
    Call(Box<Call>),
    /// Byte offset of an array element in one dimension, as a `u32`:
    /// `(index - low) * stride`; with a site, traps with code 3 unless
    /// `0 <= index - low < count` (ABI-080).
    Index {
        /// Index value, of an integer type.
        index: Box<Expr>,
        /// Lower bound of the dimension.
        low: i64,
        /// Number of elements of the dimension.
        count: u64,
        /// Bytes between two consecutive indexes.
        stride: u32,
        /// Site of the bounds check, if checked.
        site: Option<SiteId>,
    },
    /// The value, checked to be in `low..=high` (a subrange, code 4).
    Checked(Box<Expr>, i128, i128, SiteId),
    /// Statements executed before the value is computed (string
    /// temporaries of the expression).
    Seq(Vec<Stmt>, Box<Expr>),
    /// Length of a string, an `i16` (`LEN`).
    StrLen(Addr, bool),
    /// Position of the first occurrence of the second string in the first,
    /// 0 if none; an `i16` (`FIND`).
    StrFind(Addr, Addr, bool),
    /// Comparison of two strings by code: -1, 0 or 1, an `i32`.
    StrCmp(Addr, Addr, bool),
    /// First character of a string (0 if empty), a `b8` or `b16`.
    StrChar(Addr, bool),
}

/// Operations on strings. `wide` strings have 16-bit code
/// units; `cap` is the maximum number of characters of the destination;
/// positions and lengths are `i32` values clamped to the source.
#[derive(Debug, Clone, PartialEq)]
pub enum StrOp {
    /// `dst := src`, truncated; the source may overlap the destination.
    Assign {
        /// Destination.
        dst: Addr,
        /// Capacity of the destination.
        cap: u32,
        /// Source.
        src: Addr,
    },
    /// `dst := ''`.
    Clear(Addr),
    /// `dst := src` for a source of the other kind, truncated; a wide
    /// character above 255 becomes `?`.
    Convert {
        /// Destination.
        dst: Addr,
        /// Capacity of the destination.
        cap: u32,
        /// Source, of the other kind.
        src: Addr,
    },
    /// `dst` := the string of one character, empty for 0.
    SetChar {
        /// Destination (capacity at least 1).
        dst: Addr,
        /// Character, a `b8` or `b16`.
        value: Expr,
    },
    /// Appends `count` characters of `src` from the 0-based `start`
    /// (`start` clamped to the length, `count` to what remains).
    Append {
        /// Destination.
        dst: Addr,
        /// Capacity of the destination.
        cap: u32,
        /// Source.
        src: Addr,
        /// First character, from 0.
        start: Expr,
        /// Number of characters.
        count: Expr,
    },
    /// `dst := src` with its `len` characters from the 1-based `pos`
    /// replaced by `ins` (removed without `ins`); `pos` is clamped to
    /// `1..=len(src) + 1`, `len` to what remains.
    Splice {
        /// Destination.
        dst: Addr,
        /// Capacity of the destination.
        cap: u32,
        /// Source.
        src: Addr,
        /// Position, from 1.
        pos: Expr,
        /// Number of characters replaced.
        len: Expr,
        /// Inserted string.
        ins: Option<Addr>,
    },
}

/// A copy of bytes between two places (whole arrays and structures).
#[derive(Debug, Clone, PartialEq)]
pub struct Copy {
    /// Destination.
    pub dst: Addr,
    /// Source.
    pub src: Addr,
    /// Number of bytes.
    pub size: u32,
}

/// A typed expression with its span.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    /// Kind.
    pub kind: ExprKind,
    /// Type of the value (`i32` addresses use `u32`).
    pub ty: Scalar,
    /// Source span.
    pub span: Span,
}

/// A call (section 3.3).
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// Callee.
    pub func: FuncId,
    /// For a function block: the addresses of the three parts of the
    /// instance.
    pub instance: Option<[Addr; 3]>,
    /// For a function: its frame, reset before the arguments are written.
    pub frame: Option<ObjId>,
    /// Inputs: place in the callee, value (evaluated left to right before
    /// anything is written).
    pub inputs: Vec<(Place, Expr)>,
    /// In-outs: pointer slot in the callee, address passed.
    pub in_outs: Vec<(Addr, Addr)>,
    /// Arrays and structures copied into the inputs, after the inputs.
    pub copies_in: Vec<Copy>,
    /// Strings assigned to the inputs, after the copies (destination,
    /// capacity, source, wide).
    pub strings_in: Vec<(Addr, u32, Addr, bool)>,
    /// Outputs, copied after the call: place in the caller, value read from
    /// the callee.
    pub outputs: Vec<(Place, Expr)>,
    /// Arrays and structures copied from the outputs, after the outputs.
    pub copies_out: Vec<Copy>,
    /// Strings assigned from the outputs (destination, capacity, source,
    /// wide).
    pub strings_out: Vec<(Addr, u32, Addr, bool)>,
    /// Result of a function, read after the outputs.
    pub result: Option<Place>,
}

/// Statement kind (section 3.3).
#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// Scalar assignment.
    Assign(Place, Expr),
    /// Copy of an array or a structure (one part of it).
    Copy(Copy),
    /// String operation; `true` for wide strings.
    Str(Box<StrOp>, bool),
    /// Assignment of a temporary.
    SetTemp(TempId, Expr),
    /// Call as a statement.
    Call(Box<Call>),
    /// Expression evaluated for its effects.
    Eval(Expr),
    /// Two-way branch.
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    /// Multi-way branch on an integer: inclusive ranges and bodies, default.
    Switch(Expr, Vec<SwitchArm>, Vec<Stmt>),
    /// Block; `Break(label)` jumps to its end.
    Block(Label, Vec<Stmt>),
    /// Loop; `Continue(label)` jumps to its start, `Break(label)` to its
    /// end; falling off the end leaves it.
    Loop(Label, Vec<Stmt>),
    /// Exit of a block or loop.
    Break(Label),
    /// Back to the start of a loop.
    Continue(Label),
    /// End of the current function.
    Return,
    /// Trap with a code when the condition is false (ABI-080).
    Check(Expr, u32, SiteId),
    /// Debug hook call (ABI-032).
    DebugSite(SiteId),
    /// Resets an object to its initial image (frames of `VAR_TEMP`).
    Reset(ObjId),
}

/// An arm of a switch: inclusive ranges of values and body.
pub type SwitchArm = (Vec<(i128, i128)>, Vec<Stmt>);

/// A statement with its span.
#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    /// Kind.
    pub kind: StmtKind,
    /// Source span.
    pub span: Span,
}

/// Kind of function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FuncKind {
    /// Program: no parameters, absolute addresses.
    Program,
    /// Function block: three parameters, the parts of its instance.
    FunctionBlock,
    /// Function: a static frame, reset by the caller.
    Function,
}

/// A function of the module.
#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    /// Upper-case name of the POU.
    pub name: String,
    /// Kind.
    pub kind: FuncKind,
    /// Frame of a function.
    pub frame: Option<ObjId>,
    /// Types of the temporaries.
    pub temps: Vec<Scalar>,
    /// Body.
    pub body: Vec<Stmt>,
    /// Span of the POU name.
    pub span: Span,
}

/// A task.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// Name.
    pub name: String,
    /// Interval in nanoseconds.
    pub interval_ns: u64,
    /// Priority.
    pub priority: u32,
    /// Program functions in execution order, with their instance paths.
    pub programs: Vec<(FuncId, String)>,
}

/// A variable published in the symbol map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leaf {
    /// Upper-case path (SYM-010).
    pub path: String,
    /// Type name.
    pub type_name: String,
    /// Object and offset.
    pub object: ObjId,
    /// Offset in the object.
    pub offset: u32,
    /// Size in bytes.
    pub size: u32,
    /// Flags other than `RETAIN`, which follows the region.
    pub flags: u32,
    /// Direct address.
    pub location: Option<String>,
    /// Site of the declaration.
    pub declared: SiteId,
    /// Values of an enumeration: names and values.
    pub enumeration: Option<Vec<(String, i64)>>,
    /// Array of an elementary type: bounds of each dimension and stride
    /// (SYM-013).
    pub array: Option<(Vec<(i64, i64)>, u32)>,
}

/// The module.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Module {
    /// Static objects.
    pub objects: Vec<Object>,
    /// Functions.
    pub functions: Vec<Function>,
    /// Tasks.
    pub tasks: Vec<Task>,
    /// Source sites.
    pub sites: Vec<Span>,
    /// Symbol map leaves.
    pub leaves: Vec<Leaf>,
}

// ----- text form

impl Module {
    /// Text form, for snapshot tests and debugging.
    pub fn dump(&self) -> String {
        let mut out = String::new();
        for (i, o) in self.objects.iter().enumerate() {
            let nonzero: Vec<String> = o
                .init
                .iter()
                .enumerate()
                .filter(|(_, b)| **b != 0)
                .map(|(i, b)| format!("{i}:{b:#x}"))
                .collect();
            let _ = writeln!(
                out,
                "object {i} {} {:?} size {} align {} init [{}]",
                o.name,
                o.region,
                o.init.len(),
                o.align,
                nonzero.join(" ")
            );
        }
        for l in &self.leaves {
            let array = l.array.as_ref().map(|(dims, stride)| {
                let d: Vec<String> = dims.iter().map(|(a, b)| format!("{a}..{b}")).collect();
                format!(" array [{}] stride {stride}", d.join(","))
            });
            let values = l.enumeration.as_ref().map(|v| {
                let v: Vec<String> = v.iter().map(|(n, x)| format!("{n}={x}")).collect();
                format!(" enum ({})", v.join(", "))
            });
            let _ = writeln!(
                out,
                "leaf {} {} @{}+{} size {} flags {}{}{}{}",
                l.path,
                l.type_name,
                l.object,
                l.offset,
                l.size,
                l.flags,
                l.location
                    .as_ref()
                    .map(|x| format!(" {x}"))
                    .unwrap_or_default(),
                array.unwrap_or_default(),
                values.unwrap_or_default()
            );
        }
        for t in &self.tasks {
            let p: Vec<String> = t
                .programs
                .iter()
                .map(|(f, p)| format!("{p}=f{f}"))
                .collect();
            let _ = writeln!(
                out,
                "task {} {} {} [{}]",
                t.name,
                t.interval_ns,
                t.priority,
                p.join(", ")
            );
        }
        for (i, f) in self.functions.iter().enumerate() {
            let temps: Vec<String> = f.temps.iter().map(|t| t.name()).collect();
            let _ = writeln!(
                out,
                "func {i} {} {:?}{} temps [{}]",
                f.name,
                f.kind,
                f.frame.map(|o| format!(" frame {o}")).unwrap_or_default(),
                temps.join(", ")
            );
            stmts(&mut out, &f.body, 1);
        }
        out
    }
}

fn addr(a: &Addr) -> String {
    let base = match &a.base {
        Base::Object(o) => format!("o{o}"),
        Base::SelfPart(k) => format!("self{k}"),
        Base::Deref(inner) => format!("*[{}]", addr(inner)),
        Base::Index(inner, e) => format!("({} + {})", addr(inner), expr(e)),
    };
    if a.offset == 0 {
        base
    } else {
        format!("{base}+{}", a.offset)
    }
}

fn place(p: &Place) -> String {
    format!("{}:{}", addr(&p.addr), p.ty.name())
}

/// Text form of an expression.
pub fn expr(e: &Expr) -> String {
    let t = e.ty.name();
    match &e.kind {
        ExprKind::Const(c) => format!("{c:?}:{t}"),
        ExprKind::Load(p) => format!("[{}]", place(p)),
        ExprKind::Temp(i) => format!("t{i}:{t}"),
        ExprKind::AddrOf(a) => format!("&{}", addr(a)),
        ExprKind::Unary(op, a) => format!("({op:?}:{t} {})", expr(a)),
        ExprKind::Binary(op, a, b, site) => format!(
            "({}:{t}{} {} {})",
            op.symbol(),
            site.map(|s| format!(" check@{s}")).unwrap_or_default(),
            expr(a),
            expr(b)
        ),
        ExprKind::Convert(m, a) => format!("({m:?}:{t} {})", expr(a)),
        ExprKind::Select(c, a, b) => format!("(select:{t} {} {} {})", expr(c), expr(a), expr(b)),
        ExprKind::Intrinsic(i, args) => {
            let a: Vec<String> = args.iter().map(expr).collect();
            format!("({i:?}:{t} {})", a.join(" "))
        }
        ExprKind::Call(c) => format!("{}:{t}", call(c)),
        ExprKind::Index {
            index,
            low,
            count,
            stride,
            site,
        } => format!(
            "(index {} from {low} count {count} stride {stride}{})",
            expr(index),
            site.map(|s| format!(" check@{s}")).unwrap_or_default()
        ),
        ExprKind::Checked(a, low, high, site) => {
            format!("(checked {low}..{high} check@{site} {})", expr(a))
        }
        ExprKind::Seq(pre, e) => {
            let mut s = String::new();
            stmts(&mut s, pre, 0);
            format!("(seq {{{}}} {})", s.trim_end().replace('\n', "; "), expr(e))
        }
        ExprKind::StrLen(a, w) => format!("(len{} {})", wide(*w), addr(a)),
        ExprKind::StrFind(a, b, w) => format!("(find{} {} {})", wide(*w), addr(a), addr(b)),
        ExprKind::StrCmp(a, b, w) => format!("(cmp{} {} {})", wide(*w), addr(a), addr(b)),
        ExprKind::StrChar(a, w) => format!("(char{} {})", wide(*w), addr(a)),
    }
}

fn call(c: &Call) -> String {
    let mut parts = vec![];
    if let Some(i) = &c.instance {
        parts.push(format!(
            "inst {} {} {}",
            addr(&i[0]),
            addr(&i[1]),
            addr(&i[2])
        ));
    }
    if let Some(f) = c.frame {
        parts.push(format!("frame o{f}"));
    }
    for (p, e) in &c.inputs {
        parts.push(format!("{} := {}", place(p), expr(e)));
    }
    for (s, a) in &c.in_outs {
        parts.push(format!("{} := &{}", addr(s), addr(a)));
    }
    for c in &c.copies_in {
        parts.push(copy(c));
    }
    for (d, cap, s, w) in &c.strings_in {
        parts.push(format!(
            "str{} {} cap {cap} := {}",
            wide(*w),
            addr(d),
            addr(s)
        ));
    }
    for (p, e) in &c.outputs {
        parts.push(format!("{} <= {}", place(p), expr(e)));
    }
    for c in &c.copies_out {
        parts.push(copy(c));
    }
    for (d, cap, s, w) in &c.strings_out {
        parts.push(format!(
            "str{} {} cap {cap} <= {}",
            wide(*w),
            addr(d),
            addr(s)
        ));
    }
    if let Some(r) = &c.result {
        parts.push(format!("result {}", place(r)));
    }
    format!("(call f{} {})", c.func, parts.join(", "))
}

fn wide(w: bool) -> &'static str {
    if w {
        ".w"
    } else {
        ""
    }
}

fn str_op(op: &StrOp, w: bool) -> String {
    let w = wide(w);
    match op {
        StrOp::Assign { dst, cap, src } => {
            format!("str{w} {} cap {cap} := {}", addr(dst), addr(src))
        }
        StrOp::Clear(d) => format!("str{w} {} := ''", addr(d)),
        StrOp::Convert { dst, cap, src } => {
            format!("str{w} {} cap {cap} := convert {}", addr(dst), addr(src))
        }
        StrOp::SetChar { dst, value } => format!("str{w} {} := char {}", addr(dst), expr(value)),
        StrOp::Append {
            dst,
            cap,
            src,
            start,
            count,
        } => format!(
            "str{w} {} cap {cap} append {} from {} count {}",
            addr(dst),
            addr(src),
            expr(start),
            expr(count)
        ),
        StrOp::Splice {
            dst,
            cap,
            src,
            pos,
            len,
            ins,
        } => format!(
            "str{w} {} cap {cap} splice {} at {} len {} with {}",
            addr(dst),
            addr(src),
            expr(pos),
            expr(len),
            ins.as_ref().map_or("-".into(), addr)
        ),
    }
}

fn copy(c: &Copy) -> String {
    format!("copy {} <- {} size {}", addr(&c.dst), addr(&c.src), c.size)
}

fn stmts(out: &mut String, body: &[Stmt], depth: usize) {
    for s in body {
        let pad = "  ".repeat(depth);
        match &s.kind {
            StmtKind::Assign(p, e) => {
                let _ = writeln!(out, "{pad}{} := {}", place(p), expr(e));
            }
            StmtKind::SetTemp(t, e) => {
                let _ = writeln!(out, "{pad}t{t} := {}", expr(e));
            }
            StmtKind::Copy(c) => {
                let _ = writeln!(out, "{pad}{}", copy(c));
            }
            StmtKind::Str(op, w) => {
                let _ = writeln!(out, "{pad}{}", str_op(op, *w));
            }
            StmtKind::Call(c) => {
                let _ = writeln!(out, "{pad}{}", call(c));
            }
            StmtKind::Eval(e) => {
                let _ = writeln!(out, "{pad}eval {}", expr(e));
            }
            StmtKind::If(c, a, b) => {
                let _ = writeln!(out, "{pad}if {}", expr(c));
                stmts(out, a, depth + 1);
                if !b.is_empty() {
                    let _ = writeln!(out, "{pad}else");
                    stmts(out, b, depth + 1);
                }
            }
            StmtKind::Switch(sel, arms, default) => {
                let _ = writeln!(out, "{pad}switch {}", expr(sel));
                for (labels, body) in arms {
                    let l: Vec<String> = labels.iter().map(|(a, b)| format!("{a}..{b}")).collect();
                    let _ = writeln!(out, "{pad}  case {}", l.join(","));
                    stmts(out, body, depth + 2);
                }
                let _ = writeln!(out, "{pad}  default");
                stmts(out, default, depth + 2);
            }
            StmtKind::Block(l, b) => {
                let _ = writeln!(out, "{pad}block L{l}");
                stmts(out, b, depth + 1);
            }
            StmtKind::Loop(l, b) => {
                let _ = writeln!(out, "{pad}loop L{l}");
                stmts(out, b, depth + 1);
            }
            StmtKind::Break(l) => {
                let _ = writeln!(out, "{pad}break L{l}");
            }
            StmtKind::Continue(l) => {
                let _ = writeln!(out, "{pad}continue L{l}");
            }
            StmtKind::Return => {
                let _ = writeln!(out, "{pad}return");
            }
            StmtKind::Check(c, code, site) => {
                let _ = writeln!(out, "{pad}check {} code {code} site {site}", expr(c));
            }
            StmtKind::DebugSite(s) => {
                let _ = writeln!(out, "{pad}debug {s}");
            }
            StmtKind::Reset(o) => {
                let _ = writeln!(out, "{pad}reset o{o}");
            }
        }
    }
}

// ----- traversal

/// Callbacks of [`walk`]; every method does nothing by default.
pub trait Visitor {
    /// A statement, before its content.
    fn stmt(&mut self, _s: &Stmt) {}
    /// An expression, before its operands.
    fn expr(&mut self, _e: &Expr) {}
    /// A call, before its arguments.
    fn call(&mut self, _c: &Call) {}
}

/// Visits statements, calls and expressions depth first, including the
/// expressions inside addresses (array indexes).
pub fn walk(list: &[Stmt], v: &mut impl Visitor) {
    for s in list {
        v.stmt(s);
        match &s.kind {
            StmtKind::Assign(p, e) => {
                walk_addr(&p.addr, v);
                walk_expr(e, v);
            }
            StmtKind::Copy(c) => walk_copy(c, v),
            StmtKind::Str(op, _) => match &**op {
                StrOp::Assign { dst, src, .. } => {
                    walk_addr(dst, v);
                    walk_addr(src, v);
                }
                StrOp::Clear(d) => walk_addr(d, v),
                StrOp::Convert { dst, src, .. } => {
                    walk_addr(dst, v);
                    walk_addr(src, v);
                }
                StrOp::SetChar { dst, value } => {
                    walk_addr(dst, v);
                    walk_expr(value, v);
                }
                StrOp::Append {
                    dst,
                    src,
                    start,
                    count,
                    ..
                } => {
                    walk_addr(dst, v);
                    walk_addr(src, v);
                    walk_expr(start, v);
                    walk_expr(count, v);
                }
                StrOp::Splice {
                    dst,
                    src,
                    pos,
                    len,
                    ins,
                    ..
                } => {
                    walk_addr(dst, v);
                    walk_addr(src, v);
                    walk_expr(pos, v);
                    walk_expr(len, v);
                    if let Some(i) = ins {
                        walk_addr(i, v);
                    }
                }
            },
            StmtKind::SetTemp(_, e) | StmtKind::Eval(e) | StmtKind::Check(e, _, _) => {
                walk_expr(e, v)
            }
            StmtKind::Call(c) => walk_call(c, v),
            StmtKind::If(c, a, b) => {
                walk_expr(c, v);
                walk(a, v);
                walk(b, v);
            }
            StmtKind::Switch(sel, arms, default) => {
                walk_expr(sel, v);
                for (_, b) in arms {
                    walk(b, v);
                }
                walk(default, v);
            }
            StmtKind::Block(_, b) | StmtKind::Loop(_, b) => walk(b, v),
            StmtKind::Break(_)
            | StmtKind::Continue(_)
            | StmtKind::Return
            | StmtKind::DebugSite(_)
            | StmtKind::Reset(_) => {}
        }
    }
}

fn walk_copy(c: &Copy, v: &mut impl Visitor) {
    walk_addr(&c.dst, v);
    walk_addr(&c.src, v);
}

fn walk_addr(a: &Addr, v: &mut impl Visitor) {
    match &a.base {
        Base::Deref(inner) => walk_addr(inner, v),
        Base::Index(inner, e) => {
            walk_addr(inner, v);
            walk_expr(e, v);
        }
        Base::Object(_) | Base::SelfPart(_) => {}
    }
}

fn walk_call(c: &Call, v: &mut impl Visitor) {
    v.call(c);
    for a in c.instance.iter().flatten() {
        walk_addr(a, v);
    }
    for (p, e) in c.inputs.iter().chain(&c.outputs) {
        walk_addr(&p.addr, v);
        walk_expr(e, v);
    }
    for (s, a) in &c.in_outs {
        walk_addr(s, v);
        walk_addr(a, v);
    }
    for copy in c.copies_in.iter().chain(&c.copies_out) {
        walk_copy(copy, v);
    }
    for (d, _, s, _) in c.strings_in.iter().chain(&c.strings_out) {
        walk_addr(d, v);
        walk_addr(s, v);
    }
}

fn walk_expr(e: &Expr, v: &mut impl Visitor) {
    v.expr(e);
    match &e.kind {
        ExprKind::Const(_) | ExprKind::Temp(_) => {}
        ExprKind::Load(p) => walk_addr(&p.addr, v),
        ExprKind::AddrOf(a) => walk_addr(a, v),
        ExprKind::Unary(_, a) | ExprKind::Convert(_, a) | ExprKind::Checked(a, ..) => {
            walk_expr(a, v)
        }
        ExprKind::Index { index, .. } => walk_expr(index, v),
        ExprKind::Binary(_, a, b, _) => {
            walk_expr(a, v);
            walk_expr(b, v);
        }
        ExprKind::Select(a, b, c) => {
            walk_expr(a, v);
            walk_expr(b, v);
            walk_expr(c, v);
        }
        ExprKind::Intrinsic(_, args) => args.iter().for_each(|a| walk_expr(a, v)),
        ExprKind::Call(c) => walk_call(c, v),
        ExprKind::Seq(pre, e) => {
            walk(pre, v);
            walk_expr(e, v);
        }
        ExprKind::StrLen(a, _) | ExprKind::StrChar(a, _) => walk_addr(a, v),
        ExprKind::StrFind(a, b, _) | ExprKind::StrCmp(a, b, _) => {
            walk_addr(a, v);
            walk_addr(b, v);
        }
    }
}
