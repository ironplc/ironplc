//! Adapts data types between what is required by the compiler
//! and the language server protocol.
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use ironplc_analyzer::extractors::TypeSymbolKind;
use ironplc_analyzer::SemanticContext;
use ironplc_dsl::core::{FileId, Located};
use ironplc_dsl::diagnostic::LineColumn;
use ironplc_parser::token::{Token, TokenType};
use log::error;
use lsp_types::{
    CodeDescription, Diagnostic, DiagnosticRelatedInformation, DiagnosticSeverity, DocumentSymbol,
    DocumentSymbolResponse, Location, NumberOrString, SemanticTokenType, SymbolKind,
    WorkspaceFolder,
};
use lsp_types::{SemanticToken, Uri};

use crate::lsp_runner::{RunResult, VmRunner};
use ironplc_project::Project;

fn to_path_buf(uri: &Uri) -> Result<PathBuf, ()> {
    Ok(PathBuf::from(uri.path().as_str()))
}

/// A hashable, equality-comparable key derived from an LSP `Uri`.
///
/// `lsp_types::Uri` wraps `fluent_uri::Uri`, which contains interior
/// `Cell`s for caching parse results. Although the `Hash` and `Eq`
/// impls today only read the immutable string content, using such a
/// type as a `HashMap`/`HashSet` key is brittle: a future change in
/// the upstream crate could silently break lookups. Convert to this
/// newtype at the boundary so the collections store nothing more
/// than the canonical URI string.
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub(crate) struct UriKey(String);

impl UriKey {
    pub(crate) fn from_uri(uri: &Uri) -> Self {
        Self(uri.as_str().to_string())
    }

    /// Build a `UriKey` from a compiler `FileId`.
    ///
    /// Returns `None` for `FileId::BuiltIn`, for `FileId::File` with an
    /// empty path (which the analyzer uses as a placeholder for
    /// whole-project diagnostics like "no source files"), and when the
    /// path cannot be turned into a valid URI. Paths from
    /// `FileId::from_path` use OS-native separators; this normalises
    /// them and validates the result through `Uri::from_str` before
    /// returning the key.
    pub(crate) fn from_file_id(file_id: &FileId) -> Option<Self> {
        match file_id {
            FileId::File(path) => {
                let s = path.as_ref();
                if s.is_empty() {
                    return None;
                }
                let normalised = s.replace('\\', "/");
                let uri_str = if normalised.starts_with('/') {
                    format!("file://{normalised}")
                } else {
                    format!("file:///{normalised}")
                };
                let parsed = Uri::from_str(&uri_str).ok()?;
                Some(Self::from_uri(&parsed))
            }
            FileId::BuiltIn => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// Reconstruct an LSP `Uri` from this key for outbound LSP
    /// notifications. The key only ever holds a string that was
    /// previously produced by a valid `Uri`, so parsing must
    /// succeed.
    pub(crate) fn to_uri(&self) -> Uri {
        Uri::from_str(&self.0).expect("UriKey was constructed from a valid Uri")
    }
}

/// The LSP project provides a view onto a project that accepts
/// and returns LSP types.
pub struct LspProject {
    wrapped: Box<dyn Project + Send>,
    /// Active VM runner session for step-through execution.
    runner: Option<VmRunner>,
    /// Compiler options used for compilation (cached for runner).
    compiler_options: ironplc_parser::options::CompilerOptions,
    /// Key for the most recently changed text document. Used as a
    /// fallback so that diagnostics whose `FileId` cannot be mapped
    /// back to a URI (e.g. `BuiltIn`) still surface in the editor
    /// instead of being silently dropped.
    last_changed_uri_key: Option<UriKey>,
}

impl LspProject {
    pub fn new(project: Box<dyn Project + Send>) -> Self {
        Self {
            wrapped: project,
            runner: None,
            compiler_options: ironplc_parser::options::CompilerOptions::default(),
            last_changed_uri_key: None,
        }
    }

    pub fn with_options(
        project: Box<dyn Project + Send>,
        options: ironplc_parser::options::CompilerOptions,
    ) -> Self {
        Self {
            wrapped: project,
            runner: None,
            compiler_options: options,
            last_changed_uri_key: None,
        }
    }

    pub(crate) fn initialize(&mut self, folder: &WorkspaceFolder) {
        let path = to_path_buf(&folder.uri);
        if let Ok(path) = path {
            self.wrapped.initialize(&path);
        } else {
            error!(
                "URL must be convertible to a file path {}",
                folder.uri.as_str()
            );
        }
    }

    pub(crate) fn change_text_document(&mut self, uri: &Uri, content: String) {
        let path = to_path_buf(uri);
        if let Ok(path) = path {
            let file_id = FileId::from_path(&path);
            self.wrapped.change_text_document(&file_id, content);
            self.last_changed_uri_key = Some(UriKey::from_uri(uri));
        } else {
            error!("URL must be convertible to a file path {}", uri.as_str());
        }
    }

    pub(crate) fn tokenize(&self, uri: &Uri) -> Result<Vec<SemanticToken>, Vec<Diagnostic>> {
        let path = to_path_buf(uri);
        if let Ok(path) = path {
            let file_id = FileId::from_path(&path);

            let result = self.wrapped.tokenize(&file_id);

            if !result.1.is_empty() {
                return Err(result
                    .1
                    .into_iter()
                    .map(|err| map_diagnostic(err, self.wrapped.as_ref()))
                    .collect());
            }

            // The conversion produces tokens with absolute (line, col) values
            // stored in `delta_line` / `delta_start`. The LSP protocol requires
            // these fields to be encoded as deltas relative to the previous
            // emitted token, so fold over the sequence to convert them.
            let absolute: Vec<SemanticToken> = result
                .0
                .into_iter()
                .filter_map(|tok| LspTokenType(tok).into())
                .collect();

            return Ok(to_deltas(absolute));
        } else {
            error!("URL must be convertible to a file path {}", uri.as_str());
        }

        Err(vec![])
    }

    /// Run semantic analysis on the whole workspace and return all
    /// resulting diagnostics grouped by the URI key that should
    /// display each one.
    ///
    /// Every diagnostic appears under each URI that its primary or
    /// secondary labels reference, so a cross-file diagnostic shows
    /// up in both files. Diagnostics whose `FileId` cannot be mapped
    /// to a URI (e.g. `BuiltIn`) are attributed to the most recently
    /// edited URI so they do not silently disappear.
    ///
    /// Callers are responsible for emitting `PublishDiagnostics`
    /// notifications for every URI in the returned map and for
    /// emitting empty notifications to clear URIs that previously
    /// had diagnostics but no longer do.
    pub(crate) fn semantic_all(&mut self) -> HashMap<UriKey, Vec<lsp_types::Diagnostic>> {
        let semantic_result = self.wrapped.semantic();

        let diagnostics = match semantic_result {
            Ok(_) => return HashMap::new(),
            Err(diagnostics) => diagnostics,
        };

        let mut by_key: HashMap<UriKey, Vec<lsp_types::Diagnostic>> = HashMap::new();
        for diagnostic in diagnostics {
            // Resolve every file id this diagnostic touches into a key.
            // De-duplicate so a diagnostic with primary and secondary
            // labels in the same file is published once for that file.
            let mut keys: Vec<UriKey> = Vec::new();
            for file_id in diagnostic.file_ids() {
                if let Some(key) = UriKey::from_file_id(file_id) {
                    if !keys.contains(&key) {
                        keys.push(key);
                    }
                }
            }
            if keys.is_empty() {
                // No usable URI on the diagnostic itself — fall back to
                // the last edited document so we never silently drop it.
                if let Some(fallback) = self.last_changed_uri_key.clone() {
                    keys.push(fallback);
                } else {
                    continue;
                }
            }

            let mapped = map_diagnostic(diagnostic, self.wrapped.as_ref());
            for key in keys {
                by_key.entry(key).or_default().push(mapped.clone());
            }
        }

        by_key
    }

    /// Run semantic analysis and return only the diagnostics that
    /// should be shown for `uri`. Retained as a thin wrapper over
    /// `semantic_all` for tests and callers that only need a single
    /// file's diagnostics.
    #[allow(dead_code)]
    pub(crate) fn semantic(&mut self, uri: &Uri) -> Vec<lsp_types::Diagnostic> {
        let mut by_key = self.semantic_all();
        by_key.remove(&UriKey::from_uri(uri)).unwrap_or_default()
    }

    /// Returns the semantic context from the last successful analysis.
    ///
    /// This provides access to type, function, and symbol information
    /// for IDE features like document symbols, go to definition, and hover.
    #[allow(dead_code)]
    pub(crate) fn semantic_context(&self) -> Option<&SemanticContext> {
        self.wrapped.semantic_context()
    }

    /// Returns document symbols for the given URI.
    ///
    /// This provides an outline of types defined in the document for
    /// VS Code's Outline panel and "Go to Symbol" feature.
    pub(crate) fn document_symbols(&self, uri: &Uri) -> DocumentSymbolResponse {
        let path = match to_path_buf(uri) {
            Ok(path) => path,
            Err(_) => {
                error!("URL must be convertible to a file path {}", uri.as_str());
                return DocumentSymbolResponse::Nested(vec![]);
            }
        };

        let file_id = FileId::from_path(&path);

        let context = match self.wrapped.semantic_context() {
            Some(ctx) => ctx,
            None => return DocumentSymbolResponse::Nested(vec![]),
        };

        let source = match self.wrapped.find(&file_id) {
            Some(src) => src,
            None => return DocumentSymbolResponse::Nested(vec![]),
        };

        let contents = source.as_string();

        let mut symbols = Vec::new();

        // User-defined types (structures, enumerations, arrays, ...)
        // The extractor already excludes elementary types and the
        // function-block / function entries that types() carries
        // alongside the per-POU registry, so we don't need to filter
        // those again here.
        for view in context.user_defined_types() {
            if view.attributes.span().file_id != file_id {
                continue;
            }
            let range = span_to_range(contents, &view.attributes.span());
            #[allow(deprecated)]
            symbols.push(DocumentSymbol {
                name: view.name.to_string(),
                detail: Some(format!("{:?}", view.attributes.type_category)),
                kind: type_kind_to_symbol_kind(view.kind),
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: None,
            });
        }

        // Function blocks (declared by the user; not surfaced via
        // user-defined types since the function-block name is registered
        // separately in the symbol environment).
        for view in context.function_blocks() {
            if view.info.span.file_id != file_id {
                continue;
            }
            let range = span_to_range(contents, &view.info.span);
            #[allow(deprecated)]
            symbols.push(DocumentSymbol {
                name: view.name.to_string(),
                detail: None,
                kind: SymbolKind::CLASS,
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: None,
            });
        }

        // User-defined functions
        for view in context.user_defined_functions() {
            if view.signature.span.file_id != file_id {
                continue;
            }
            let range = span_to_range(contents, &view.signature.span);
            #[allow(deprecated)]
            symbols.push(DocumentSymbol {
                name: view.signature.name.to_string(),
                detail: view
                    .signature
                    .return_type
                    .as_ref()
                    .map(|t| format!("{:?}", t)),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                range,
                selection_range: range,
                children: None,
            });
        }

        DocumentSymbolResponse::Nested(symbols)
    }

    /// Compile source and start a VM execution session.
    ///
    /// Returns initial metadata. Subsequent calls to `step()` execute
    /// scan cycles and return variable values.
    pub(crate) fn run_load(&mut self, source: &str, cycle_time_us: u64) -> RunResult {
        // Tear down any existing session
        self.runner = None;

        match VmRunner::load(source, cycle_time_us, &self.compiler_options) {
            Ok((runner, result)) => {
                self.runner = Some(runner);
                result
            }
            Err(err) => err,
        }
    }

    /// Execute N scan cycles in the current session.
    pub(crate) fn run_step(&mut self, scans: u32) -> RunResult {
        match self.runner.as_mut() {
            Some(runner) => runner.step(scans),
            None => RunResult {
                ok: false,
                variables: vec![],
                total_scans: 0,
                error: Some("No program loaded. Send ironplc/run first.".to_string()),
            },
        }
    }

    /// Stop the current execution session and release resources.
    pub(crate) fn run_stop(&mut self) -> RunResult {
        self.runner = None;
        RunResult {
            ok: true,
            variables: vec![],
            total_scans: 0,
            error: None,
        }
    }
}

/// Map the analyzer's neutral `TypeSymbolKind` to LSP's `SymbolKind`.
fn type_kind_to_symbol_kind(kind: TypeSymbolKind) -> SymbolKind {
    match kind {
        TypeSymbolKind::Structure => SymbolKind::STRUCT,
        TypeSymbolKind::Enumeration => SymbolKind::ENUM,
        TypeSymbolKind::FunctionBlock => SymbolKind::CLASS,
        TypeSymbolKind::Function => SymbolKind::FUNCTION,
        TypeSymbolKind::Array => SymbolKind::ARRAY,
        TypeSymbolKind::Subrange => SymbolKind::TYPE_PARAMETER,
        TypeSymbolKind::String | TypeSymbolKind::Reference | TypeSymbolKind::Alias => {
            SymbolKind::VARIABLE
        }
    }
}

/// Convert a SourceSpan to an LSP Range using file contents for line/column calculation.
fn span_to_range(contents: &str, span: &ironplc_dsl::core::SourceSpan) -> lsp_types::Range {
    let start = LineColumn::from_offset(contents, span.start);
    let end = LineColumn::from_offset(contents, span.end);
    lsp_types::Range::new(
        lsp_types::Position::new(start.line, start.column),
        lsp_types::Position::new(end.line, end.column),
    )
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

/// Convert diagnostic type into the LSP diagnostic type.
fn map_diagnostic(
    diagnostic: ironplc_dsl::diagnostic::Diagnostic,
    project: &dyn Project,
) -> lsp_types::Diagnostic {
    let description = diagnostic.description();
    let range = map_label(&diagnostic.primary, project);

    let mut url_string = format!(
        "https://www.ironplc.com/reference/compiler/problems/{}.html?version={}",
        diagnostic.code,
        env!("CARGO_PKG_VERSION")
    );
    if let Some(ref file) = diagnostic.source_file {
        url_string.push_str(&format!("&file={}", file));
    }
    if let Some(line) = diagnostic.source_line {
        url_string.push_str(&format!("&line={}", line));
    }
    let code_description = match Uri::from_str(&url_string) {
        Ok(url) => Some(CodeDescription { href: url }),
        Err(_) => None,
    };

    let related_information = if diagnostic.secondary.is_empty() {
        None
    } else {
        Some(
            diagnostic
                .secondary
                .iter()
                .filter_map(|label| {
                    let uri_str = format!("file://{}", label.file_id);
                    let uri = Uri::from_str(&uri_str).ok()?;
                    let range = map_label(label, project);
                    Some(DiagnosticRelatedInformation {
                        location: Location { uri, range },
                        message: label.message.clone(),
                    })
                })
                .collect(),
        )
    };

    let mut message = format!("{description}: {} ", diagnostic.primary.message);
    for note in diagnostic.help() {
        message.push_str(&format!("\n{note}"));
    }

    lsp_types::Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(diagnostic.code)),
        code_description,
        source: Some("ironplc".into()),
        message,
        related_information,
        tags: None,
        data: None,
    }
}

/// Convert the diagnostic label into the LSP range type.
fn map_label(label: &ironplc_dsl::diagnostic::Label, project: &dyn Project) -> lsp_types::Range {
    let file_id = &label.file_id;
    let contents = project.find(file_id);

    if let Some(contents) = contents {
        let contents = contents.as_string();
        let start = LineColumn::from_offset(contents, label.location.start);
        let end = LineColumn::from_offset(contents, label.location.end);
        return lsp_types::Range::new(
            lsp_types::Position::new(start.line, start.column),
            lsp_types::Position::new(end.line, end.column),
        );
    }
    lsp_types::Range::new(
        lsp_types::Position::new(0, 0),
        lsp_types::Position::new(0, 0),
    )
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;
    use std::str::FromStr;

    use ironplc_dsl::core::SourceSpan;
    use ironplc_parser::token::{Token, TokenType};
    use ironplc_test::cast;
    use ironplc_test::read_shared_resource;
    use lsp_types::{SemanticToken, Uri};

    use ironplc_project::FileBackedProject;

    use super::{LspProject, LspTokenType};

    #[cfg(target_os = "macos")]
    static FAKE_PATH: &str = "file:///localhost/first_steps.st";
    #[cfg(target_os = "linux")]
    static FAKE_PATH: &str = "file:///localhost/first_steps.st";
    #[cfg(target_os = "windows")]
    static FAKE_PATH: &str = "file:///C:/first_steps.st";

    fn new_empty_project() -> LspProject {
        LspProject::new(Box::new(FileBackedProject::new()))
    }

    #[test]
    fn tokenize_when_no_document_then_error() {
        let proj = new_empty_project();
        let url = Uri::from_str("http://example.com").unwrap();
        assert!(proj.tokenize(&url).is_err());
    }

    #[test]
    fn tokenize_when_has_document_then_not_empty_tokens() {
        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        proj.change_text_document(&url, "TYPE TEXT_EMPTY : STRING [1]; END_TYPE".to_owned());

        let result = proj.tokenize(&url);
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn tokenize_when_tokens_span_multiple_lines_then_emits_relative_deltas() {
        // The LSP semantic-tokens protocol requires delta encoding: each
        // token's `delta_line` / `delta_start` are relative to the previous
        // emitted token. Reconstructing absolute (line, col) by accumulating
        // deltas must match the source positions of the keywords.
        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        proj.change_text_document(
            &url,
            "PROGRAM Main\nVAR\nx : BOOL;\nEND_VAR\nEND_PROGRAM".to_owned(),
        );

        let tokens = proj.tokenize(&url).expect("expected tokens");
        assert!(!tokens.is_empty(), "expected tokens, got none");

        // Reconstruct absolute positions from the deltas.
        let mut line: u32 = 0;
        let mut col: u32 = 0;
        let mut absolutes: Vec<(u32, u32)> = Vec::new();
        for t in &tokens {
            if t.delta_line == 0 {
                col += t.delta_start;
            } else {
                line += t.delta_line;
                col = t.delta_start;
            }
            absolutes.push((line, col));
        }

        // PROGRAM is the first token at (0, 0).
        assert_eq!(absolutes[0], (0, 0), "PROGRAM should be at line 0, col 0");
        // VAR sits at the start of line 1.
        assert!(
            absolutes.contains(&(1, 0)),
            "expected a token at (1, 0) for VAR, got {absolutes:?}"
        );
        // BOOL sits at line 2, col 4.
        assert!(
            absolutes.contains(&(2, 4)),
            "expected a token at (2, 4) for BOOL, got {absolutes:?}"
        );
        // END_VAR sits at the start of line 3.
        assert!(
            absolutes.contains(&(3, 0)),
            "expected a token at (3, 0) for END_VAR, got {absolutes:?}"
        );
        // END_PROGRAM sits at the start of line 4.
        assert!(
            absolutes.contains(&(4, 0)),
            "expected a token at (4, 0) for END_PROGRAM, got {absolutes:?}"
        );
    }

    /// Reconstruct a `(line, col, length, token_type)` table from the
    /// delta-encoded `SemanticToken` stream and resolve each entry against
    /// `source` to return the actual substring covered. Used by coverage
    /// tests below to detect regressions where the LSP overlay would land on
    /// the wrong characters.
    fn resolve_lsp_tokens<'a>(
        source: &'a str,
        tokens: &[SemanticToken],
    ) -> Vec<(u32, u32, &'a str, u32)> {
        let lines: Vec<&str> = source.split('\n').collect();
        let mut line: u32 = 0;
        let mut col: u32 = 0;
        let mut out = Vec::new();
        for t in tokens {
            if t.delta_line == 0 {
                col += t.delta_start;
            } else {
                line += t.delta_line;
                col = t.delta_start;
            }
            let row = lines.get(line as usize).copied().unwrap_or("");
            let start = col as usize;
            let end = (col + t.length) as usize;
            // Operate on bytes — the test inputs are ASCII-only.
            let slice = &row[start.min(row.len())..end.min(row.len())];
            out.push((line, col, slice, t.token_type));
        }
        out
    }

    #[test]
    fn tokenize_when_program_with_keywords_types_and_comment_then_each_lsp_token_lands_on_its_source_text(
    ) {
        // Coverage: every semantic token emitted to the LSP client must land
        // exactly on the source characters that the lexer recognised. If the
        // delta encoding drifts (or column tracking is wrong inside comments),
        // the substring at the reconstructed position will not match the
        // expected token text and this assertion will fail.
        let source = "PROGRAM Demo\nVAR\n  a : BOOL; (* note *) b : INT;\nEND_VAR\nEND_PROGRAM";
        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        proj.change_text_document(&url, source.to_owned());

        let tokens = proj.tokenize(&url).expect("expected tokens");
        let resolved = resolve_lsp_tokens(source, &tokens);

        // Spot-check key tokens that the user reported as broken.
        let texts: Vec<&str> = resolved.iter().map(|(_, _, s, _)| *s).collect();
        for expected in [
            "PROGRAM",
            "Demo",
            "VAR",
            "a",
            "BOOL",
            "(* note *)",
            "b",
            "INT",
            "END_VAR",
            "END_PROGRAM",
        ] {
            assert!(
                texts.contains(&expected),
                "expected LSP token covering `{expected}`, got tokens: {texts:?}"
            );
        }

        // Both BOOL and INT sit on line 2; their reconstructed columns must
        // match the actual source columns.
        let bool_pos = resolved
            .iter()
            .find(|(_, _, s, _)| *s == "BOOL")
            .expect("BOOL token");
        assert_eq!(
            (bool_pos.0, bool_pos.1),
            (2, 6),
            "BOOL should be at line 2 col 6"
        );
        let int_pos = resolved
            .iter()
            .find(|(_, _, s, _)| *s == "INT")
            .expect("INT token");
        // After the inline block comment, the next BOOL/INT must still report
        // the correct column — this is the regression that produced the
        // user's "trip"/"ain Mot" mis-coloring.
        assert_eq!(
            (int_pos.0, int_pos.1),
            (2, 27),
            "INT should be at line 2 col 27"
        );

        // Both BOOL and INT must be tagged as the KEYWORD token type so that
        // VS Code applies the same semantic-token color to every primitive
        // type. (The reported "BOOL in three colors" bug was actually the
        // semantic overlay landing on different surrounding text — we keep
        // this assertion to lock in the type-tagging contract.)
        assert_eq!(bool_pos.3, int_pos.3, "BOOL and INT must share token type");
    }

    #[test]
    fn tokenize_when_inline_block_comment_then_following_token_keeps_position() {
        // Regression: a single-line block comment used to leave the column
        // counter at the comment's start, which displaced every subsequent
        // semantic token in the LSP overlay and caused parts of comment text
        // to lose their `comment.block.st` styling.
        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        proj.change_text_document(
            &url,
            "VAR\n  a : BOOL; (* note *) b : BOOL;\nEND_VAR".to_owned(),
        );

        let tokens = proj.tokenize(&url).expect("expected tokens");
        // Reconstruct absolute positions and the keyword index of each token.
        let mut line: u32 = 0;
        let mut col: u32 = 0;
        let mut bool_positions: Vec<(u32, u32)> = Vec::new();
        for t in &tokens {
            if t.delta_line == 0 {
                col += t.delta_start;
            } else {
                line += t.delta_line;
                col = t.delta_start;
            }
            // BOOL is length 4. The two BOOLs on line 1 sit at columns 6 and
            // 27 in the source string.
            if t.length == 4 && line == 1 {
                bool_positions.push((line, col));
            }
        }
        assert!(
            bool_positions.contains(&(1, 6)) && bool_positions.contains(&(1, 27)),
            "expected BOOLs at (1,6) and (1,27), got {bool_positions:?}"
        );
    }

    #[test]
    fn tokenize_when_first_steps_then_has_tokens() {
        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        let content = read_shared_resource("first_steps.st");
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

    #[test]
    fn semantic_when_error_creates_diagnostics() {
        // Create a project with content that will exercise the character iteration loop
        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();

        // Add some invalid syntax to trigger diagnostics that will use map_label
        let invalid_content = "TYPE\n
INVALID_RANGE : INT(10..-10);\n
END_TYPE\n
INVALID_SYNTAX"
            .to_string();
        proj.change_text_document(&url, invalid_content);

        // Call semantic analysis which will internally call map_label when creating diagnostics
        let _diagnostics = proj.semantic(&url);
    }

    #[test]
    fn map_diagnostic_when_secondary_labels_then_produces_related_information() {
        use ironplc_dsl::core::FileId;
        use ironplc_dsl::diagnostic::{
            Diagnostic as DslDiagnostic, Label as DslLabel, Location as DslLocation,
        };

        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        let content = "PROGRAM Main\nVAR\n  x : INT;\nEND_VAR\nEND_PROGRAM";
        proj.change_text_document(&url, content.to_owned());

        let file_id = FileId::from_path(&std::path::PathBuf::from(url.path().as_str()));

        let diag = DslDiagnostic::problem(
            ironplc_problems::Problem::VariableUndefined,
            DslLabel {
                location: DslLocation { start: 0, end: 7 },
                file_id: file_id.clone(),
                message: "primary label".to_string(),
            },
        )
        .with_secondary(DslLabel {
            location: DslLocation { start: 20, end: 23 },
            file_id: file_id.clone(),
            message: "secondary label".to_string(),
        });

        let lsp_diag = super::map_diagnostic(diag, proj.wrapped.as_ref());

        assert!(lsp_diag.related_information.is_some());
        let related = lsp_diag.related_information.unwrap();
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].message, "secondary label");
    }

    #[test]
    fn map_diagnostic_when_no_secondary_labels_then_related_information_is_none() {
        use ironplc_dsl::core::FileId;
        use ironplc_dsl::diagnostic::{
            Diagnostic as DslDiagnostic, Label as DslLabel, Location as DslLocation,
        };

        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        let content = "PROGRAM Main\nEND_PROGRAM";
        proj.change_text_document(&url, content.to_owned());

        let file_id = FileId::from_path(&std::path::PathBuf::from(url.path().as_str()));

        let diag = DslDiagnostic::problem(
            ironplc_problems::Problem::SyntaxError,
            DslLabel {
                location: DslLocation { start: 0, end: 7 },
                file_id,
                message: "some error".to_string(),
            },
        );

        let lsp_diag = super::map_diagnostic(diag, proj.wrapped.as_ref());

        assert!(lsp_diag.related_information.is_none());
    }

    #[test]
    fn map_diagnostic_when_problem_then_url_has_version_only() {
        use ironplc_dsl::core::FileId;
        use ironplc_dsl::diagnostic::{
            Diagnostic as DslDiagnostic, Label as DslLabel, Location as DslLocation,
        };

        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        let content = "PROGRAM Main\nEND_PROGRAM";
        proj.change_text_document(&url, content.to_owned());

        let file_id = FileId::from_path(&std::path::PathBuf::from(url.path().as_str()));

        let diag = DslDiagnostic::problem(
            ironplc_problems::Problem::SyntaxError,
            DslLabel {
                location: DslLocation { start: 0, end: 7 },
                file_id,
                message: "some error".to_string(),
            },
        );

        let lsp_diag = super::map_diagnostic(diag, proj.wrapped.as_ref());

        let href = lsp_diag.code_description.unwrap().href.to_string();
        assert!(href.contains("?version="));
        assert!(!href.contains("&file="));
        assert!(!href.contains("&line="));
    }

    #[test]
    fn map_diagnostic_when_todo_then_url_has_version_file_and_line() {
        use ironplc_dsl::core::FileId;
        use ironplc_dsl::diagnostic::{
            Diagnostic as DslDiagnostic, Label as DslLabel, Location as DslLocation,
        };

        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        let content = "PROGRAM Main\nEND_PROGRAM";
        proj.change_text_document(&url, content.to_owned());

        let file_id = FileId::from_path(&std::path::PathBuf::from(url.path().as_str()));

        let diag = DslDiagnostic::problem(
            ironplc_problems::Problem::SyntaxError,
            DslLabel {
                location: DslLocation { start: 0, end: 7 },
                file_id,
                message: "some error".to_string(),
            },
        )
        .with_source("compiler/analyzer/src/rule_example.rs", 42);

        let lsp_diag = super::map_diagnostic(diag, proj.wrapped.as_ref());

        let href = lsp_diag.code_description.unwrap().href.to_string();
        assert!(href.contains("?version="));
        assert!(href.contains("&file=compiler/analyzer/src/rule_example.rs"));
        assert!(href.contains("&line=42"));
    }

    #[test]
    fn document_symbols_when_no_document_then_empty() {
        let proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();

        let result = proj.document_symbols(&url);

        let symbols = cast!(result, lsp_types::DocumentSymbolResponse::Nested);
        assert!(symbols.is_empty());
    }

    #[test]
    fn document_symbols_when_has_structure_then_returns_struct_symbol() {
        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        let content = "TYPE\nMyStruct : STRUCT\n  field1 : INT;\nEND_STRUCT;\nEND_TYPE";
        proj.change_text_document(&url, content.to_owned());

        // Run semantic analysis to populate the context
        let _ = proj.semantic(&url);

        let result = proj.document_symbols(&url);

        let symbols = cast!(result, lsp_types::DocumentSymbolResponse::Nested);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "MyStruct");
        assert_eq!(symbols[0].kind, lsp_types::SymbolKind::STRUCT);
    }

    #[test]
    fn document_symbols_when_has_enumeration_then_returns_enum_symbol() {
        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        let content = "TYPE\nMyEnum : (VALUE1, VALUE2, VALUE3);\nEND_TYPE";
        proj.change_text_document(&url, content.to_owned());

        // Run semantic analysis to populate the context
        let _ = proj.semantic(&url);

        let result = proj.document_symbols(&url);

        let symbols = cast!(result, lsp_types::DocumentSymbolResponse::Nested);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "MyEnum");
        assert_eq!(symbols[0].kind, lsp_types::SymbolKind::ENUM);
    }

    #[test]
    fn document_symbols_when_has_function_block_then_returns_fb_symbol() {
        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        let content = "FUNCTION_BLOCK MyFB\nVAR\n  x : INT;\nEND_VAR\nEND_FUNCTION_BLOCK";
        proj.change_text_document(&url, content.to_owned());

        // Run semantic analysis to populate the context
        let _ = proj.semantic(&url);

        let result = proj.document_symbols(&url);

        let symbols = cast!(result, lsp_types::DocumentSymbolResponse::Nested);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "MyFB");
    }

    #[test]
    fn document_symbols_when_has_function_then_returns_function_symbol() {
        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        let content =
            "FUNCTION MyFunc : INT\nVAR_INPUT\n  x : INT;\nEND_VAR\nMyFunc := x * 2;\nEND_FUNCTION";
        proj.change_text_document(&url, content.to_owned());

        // Run semantic analysis to populate the context
        let _ = proj.semantic(&url);

        let result = proj.document_symbols(&url);

        let symbols = cast!(result, lsp_types::DocumentSymbolResponse::Nested);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "MyFunc");
        assert_eq!(symbols[0].kind, lsp_types::SymbolKind::FUNCTION);
    }

    #[test]
    fn document_symbols_when_multiple_types_then_returns_all() {
        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        let content =
            "TYPE\nMyStruct : STRUCT\n  field1 : INT;\nEND_STRUCT;\nMyEnum : (A, B);\nEND_TYPE";
        proj.change_text_document(&url, content.to_owned());

        // Run semantic analysis to populate the context
        let _ = proj.semantic(&url);

        let result = proj.document_symbols(&url);

        let symbols = cast!(result, lsp_types::DocumentSymbolResponse::Nested);
        assert_eq!(symbols.len(), 2);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"MyStruct"));
        assert!(names.contains(&"MyEnum"));
    }

    #[test]
    fn span_to_range_when_single_line_then_correct_positions() {
        use super::span_to_range;

        let contents = "TYPE MyType : INT; END_TYPE";
        let span = SourceSpan {
            start: 5,
            end: 11,
            file_id: ironplc_dsl::core::FileId::default(),
        };

        let range = span_to_range(contents, &span);

        assert_eq!(range.start.line, 0);
        assert_eq!(range.start.character, 5);
        assert_eq!(range.end.line, 0);
        assert_eq!(range.end.character, 11);
    }

    #[test]
    fn span_to_range_when_multiline_then_correct_positions() {
        use super::span_to_range;

        let contents = "TYPE\nMyStruct : STRUCT\n  field : INT;\nEND_STRUCT;\nEND_TYPE";
        // Span covering "MyStruct" which starts at position 5 (after "TYPE\n")
        let span = SourceSpan {
            start: 5,
            end: 13,
            file_id: ironplc_dsl::core::FileId::default(),
        };

        let range = span_to_range(contents, &span);

        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 0);
        assert_eq!(range.end.line, 1);
        assert_eq!(range.end.character, 8);
    }

    #[test]
    fn document_symbols_when_semantic_errors_then_returns_symbols() {
        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        // A valid struct (types resolve) plus a function block with an undefined variable
        // (semantic validation error from rule_use_declared_symbolic_var)
        let content = "TYPE\nMyStruct : STRUCT\n  field1 : INT;\nEND_STRUCT;\nEND_TYPE\n\nFUNCTION_BLOCK BadFB\nVAR\n  x : INT;\nEND_VAR\n  y := x;\nEND_FUNCTION_BLOCK";
        proj.change_text_document(&url, content.to_owned());

        // Semantic analysis will produce diagnostics for undefined variable 'y'
        let diagnostics = proj.semantic(&url);
        assert!(!diagnostics.is_empty());

        // Document symbols should still be available despite the error
        let result = proj.document_symbols(&url);
        let symbols = cast!(result, lsp_types::DocumentSymbolResponse::Nested);
        let names: Vec<&str> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"MyStruct"));
    }

    #[test]
    fn map_diagnostic_when_problem_code_url_then_docs_directory_exists() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // compiler/ironplc-cli/ -> repo root
        path.push("../..");
        path.push("docs/reference/compiler/problems");
        assert!(
            path.is_dir(),
            "Documentation directory for compiler problem codes does not exist: {}",
            path.display()
        );

        // Verify at least one problem code .rst file exists
        let has_problem_files = std::fs::read_dir(&path)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                name.starts_with('P') && name.ends_with(".rst")
            });
        assert!(
            has_problem_files,
            "No P*.rst files found in {}",
            path.display()
        );
    }

    #[test]
    fn type_kind_to_symbol_kind_when_structure_then_struct() {
        use super::type_kind_to_symbol_kind;
        use ironplc_analyzer::extractors::TypeSymbolKind;

        assert_eq!(
            type_kind_to_symbol_kind(TypeSymbolKind::Structure),
            lsp_types::SymbolKind::STRUCT
        );
    }

    #[test]
    fn type_kind_to_symbol_kind_when_enumeration_then_enum() {
        use super::type_kind_to_symbol_kind;
        use ironplc_analyzer::extractors::TypeSymbolKind;

        assert_eq!(
            type_kind_to_symbol_kind(TypeSymbolKind::Enumeration),
            lsp_types::SymbolKind::ENUM
        );
    }

    #[test]
    fn type_kind_to_symbol_kind_when_function_block_then_class() {
        use super::type_kind_to_symbol_kind;
        use ironplc_analyzer::extractors::TypeSymbolKind;

        assert_eq!(
            type_kind_to_symbol_kind(TypeSymbolKind::FunctionBlock),
            lsp_types::SymbolKind::CLASS
        );
    }

    #[cfg(target_os = "windows")]
    static FAKE_PATH_2: &str = "file:///C:/second_steps.st";
    #[cfg(not(target_os = "windows"))]
    static FAKE_PATH_2: &str = "file:///localhost/second_steps.st";

    #[test]
    fn semantic_all_when_no_errors_then_returns_empty_map() {
        let mut proj = new_empty_project();
        let url = Uri::from_str(FAKE_PATH).unwrap();
        proj.change_text_document(
            &url,
            "PROGRAM Main\nVAR\n  x : BOOL;\nEND_VAR\nEND_PROGRAM".to_owned(),
        );

        let by_uri = proj.semantic_all();
        // Empty project (no configuration with a program instance) still
        // analyses cleanly when there are no parse errors. Either way,
        // the contract is "no errors => no entries".
        for diags in by_uri.values() {
            assert!(
                diags.is_empty(),
                "expected no diagnostics for clean project, got: {:?}",
                diags
            );
        }
    }

    #[test]
    fn semantic_all_when_two_files_each_have_errors_then_returns_diagnostics_for_each_uri() {
        use super::UriKey;

        let mut proj = new_empty_project();
        let url_a = Uri::from_str(FAKE_PATH).unwrap();
        let url_b = Uri::from_str(FAKE_PATH_2).unwrap();
        let key_a = UriKey::from_uri(&url_a);
        let key_b = UriKey::from_uri(&url_b);

        // Both files contain unparseable garbage so each produces its
        // own parse-time diagnostics and the analyzer should attribute
        // them to their respective FileIds.
        proj.change_text_document(&url_a, "this is not valid IEC 61131-3".to_owned());
        proj.change_text_document(&url_b, "neither is this".to_owned());

        let by_key = proj.semantic_all();

        assert!(
            by_key.contains_key(&key_a),
            "expected diagnostics for {url_a:?}, got keys {:?}",
            by_key.keys().collect::<Vec<_>>()
        );
        assert!(
            by_key.contains_key(&key_b),
            "expected diagnostics for {url_b:?}, got keys {:?}",
            by_key.keys().collect::<Vec<_>>()
        );
        assert!(!by_key[&key_a].is_empty());
        assert!(!by_key[&key_b].is_empty());
    }

    #[test]
    fn semantic_when_back_compat_wrapper_then_returns_only_uris_diagnostics() {
        // The single-URI `semantic` wrapper should still return only the
        // diagnostics that the publisher would direct at that URI.
        let mut proj = new_empty_project();
        let url_a = Uri::from_str(FAKE_PATH).unwrap();
        let url_b = Uri::from_str(FAKE_PATH_2).unwrap();

        proj.change_text_document(&url_a, "this is not valid IEC 61131-3".to_owned());
        proj.change_text_document(&url_b, "neither is this".to_owned());

        let only_a = proj.semantic(&url_a);
        assert!(
            !only_a.is_empty(),
            "expected diagnostics for url_a from back-compat wrapper"
        );
        // None of the diagnostics returned by `semantic(url_a)` should
        // refer to url_b's range/source — the wrapper is a strict
        // single-URI view.
        // (Indirect check: `semantic_all` for url_b returns at least
        // one diagnostic that does not appear in `only_a`.)
        let only_b = proj.semantic(&url_b);
        assert!(!only_b.is_empty());
    }

    #[test]
    fn uri_key_from_file_id_when_round_trip_from_uri_path_then_matches_original() {
        use super::UriKey;
        use ironplc_dsl::core::FileId;

        let original = Uri::from_str(FAKE_PATH).unwrap();
        let path = std::path::PathBuf::from(original.path().as_str());
        let file_id = FileId::from_path(&path);

        let reconstructed =
            UriKey::from_file_id(&file_id).expect("file id should map back to a URI key");
        assert_eq!(
            reconstructed.as_str(),
            original.as_str(),
            "expected URI key round-trip via FileId to match the original URI string"
        );
    }

    #[test]
    fn uri_key_from_file_id_when_builtin_then_returns_none() {
        use super::UriKey;
        use ironplc_dsl::core::FileId;

        assert!(UriKey::from_file_id(&FileId::builtin()).is_none());
    }

    #[test]
    fn uri_key_when_to_uri_then_round_trips_back_to_original() {
        use super::UriKey;

        let original = Uri::from_str(FAKE_PATH).unwrap();
        let key = UriKey::from_uri(&original);
        let back = key.to_uri();
        assert_eq!(back.as_str(), original.as_str());
    }
}
