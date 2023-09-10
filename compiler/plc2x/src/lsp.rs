//! Implements the language server protocol for integration with an IDE such
//! as Visual Studio Code.

use log::trace;
use lsp_server::{Connection, ExtractError, Message};
use lsp_types::{
    notification::{self, PublishDiagnostics},
    DiagnosticSeverity, NumberOrString, PublishDiagnosticsParams, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncKind, Url,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::project::Project;

/// Start the LSP server with the specified project as the context.
pub fn start(project: Box<dyn Project>) -> Result<(), String> {
    let (connection, io_threads) = Connection::stdio();
    let result = start_with_connection(connection, project);

    io_threads.join().map_err(|e| e.to_string())?;

    result
}

/// Start the LSP server using the connection for communication.
fn start_with_connection(connection: Connection, project: Box<dyn Project>) -> Result<(), String> {
    let server_capabilities =
        serde_json::to_value(LspServer::server_capabilities()).map_err(|e| e.to_string())?;
    connection
        .initialize(server_capabilities)
        .map_err(|e| e.to_string())?;

    let server = LspServer::new(connection, project);
    server.main_loop()?;
    Ok(())
}

struct LspServer {
    connection: Connection,
    project: Box<dyn Project>,
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

    fn new(connection: Connection, project: Box<dyn Project>) -> Self {
        Self {
            connection,
            project,
        }
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
                Ok(params) => {
                    trace!("DidChangeTextDocument {}", params.text_document.uri);
                    let contents = params.content_changes.into_iter().next().unwrap().text;
                    let uri = params.text_document.uri;
                    let version = params.text_document.version;

                    let diagnostics = self.project.on_did_change_text_document(
                        &String::from(uri.as_str()),
                        contents.as_str(),
                    );

                    self.notify_analyze_result(uri, version, contents, diagnostics);
                    return;
                }
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

/// Convert diagnostic type into the LSP diagnostic type.
fn map_diagnostic(
    diagnostic: ironplc_dsl::diagnostic::Diagnostic,
    contents: &str,
) -> lsp_types::Diagnostic {
    let description = diagnostic.description();
    let range = map_label(&diagnostic.primary, contents);
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
fn map_label(label: &ironplc_dsl::diagnostic::Label, contents: &str) -> lsp_types::Range {
    match &label.location {
        ironplc_dsl::diagnostic::Location::QualifiedPosition(qualified) => lsp_types::Range::new(
            lsp_types::Position::new((qualified.line - 1) as u32, (qualified.column - 1) as u32),
            lsp_types::Position::new((qualified.line - 1) as u32, (qualified.column - 1) as u32),
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

#[cfg(test)]
mod test {
    use core::time::Duration;
    use ironplc_dsl::core::FileId;
    use ironplc_dsl::diagnostic::{Diagnostic, Label};
    use ironplc_problems::Problem;
    use lsp_server::{Connection, Message, RequestId};
    use lsp_server::{Notification, Response};
    use lsp_types::notification;
    use lsp_types::DidChangeTextDocumentParams;
    use lsp_types::Url;
    use lsp_types::VersionedTextDocumentIdentifier;
    use lsp_types::{
        request, ClientCapabilities, InitializeParams, InitializeResult, InitializedParams,
        PublishDiagnosticsParams, TextDocumentContentChangeEvent,
    };
    use serde::de::DeserializeOwned;
    use serde::Serialize;
    use std::collections::HashMap;

    use crate::project::Project;

    use super::start_with_connection;

    /// A test double for a real project. The mock returns expected responses
    /// based on input parameters.
    struct MockProject {}

    impl Project for MockProject {
        fn on_did_change_text_document(
            &self,
            _: &ironplc_dsl::core::FileId,
            _: &str,
        ) -> Option<Diagnostic> {
            Some(
                Diagnostic::problem(
                    // Just an arbitrary error
                    Problem::OpenComment,
                    Label::offset(FileId::default(), 0..0, "First location"),
                )
                .with_secondary(Label::offset(
                    FileId::default(),
                    1..1,
                    "Second place",
                )),
            )
        }
    }

    struct TestServer {
        server_thread: Option<std::thread::JoinHandle<()>>,
        client_connection: Connection,
        request_id_counter: i32,

        responses: HashMap<RequestId, Response>,
        notifications: Vec<Notification>,
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.send_request::<request::Shutdown>(Default::default());
            self.send_notification::<notification::Exit>(Default::default());

            if let Some(server_thread) = self.server_thread.take() {
                server_thread.join().unwrap();
            }
        }
    }

    impl TestServer {
        #[allow(deprecated)]
        fn new(project: Box<dyn Project + Send>) -> Self {
            let (server_connection, client_connection) = Connection::memory();

            let server_thread = std::thread::spawn(|| {
                start_with_connection(server_connection, project).unwrap();
            });

            let mut server = Self {
                server_thread: Some(server_thread),
                client_connection,
                request_id_counter: 0,
                responses: HashMap::new(),
                notifications: Vec::new(),
            };

            let init = InitializeParams {
                process_id: None,
                root_path: None,
                root_uri: None,
                initialization_options: None,
                capabilities: ClientCapabilities {
                    workspace: None,
                    text_document: None,
                    window: None,
                    general: None,
                    experimental: None,
                },
                trace: None,
                workspace_folders: None,
                client_info: None,
                locale: None,
            };

            let initialize_id = server.send_request::<request::Initialize>(init);
            server.receive_response::<InitializeResult>(initialize_id);

            server.send_notification::<notification::Initialized>(InitializedParams {});

            server
        }

        fn send_request<N>(&mut self, params: N::Params) -> RequestId
        where
            N: lsp_types::request::Request,
            N::Params: Serialize,
        {
            self.request_id_counter += 1;
            let message = lsp_server::Request::new(
                RequestId::from(self.request_id_counter),
                N::METHOD.to_string(),
                params,
            );
            self.client_connection
                .sender
                .send(Message::Request(message))
                .unwrap();
            RequestId::from(self.request_id_counter)
        }

        /// Sends a message from the client, such as VSCode, to the server.
        fn send_notification<N>(&mut self, params: N::Params)
        where
            N: lsp_types::notification::Notification,
            N::Params: Serialize,
        {
            let message = lsp_server::Notification::new(N::METHOD.to_string(), params);
            self.client_connection
                .sender
                .send(Message::Notification(message))
                .unwrap();
        }

        fn receive(&mut self) {
            let timeout = Duration::from_secs(60);
            let message = self
                .client_connection
                .receiver
                .recv_timeout(timeout)
                .unwrap();

            match message {
                Message::Request(_) => panic!(),
                Message::Response(response) => {
                    let id = response.id.clone();
                    self.responses.insert(id, response);
                }
                Message::Notification(notification) => {
                    self.notifications.push(notification);
                }
            }
        }

        fn receive_response<T: DeserializeOwned>(&mut self, request_id: RequestId) -> T {
            self.receive();
            let response = self.responses.get(&request_id).expect("No request");
            let value = response.result.clone().unwrap();
            serde_json::from_value::<T>(value).unwrap()
        }

        fn receive_notification<T: DeserializeOwned>(&mut self) -> T {
            self.receive();
            let notification = self.notifications.pop().expect("Must have notification");
            serde_json::from_value::<T>(notification.params).unwrap()
        }
    }

    #[test]
    fn text_document_changed_then_returns_diagnostics() {
        let proj = Box::new(MockProject {});
        let mut server = TestServer::new(proj);
        server.send_notification::<notification::DidChangeTextDocument>(
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: Url::parse("file://example.net/a/b.html").unwrap(),
                    version: 1,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: String::from("this is some text"),
                }],
            },
        );

        server.receive_notification::<PublishDiagnosticsParams>();
    }
}
