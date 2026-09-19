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
pub const TOKEN_TYPE_LEGEND: [SemanticTokenType; 6] = [
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
            TokenType::String => Some(STRING_INDEX),
            TokenType::Identifier => Some(VARIABLE_INDEX),
            TokenType::HexDigits => None,
            TokenType::OctDigits => None,
            TokenType::BinDigits => None,
            TokenType::FloatingPoint => None,
            TokenType::FixedPoint => None,
            TokenType::Digits => None,
            TokenType::Type => Some(KEYWORD_INDEX),
            TokenType::EndType => Some(KEYWORD_INDEX),
            TokenType::Array => None,
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
            TokenType::NonRetain => Some(KEYWORD_INDEX),
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
            TokenType::Range => Some(KEYWORD_INDEX),
            TokenType::SingleByteString => None,
            TokenType::DoubleByteString => None,
            TokenType::Lreal => Some(KEYWORD_INDEX),
            TokenType::RightArrow => Some(KEYWORD_INDEX),
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
    use ironplc_dsl::core::SourceSpan;
    use ironplc_parser::token::{Token, TokenType};
    use lsp_types::SemanticToken;

    use super::LspTokenType;

    #[test]
    fn from_lsp_token_type_for_semantic_token() {
        // This test exists mostly for the purpose of code coverage.
        let tok_types = vec![
            TokenType::Newline,
            TokenType::Whitespace,
            TokenType::Comment,
            TokenType::LeftParen,
            TokenType::RightParen,
            TokenType::LeftBrace,
            TokenType::RightBrace,
            TokenType::Comma,
            TokenType::Semicolon,
            TokenType::Colon,
            TokenType::Period,
            TokenType::Range,
            TokenType::Hash,
            TokenType::SingleByteString,
            TokenType::DoubleByteString,
            TokenType::Identifier,
            TokenType::Digits,
            TokenType::Action,
            TokenType::EndAction,
            TokenType::Array,
            TokenType::Of,
            TokenType::At,
            TokenType::Case,
            TokenType::Else,
            TokenType::EndCase,
            TokenType::For,
            TokenType::Constant,
            TokenType::Configuration,
            TokenType::EndConfiguration,
            TokenType::En,
            TokenType::Eno,
            TokenType::Exit,
            TokenType::False,
            TokenType::FEdge,
            TokenType::To,
            TokenType::By,
            TokenType::Do,
            TokenType::EndFor,
            TokenType::Function,
            TokenType::EndFunction,
            TokenType::FunctionBlock,
            TokenType::EndFunctionBlock,
            TokenType::If,
            TokenType::Then,
            TokenType::Elsif,
            TokenType::EndIf,
            TokenType::InitialStep,
            TokenType::EndStep,
            TokenType::Program,
            TokenType::With,
            TokenType::EndProgram,
            TokenType::REdge,
            TokenType::ReadOnly,
            TokenType::ReadWrite,
            TokenType::Repeat,
            TokenType::Until,
            TokenType::EndRepeat,
            TokenType::Resource,
            TokenType::On,
            TokenType::EndResource,
            TokenType::Retain,
            TokenType::NonRetain,
            TokenType::Persistent,
            TokenType::Return,
            TokenType::Step,
            TokenType::Struct,
            TokenType::EndStruct,
            TokenType::Task,
            TokenType::EndTask,
            TokenType::Transition,
            TokenType::From,
            TokenType::EndTransition,
            TokenType::True,
            TokenType::Type,
            TokenType::EndType,
            TokenType::Var,
            TokenType::EndVar,
            TokenType::VarInput,
            TokenType::VarOutput,
            TokenType::VarInOut,
            TokenType::VarTemp,
            TokenType::VarExternal,
            TokenType::VarAccess,
            TokenType::VarConfig,
            TokenType::VarGlobal,
            TokenType::While,
            TokenType::EndWhile,
            TokenType::Bool,
            TokenType::Sint,
            TokenType::Int,
            TokenType::Dint,
            TokenType::Lint,
            TokenType::Usint,
            TokenType::Uint,
            TokenType::Udint,
            TokenType::Ulint,
            TokenType::Real,
            TokenType::Lreal,
            TokenType::Time,
            TokenType::Date,
            TokenType::TimeOfDay,
            TokenType::DateAndTime,
            TokenType::String,
            TokenType::Byte,
            TokenType::Word,
            TokenType::Dword,
            TokenType::Lword,
            TokenType::WString,
            TokenType::DirectAddressIncomplete,
            TokenType::DirectAddress,
            TokenType::Or,
            TokenType::Xor,
            TokenType::And,
            TokenType::Equal,
            TokenType::NotEqual,
            TokenType::Less,
            TokenType::Greater,
            TokenType::LessEqual,
            TokenType::GreaterEqual,
            TokenType::Div,
            TokenType::Star,
            TokenType::Plus,
            TokenType::Minus,
            TokenType::Mod,
            TokenType::Power,
            TokenType::Caret,
            TokenType::Not,
            TokenType::Assignment,
            TokenType::RightArrow,
        ];

        for tok_type in tok_types {
            let token = Token {
                token_type: tok_type,
                text: "test".to_string(),
                span: SourceSpan::default(),
                line: 0,
                col: 0,
            };
            let lsp_token = LspTokenType(token);
            let _result: Option<SemanticToken> = lsp_token.into();
        }
    }
}
