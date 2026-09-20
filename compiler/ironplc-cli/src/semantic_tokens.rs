//! Maps lexer tokens to LSP semantic tokens.
//!
//! The editor colours a document from the token types in
//! [`TOKEN_TYPE_LEGEND`]; each lexer [`Token`] either maps to one legend entry
//! or is dropped (punctuation, numbers and trivia, which the editor's own
//! grammar already styles).
use ironplc_parser::token::{Token, TokenType};
use lsp_types::{SemanticToken, SemanticTokenType};

/// Convert lexer tokens into the delta-encoded semantic token stream that the
/// LSP `textDocument/semanticTokens/full` response carries.
pub(crate) fn to_semantic_tokens(tokens: Vec<Token>) -> Vec<SemanticToken> {
    // The conversion produces tokens with absolute (line, col) values
    // stored in `delta_line` / `delta_start`. The LSP protocol requires
    // these fields to be encoded as deltas relative to the previous
    // emitted token, so fold over the sequence to convert them.
    let absolute: Vec<SemanticToken> = tokens
        .into_iter()
        .filter_map(|tok| LspTokenType(tok).into())
        .collect();
    to_deltas(absolute)
}

/// Encode an ordered sequence of `SemanticToken`s — whose `delta_line` and
/// `delta_start` fields hold absolute line/column values — as deltas relative
/// to the previous emitted token, per the LSP semantic-tokens spec.
fn to_deltas(absolute: Vec<SemanticToken>) -> Vec<SemanticToken> {
    let mut prev_line: u32 = 0;
    let mut prev_col: u32 = 0;
    let mut out = Vec::with_capacity(absolute.len());
    for tok in absolute {
        let line = tok.delta_line;
        let col = tok.delta_start;
        let delta_line = line.saturating_sub(prev_line);
        let delta_start = if delta_line == 0 {
            col.saturating_sub(prev_col)
        } else {
            col
        };
        out.push(SemanticToken {
            delta_line,
            delta_start,
            length: tok.length,
            token_type: tok.token_type,
            token_modifiers_bitset: tok.token_modifiers_bitset,
        });
        prev_line = line;
        prev_col = col;
    }
    out
}

// Token types that this produces.
pub static TOKEN_TYPE_LEGEND: [SemanticTokenType; 6] = [
    SemanticTokenType::VARIABLE,
    SemanticTokenType::KEYWORD,
    SemanticTokenType::MODIFIER,
    SemanticTokenType::COMMENT,
    SemanticTokenType::STRING,
    SemanticTokenType::OPERATOR,
];

const VARIABLE_INDEX: u32 = 0;
const KEYWORD_INDEX: u32 = 1;
const MODIFIER_INDEX: u32 = 2;
const COMMENT_INDEX: u32 = 3;
const STRING_INDEX: u32 = 4;
const OPERATOR_INDEX: u32 = 5;

struct LspTokenType(Token);

impl From<LspTokenType> for Option<SemanticToken> {
    fn from(val: LspTokenType) -> Self {
        let token_type = match val.0.token_type {
            TokenType::Newline => None,
            TokenType::Whitespace => None,
            TokenType::Comment => Some(COMMENT_INDEX),
            TokenType::LeftParen => None,
            TokenType::RightParen => None,
            TokenType::LeftBrace => None,
            TokenType::RightBrace => None,
            TokenType::Pragma => Some(KEYWORD_INDEX),
            TokenType::LeftBracket => None,
            TokenType::RightBracket => None,
            TokenType::Comma => None,
            TokenType::Semicolon => None,
            TokenType::Colon => None,
            TokenType::Period => None,
            TokenType::Hash => None,
            TokenType::String => Some(KEYWORD_INDEX),
            TokenType::Identifier => Some(VARIABLE_INDEX),
            TokenType::HexDigits => None,
            TokenType::OctDigits => None,
            TokenType::BinDigits => None,
            TokenType::FloatingPoint => None,
            TokenType::FixedPoint => None,
            TokenType::Digits => None,
            TokenType::Type => Some(KEYWORD_INDEX),
            TokenType::EndType => Some(KEYWORD_INDEX),
            TokenType::Array => Some(KEYWORD_INDEX),
            TokenType::Struct => Some(KEYWORD_INDEX),
            TokenType::EndStruct => Some(KEYWORD_INDEX),
            TokenType::WString => Some(KEYWORD_INDEX),
            TokenType::Var => Some(KEYWORD_INDEX),
            TokenType::VarInput => Some(KEYWORD_INDEX),
            TokenType::VarOutput => Some(KEYWORD_INDEX),
            TokenType::VarInOut => Some(KEYWORD_INDEX),
            TokenType::VarExternal => Some(KEYWORD_INDEX),
            TokenType::VarGlobal => Some(KEYWORD_INDEX),
            TokenType::EndVar => Some(KEYWORD_INDEX),
            TokenType::Retain => Some(MODIFIER_INDEX),
            TokenType::Constant => Some(MODIFIER_INDEX),
            TokenType::Persistent => Some(MODIFIER_INDEX),
            TokenType::At => Some(KEYWORD_INDEX),
            TokenType::DirectAddress => Some(OPERATOR_INDEX),
            TokenType::PartialAccessBit => Some(OPERATOR_INDEX),
            TokenType::PartialAccessByte => Some(OPERATOR_INDEX),
            TokenType::PartialAccessWord => Some(OPERATOR_INDEX),
            TokenType::PartialAccessDWord => Some(OPERATOR_INDEX),
            TokenType::PartialAccessLWord => Some(OPERATOR_INDEX),
            TokenType::DirectAddressIncomplete => Some(OPERATOR_INDEX),
            TokenType::Function => Some(KEYWORD_INDEX),
            TokenType::EndFunction => Some(KEYWORD_INDEX),
            TokenType::FunctionBlock => Some(KEYWORD_INDEX),
            TokenType::EndFunctionBlock => Some(KEYWORD_INDEX),
            TokenType::Extends => Some(KEYWORD_INDEX),
            TokenType::Implements => Some(KEYWORD_INDEX),
            TokenType::Interface => Some(KEYWORD_INDEX),
            TokenType::EndInterface => Some(KEYWORD_INDEX),
            TokenType::Abstract => Some(KEYWORD_INDEX),
            TokenType::Method => Some(KEYWORD_INDEX),
            TokenType::This => Some(KEYWORD_INDEX),
            TokenType::Super => Some(KEYWORD_INDEX),
            TokenType::EndMethod => Some(KEYWORD_INDEX),
            TokenType::Configuration => Some(KEYWORD_INDEX),
            TokenType::EndConfiguration => Some(KEYWORD_INDEX),
            TokenType::Resource => Some(KEYWORD_INDEX),
            TokenType::On => Some(KEYWORD_INDEX),
            TokenType::EndResource => Some(KEYWORD_INDEX),
            TokenType::Task => Some(KEYWORD_INDEX),
            TokenType::EndTask => Some(KEYWORD_INDEX),
            TokenType::Program => Some(KEYWORD_INDEX),
            TokenType::With => Some(KEYWORD_INDEX),
            TokenType::EndProgram => Some(KEYWORD_INDEX),
            TokenType::Or => Some(OPERATOR_INDEX),
            TokenType::Xor => Some(OPERATOR_INDEX),
            TokenType::And => Some(OPERATOR_INDEX),
            TokenType::AndThen => Some(OPERATOR_INDEX),
            TokenType::OrElse => Some(OPERATOR_INDEX),
            TokenType::Equal => Some(OPERATOR_INDEX),
            TokenType::NotEqual => Some(OPERATOR_INDEX),
            TokenType::Less => Some(OPERATOR_INDEX),
            TokenType::Greater => Some(OPERATOR_INDEX),
            TokenType::LessEqual => Some(OPERATOR_INDEX),
            TokenType::GreaterEqual => Some(OPERATOR_INDEX),
            TokenType::Div => Some(OPERATOR_INDEX),
            TokenType::Star => Some(OPERATOR_INDEX),
            TokenType::Plus => Some(OPERATOR_INDEX),
            TokenType::Minus => Some(OPERATOR_INDEX),
            TokenType::Mod => Some(OPERATOR_INDEX),
            TokenType::Power => Some(OPERATOR_INDEX),
            TokenType::Caret => Some(OPERATOR_INDEX),
            TokenType::Not => Some(OPERATOR_INDEX),
            TokenType::Assignment => Some(OPERATOR_INDEX),
            TokenType::If => Some(KEYWORD_INDEX),
            TokenType::Then => Some(KEYWORD_INDEX),
            TokenType::Elsif => Some(KEYWORD_INDEX),
            TokenType::Else => Some(KEYWORD_INDEX),
            TokenType::EndIf => Some(KEYWORD_INDEX),
            TokenType::Case => Some(KEYWORD_INDEX),
            TokenType::Of => Some(KEYWORD_INDEX),
            TokenType::EndCase => Some(KEYWORD_INDEX),
            TokenType::For => Some(KEYWORD_INDEX),
            TokenType::Do => Some(KEYWORD_INDEX),
            TokenType::EndFor => Some(KEYWORD_INDEX),
            TokenType::While => Some(KEYWORD_INDEX),
            TokenType::EndWhile => Some(KEYWORD_INDEX),
            TokenType::Repeat => Some(KEYWORD_INDEX),
            TokenType::Until => Some(KEYWORD_INDEX),
            TokenType::EndRepeat => Some(KEYWORD_INDEX),
            TokenType::Exit => Some(KEYWORD_INDEX),
            TokenType::Action => Some(KEYWORD_INDEX),
            TokenType::EndAction => Some(KEYWORD_INDEX),
            TokenType::En => Some(KEYWORD_INDEX),
            TokenType::Eno => Some(KEYWORD_INDEX),
            TokenType::False => Some(KEYWORD_INDEX),
            TokenType::FEdge => Some(KEYWORD_INDEX),
            TokenType::To => Some(KEYWORD_INDEX),
            TokenType::By => Some(KEYWORD_INDEX),
            TokenType::InitialStep => Some(KEYWORD_INDEX),
            TokenType::EndStep => Some(KEYWORD_INDEX),
            TokenType::REdge => Some(KEYWORD_INDEX),
            TokenType::ReadOnly => Some(KEYWORD_INDEX),
            TokenType::ReadWrite => Some(KEYWORD_INDEX),
            TokenType::NonRetain => Some(MODIFIER_INDEX),
            TokenType::Return => Some(KEYWORD_INDEX),
            TokenType::Step => Some(KEYWORD_INDEX),
            TokenType::Transition => Some(KEYWORD_INDEX),
            TokenType::From => Some(KEYWORD_INDEX),
            TokenType::EndTransition => Some(KEYWORD_INDEX),
            TokenType::True => Some(KEYWORD_INDEX),
            TokenType::VarTemp => Some(KEYWORD_INDEX),
            TokenType::VarAccess => Some(KEYWORD_INDEX),
            TokenType::VarConfig => Some(KEYWORD_INDEX),
            TokenType::Bool => Some(KEYWORD_INDEX),
            TokenType::Sint => Some(KEYWORD_INDEX),
            TokenType::Int => Some(KEYWORD_INDEX),
            TokenType::Dint => Some(KEYWORD_INDEX),
            TokenType::Lint => Some(KEYWORD_INDEX),
            TokenType::Usint => Some(KEYWORD_INDEX),
            TokenType::Uint => Some(KEYWORD_INDEX),
            TokenType::Udint => Some(KEYWORD_INDEX),
            TokenType::Ulint => Some(KEYWORD_INDEX),
            TokenType::Real => Some(KEYWORD_INDEX),
            TokenType::Time => Some(KEYWORD_INDEX),
            TokenType::Ltime => Some(KEYWORD_INDEX),
            TokenType::RefTo => Some(KEYWORD_INDEX),
            TokenType::Ref => Some(KEYWORD_INDEX),
            TokenType::Null => Some(KEYWORD_INDEX),
            TokenType::Reference => Some(KEYWORD_INDEX),
            TokenType::Pointer => Some(KEYWORD_INDEX),
            TokenType::Date => Some(KEYWORD_INDEX),
            TokenType::TimeOfDay => Some(KEYWORD_INDEX),
            TokenType::DateAndTime => Some(KEYWORD_INDEX),
            TokenType::Byte => Some(KEYWORD_INDEX),
            TokenType::Word => Some(KEYWORD_INDEX),
            TokenType::Dword => Some(KEYWORD_INDEX),
            TokenType::Lword => Some(KEYWORD_INDEX),
            TokenType::Range => None,
            TokenType::SingleByteString => Some(STRING_INDEX),
            TokenType::DoubleByteString => Some(STRING_INDEX),
            TokenType::Lreal => Some(KEYWORD_INDEX),
            TokenType::RightArrow => Some(OPERATOR_INDEX),
            // Generic type names
            TokenType::Any => Some(KEYWORD_INDEX),
            TokenType::AnyDerived => Some(KEYWORD_INDEX),
            TokenType::AnyElementary => Some(KEYWORD_INDEX),
            TokenType::AnyMagnitude => Some(KEYWORD_INDEX),
            TokenType::AnyNum => Some(KEYWORD_INDEX),
            TokenType::AnyReal => Some(KEYWORD_INDEX),
            TokenType::AnyInt => Some(KEYWORD_INDEX),
            TokenType::AnyBit => Some(KEYWORD_INDEX),
            TokenType::AnyString => Some(KEYWORD_INDEX),
            TokenType::AnyDate => Some(KEYWORD_INDEX),
            TokenType::Ldate => Some(KEYWORD_INDEX),
            TokenType::Ltod => Some(KEYWORD_INDEX),
            TokenType::Ldt => Some(KEYWORD_INDEX),
        };

        token_type.map(|token_type| SemanticToken {
            delta_line: val.0.line as u32,
            delta_start: val.0.col as u32,
            length: val.0.text.len() as u32,
            token_type,
            token_modifiers_bitset: 0,
        })
    }
}

#[cfg(test)]
mod test {
    use ironplc_dsl::core::FileId;
    use ironplc_parser::options::CompilerOptions;
    use ironplc_parser::token::TokenType;
    use ironplc_parser::tokenize_program;
    use lsp_types::SemanticToken;

    use super::{to_semantic_tokens, TOKEN_TYPE_LEGEND};
    use crate::test_helpers::resolve_lsp_tokens;

    // The legend entries by the name the editor sees them under.
    const COMMENT: &str = "comment";
    const KEYWORD: &str = "keyword";
    const MODIFIER: &str = "modifier";
    const OPERATOR: &str = "operator";
    const STRING: &str = "string";
    const VARIABLE: &str = "variable";

    /// Source that lexes to every `TokenType` except the braces, which only
    /// survive as tokens when pragma collapsing is off (see the test below).
    /// It does not need to parse, only to lex: `END_TASK` is not IEC 61131-3
    /// but the lexer knows it, and the OOP, reference, pointer, long time
    /// type, short-circuit, persistent, partial access, pragma and C-style
    /// comment forms need their dialect gates on so nothing is demoted to an
    /// identifier before it reaches the mapping.
    const SOURCE: &str = r#"(* Every token type the lexer produces, so the tag of each one is checked. *)
// C-style comments are tokens too
/* and so are block ones */
{attribute 'qualified_only'}
TYPE
  Point : STRUCT x : INT := 0; s : STRING[3] := 'abc'; w : WSTRING[3] := "abc"; END_STRUCT;
  Level : (Low, High) := Low;
  Small : INT (-1..1);
  Words : ARRAY [0..3] OF WORD;
END_TYPE

INTERFACE ICounter
  METHOD Reset END_METHOD
END_INTERFACE

FUNCTION_BLOCK ABSTRACT Counter IMPLEMENTS ICounter
  VAR_INPUT Up : BOOL R_EDGE; Down : BOOL F_EDGE; END_VAR
  VAR_OUTPUT Count : DINT; END_VAR
  VAR_IN_OUT Shared : LINT; END_VAR
  VAR_TEMP Scratch : SINT; END_VAR
  VAR RETAIN Kept : USINT; END_VAR
  VAR NON_RETAIN Lost : UINT; END_VAR
  VAR CONSTANT Limit : UDINT := UDINT#16#FF; END_VAR
  VAR PERSISTENT Saved : ULINT := 8#17; END_VAR
  VAR
    Bits : BYTE := 2#1010; Wide : DWORD; Wider : LWORD;
    Ratio : REAL := 1.5; Precise : LREAL := 2.5E3;
    Elapsed : TIME; Long : LTIME; Day : DATE; LDay : LDATE;
    Tick : TIME_OF_DAY; Tock : TOD; LTick : LTIME_OF_DAY; LTock : LTOD;
    Stamp : DATE_AND_TIME; Stamp2 : DT; LStamp : LDATE_AND_TIME; LStamp2 : LDT;
    Ptr : REF_TO INT := NULL; Alias : REFERENCE TO INT; Raw : POINTER TO INT;
    Io AT %IX0.0 : BOOL; Partial AT %Q* : BOOL;
  END_VAR
  METHOD Reset
    THIS^.Count := 0;
    SUPER^.Reset();
  END_METHOD
  Count := 1 + 2 - 3 * 4 / 5 MOD 6 ** 7;
  Count := REF(Count)^;
  Up := Up AND Down OR Up XOR Down & NOT Up;
  Up := Up AND_THEN Down OR_ELSE Up;
  Up := 1 = 2 OR 3 <> 4 OR 5 < 6 OR 7 > 8 OR 9 <= 10 OR 11 >= 12;
  Wider := Wide.%X0 + Wide.%B0 + Wide.%W0 + Wide.%D0 + Wide.%L0;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Counter
END_FUNCTION_BLOCK

FUNCTION Clamp : ANY_NUM
  VAR_INPUT
    Value : ANY; Derived : ANY_DERIVED; Elementary : ANY_ELEMENTARY;
    Magnitude : ANY_MAGNITUDE; RealValue : ANY_REAL; IntValue : ANY_INT;
    BitValue : ANY_BIT; StringValue : ANY_STRING; DateValue : ANY_DATE;
  END_VAR
  VAR_EXTERNAL Shared : Point; END_VAR
  IF Value > 1 THEN RETURN; ELSIF Value < 0 THEN Clamp := 0; ELSE Clamp := Value; END_IF;
  CASE IntValue OF 1, 2..3: Clamp := 1; ELSE Clamp := 0; END_CASE;
  FOR IntValue := 0 TO 10 BY 2 DO EXIT; END_FOR;
  WHILE Value > 0 DO Value := Value - 1; END_WHILE;
  REPEAT Value := Value + 1; UNTIL Value > 0 END_REPEAT;
  Clamp := MAX(EN := TRUE, ENO => BitValue, IN1 := 1, IN2 := 2);
  Clamp := SEL(G := FALSE, IN0 := 0, IN1 := 1);
END_FUNCTION

PROGRAM Main
  VAR Counter1 : Counter; Running : BOOL; END_VAR
  INITIAL_STEP Start: Startup(N); END_STEP
  TRANSITION FROM Start TO Run := Running; END_TRANSITION
  STEP Run: END_STEP
  ACTION Startup: Running := TRUE; END_ACTION
END_PROGRAM

CONFIGURATION Config
  VAR_GLOBAL Global : INT; END_VAR
  VAR_ACCESS
    Remote : Plc.Main1.Running : BOOL READ_ONLY;
    Writable : Plc.Main1.Running : BOOL READ_WRITE;
  END_VAR
  RESOURCE Plc ON PLC
    TASK Fast (INTERVAL := Period, PRIORITY := 1); END_TASK
    PROGRAM Main1 WITH Fast : Main;
  END_RESOURCE
  VAR_CONFIG Plc.Main1.Counter1.Kept : USINT := 1; END_VAR
END_CONFIGURATION"#;

    /// The `(lexeme, legend name)` of every semantic token [`SOURCE`]
    /// produces, one line here per line of source. Whatever is missing
    /// (punctuation, numbers, whitespace) is dropped by the mapping.
    #[rustfmt::skip]
    const EXPECTED: &[(&str, &str)] = &[
    ("(* Every token type the lexer produces, so the tag of each one is checked. *)", COMMENT),
    ("// C-style comments are tokens too", COMMENT),
    ("/* and so are block ones */", COMMENT),
    ("{attribute 'qualified_only'}", KEYWORD),
    ("TYPE", KEYWORD),
    ("Point", VARIABLE), ("STRUCT", KEYWORD), ("x", VARIABLE), ("INT", KEYWORD),
    (":=", OPERATOR), ("s", VARIABLE), ("STRING", KEYWORD), (":=", OPERATOR), ("'abc'", STRING),
    ("w", VARIABLE), ("WSTRING", KEYWORD), (":=", OPERATOR), ("\"abc\"", STRING),
    ("END_STRUCT", KEYWORD),
    ("Level", VARIABLE), ("Low", VARIABLE), ("High", VARIABLE), (":=", OPERATOR),
    ("Low", VARIABLE),
    ("Small", VARIABLE), ("INT", KEYWORD), ("-", OPERATOR),
    ("Words", VARIABLE), ("ARRAY", KEYWORD), ("OF", KEYWORD), ("WORD", KEYWORD),
    ("END_TYPE", KEYWORD),
    ("INTERFACE", KEYWORD), ("ICounter", VARIABLE),
    ("METHOD", KEYWORD), ("Reset", VARIABLE), ("END_METHOD", KEYWORD),
    ("END_INTERFACE", KEYWORD),
    ("FUNCTION_BLOCK", KEYWORD), ("ABSTRACT", KEYWORD), ("Counter", VARIABLE),
    ("IMPLEMENTS", KEYWORD), ("ICounter", VARIABLE),
    ("VAR_INPUT", KEYWORD), ("Up", VARIABLE), ("BOOL", KEYWORD), ("R_EDGE", KEYWORD),
    ("Down", VARIABLE), ("BOOL", KEYWORD), ("F_EDGE", KEYWORD), ("END_VAR", KEYWORD),
    ("VAR_OUTPUT", KEYWORD), ("Count", VARIABLE), ("DINT", KEYWORD), ("END_VAR", KEYWORD),
    ("VAR_IN_OUT", KEYWORD), ("Shared", VARIABLE), ("LINT", KEYWORD), ("END_VAR", KEYWORD),
    ("VAR_TEMP", KEYWORD), ("Scratch", VARIABLE), ("SINT", KEYWORD), ("END_VAR", KEYWORD),
    ("VAR", KEYWORD), ("RETAIN", MODIFIER), ("Kept", VARIABLE), ("USINT", KEYWORD),
    ("END_VAR", KEYWORD),
    ("VAR", KEYWORD), ("NON_RETAIN", MODIFIER), ("Lost", VARIABLE), ("UINT", KEYWORD),
    ("END_VAR", KEYWORD),
    ("VAR", KEYWORD), ("CONSTANT", MODIFIER), ("Limit", VARIABLE), ("UDINT", KEYWORD),
    (":=", OPERATOR), ("UDINT", KEYWORD), ("END_VAR", KEYWORD),
    ("VAR", KEYWORD), ("PERSISTENT", MODIFIER), ("Saved", VARIABLE), ("ULINT", KEYWORD),
    (":=", OPERATOR), ("END_VAR", KEYWORD),
    ("VAR", KEYWORD),
    ("Bits", VARIABLE), ("BYTE", KEYWORD), (":=", OPERATOR), ("Wide", VARIABLE),
    ("DWORD", KEYWORD), ("Wider", VARIABLE), ("LWORD", KEYWORD),
    ("Ratio", VARIABLE), ("REAL", KEYWORD), (":=", OPERATOR), ("Precise", VARIABLE),
    ("LREAL", KEYWORD), (":=", OPERATOR),
    ("Elapsed", VARIABLE), ("TIME", KEYWORD), ("Long", VARIABLE), ("LTIME", KEYWORD),
    ("Day", VARIABLE), ("DATE", KEYWORD), ("LDay", VARIABLE), ("LDATE", KEYWORD),
    ("Tick", VARIABLE), ("TIME_OF_DAY", KEYWORD), ("Tock", VARIABLE), ("TOD", KEYWORD),
    ("LTick", VARIABLE), ("LTIME_OF_DAY", KEYWORD), ("LTock", VARIABLE), ("LTOD", KEYWORD),
    ("Stamp", VARIABLE), ("DATE_AND_TIME", KEYWORD), ("Stamp2", VARIABLE), ("DT", KEYWORD),
    ("LStamp", VARIABLE), ("LDATE_AND_TIME", KEYWORD), ("LStamp2", VARIABLE), ("LDT", KEYWORD),
    ("Ptr", VARIABLE), ("REF_TO", KEYWORD), ("INT", KEYWORD), (":=", OPERATOR),
    ("NULL", KEYWORD), ("Alias", VARIABLE), ("REFERENCE", KEYWORD), ("TO", KEYWORD),
    ("INT", KEYWORD), ("Raw", VARIABLE), ("POINTER", KEYWORD), ("TO", KEYWORD),
    ("INT", KEYWORD),
    ("Io", VARIABLE), ("AT", KEYWORD), ("%IX0.0", OPERATOR), ("BOOL", KEYWORD),
    ("Partial", VARIABLE), ("AT", KEYWORD), ("%Q*", OPERATOR), ("BOOL", KEYWORD),
    ("END_VAR", KEYWORD),
    ("METHOD", KEYWORD), ("Reset", VARIABLE),
    ("THIS", KEYWORD), ("^", OPERATOR), ("Count", VARIABLE), (":=", OPERATOR),
    ("SUPER", KEYWORD), ("^", OPERATOR), ("Reset", VARIABLE),
    ("END_METHOD", KEYWORD),
    ("Count", VARIABLE), (":=", OPERATOR), ("+", OPERATOR), ("-", OPERATOR), ("*", OPERATOR),
    ("/", OPERATOR), ("MOD", OPERATOR), ("**", OPERATOR),
    ("Count", VARIABLE), (":=", OPERATOR), ("REF", KEYWORD), ("Count", VARIABLE),
    ("^", OPERATOR),
    ("Up", VARIABLE), (":=", OPERATOR), ("Up", VARIABLE), ("AND", OPERATOR), ("Down", VARIABLE),
    ("OR", OPERATOR), ("Up", VARIABLE), ("XOR", OPERATOR), ("Down", VARIABLE), ("&", OPERATOR),
    ("NOT", OPERATOR), ("Up", VARIABLE),
    ("Up", VARIABLE), (":=", OPERATOR), ("Up", VARIABLE), ("AND_THEN", OPERATOR),
    ("Down", VARIABLE), ("OR_ELSE", OPERATOR), ("Up", VARIABLE),
    ("Up", VARIABLE), (":=", OPERATOR), ("=", OPERATOR), ("OR", OPERATOR), ("<>", OPERATOR),
    ("OR", OPERATOR), ("<", OPERATOR), ("OR", OPERATOR), (">", OPERATOR), ("OR", OPERATOR),
    ("<=", OPERATOR), ("OR", OPERATOR), (">=", OPERATOR),
    ("Wider", VARIABLE), (":=", OPERATOR), ("Wide", VARIABLE), ("%X0", OPERATOR),
    ("+", OPERATOR), ("Wide", VARIABLE), ("%B0", OPERATOR), ("+", OPERATOR), ("Wide", VARIABLE),
    ("%W0", OPERATOR), ("+", OPERATOR), ("Wide", VARIABLE), ("%D0", OPERATOR), ("+", OPERATOR),
    ("Wide", VARIABLE), ("%L0", OPERATOR),
    ("END_FUNCTION_BLOCK", KEYWORD),
    ("FUNCTION_BLOCK", KEYWORD), ("Derived", VARIABLE), ("EXTENDS", KEYWORD),
    ("Counter", VARIABLE),
    ("END_FUNCTION_BLOCK", KEYWORD),
    ("FUNCTION", KEYWORD), ("Clamp", VARIABLE), ("ANY_NUM", KEYWORD),
    ("VAR_INPUT", KEYWORD),
    ("Value", VARIABLE), ("ANY", KEYWORD), ("Derived", VARIABLE), ("ANY_DERIVED", KEYWORD),
    ("Elementary", VARIABLE), ("ANY_ELEMENTARY", KEYWORD),
    ("Magnitude", VARIABLE), ("ANY_MAGNITUDE", KEYWORD), ("RealValue", VARIABLE),
    ("ANY_REAL", KEYWORD), ("IntValue", VARIABLE), ("ANY_INT", KEYWORD),
    ("BitValue", VARIABLE), ("ANY_BIT", KEYWORD), ("StringValue", VARIABLE),
    ("ANY_STRING", KEYWORD), ("DateValue", VARIABLE), ("ANY_DATE", KEYWORD),
    ("END_VAR", KEYWORD),
    ("VAR_EXTERNAL", KEYWORD), ("Shared", VARIABLE), ("Point", VARIABLE), ("END_VAR", KEYWORD),
    ("IF", KEYWORD), ("Value", VARIABLE), (">", OPERATOR), ("THEN", KEYWORD),
    ("RETURN", KEYWORD), ("ELSIF", KEYWORD), ("Value", VARIABLE), ("<", OPERATOR),
    ("THEN", KEYWORD), ("Clamp", VARIABLE), (":=", OPERATOR), ("ELSE", KEYWORD),
    ("Clamp", VARIABLE), (":=", OPERATOR), ("Value", VARIABLE), ("END_IF", KEYWORD),
    ("CASE", KEYWORD), ("IntValue", VARIABLE), ("OF", KEYWORD), ("Clamp", VARIABLE),
    (":=", OPERATOR), ("ELSE", KEYWORD), ("Clamp", VARIABLE), (":=", OPERATOR),
    ("END_CASE", KEYWORD),
    ("FOR", KEYWORD), ("IntValue", VARIABLE), (":=", OPERATOR), ("TO", KEYWORD),
    ("BY", KEYWORD), ("DO", KEYWORD), ("EXIT", KEYWORD), ("END_FOR", KEYWORD),
    ("WHILE", KEYWORD), ("Value", VARIABLE), (">", OPERATOR), ("DO", KEYWORD),
    ("Value", VARIABLE), (":=", OPERATOR), ("Value", VARIABLE), ("-", OPERATOR),
    ("END_WHILE", KEYWORD),
    ("REPEAT", KEYWORD), ("Value", VARIABLE), (":=", OPERATOR), ("Value", VARIABLE),
    ("+", OPERATOR), ("UNTIL", KEYWORD), ("Value", VARIABLE), (">", OPERATOR),
    ("END_REPEAT", KEYWORD),
    ("Clamp", VARIABLE), (":=", OPERATOR), ("MAX", VARIABLE), ("EN", KEYWORD), (":=", OPERATOR),
    ("TRUE", KEYWORD), ("ENO", KEYWORD), ("=>", OPERATOR), ("BitValue", VARIABLE),
    ("IN1", VARIABLE), (":=", OPERATOR), ("IN2", VARIABLE), (":=", OPERATOR),
    ("Clamp", VARIABLE), (":=", OPERATOR), ("SEL", VARIABLE), ("G", VARIABLE), (":=", OPERATOR),
    ("FALSE", KEYWORD), ("IN0", VARIABLE), (":=", OPERATOR), ("IN1", VARIABLE),
    (":=", OPERATOR),
    ("END_FUNCTION", KEYWORD),
    ("PROGRAM", KEYWORD), ("Main", VARIABLE),
    ("VAR", KEYWORD), ("Counter1", VARIABLE), ("Counter", VARIABLE), ("Running", VARIABLE),
    ("BOOL", KEYWORD), ("END_VAR", KEYWORD),
    ("INITIAL_STEP", KEYWORD), ("Start", VARIABLE), ("Startup", VARIABLE), ("N", VARIABLE),
    ("END_STEP", KEYWORD),
    ("TRANSITION", KEYWORD), ("FROM", KEYWORD), ("Start", VARIABLE), ("TO", KEYWORD),
    ("Run", VARIABLE), (":=", OPERATOR), ("Running", VARIABLE), ("END_TRANSITION", KEYWORD),
    ("STEP", KEYWORD), ("Run", VARIABLE), ("END_STEP", KEYWORD),
    ("ACTION", KEYWORD), ("Startup", VARIABLE), ("Running", VARIABLE), (":=", OPERATOR),
    ("TRUE", KEYWORD), ("END_ACTION", KEYWORD),
    ("END_PROGRAM", KEYWORD),
    ("CONFIGURATION", KEYWORD), ("Config", VARIABLE),
    ("VAR_GLOBAL", KEYWORD), ("Global", VARIABLE), ("INT", KEYWORD), ("END_VAR", KEYWORD),
    ("VAR_ACCESS", KEYWORD),
    ("Remote", VARIABLE), ("Plc", VARIABLE), ("Main1", VARIABLE), ("Running", VARIABLE),
    ("BOOL", KEYWORD), ("READ_ONLY", KEYWORD),
    ("Writable", VARIABLE), ("Plc", VARIABLE), ("Main1", VARIABLE), ("Running", VARIABLE),
    ("BOOL", KEYWORD), ("READ_WRITE", KEYWORD),
    ("END_VAR", KEYWORD),
    ("RESOURCE", KEYWORD), ("Plc", VARIABLE), ("ON", KEYWORD), ("PLC", VARIABLE),
    ("TASK", KEYWORD), ("Fast", VARIABLE), ("INTERVAL", VARIABLE), (":=", OPERATOR),
    ("Period", VARIABLE), ("PRIORITY", VARIABLE), (":=", OPERATOR), ("END_TASK", KEYWORD),
    ("PROGRAM", KEYWORD), ("Main1", VARIABLE), ("WITH", KEYWORD), ("Fast", VARIABLE),
    ("Main", VARIABLE),
    ("END_RESOURCE", KEYWORD),
    ("VAR_CONFIG", KEYWORD), ("Plc", VARIABLE), ("Main1", VARIABLE), ("Counter1", VARIABLE),
    ("Kept", VARIABLE), ("USINT", KEYWORD), (":=", OPERATOR), ("END_VAR", KEYWORD),
    ("END_CONFIGURATION", KEYWORD),
    ];

    /// The token types the mapping drops. Each must lex from [`SOURCE`] so
    /// that its absence from [`EXPECTED`] shows it was dropped, not that the
    /// source never contained it.
    const DROPPED: &[TokenType] = &[
        TokenType::Newline,
        TokenType::Whitespace,
        TokenType::LeftParen,
        TokenType::RightParen,
        TokenType::LeftBracket,
        TokenType::RightBracket,
        TokenType::Comma,
        TokenType::Semicolon,
        TokenType::Colon,
        TokenType::Period,
        TokenType::Range,
        TokenType::Hash,
        TokenType::HexDigits,
        TokenType::OctDigits,
        TokenType::BinDigits,
        TokenType::FloatingPoint,
        TokenType::FixedPoint,
        TokenType::Digits,
    ];

    /// Every keyword gate on, so the lexer keeps each dialect keyword as its
    /// own token type instead of demoting it to an identifier.
    fn every_keyword_enabled() -> CompilerOptions {
        CompilerOptions {
            allow_c_style_comments: true,
            allow_fb_inheritance: true,
            allow_long_time_types: true,
            allow_partial_access_syntax: true,
            allow_persistent_var: true,
            allow_pointer_to: true,
            allow_pragmas: true,
            allow_ref_to: true,
            allow_reference_to: true,
            allow_short_circuit_operators: true,
            ..CompilerOptions::default()
        }
    }

    /// Decode the delta-encoded stream back to `(lexeme, legend name)` pairs.
    fn tags<'a>(source: &'a str, tokens: &[SemanticToken]) -> Vec<(&'a str, &'static str)> {
        resolve_lsp_tokens(source, tokens)
            .into_iter()
            .map(|(_, _, lexeme, ty)| (lexeme, TOKEN_TYPE_LEGEND[ty as usize].as_str()))
            .collect()
    }

    #[test]
    fn to_semantic_tokens_when_every_token_type_then_each_lexeme_carries_its_tag() {
        let (tokens, diagnostics) =
            tokenize_program(SOURCE, &FileId::default(), &every_keyword_enabled(), 0, 0);
        assert!(
            diagnostics.is_empty(),
            "source must lex cleanly: {diagnostics:?}"
        );

        let lexed: Vec<TokenType> = tokens.iter().map(|t| t.token_type.clone()).collect();
        let missing: Vec<&TokenType> = DROPPED.iter().filter(|t| !lexed.contains(t)).collect();
        assert!(missing.is_empty(), "the source never lexes {missing:?}");

        let actual = tags(SOURCE, &to_semantic_tokens(tokens));

        let first_diff = actual.iter().zip(EXPECTED).position(|(a, e)| a != e);
        assert!(
            first_diff.is_none(),
            "token {} is {:?}, expected {:?}",
            first_diff.unwrap(),
            actual[first_diff.unwrap()],
            EXPECTED[first_diff.unwrap()]
        );
        assert_eq!(
            actual.len(),
            EXPECTED.len(),
            "unexpected trailing tokens: {:?}",
            &actual[actual.len().min(EXPECTED.len())..]
        );
    }

    #[test]
    fn to_semantic_tokens_when_pragmas_disabled_then_braces_dropped_and_contents_tagged() {
        let source = "{attribute 'qualified_only'}";
        let (tokens, diagnostics) = tokenize_program(
            source,
            &FileId::default(),
            &CompilerOptions::default(),
            0,
            0,
        );
        assert!(
            diagnostics.is_empty(),
            "source must lex cleanly: {diagnostics:?}"
        );

        let lexed: Vec<TokenType> = tokens.iter().map(|t| t.token_type.clone()).collect();
        assert!(lexed.contains(&TokenType::LeftBrace));
        assert!(lexed.contains(&TokenType::RightBrace));

        let actual = tags(source, &to_semantic_tokens(tokens));
        assert_eq!(
            actual,
            [("attribute", VARIABLE), ("'qualified_only'", STRING)]
        );
    }
}
