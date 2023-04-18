//! Implements the language server protocol for integration with an IDE such
//! as Visual Studio Code.
use std::path::PathBuf;

use lsp_server::{Connection, ExtractError, Message};
use lsp_types::{
    notification::{self, PublishDiagnostics},
    DiagnosticSeverity, DidChangeTextDocumentParams, NumberOrString, PublishDiagnosticsParams,
    ServerCapabilities, TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::stages::analyze;

// TODO give a real error
pub fn start() -> Result<(), String> {
    let (connection, io_threads) = Connection::stdio();

    let server_capabilities = serde_json::to_value(LspServer::server_capabilities()).unwrap();
    connection
        .initialize(server_capabilities)
        .map_err(|e| e.to_string())?;

    let server = LspServer::new(connection);
    server.main_loop()?;

    // TODO remove the unwrap
    io_threads.join().unwrap();

    Ok(())
}

struct LspServer {
    connection: Connection,
}

impl LspServer {
    /// Returns the set of capabilities that this language server supports.
    ///
    /// This effectively declares to the other end of the channel what we can
    /// do.
    fn server_capabilities() -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            ..ServerCapabilities::default()
        }
    }

    fn new(connection: Connection) -> Self {
        Self { connection }
    }

    /// The main event loop. The event loop receives messages from the other
    /// end of the channel.
    fn main_loop(&self) -> Result<(), String> {
        for msg in &self.connection.receiver {
            match msg {
                lsp_server::Message::Request(req) => {
                    if self
                        .connection
                        .handle_shutdown(&req)
                        .map_err(|_e| "Shutdown error")?
                    {
                        return Ok(());
                    }
                    self.handle_request(req)
                }
                lsp_server::Message::Response(_) => todo!(),
                lsp_server::Message::Notification(notification) => {
                    self.handle_notification(notification)
                }
            }
        }

        Ok(())
    }

    fn handle_request(&self, _request: lsp_server::Request) {
        // TODO handle requests
    }

    fn handle_notification(&self, notification: lsp_server::Notification) {
        let _notification =
            match Self::cast_notification::<notification::DidChangeTextDocument>(notification) {
                Ok(params) => return self.on_did_change_text_document(params),
                Err(notification) => notification,
            };

        // TODO other possible messages
    }

    fn cast_notification<T>(
        notification: lsp_server::Notification,
    ) -> Result<T::Params, lsp_server::Notification>
    where
        T: lsp_types::notification::Notification,
        T::Params: DeserializeOwned,
    {
        notification.extract(T::METHOD).map_err(|e| match e {
            ExtractError::MethodMismatch(n) => n,
            err @ ExtractError::JsonError { .. } => panic!("Invalid notification: {err:?}"),
        })
    }

    fn send_notification<N>(&self, params: N::Params)
    where
        N: lsp_types::notification::Notification,
        N::Params: Serialize,
    {
        let notification = lsp_server::Notification::new(N::METHOD.to_string(), params);
        self.connection
            .sender
            .send(Message::Notification(notification))
            .unwrap()
    }

    fn on_did_change_text_document(&self, params: DidChangeTextDocumentParams) {
        // The server capabilities asks for the full text in the message so
        // use that data here.
        let change = params.content_changes.into_iter().next().unwrap();
        self.check_document(
            params.text_document.uri,
            params.text_document.version,
            change.text,
        );
    }

    fn check_document(&self, uri: Url, version: i32, contents: String) {
        let diagnostic = analyze(contents.as_str(), &PathBuf::default()).err();
        self.notify_analyze_result(uri, version, contents, diagnostic);
    }

    fn notify_analyze_result(
        &self,
        uri: Url,
        version: i32,
        contents: String,
        diagnostic: Option<ironplc_dsl::diagnostic::Diagnostic>,
    ) {
        self.send_notification::<PublishDiagnostics>(PublishDiagnosticsParams {
            uri,
            diagnostics: diagnostic.map_or_else(Vec::new, |f| vec![map_diagnostic(f, &contents)]),
            version: Some(version),
        });
    }
}

fn map_diagnostic(
    diagnostic: ironplc_dsl::diagnostic::Diagnostic,
    contents: &str,
) -> lsp_types::Diagnostic {
    let range = map_label(diagnostic.primary, contents);
    lsp_types::Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String(diagnostic.code)),
        code_description: None,
        source: Some("ironplc".into()),
        message: diagnostic.description,
        related_information: None,
        tags: None,
        data: None,
    }
}

fn map_label(label: ironplc_dsl::diagnostic::Label, contents: &str) -> lsp_types::Range {
    // TODO this ignores the position and doesn't include secondary information

    match label.location {
        ironplc_dsl::diagnostic::Location::QualifiedPosition(qualified) => lsp_types::Range::new(
            lsp_types::Position::new(qualified.line as u32, qualified.column as u32),
            lsp_types::Position::new(qualified.line as u32, qualified.column as u32),
        ),
        ironplc_dsl::diagnostic::Location::OffsetRange(offset) => {
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

            lsp_types::Range::new(
                lsp_types::Position::new(start_line, start_offset),
                lsp_types::Position::new(end_line, end_offset),
            )
        }
    }
}
