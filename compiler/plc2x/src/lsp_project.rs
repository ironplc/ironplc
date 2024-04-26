//! Adapts data types between what is required by the compiler
//! and the language server protocol.
use ironplc_dsl::core::FileId;
use ironplc_parser::token::TokenType;
use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString, SemanticTokenType};
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
    pub(crate) fn change_text_document(&mut self, url: &Url, content: &str) {
        let file_id = FileId::from_string(url.as_str());
        self.wrapped.change_text_document(&file_id, content);
    }

    pub(crate) fn tokenize(&self, url: &Url) -> Result<Vec<SemanticToken>, Vec<Diagnostic>> {
        let file_id = FileId::from_string(url.as_str());
        let result = self.wrapped.tokenize(&file_id);

        if !result.1.is_empty() {
            let compilation_set = self.wrapped.compilation_set();
            return Err(result
                .1
                .into_iter()
                .map(|err| map_diagnostic(err, &compilation_set))
                .collect());
        }

        Ok(result
            .0
            .into_iter()
            .filter_map(|tok| LspTokenType(tok).into())
            .collect())
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

struct LspTokenType(TokenType);

impl From<LspTokenType> for Option<SemanticToken> {
    fn from(val: LspTokenType) -> Self {
        let token_info = match val.0 {
            TokenType::Newline(pos) => (pos, None),
            TokenType::Whitespace(pos) => (pos, None),
            TokenType::Comment(pos) => (pos, Some(COMMENT_INDEX)),
            TokenType::LeftParen(pos) => (pos, None),
            TokenType::RightParen(pos) => (pos, None),
            TokenType::LeftBrace(pos) => (pos, None),
            TokenType::RightBrace(pos) => (pos, None),
            TokenType::LeftBracket(pos) => (pos, None),
            TokenType::RightBracket(pos) => (pos, None),
            TokenType::Comma(pos) => (pos, None),
            TokenType::Semicolon(pos) => (pos, None),
            TokenType::Colon(pos) => (pos, None),
            TokenType::Period(pos) => (pos, None),
            TokenType::Hash(pos) => (pos, None),
            TokenType::StringLiteral(pos) => (pos, Some(STRING_INDEX)),
            TokenType::Identifier(pos) => (pos, Some(VARIABLE_INDEX)),
            TokenType::Integer(pos) => (pos, None),
            TokenType::Type(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::EndType(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Array(pos) => (pos, None),
            TokenType::Struct(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::EndStruct(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::String(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::WString(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Var(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::VarInput(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::VarOutput(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::VarInOut(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::VarExternal(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::VarGlobal(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::VarEnd(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Retain(pos) => (pos, Some(MODIFIER_INDEX)),
            TokenType::Constant(pos) => (pos, Some(MODIFIER_INDEX)),
            TokenType::At(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Location(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::Function(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::EndFunction(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::FunctionBlock(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::EndFunctionBlock(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Configuration(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::EndConfiguration(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Resource(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::On(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::EndResource(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Task(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::EndTask(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Program(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::With(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::EndProgram(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Or(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::Xor(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::And(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::Equal(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::NotEqual(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::Less(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::Greater(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::LessEqual(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::GreaterEqual(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::Div(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::Star(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::Plus(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::Minus(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::Mod(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::Power(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::Not(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::Assignment(pos) => (pos, Some(OPERATOR_INDEX)),
            TokenType::If(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Then(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Elsif(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Else(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::IfEnd(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Case(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Of(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::CaseEnd(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::For(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Do(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::EndFor(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::While(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::EndWhile(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Repeat(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Until(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::RepeatEnd(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Exit(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Action(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::EndAction(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::En(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Eno(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::False(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::FEdge(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::To(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::By(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::InitialStep(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::EndStep(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::REdge(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::ReadOnly(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::ReadWrite(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::NonRetain(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Return(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Step(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Transition(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::From(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::EndTransition(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::True(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::VarTemp(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::VarAccess(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::VarConfig(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Bool(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Sint(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Int(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Dint(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Lint(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Usint(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Uint(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Udint(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Ulint(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Real(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Time(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Date(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::TimeOfDay(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::DateAndTime(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Byte(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Word(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Dword(pos) => (pos, Some(KEYWORD_INDEX)),
            TokenType::Lword(pos) => (pos, Some(KEYWORD_INDEX)),
        };

        let pos = token_info.0;
        let token_type = token_info.1;

        token_type.map(|token_type| SemanticToken {
            delta_line: pos.line as u32,
            delta_start: pos.column as u32,
            // TODO
            length: 1,
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
    lsp_types::Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(diagnostic.code)),
        code_description: None,
        source: Some("ironplc".into()),
        message: format!("{}: {}", description, diagnostic.primary.message),
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
    match &label.location {
        ironplc_dsl::diagnostic::Location::QualifiedPosition(qualified) => lsp_types::Range::new(
            lsp_types::Position::new((qualified.line - 1) as u32, (qualified.column - 1) as u32),
            lsp_types::Position::new((qualified.line - 1) as u32, (qualified.column - 1) as u32),
        ),
        ironplc_dsl::diagnostic::Location::OffsetRange(offset) => {
            if let Some(contents) = contents {
                match contents {
                    compilation_set::CompilationSource::Library(_lib) => {}
                    compilation_set::CompilationSource::Text((contents, _id)) => {
                        let mut start_line = 0;
                        let mut start_offset = 0;

                        for char in contents[0..offset.start].chars() {
                            if char == '\n' {
                                start_line += 1;
                                start_offset = 0;
                            } else {
                                start_offset += 1;
                            }
                        }

                        let mut end_line = start_line;
                        let mut end_offset = start_offset;
                        for char in contents[offset.start..offset.start].chars() {
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

                        for char in contents[0..offset.start].chars() {
                            if char == '\n' {
                                start_line += 1;
                                start_offset = 0;
                            } else {
                                start_offset += 1;
                            }
                        }

                        let mut end_line = start_line;
                        let mut end_offset = start_offset;
                        for char in contents[offset.start..offset.start].chars() {
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
    }
}

#[cfg(test)]
mod test {
    use lsp_types::Url;

    use crate::{project::FileBackedProject, test_helpers::read_resource};

    use super::LspProject;

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
        let url = Url::parse("http://example.com").unwrap();
        proj.change_text_document(&url, "TYPE TEXT_EMPTY : STRING [1]; END_TYPE");

        let result = proj.tokenize(&url);
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn tokenize_when_first_steps_then_has_tokens() {
        let mut proj = new_empty_project();
        let url = Url::parse("http://example.com").unwrap();
        let content = read_resource("first_steps.st");
        proj.change_text_document(&url, content.as_str());

        let result = proj.tokenize(&url);

        assert!(result.is_ok());
    }
}
