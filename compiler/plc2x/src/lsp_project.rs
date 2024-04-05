//! Adapts data types between what is required by the compiler
//! and the language server protocol.
use ironplc_dsl::core::FileId;
use ironplc_parser::token::TokenType;
use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString};
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

        match result {
            Ok(tokens) => Ok(tokens
                .into_iter()
                .map(|tok| LspTokenType(tok).into())
                .collect()),
            Err(errors) => {
                let compilation_set = self.wrapped.compilation_set();
                Err(errors
                    .into_iter()
                    .map(|err| map_diagnostic(err, &compilation_set))
                    .collect())
            }
        }
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

struct LspTokenType(TokenType);

impl From<LspTokenType> for SemanticToken {
    fn from(val: LspTokenType) -> Self {
        let pos = match val.0 {
            TokenType::Newline(pos) => pos,
            TokenType::Whitespace(pos) => pos,
            TokenType::Comment(pos) => pos,
            TokenType::LeftParen(pos) => pos,
            TokenType::RightParen(pos) => pos,
            TokenType::LeftBrace(pos) => pos,
            TokenType::RightBrace(pos) => pos,
            TokenType::LeftBracket(pos) => pos,
            TokenType::RightBracket(pos) => pos,
            TokenType::Comma(pos) => pos,
            TokenType::Semicolon(pos) => pos,
            TokenType::Colon(pos) => pos,
            TokenType::Period(pos) => pos,
            TokenType::Hash(pos) => pos,
            TokenType::String(pos) => pos,
            TokenType::Identifier(pos) => pos,
            TokenType::Array(pos) => pos,
            TokenType::Var(pos) => pos,
            TokenType::VarEnd(pos) => pos,
            TokenType::Retain(pos) => pos,
            TokenType::Constant(pos) => pos,
            TokenType::At(pos) => pos,
            TokenType::Percent(pos) => pos,
            TokenType::Function(pos) => pos,
            TokenType::EndFunction(pos) => pos,
            TokenType::FunctionBlock(pos) => pos,
            TokenType::EndFunctionBlock(pos) => pos,
            TokenType::Configuration(pos) => pos,
            TokenType::EndConfiguration(pos) => pos,
            TokenType::Resource(pos) => pos,
            TokenType::On(pos) => pos,
            TokenType::EndResource(pos) => pos,
            TokenType::Task(pos) => pos,
            TokenType::Interval(pos) => pos,
            TokenType::Priority(pos) => pos,
            TokenType::EndTask(pos) => pos,
            TokenType::Program(pos) => pos,
            TokenType::With(pos) => pos,
            TokenType::EndProgram(pos) => pos,
            TokenType::Or(pos) => pos,
            TokenType::Xor(pos) => pos,
            TokenType::And(pos) => pos,
            TokenType::Equal(pos) => pos,
            TokenType::NotEqual(pos) => pos,
            TokenType::Less(pos) => pos,
            TokenType::Greater(pos) => pos,
            TokenType::LessEqual(pos) => pos,
            TokenType::GreaterEqual(pos) => pos,
            TokenType::Div(pos) => pos,
            TokenType::Star(pos) => pos,
            TokenType::Plus(pos) => pos,
            TokenType::Minus(pos) => pos,
            TokenType::Mod(pos) => pos,
            TokenType::Power(pos) => pos,
            TokenType::Not(pos) => pos,
            TokenType::Assignment(pos) => pos,
            TokenType::If(pos) => pos,
            TokenType::Then(pos) => pos,
            TokenType::Elsif(pos) => pos,
            TokenType::Else(pos) => pos,
            TokenType::IfEnd(pos) => pos,
            TokenType::Case(pos) => pos,
            TokenType::Of(pos) => pos,
            TokenType::CaseEnd(pos) => pos,
            TokenType::For(pos) => pos,
            TokenType::Do(pos) => pos,
            TokenType::ForEnd(pos) => pos,
            TokenType::While(pos) => pos,
            TokenType::EndWhile(pos) => pos,
            TokenType::Repeat(pos) => pos,
            TokenType::Until(pos) => pos,
            TokenType::RepeatEnd(pos) => pos,
            TokenType::Exit(pos) => pos,
        };

        SemanticToken {
            delta_line: pos.line as u32,
            delta_start: pos.column as u32,
            // TODO
            length: 1,
            token_type: 0,
            token_modifiers_bitset: 0,
        }
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

        assert!(!proj.tokenize(&url).unwrap().is_empty());
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
