//! Provides definitions of tokens from IEC 61131-3.
use core::fmt;
use std::fmt::Debug;

use dsl::core::SourceSpan;
use logos::Logos;
/// The position of a token in a document.

#[derive(Debug)]
pub struct Token {
    /// The type of the token (what does this token represent).
    pub token_type: TokenType,
    /// The location in the source text where the token begins.
    pub span: SourceSpan,

    /// The line in the source text where the token begins.
    /// This is public only in the crate for the purpose of nice error messages.
    pub line: usize,

    /// The column in the source text where the token begins.
    /// This is public only in the crate for the purpose of nice error messages.
    pub col: usize,

    /// The text that this token matched.
    pub text: String,
}

impl Token {
    pub fn describe(&self) -> String {
        format!(
            "Type: {:?}, Value: '{}', At: Ln {},Col {}",
            self.token_type,
            self.text.replace('\n', "\\n").replace('\r', "\\r"),
            self.line,
            self.col
        )
    }
}

#[cfg(feature = "trace")]
impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "Type: {:?}, Value: '{}', At: Ln {},Col {}",
            self.token_type,
            self.text.replace('\n', "\\n").replace('\r', "\\r"),
            self.line,
            self.col
        ))
    }
}

#[cfg(not(feature = "trace"))]
impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "{}",
            self.text.replace('\n', "\\n").replace('\r', "\\r"),
        ))
    }
}

#[derive(Clone, Logos, Debug, PartialEq)]
pub enum TokenType {
    #[regex(r"\r\n")]
    #[regex(r"\n")]
    #[regex(r"\f")]
    Newline,

    #[regex(r"[ \t]+")]
    Whitespace,

    #[regex(r"\(\*(?:[^*]|\*[^\)])*\*\)", priority = 0)]
    // TODO The following is common but not valid. We want to recognize the token
    // so that we can generate meaningful errors.
    #[regex(r"//[^\r\n]*(\r\n|\n)?", priority = 0)]
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
    #[token("..")]
    Range,

    #[token("#")]
    Hash,

    // Separate the single byte and double byte representations
    // because those have different valid prefixes.
    #[regex(r"'[^']*'")]
    SingleByteString,
    #[regex("\"[^\"]*\"")]
    DoubleByteString,

    // B.1.1 Letters, digits and identifier
    // Lower priority than any keyword.
    #[regex(r"[A-Za-z_][A-Za-z0-9_]*", priority = 1)]
    Identifier,

    // B.1.2 Constants
    #[regex(r"16#[0-9A-F][0-9A-F_]*")]
    HexDigits,
    #[regex(r"8#[0-7][0-7_]*")]
    OctDigits,
    #[regex(r"2#[0-1][0-1_]*")]
    BinDigits,
    // Omit the -/+ prefix so that real does not consume the sign (just like digits)
    #[regex(r"(?:[0-9][0-9_]*)(?:\.[0-9_]+)(?:[eE][+-]?[0-9_]+)", priority = 1)]
    FloatingPoint,
    // Same as fixed point but without the exponential. This enables us to support
    // time values as fixed point that do not have the
    #[regex(r"(?:[0-9][0-9_]*)(?:\.[0-9_]+)", priority = 2)]
    FixedPoint,
    // We don't try to understand the literals here with complex regular expression
    // matching and precedence. Rather we identify some of the relevant constituent
    // parts and piece them together later.
    #[regex(r"[0-9][0-9_]*")]
    Digits,

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
    EndCase,

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
    EndIf,

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
    EndRepeat,

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
    EndVar,
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
    #[token("LREAL", ignore(case))]
    Lreal,
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

    #[regex(r"%[IQM]\*", ignore(case))]
    DirectAddressIncomplete,
    #[regex(r"%[IQM]([XBWDL])?(\d(\.\d)*)", ignore(case))]
    DirectAddress,

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

    #[token("=>")]
    RightArrow,
}

impl TokenType {
    pub fn describe(&self) -> &'static str {
        match self {
            TokenType::Newline => "'\\n' (new line)",
            TokenType::Whitespace => "' ' (space) | '\\t' (tab)",
            TokenType::Comment => "'(* ... *)' (comment)",
            TokenType::LeftParen => "'('",
            TokenType::RightParen => "')'",
            TokenType::LeftBrace => "'{'",
            TokenType::RightBrace => "'}'",
            TokenType::LeftBracket => "'['",
            TokenType::RightBracket => "']'",
            TokenType::Comma => "','",
            TokenType::Semicolon => "';'",
            TokenType::Colon => "':'",
            TokenType::Period => "'.'",
            TokenType::Range => "'..' (range)",
            TokenType::Hash => "'#'",
            TokenType::SingleByteString => "\\'[^\\']*\\' (single byte string)",
            TokenType::DoubleByteString => "\"[^\"]*\" (double byte string)",
            TokenType::Identifier => "(identifier)",
            TokenType::HexDigits => "16#[0-9A-F][0-9A-F_]* (hexadecimal bit string)",
            TokenType::OctDigits => "8#[0-7][0-7]* (octal bit string)",
            TokenType::BinDigits => "2#[0-1][0-1]* (binary bit string)",
            TokenType::FloatingPoint => "(floating point)",
            TokenType::FixedPoint => "(fixed point)",
            TokenType::Digits => "[0-9][0-9_]* (integer)",
            TokenType::Action => "'ACTION'",
            TokenType::EndAction => "'END_ACTION'",
            TokenType::Array => "'ARRAY'",
            TokenType::Of => "'OF'",
            TokenType::At => "'AT'",
            TokenType::Case => "'CASE'",
            TokenType::Else => "'ELSE'",
            TokenType::EndCase => "'END_CASE'",
            TokenType::Constant => "'CONSTANT'",
            TokenType::Configuration => "'CONFIGURATION'",
            TokenType::EndConfiguration => "'END_CONFIGURATION'",
            TokenType::En => "'EN'",
            TokenType::Eno => "'ENO'",
            TokenType::Exit => "'EXIT'",
            TokenType::False => "'FALSE'",
            TokenType::FEdge => "'F_EDGE'",
            TokenType::For => "'FOR'",
            TokenType::To => "'TO'",
            TokenType::By => "'BY'",
            TokenType::Do => "'DO'",
            TokenType::EndFor => "'END_FOR'",
            TokenType::Function => "'FUNCTION'",
            TokenType::EndFunction => "'END_FUNCTION'",
            TokenType::FunctionBlock => "'FUNCTION_BLOCK'",
            TokenType::EndFunctionBlock => "'END_FUNCTION_BLOCK'",
            TokenType::If => "'IF'",
            TokenType::Then => "'THEN'",
            TokenType::Elsif => "'ELSIF'",
            TokenType::EndIf => "'END_IF'",
            TokenType::InitialStep => "'INITIAL_STEP'",
            TokenType::EndStep => "'END_STEP'",
            TokenType::Program => "'PROGRAM'",
            TokenType::With => "'WITH'",
            TokenType::EndProgram => "'END_PROGRAM'",
            TokenType::REdge => "'R_EDGE'",
            TokenType::ReadOnly => "'READ_ONLY'",
            TokenType::ReadWrite => "'READ_WRITE'",
            TokenType::Repeat => "'REPEAT'",
            TokenType::Until => "'UNTIL'",
            TokenType::EndRepeat => "'END_REPEAT'",
            TokenType::Resource => "'RESOURCE'",
            TokenType::On => "'ON'",
            TokenType::EndResource => "'END_RESOURCE'",
            TokenType::Retain => "'RETAIN'",
            TokenType::NonRetain => "'NON_RETAIN'",
            TokenType::Return => "'RETURN'",
            TokenType::Step => "'STEP'",
            TokenType::Struct => "'STRUCT'",
            TokenType::EndStruct => "'END_STRUCT'",
            TokenType::Task => "'TASK'",
            TokenType::EndTask => "'END_TASK'",
            TokenType::Transition => "'TRANSITION'",
            TokenType::From => "'FROM'",
            TokenType::EndTransition => "'END_TRANSITION'",
            TokenType::True => "'TRUE'",
            TokenType::Type => "'TYPE'",
            TokenType::EndType => "'END_TYPE'",
            TokenType::Var => "'VAR'",
            TokenType::EndVar => "'END_VAR'",
            TokenType::VarInput => "'VAR_INPUT'",
            TokenType::VarOutput => "'VAR_OUTPUT'",
            TokenType::VarInOut => "'VAR_IN_OUT'",
            TokenType::VarTemp => "'VAR_TEMP'",
            TokenType::VarExternal => "'VAR_EXTERNAL'",
            TokenType::VarAccess => "'VAR_ACCESS'",
            TokenType::VarConfig => "'VAR_CONFIG'",
            TokenType::VarGlobal => "'VAR_GLOBAL'",
            TokenType::While => "'WHILE'",
            TokenType::EndWhile => "'END_WHILE'",
            TokenType::Bool => "'BOOL'",
            TokenType::Sint => "'SINT'",
            TokenType::Int => "'INT'",
            TokenType::Dint => "'DINT'",
            TokenType::Lint => "'LINT'",
            TokenType::Usint => "'USINT'",
            TokenType::Uint => "'UINT'",
            TokenType::Udint => "'UDINT'",
            TokenType::Ulint => "'ULINT'",
            TokenType::Real => "'REAL'",
            TokenType::Lreal => "'LREAL'",
            TokenType::Time => "'TIME'",
            TokenType::Date => "'DATE' | 'D'",
            TokenType::TimeOfDay => "'TIME_OF_DAY' | 'TOD'",
            TokenType::DateAndTime => "'DATE_AND_TIME' | 'DT'",
            TokenType::String => "'STRING'",
            TokenType::Byte => "'BYTE'",
            TokenType::Word => "'WORD'",
            TokenType::Dword => "'DWORD'",
            TokenType::Lword => "'LWORD'",
            TokenType::WString => "'WSTRING'",
            TokenType::DirectAddressIncomplete => "'%I*' | '%Q*' | '%M*' (incomplete address)",
            TokenType::DirectAddress => "%[IQM]([XBWDL])?(\\d(\\.\\d)*) (direct address)",
            TokenType::Or => "'OR'",
            TokenType::Xor => "'XOR'",
            TokenType::And => "'AND' | '&'",
            TokenType::Equal => "'='",
            TokenType::NotEqual => "'<>'",
            TokenType::Less => "'<'",
            TokenType::Greater => "'>'",
            TokenType::LessEqual => "'<='",
            TokenType::GreaterEqual => "'>='",
            TokenType::Div => "'/'",
            TokenType::Star => "'*'",
            TokenType::Plus => "'+'",
            TokenType::Minus => "'-'",
            TokenType::Mod => "'MOD'",
            TokenType::Power => "'**'",
            TokenType::Not => "'NOT'",
            TokenType::Assignment => "':='",
            TokenType::RightArrow => "'=>'",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FileId;
    use dsl::core::SourceSpan;

    #[test]
    fn test_token_describe() {
        let token = Token {
            token_type: TokenType::Identifier,
            span: SourceSpan {
                start: 0,
                end: 5,
                file_id: FileId::new(),
            },
            line: 1,
            col: 2,
            text: "hello".to_string(),
        };
        let desc = token.describe();
        assert!(desc.contains("Identifier"));
        assert!(desc.contains("hello"));
        assert!(desc.contains("Ln 1,Col 2"));
    }

    #[test]
    fn test_token_describe_with_newlines() {
        let token = Token {
            token_type: TokenType::Newline,
            span: SourceSpan {
                start: 0,
                end: 1,
                file_id: FileId::new(),
            },
            line: 3,
            col: 1,
            text: "\n".to_string(),
        };
        let desc = token.describe();
        assert!(desc.contains("\\n"));
    }

    #[test]
    fn test_token_describe_variants() {
        use TokenType::*;
        let file_id = FileId::new();
        let span = SourceSpan {
            start: 0,
            end: 1,
            file_id,
        };
        let cases = vec![
            (Newline, "\\n"),
            (Whitespace, " "),
            (Comment, "(* comment *)"),
            (LeftParen, "("),
            (RightParen, ")"),
            (LeftBrace, "{"),
            (RightBrace, "}"),
            (LeftBracket, "["),
            (RightBracket, "]"),
            (Comma, ","),
            (Semicolon, ";"),
            (Colon, ":"),
            (Period, "."),
            (Range, ".."),
            (Hash, "#"),
            (SingleByteString, "'abc'"),
            (DoubleByteString, "\"abc\""),
            (Identifier, "ident"),
            (HexDigits, "16#A1"),
            (OctDigits, "8#77"),
            (BinDigits, "2#01"),
            (FloatingPoint, "1.23e4"),
            (FixedPoint, "1.23"),
            (Digits, "123"),
            (Action, "ACTION"),
            (EndAction, "END_ACTION"),
            (Array, "ARRAY"),
            (Of, "OF"),
            (At, "AT"),
            (Case, "CASE"),
            (Else, "ELSE"),
            (EndCase, "END_CASE"),
            (Constant, "CONSTANT"),
            (Configuration, "CONFIGURATION"),
            (EndConfiguration, "END_CONFIGURATION"),
            (En, "EN"),
            (Eno, "ENO"),
            (Exit, "EXIT"),
            (False, "FALSE"),
            (FEdge, "F_EDGE"),
            (For, "FOR"),
            (To, "TO"),
            (By, "BY"),
            (Do, "DO"),
            (EndFor, "END_FOR"),
            (Function, "FUNCTION"),
            (EndFunction, "END_FUNCTION"),
            (FunctionBlock, "FUNCTION_BLOCK"),
            (EndFunctionBlock, "END_FUNCTION_BLOCK"),
            (If, "IF"),
            (Then, "THEN"),
            (Elsif, "ELSIF"),
            (EndIf, "END_IF"),
            (InitialStep, "INITIAL_STEP"),
            (EndStep, "END_STEP"),
            (Program, "PROGRAM"),
            (With, "WITH"),
            (EndProgram, "END_PROGRAM"),
            (REdge, "R_EDGE"),
            (ReadOnly, "READ_ONLY"),
            (ReadWrite, "READ_WRITE"),
            (Repeat, "REPEAT"),
            (Until, "UNTIL"),
            (EndRepeat, "END_REPEAT"),
            (Resource, "RESOURCE"),
            (On, "ON"),
            (EndResource, "END_RESOURCE"),
            (Retain, "RETAIN"),
            (NonRetain, "NON_RETAIN"),
            (Return, "RETURN"),
            (Step, "STEP"),
            (Struct, "STRUCT"),
            (EndStruct, "END_STRUCT"),
            (Task, "TASK"),
            (EndTask, "END_TASK"),
            (Transition, "TRANSITION"),
            (From, "FROM"),
            (EndTransition, "END_TRANSITION"),
            (True, "TRUE"),
            (Type, "TYPE"),
            (EndType, "END_TYPE"),
            (Var, "VAR"),
            (EndVar, "END_VAR"),
            (VarInput, "VAR_INPUT"),
            (VarOutput, "VAR_OUTPUT"),
            (VarInOut, "VAR_IN_OUT"),
            (VarTemp, "VAR_TEMP"),
            (VarExternal, "VAR_EXTERNAL"),
            (VarAccess, "VAR_ACCESS"),
            (VarConfig, "VAR_CONFIG"),
            (VarGlobal, "VAR_GLOBAL"),
            (While, "WHILE"),
            (EndWhile, "END_WHILE"),
            (Bool, "BOOL"),
            (Sint, "SINT"),
            (Int, "INT"),
            (Dint, "DINT"),
            (Lint, "LINT"),
            (Usint, "USINT"),
            (Uint, "UINT"),
            (Udint, "UDINT"),
            (Ulint, "ULINT"),
            (Real, "REAL"),
            (Lreal, "LREAL"),
            (Time, "TIME"),
            (Date, "DATE"),
            (TimeOfDay, "TIME_OF_DAY"),
            (DateAndTime, "DATE_AND_TIME"),
            (String, "STRING"),
            (Byte, "BYTE"),
            (Word, "WORD"),
            (Dword, "DWORD"),
            (Lword, "LWORD"),
            (WString, "WSTRING"),
            (DirectAddressIncomplete, "%I*"),
            (DirectAddress, "%I0.0"),
            (Or, "OR"),
            (Xor, "XOR"),
            (And, "AND"),
            (Equal, "="),
            (NotEqual, "<>"),
            (Less, "<"),
            (Greater, ">"),
            (LessEqual, "<="),
            (GreaterEqual, ">="),
            (Div, "/"),
            (Star, "*"),
            (Plus, "+"),
            (Minus, "-"),
            (Mod, "MOD"),
            (Power, "**"),
            (Not, "NOT"),
            (Assignment, ":="),
            (RightArrow, "=>"),
        ];
        for (token_type, text) in cases {
            let token = Token {
                token_type,
                span: span.clone(),
                line: 1,
                col: 1,
                text: text.to_string(),
            };
            let desc = token.describe();
            // Check that the variant name or text appears in the description
            assert!(
                desc.contains(&format!("{:?}", token.token_type)),
                "Missing {:?} in {}",
                token.token_type,
                desc
            );
        }
    }
}
