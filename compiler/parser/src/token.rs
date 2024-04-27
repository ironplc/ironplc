//! Provides definitions of tokens from IEC 61131-3.
use std::fmt::Debug;

use logos::Logos;

/// The position of a token in a document.
#[derive(Copy, Clone, Default, PartialEq)]
pub struct Position {
    /// The line number (0-indexed)
    pub line: usize,
    /// The column number (0-indexed)
    pub column: usize,
}

impl Debug for Position {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Ln {} Col {}", self.line, self.column))
    }
}

#[derive(Debug)]
pub struct Token {
    /// The type of the token (what does this token represent).
    pub token_type: TokenType,
    /// The location in the source text where the token begins.
    pub position: Position,
    /// The text that this token matched.
    pub text: String,
}

#[derive(Clone, Logos, Debug, PartialEq)]
#[logos(extras = Position)]
pub enum TokenType {
    #[regex(r"[\n\r\f]")]
    Newline,

    #[regex(r"[ \t]+")]
    Whitespace,

    // TODO this will not necessarily detect the right end position
    #[regex(r"\(\*[^\*\)]*\*\)", priority = 0)]
    Comment,

    // Grouping and other markers
    #[token("(", priority = 1)]
    LeftParen,
    #[token(")")]
    RightParen,
    #[token("{")]
    LeftBrace,
    #[token("}")]
    RightBrace,
    #[token("[")]
    LeftBracket,
    #[token("]")]
    RightBracket,
    #[token(",")]
    Comma,
    #[token(";")]
    Semicolon,
    #[token(":")]
    Colon,
    #[token(".")]
    Period,

    // TODO It would be nice for this to be associated with a type
    #[token("#")]
    Hash,
    #[regex(r"'[^']*'")]
    #[regex("\"[^\"]*\"")]
    StringLiteral,

    // B.1.1 Letters, digits and identifier
    // Lower priority than any keyword.
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*", priority = 1)]
    Identifier,

    // B.1.2 Constants
    // We don't try to understand the literals here with complex regular expression
    // matching and precedence. Rather we identify some of the relevant constituent
    // parts and piece them together later.
    #[regex(r"[0-9][0-9_]*")]
    Integer,

    #[token("ACTION", ignore(case))]
    Action,
    #[token("END_ACTION", ignore(case))]
    EndAction,

    #[token("ARRAY", ignore(case))]
    Array,
    #[token("OF", ignore(case))]
    Of,

    #[token("AT", ignore(case))]
    At,

    #[token("CASE", ignore(case))]
    Case,
    #[token("ELSE", ignore(case))]
    Else,
    #[token("END_CASE", ignore(case))]
    CaseEnd,

    #[token("CONSTANT", ignore(case))]
    Constant,

    #[token("CONFIGURATION", ignore(case))]
    Configuration,
    #[token("END_CONFIGURATION", ignore(case))]
    EndConfiguration,

    #[token("EN", ignore(case))]
    En,
    #[token("ENO", ignore(case))]
    Eno,

    #[token("EXIT", ignore(case))]
    Exit,

    #[token("FALSE", ignore(case))]
    False,

    #[token("F_EDGE", ignore(case))]
    FEdge,

    #[token("FOR", ignore(case))]
    For,
    #[token("TO", ignore(case))]
    To,
    #[token("BY", ignore(case))]
    By,
    #[token("DO", ignore(case))]
    Do,
    #[token("END_FOR", ignore(case))]
    EndFor,

    #[token("FUNCTION", ignore(case))]
    Function,
    #[token("END_FUNCTION", ignore(case))]
    EndFunction,

    #[token("FUNCTION_BLOCK", ignore(case))]
    FunctionBlock,
    #[token("END_FUNCTION_BLOCK", ignore(case))]
    EndFunctionBlock,

    #[token("IF", ignore(case))]
    If,
    #[token("THEN", ignore(case))]
    Then,
    #[token("ELSIF", ignore(case))]
    Elsif,
    #[token("END_IF", ignore(case))]
    IfEnd,

    #[token("INITIAL_STEP", ignore(case))]
    InitialStep,
    #[token("END_STEP", ignore(case))]
    EndStep,

    #[token("PROGRAM", ignore(case))]
    Program,
    #[token("WITH", ignore(case))]
    With,
    #[token("END_PROGRAM", ignore(case))]
    EndProgram,

    #[token("R_EDGE", ignore(case))]
    REdge,

    #[token("READ_ONLY", ignore(case))]
    ReadOnly,
    #[token("READ_WRITE", ignore(case))]
    ReadWrite,

    #[token("REPEAT", ignore(case))]
    Repeat,
    #[token("UNTIL", ignore(case))]
    Until,
    #[token("END_REPEAT", ignore(case))]
    RepeatEnd,

    #[token("RESOURCE", ignore(case))]
    Resource,
    #[token("ON", ignore(case))]
    On,
    #[token("END_RESOURCE", ignore(case))]
    EndResource,

    #[token("RETAIN", ignore(case))]
    Retain,
    #[token("NON_RETAIN", ignore(case))]
    NonRetain,

    #[token("RETURN", ignore(case))]
    Return,

    #[token("STEP", ignore(case))]
    Step,

    #[token("STRUCT", ignore(case))]
    Struct,
    #[token("END_STRUCT", ignore(case))]
    EndStruct,

    #[token("TASK", ignore(case))]
    Task,
    #[token("END_TASK", ignore(case))]
    EndTask,

    #[token("TRANSITION", ignore(case))]
    Transition,
    #[token("FROM", ignore(case))]
    From,
    #[token("END_TRANSITION", ignore(case))]
    EndTransition,

    #[token("TRUE", ignore(case))]
    True,

    #[token("TYPE", ignore(case))]
    Type,
    #[token("END_TYPE", ignore(case))]
    EndType,

    #[token("VAR", ignore(case))]
    Var,
    #[token("END_VAR", ignore(case))]
    VarEnd,
    #[token("VAR_INPUT", ignore(case))]
    VarInput,
    #[token("VAR_OUTPUT", ignore(case))]
    VarOutput,
    #[token("VAR_IN_OUT", ignore(case))]
    VarInOut,
    #[token("VAR_TEMP", ignore(case))]
    VarTemp,
    #[token("VAR_EXTERNAL", ignore(case))]
    VarExternal,
    #[token("VAR_ACCESS", ignore(case))]
    VarAccess,
    #[token("VAR_CONFIG", ignore(case))]
    VarConfig,
    #[token("VAR_GLOBAL", ignore(case))]
    VarGlobal,

    #[token("WHILE", ignore(case))]
    While,
    #[token("END_WHILE", ignore(case))]
    EndWhile,

    #[token("BOOL", ignore(case))]
    Bool,
    #[token("SINT", ignore(case))]
    Sint,
    #[token("INT", ignore(case))]
    Int,
    #[token("DINT", ignore(case))]
    Dint,
    #[token("LINT", ignore(case))]
    Lint,
    #[token("USINT", ignore(case))]
    Usint,
    #[token("UINT", ignore(case))]
    Uint,
    #[token("UDINT", ignore(case))]
    Udint,
    #[token("ULINT", ignore(case))]
    Ulint,
    #[token("REAL", ignore(case))]
    Real,
    #[token("TIME", ignore(case))]
    Time,
    #[token("DATE", ignore(case))]
    Date,
    #[token("TIME_OF_DAY", ignore(case))]
    #[token("TOD", ignore(case))]
    TimeOfDay,
    #[token("DATE_AND_TIME", ignore(case))]
    #[token("DT", ignore(case))]
    DateAndTime,
    #[token("STRING", ignore(case))]
    String,
    #[token("BYTE", ignore(case))]
    Byte,
    #[token("WORD", ignore(case))]
    Word,
    #[token("DWORD", ignore(case))]
    Dword,
    #[token("LWORD", ignore(case))]
    Lword,
    #[token("WSTRING", ignore(case))]
    WString,

    #[regex(r"%[IQM]", ignore(case))]
    Location,

    // Expressions
    #[token("OR", ignore(case))]
    Or,
    #[token("XOR", ignore(case))]
    Xor,
    #[token("AND", ignore(case))]
    #[token("&")]
    And,
    #[token("=")]
    Equal,
    #[token("<>")]
    NotEqual,
    #[token("<")]
    Less,
    #[token(">")]
    Greater,
    #[token("<=")]
    LessEqual,
    #[token(">=")]
    GreaterEqual,
    #[token("/")]
    Div,
    #[token("*")]
    Star,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("MOD")]
    Mod,
    #[token("**")]
    Power,
    #[token("NOT")]
    Not,

    #[token(":=")]
    Assignment,
}
