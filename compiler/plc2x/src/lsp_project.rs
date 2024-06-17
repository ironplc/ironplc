//! Adapts data types between what is required by the compiler
//! and the language server protocol.
use ironplc_dsl::core::FileId;
use ironplc_parser::token::{Token, TokenType};
use log::error;
use lsp_types::{
    CodeDescription, Diagnostic, DiagnosticSeverity, NumberOrString, SemanticTokenType,
    WorkspaceFolder,
};
use lsp_types::{SemanticToken, Url};

use crate::compilation_set::{self, CompilationSet};
use crate::project::Project;

/// The LSP project provides a view onto a project that accepts
/// and returns LSP types.
pub struct LspProject {
    wrapped: Box<dyn Project + Send>,
}

impl LspProject {
    pub fn new(project: Box<dyn Project + Send>) -> Self {
        Self { wrapped: project }
    }

    pub(crate) fn initialize(&mut self, folder: &WorkspaceFolder) {
        let path = folder.uri.to_file_path();
        if let Ok(path) = path {
            self.wrapped.initialize(&path);
        }  else {
            error!("URL must be convertible to a file path {}", url);
        }
    }

    pub(crate) fn change_text_document(&mut self, url: &Url, content: String) {
        let path = url.to_file_path();
        if let Ok(path) = path {
            let file_id = FileId::from_path(&path);
            self.wrapped.change_text_document(&file_id, content);
        } else {
            error!("URL must be convertible to a file path {}", url);
        }
    }

    pub(crate) fn tokenize(&self, url: &Url) -> Result<Vec<SemanticToken>, Vec<Diagnostic>> {
        let path = url.to_file_path();
        if let Ok(path) = path {
            let file_id = FileId::from_path(&path);

            let result = self.wrapped.tokenize(&file_id);

            if !result.1.is_empty() {
                let compilation_set = self.wrapped.compilation_set();
                return Err(result
                    .1
                    .into_iter()
                    .map(|err| map_diagnostic(err, &compilation_set))
                    .collect());
            }

            return Ok(result
                .0
                .into_iter()
                .filter_map(|tok| LspTokenType(tok).into())
                .collect());
        } else {
            error!("URL must be convertible to a file path {}", url);
        }

        Err(vec![])
    }

    pub(crate) fn semantic(&self) -> Vec<Diagnostic> {
        let compilation_set = self.wrapped.compilation_set();
        let diagnostics: Vec<lsp_types::Diagnostic> = self.wrapped.semantic().map_or_else(
            |d| {
                d.into_iter()
                    .map(|d| map_diagnostic(d, &compilation_set))
                    .collect()
            },
            |()| Vec::new(),
        );
        diagnostics
    }
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
            TokenType::LeftBracket => None,
            TokenType::RightBracket => None,
            TokenType::Comma => None,
            TokenType::Semicolon => None,
            TokenType::Colon => None,
            TokenType::Period => None,
            TokenType::Hash => None,
            TokenType::String => Some(STRING_INDEX),
            TokenType::Identifier => Some(VARIABLE_INDEX),
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
            TokenType::At => Some(KEYWORD_INDEX),
            TokenType::DirectAddress => Some(OPERATOR_INDEX),
            TokenType::DirectAddressIncomplete => Some(OPERATOR_INDEX),
            TokenType::Function => Some(KEYWORD_INDEX),
            TokenType::EndFunction => Some(KEYWORD_INDEX),
            TokenType::FunctionBlock => Some(KEYWORD_INDEX),
            TokenType::EndFunctionBlock => Some(KEYWORD_INDEX),
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
        };

        // TODO fix this mapping
        token_type.map(|token_type| SemanticToken {
            delta_line: val.0.line as u32,
            delta_start: val.0.col as u32,
            length: val.0.text.len() as u32,
            token_type,
            token_modifiers_bitset: 0,
        })
    }
}

/// Convert diagnostic type into the LSP diagnostic type.
fn map_diagnostic(
    diagnostic: ironplc_dsl::diagnostic::Diagnostic,
    compilation_set: &CompilationSet,
) -> lsp_types::Diagnostic {
    let description = diagnostic.description();
    let range = map_label(&diagnostic.primary, compilation_set);

    let code_description = match Url::parse(
        format!(
            "https://www.ironplc.com/compiler/problems/{}.html",
            diagnostic.code
        )
        .as_str(),
    ) {
        Ok(url) => Some(CodeDescription { href: url }),
        Err(_) => None,
    };

    lsp_types::Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(diagnostic.code)),
        code_description,
        source: Some("ironplc".into()),
        message: format!("{}: {} ", description, diagnostic.primary.message),
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Convert the diagnostic label into the LSP range type.
fn map_label(
    label: &ironplc_dsl::diagnostic::Label,
    compilation_set: &CompilationSet,
) -> lsp_types::Range {
    let file_id = &label.file_id;
    let contents = compilation_set.find(file_id);

    if let Some(contents) = contents {
        match contents {
            compilation_set::CompilationSource::Library(_lib) => {}
            compilation_set::CompilationSource::Text((contents, _id)) => {
                let mut start_line = 0;
                let mut start_offset = 0;

                for char in contents[0..label.location.start].chars() {
                    if char == '\n' {
                        start_line += 1;
                        start_offset = 0;
                    } else {
                        start_offset += 1;
                    }
                }

                let mut end_line = start_line;
                let mut end_offset = start_offset;
                for char in contents[label.location.start..label.location.start].chars() {
                    if char == '\n' {
                        end_line += 1;
                        end_offset = 0;
                    } else {
                        end_offset += 1;
                    }
                }

                return lsp_types::Range::new(
                    lsp_types::Position::new(start_line, start_offset),
                    lsp_types::Position::new(end_line, end_offset),
                );
            }
            compilation_set::CompilationSource::TextRef((contents, _id)) => {
                let mut start_line = 0;
                let mut start_offset = 0;

                for char in contents[0..label.location.start].chars() {
                    if char == '\n' {
                        start_line += 1;
                        start_offset = 0;
                    } else {
                        start_offset += 1;
                    }
                }

                let mut end_line = start_line;
                let mut end_offset = start_offset;
                for char in contents[label.location.start..label.location.start].chars() {
                    if char == '\n' {
                        end_line += 1;
                        end_offset = 0;
                    } else {
                        end_offset += 1;
                    }
                }

                return lsp_types::Range::new(
                    lsp_types::Position::new(start_line, start_offset),
                    lsp_types::Position::new(end_line, end_offset),
                );
            }
        }
    }
    lsp_types::Range::new(
        lsp_types::Position::new(0, 0),
        lsp_types::Position::new(0, 0),
    )
}

#[cfg(test)]
mod test {
    use ironplc_dsl::core::SourceSpan;
    use ironplc_parser::token::{Token, TokenType};
    use lsp_types::{SemanticToken, Url};

    use crate::{project::FileBackedProject, test_helpers::read_resource};

    use super::{LspProject, LspTokenType};

    #[cfg(target_os = "macos")]
    static FAKE_PATH: &str = "file:///localhost/first_steps.st";
    #[cfg(target_os = "linux")]
    static FAKE_PATH: &str = "file:///localhost/first_steps.st";
    #[cfg(target_os = "windows")]
    static FAKE_PATH: &str = "file:///C|first_steps.st";

    fn new_empty_project() -> LspProject {
        LspProject::new(Box::new(FileBackedProject::new()))
    }

    #[test]
    fn tokenize_when_no_document_then_error() {
        let proj = new_empty_project();
        let url = Url::parse("http://example.com").unwrap();
        assert!(proj.tokenize(&url).is_err());
    }

    #[test]
    fn tokenize_when_has_document_then_not_empty_tokens() {
        let mut proj = new_empty_project();
        let url = Url::parse(FAKE_PATH).unwrap();
        proj.change_text_document(&url, "TYPE TEXT_EMPTY : STRING [1]; END_TYPE".to_owned());

        let result = proj.tokenize(&url);
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn tokenize_when_first_steps_then_has_tokens() {
        let mut proj = new_empty_project();
        let url = Url::parse(FAKE_PATH).unwrap();
        let content = read_resource("first_steps.st");
        proj.change_text_document(&url, content);

        let result = proj.tokenize(&url);

        assert!(result.is_ok());
    }

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
            TokenType::Not,
            TokenType::Assignment,
            TokenType::RightArrow,
        ];

        for tok_type in tok_types.into_iter() {
            let lsp_token_type = LspTokenType(Token {
                token_type: tok_type,
                span: SourceSpan::default(),
                line: 0,
                col: 0,
                text: "".to_owned(),
            });

            let _semantic_token: Option<SemanticToken> = lsp_token_type.into();
        }
    }
}
