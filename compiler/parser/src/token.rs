//! Provides definitions of tokens from IEC 61131-3.
use logos::{Lexer, Logos, Skip};

/// The position of a token in a document.
#[derive(Debug, Default, PartialEq)]
pub struct Position {
    /// The line number (0-indexed)
    line: usize,
    /// The column number (0-indexed)
    column: usize,
}

/// Update the line count and the char index.
fn newline_callback(lex: &mut Lexer<TokenType>) -> Skip {
    lex.extras.line += 1;
    lex.extras.column = lex.span().end;
    Skip
}

/// Compute the line and column position for the current token.
fn token_callback(lex: &mut Lexer<TokenType>) -> Position {
    let line = lex.extras.line;
    let column = lex.span().start - lex.extras.column;

    Position { line, column }
}

#[derive(Logos, Debug, PartialEq)]
#[logos(extras = Position)]
pub enum TokenType {
    #[regex(r"[\n\r\f]", newline_callback)]
    Newline,

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
    String(Position),

    // B.1.1 Letters, digits and identifier
    #[regex(r"[A-Za-z0-9_]+", token_callback)]
    Identifier(Position),

    // B.1.3.3
    #[token("ARRAY", token_callback, ignore(case))]
    Array(Position),

    // B.1.4.3 Declarations and initialization
    #[token("VAR", token_callback, ignore(case))]
    Var(Position),
    #[token("END_VAR", token_callback, ignore(case))]
    VarEnd(Position),
    #[token("RETAIN", token_callback, ignore(case))]
    Retain(Position),
    #[token("CONSTANT", token_callback, ignore(case))]
    Constant(Position),

    // B.1.4.3 Declarations and initialization
    #[token("AT", token_callback, ignore(case))]
    At(Position),
    #[token("%", token_callback, ignore(case))]
    Percent(Position),

    // B.1.5.1 Functions
    #[token("FUNCTION", token_callback, ignore(case))]
    Function(Position),
    #[token("END_FUNCTION", token_callback, ignore(case))]
    EndFunction(Position),

    // B.1.5.2 Function blocks
    #[token("FUNCTION_BLOCK", token_callback, ignore(case))]
    FunctionBlock(Position),
    #[token("END_FUNCTION_BLOCK", token_callback, ignore(case))]
    EndFunctionBlock(Position),

    // B.1.7 Configuration elements
    #[token("CONFIGURATION", token_callback, ignore(case))]
    Configuration(Position),
    #[token("END_CONFIGURATION", token_callback, ignore(case))]
    EndConfiguration(Position),
    #[token("RESOURCE", token_callback, ignore(case))]
    Resource(Position),
    #[token("ON", token_callback, ignore(case))]
    On(Position),
    #[token("END_RESOURCE", token_callback, ignore(case))]
    EndResource(Position),
    #[token("TASK", token_callback, ignore(case))]
    Task(Position),
    #[token("INTERNAL", token_callback, ignore(case))]
    Interval(Position),
    #[token("PRIORITY", token_callback, ignore(case))]
    Priority(Position),
    #[token("END_TASK", token_callback, ignore(case))]
    EndTask(Position),
    #[token("PROGRAM", token_callback, ignore(case))]
    Program(Position),
    #[token("WITH", token_callback, ignore(case))]
    With(Position),
    #[token("END_PROGRAM", token_callback, ignore(case))]
    EndProgram(Position),

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

    // B.3.2.3 Selection statements
    #[token("IF", token_callback, ignore(case))]
    If(Position),
    #[token("THEN", token_callback, ignore(case))]
    Then(Position),
    #[token("ELSIF", token_callback, ignore(case))]
    Elsif(Position),
    #[token("ELSE", token_callback, ignore(case))]
    Else(Position),
    #[token("END_IF", token_callback, ignore(case))]
    IfEnd(Position),

    #[token("CASE", token_callback, ignore(case))]
    Case(Position),
    #[token("OF", token_callback, ignore(case))]
    Of(Position),
    #[token("END_CASE", token_callback, ignore(case))]
    CaseEnd(Position),

    #[token("FOR", token_callback, ignore(case))]
    For(Position),
    #[token("DO", token_callback, ignore(case))]
    Do(Position),
    #[token("END_FOR", token_callback, ignore(case))]
    ForEnd(Position),

    #[token("WHILE", token_callback, ignore(case))]
    While(Position),
    #[token("END_WHILE", token_callback, ignore(case))]
    EndWhile(Position),

    #[token("REPEAT", token_callback, ignore(case))]
    Repeat(Position),
    #[token("UNTIL", token_callback, ignore(case))]
    Until(Position),
    #[token("END_REPEAT", token_callback, ignore(case))]
    RepeatEnd(Position),

    #[token("EXIT", token_callback, ignore(case))]
    Exit(Position),
}
