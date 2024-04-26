//! Provides definitions of tokens from IEC 61131-3.
use std::fmt::Debug;

use logos::{Lexer, Logos};

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
        //f.debug_struct("Pos").field("l", &self.line).field("c", &self.column).finish()
    }
}

/// Update the line count and the char index.
fn newline_callback(lex: &mut Lexer<TokenType>) -> Position {
    let pos = Position {
        line: lex.extras.line,
        column: lex.extras.column,
    };
    lex.extras.line += 1;
    lex.extras.column = lex.span().end;
    pos
}

/// Compute the line and column position for the current token.
fn token_callback(lex: &mut Lexer<TokenType>) -> Position {
    let line = lex.extras.line;
    let column = lex.span().start - lex.extras.column;

    Position { line, column }
}

#[derive(Copy, Clone, Logos, Debug, PartialEq)]
#[logos(extras = Position)]
pub enum TokenType {
    #[regex(r"[\n\r\f]", newline_callback)]
    Newline(Position),

    #[regex(r"[ \t]+", token_callback)]
    Whitespace(Position),

    // TODO this will not necessarily detect the right end position
    #[regex(r"\(\*[^\*\)]*\*\)", token_callback, priority = 0)]
    Comment(Position),

    // Grouping and other markers
    #[token("(", token_callback, priority = 1)]
    LeftParen(Position),
    #[token(")", token_callback)]
    RightParen(Position),
    #[token("{", token_callback)]
    LeftBrace(Position),
    #[token("}", token_callback)]
    RightBrace(Position),
    #[token("[", token_callback)]
    LeftBracket(Position),
    #[token("]", token_callback)]
    RightBracket(Position),
    #[token(",", token_callback)]
    Comma(Position),
    #[token(";", token_callback)]
    Semicolon(Position),
    #[token(":", token_callback)]
    Colon(Position),
    #[token(".", token_callback)]
    Period(Position),

    // TODO It would be nice for this to be associated with a type
    #[token("#", token_callback)]
    Hash(Position),
    #[regex(r"'[^']*'", token_callback)]
    #[regex("\"[^\"]*\"", token_callback)]
    StringLiteral(Position),

    // B.1.1 Letters, digits and identifier
    // Lower priority than any keyword.
    #[regex(r"[A-Za-z][A-Za-z0-9_]*", token_callback, priority = 1)]
    Identifier(Position),

    // B.1.2 Constants
    // We don't try to understand the literals here with complex regular expression
    // matching and precedence. Rather we identify some of the relevant constituent
    // parts and piece them together later.
    #[regex(r"[0-9_]+", token_callback)]
    Integer(Position),

    #[token("ACTION", token_callback, ignore(case))]
    Action(Position),
    #[token("END_ACTION", token_callback, ignore(case))]
    EndAction(Position),

    #[token("ARRAY", token_callback, ignore(case))]
    Array(Position),
    #[token("OF", token_callback, ignore(case))]
    Of(Position),

    #[token("AT", token_callback, ignore(case))]
    At(Position),

    #[token("CASE", token_callback, ignore(case))]
    Case(Position),
    #[token("ELSE", token_callback, ignore(case))]
    Else(Position),
    #[token("END_CASE", token_callback, ignore(case))]
    CaseEnd(Position),

    #[token("CONSTANT", token_callback, ignore(case))]
    Constant(Position),

    #[token("CONFIGURATION", token_callback, ignore(case))]
    Configuration(Position),
    #[token("END_CONFIGURATION", token_callback, ignore(case))]
    EndConfiguration(Position),

    #[token("EN", token_callback, ignore(case))]
    En(Position),
    #[token("ENO", token_callback, ignore(case))]
    Eno(Position),

    #[token("EXIT", token_callback, ignore(case))]
    Exit(Position),

    #[token("FALSE", token_callback, ignore(case))]
    False(Position),

    #[token("F_EDGE", token_callback, ignore(case))]
    FEdge(Position),

    #[token("FOR", token_callback, ignore(case))]
    For(Position),
    #[token("TO", token_callback, ignore(case))]
    To(Position),
    #[token("BY", token_callback, ignore(case))]
    By(Position),
    #[token("DO", token_callback, ignore(case))]
    Do(Position),
    #[token("END_FOR", token_callback, ignore(case))]
    EndFor(Position),

    #[token("FUNCTION", token_callback, ignore(case))]
    Function(Position),
    #[token("END_FUNCTION", token_callback, ignore(case))]
    EndFunction(Position),

    #[token("FUNCTION_BLOCK", token_callback, ignore(case))]
    FunctionBlock(Position),
    #[token("END_FUNCTION_BLOCK", token_callback, ignore(case))]
    EndFunctionBlock(Position),

    #[token("IF", token_callback, ignore(case))]
    If(Position),
    #[token("THEN", token_callback, ignore(case))]
    Then(Position),
    #[token("ELSIF", token_callback, ignore(case))]
    Elsif(Position),
    #[token("END_IF", token_callback, ignore(case))]
    IfEnd(Position),

    #[token("INITIAL_STEP", token_callback, ignore(case))]
    InitialStep(Position),
    #[token("END_STEP", token_callback, ignore(case))]
    EndStep(Position),

    #[token("PROGRAM", token_callback, ignore(case))]
    Program(Position),
    #[token("WITH", token_callback, ignore(case))]
    With(Position),
    #[token("END_PROGRAM", token_callback, ignore(case))]
    EndProgram(Position),

    #[token("R_EDGE", token_callback, ignore(case))]
    REdge(Position),

    #[token("READ_ONLY", token_callback, ignore(case))]
    ReadOnly(Position),
    #[token("READ_WRITE", token_callback, ignore(case))]
    ReadWrite(Position),

    #[token("REPEAT", token_callback, ignore(case))]
    Repeat(Position),
    #[token("UNTIL", token_callback, ignore(case))]
    Until(Position),
    #[token("END_REPEAT", token_callback, ignore(case))]
    RepeatEnd(Position),

    #[token("RESOURCE", token_callback, ignore(case))]
    Resource(Position),
    #[token("ON", token_callback, ignore(case))]
    On(Position),
    #[token("END_RESOURCE", token_callback, ignore(case))]
    EndResource(Position),

    #[token("RETAIN", token_callback, ignore(case))]
    Retain(Position),
    #[token("NON_RETAIN", token_callback, ignore(case))]
    NonRetain(Position),

    #[token("RETURN", token_callback, ignore(case))]
    Return(Position),

    #[token("STEP", token_callback, ignore(case))]
    Step(Position),

    #[token("STRUCT", token_callback, ignore(case))]
    Struct(Position),
    #[token("END_STRUCT", token_callback, ignore(case))]
    EndStruct(Position),

    #[token("TASK", token_callback, ignore(case))]
    Task(Position),
    #[token("END_TASK", token_callback, ignore(case))]
    EndTask(Position),

    #[token("TRANSITION", token_callback, ignore(case))]
    Transition(Position),
    #[token("FROM", token_callback, ignore(case))]
    From(Position),
    #[token("END_TRANSITION", token_callback, ignore(case))]
    EndTransition(Position),

    #[token("TRUE", token_callback, ignore(case))]
    True(Position),

    #[token("TYPE", token_callback, ignore(case))]
    Type(Position),
    #[token("END_TYPE", token_callback, ignore(case))]
    EndType(Position),

    #[token("VAR", token_callback, ignore(case))]
    Var(Position),
    #[token("END_VAR", token_callback, ignore(case))]
    VarEnd(Position),
    #[token("VAR_INPUT", token_callback, ignore(case))]
    VarInput(Position),
    #[token("VAR_OUTPUT", token_callback, ignore(case))]
    VarOutput(Position),
    #[token("VAR_IN_OUT", token_callback, ignore(case))]
    VarInOut(Position),
    #[token("VAR_TEMP", token_callback, ignore(case))]
    VarTemp(Position),
    #[token("VAR_EXTERNAL", token_callback, ignore(case))]
    VarExternal(Position),
    #[token("VAR_ACCESS", token_callback, ignore(case))]
    VarAccess(Position),
    #[token("VAR_CONFIG", token_callback, ignore(case))]
    VarConfig(Position),
    #[token("VAR_GLOBAL", token_callback, ignore(case))]
    VarGlobal(Position),

    #[token("WHILE", token_callback, ignore(case))]
    While(Position),
    #[token("END_WHILE", token_callback, ignore(case))]
    EndWhile(Position),

    #[token("BOOL", token_callback, ignore(case))]
    Bool(Position),
    #[token("SINT", token_callback, ignore(case))]
    Sint(Position),
    #[token("INT", token_callback, ignore(case))]
    Int(Position),
    #[token("DINT", token_callback, ignore(case))]
    Dint(Position),
    #[token("LINT", token_callback, ignore(case))]
    Lint(Position),
    #[token("USINT", token_callback, ignore(case))]
    Usint(Position),
    #[token("UINT", token_callback, ignore(case))]
    Uint(Position),
    #[token("UDINT", token_callback, ignore(case))]
    Udint(Position),
    #[token("ULINT", token_callback, ignore(case))]
    Ulint(Position),
    #[token("REAL", token_callback, ignore(case))]
    Real(Position),
    #[token("TIME", token_callback, ignore(case))]
    Time(Position),
    #[token("DATE", token_callback, ignore(case))]
    Date(Position),
    #[token("TIME_OF_DAY", token_callback, ignore(case))]
    #[token("TOD", token_callback, ignore(case))]
    TimeOfDay(Position),
    #[token("DATE_AND_TIME", token_callback, ignore(case))]
    #[token("DT", token_callback, ignore(case))]
    DateAndTime(Position),
    #[token("STRING", token_callback, ignore(case))]
    String(Position),
    #[token("BYTE", token_callback, ignore(case))]
    Byte(Position),
    #[token("WORD", token_callback, ignore(case))]
    Word(Position),
    #[token("DWORD", token_callback, ignore(case))]
    Dword(Position),
    #[token("LWORD", token_callback, ignore(case))]
    Lword(Position),
    #[token("WSTRING", token_callback, ignore(case))]
    WString(Position),

    #[regex(r"%[IQM]", token_callback, ignore(case))]
    Location(Position),

    // Expressions
    #[token("OR", token_callback, ignore(case))]
    Or(Position),
    #[token("XOR", token_callback, ignore(case))]
    Xor(Position),
    #[token("AND", token_callback, ignore(case))]
    #[token("&", token_callback)]
    And(Position),
    #[token("=", token_callback)]
    Equal(Position),
    #[token("<>", token_callback)]
    NotEqual(Position),
    #[token("<", token_callback)]
    Less(Position),
    #[token(">", token_callback)]
    Greater(Position),
    #[token("<=", token_callback)]
    LessEqual(Position),
    #[token(">=", token_callback)]
    GreaterEqual(Position),
    #[token("/", token_callback)]
    Div(Position),
    #[token("*", token_callback)]
    Star(Position),
    #[token("+", token_callback)]
    Plus(Position),
    #[token("-", token_callback)]
    Minus(Position),
    #[token("MOD", token_callback)]
    Mod(Position),
    #[token("**", token_callback)]
    Power(Position),
    #[token("NOT", token_callback)]
    Not(Position),

    #[token(":=", token_callback)]
    Assignment(Position),
}
