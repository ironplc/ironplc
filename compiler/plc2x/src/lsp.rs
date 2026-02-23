//! Implements the language server protocol for integration with an IDE such
//! as Visual Studio Code.

use crossbeam_channel::{Receiver, Sender};
use log::{debug, trace};
use lsp_server::{Connection, ExtractError, Message, RequestId};
use lsp_types::{
    notification::{self, Notification, PublishDiagnostics},
    request::{self, Request},
    InitializeParams, OneOf, PublishDiagnosticsParams, SemanticTokens, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, WorkDoneProgressOptions, WorkspaceFoldersServerCapabilities,
    WorkspaceServerCapabilities,
};
use serde::{de::DeserializeOwned, Serialize};
use std::str::FromStr;

use crate::lsp_project::{LspProject, TOKEN_TYPE_LEGEND};

/// Start the LSP server with the specified project as the context.
pub fn start(project: LspProject) -> Result<(), String> {
    let (connection, io_threads) = Connection::stdio();
    let result = start_with_connection(connection, project);

    io_threads.join().map_err(|e| e.to_string())?;

    result
}

/// Start the LSP server using the connection for communication.
fn start_with_connection(connection: Connection, project: LspProject) -> Result<(), String> {
    // Declare what capabilities this server supports
    let server_capabilities =
        serde_json::to_value(LspServer::server_capabilities()).map_err(|e| e.to_string())?;

    // Send the capabilities to the client and receive back the initialization.
    let initialize_params = connection
        .initialize(server_capabilities)
        .map_err(|e| e.to_string())?;

    // Configure a project based on the initialize params
    let initialize_params: InitializeParams =
        serde_json::from_value(initialize_params).map_err(|e| e.to_string())?;

    let mut server = LspServer::new(&connection.sender, project);

    match initialize_params.workspace_folders {
        Some(folders) => {
            debug!("Initialize server with workspace folders {folders:?}");
            if let Some(folder) = folders.first() {
                server.project.initialize(folder);
            }
        }
        None => {
            debug!("Initialize server without a workspace folder");
        }
    }

    match server.run(&connection.receiver) {
        Ok(shutdown_request) => connection
            .handle_shutdown(&shutdown_request)
            .map(|_v| ())
            .map_err(|err| err.to_string()),
        Err(err) => Err(err),
    }
}

struct LspServer<'a> {
    sender: &'a Sender<Message>,
    project: LspProject,
}

impl<'a> LspServer<'a> {
    /// Returns the set of capabilities that this language server supports.
    ///
    /// This effectively declares to the other end of the channel what we can
    /// do.
    fn server_capabilities() -> ServerCapabilities {
        ServerCapabilities {
            text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
            semantic_tokens_provider: Some(
                SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
                    // We don't report progress in generating tokens so
                    // there is no work to report on
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                    legend: SemanticTokensLegend {
                        token_types: TOKEN_TYPE_LEGEND.into(),
                        token_modifiers: vec![],
                    },
                    range: None,
                    full: Some(SemanticTokensFullOptions::Bool(true)),
                }),
            ),
            workspace: Some(WorkspaceServerCapabilities {
                workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                    supported: Some(true),
                    change_notifications: None,
                }),
                file_operations: None,
            }),
            document_symbol_provider: Some(OneOf::Left(true)),
            ..ServerCapabilities::default()
        }
    }

    fn new(sender: &'a Sender<Message>, project: LspProject) -> Self {
        Self { sender, project }
    }

    /// The main event loop. The event loop receives messages from the other
    /// end of the channel.
    fn run(&mut self, receiver: &Receiver<Message>) -> Result<lsp_server::Request, String> {
        for msg in receiver {
            match msg {
                lsp_server::Message::Request(req) => {
                    if req.method == request::Shutdown::METHOD {
                        return Ok(req);
                    }
                    self.handle_request(req);
                }
                lsp_server::Message::Response(_) => {
                    // LSP responses are typically handled by the client, not the server
                    // For now, we just ignore them
                }
                lsp_server::Message::Notification(notification) => {
                    self.handle_notification(notification);
                }
            }
        }

        Err("terminated but no shutdown".to_owned())
    }

    fn handle_request(&self, req: lsp_server::Request) -> &'static str {
        let req_id = req.id.clone();
        let req = match Self::cast_request::<request::Shutdown>(req) {
            Ok(_params) => {
                return request::Shutdown::METHOD;
            }
            Err(req) => req,
        };
        let req = match Self::cast_request::<request::SemanticTokensFullRequest>(req) {
            Ok(params) => {
                let uri = params.text_document.uri;
                let token_result = self.project.tokenize(&uri);

                match token_result {
                    Ok(tokens) => {
                        trace!("SemanticTokensFullRequest Success Response {tokens:?}");
                        self.send_response::<request::SemanticTokensFullRequest>(
                            req_id,
                            Some(SemanticTokensResult::Tokens(SemanticTokens {
                                result_id: None,
                                data: tokens,
                            })),
                        );
                    }
                    Err(diagnostic) => {
                        trace!("SemanticTokensFullRequest Error Response {diagnostic:?}");
                        self.send_response::<request::SemanticTokensFullRequest>(req_id, None);
                    }
                }

                return request::SemanticTokensFullRequest::METHOD;
            }
            Err(req) => req,
        };
        let _req = match Self::cast_request::<request::DocumentSymbolRequest>(req) {
            Ok(params) => {
                let uri = params.text_document.uri;
                let symbols = self.project.document_symbols(&uri);

                trace!("DocumentSymbolRequest Response {symbols:?}");
                self.send_response::<request::DocumentSymbolRequest>(req_id, Some(symbols));

                return request::DocumentSymbolRequest::METHOD;
            }
            Err(req) => req,
        };

        // Handle custom requests by method name
        if _req.method == "ironplc/disassemble" {
            let params: serde_json::Value = serde_json::from_value(_req.params).unwrap_or_default();
            let uri_str = params["uri"].as_str().unwrap_or("");

            let result = match lsp_types::Uri::from_str(uri_str) {
                Ok(uri) => {
                    let path_str = uri.path().to_string();
                    let path = std::path::Path::new(&path_str);
                    crate::disassemble::disassemble_file(path)
                }
                Err(_) => serde_json::json!({"error": "Invalid URI"}),
            };

            let response = lsp_server::Response::new_ok(req_id, result);
            self.sender
                .send(lsp_server::Message::Response(response))
                .unwrap();
            return "ironplc/disassemble";
        }

        ""
    }

    fn cast_request<T>(request: lsp_server::Request) -> Result<T::Params, lsp_server::Request>
    where
        T: lsp_types::request::Request,
        T::Params: DeserializeOwned,
    {
        request
            .extract(T::METHOD)
            .map(|val| val.1)
            .map_err(|e| match e {
                ExtractError::MethodMismatch(n) => n,
                err @ ExtractError::JsonError { .. } => panic!("Invalid request: {err:?}"),
            })
    }

    fn send_response<R>(&self, request_id: RequestId, params: R::Result)
    where
        R: lsp_types::request::Request,
        R::Result: Serialize,
    {
        trace!("Response for method {}", R::METHOD);
        let response = lsp_server::Response::new_ok(request_id, params);
        self.sender.send(Message::Response(response)).unwrap()
    }

    fn handle_notification(&mut self, notification: lsp_server::Notification) -> &'static str {
        let notification = match Self::cast_notification::<notification::Exit>(notification) {
            Ok(_params) => {
                return notification::Exit::METHOD;
            }
            Err(notification) => notification,
        };

        let notification =
            match Self::cast_notification::<notification::DidOpenTextDocument>(notification) {
                Ok(params) => {
                    trace!(
                        "DidChangeTextDocument {}",
                        params.text_document.uri.as_str()
                    );
                    let contents = params.text_document.text;
                    let uri = params.text_document.uri;
                    let version = params.text_document.version;

                    self.project
                        .change_text_document(&uri, contents.as_str().to_string());
                    let diagnostics = self.project.semantic(&uri);

                    self.send_notification::<PublishDiagnostics>(PublishDiagnosticsParams {
                        uri,
                        diagnostics,
                        version: Some(version),
                    });

                    return notification::DidChangeTextDocument::METHOD;
                }
                Err(notification) => notification,
            };

        let _notification =
            match Self::cast_notification::<notification::DidChangeTextDocument>(notification) {
                Ok(params) => {
                    trace!(
                        "DidChangeTextDocument {}",
                        params.text_document.uri.as_str()
                    );
                    let contents = params.content_changes.into_iter().next().unwrap().text;
                    let uri = params.text_document.uri;
                    let version = params.text_document.version;

                    self.project
                        .change_text_document(&uri, contents.as_str().to_string());
                    let diagnostics = self.project.semantic(&uri);

                    self.send_notification::<PublishDiagnostics>(PublishDiagnosticsParams {
                        uri,
                        diagnostics,
                        version: Some(version),
                    });

                    return notification::DidChangeTextDocument::METHOD;
                }
                Err(notification) => notification,
            };

        ""
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
        self.sender
            .send(Message::Notification(notification))
            .unwrap()
    }
}

#[cfg(test)]
mod test {
    use core::time::Duration;
    use lsp_server::{Connection, Message, RequestId};
    use lsp_server::{Notification, Response};
    use lsp_types::DidChangeTextDocumentParams;
    use lsp_types::Uri;
    use lsp_types::VersionedTextDocumentIdentifier;
    use lsp_types::{notification, WorkDoneProgressParams};
    use lsp_types::{
        request, ClientCapabilities, InitializeParams, InitializeResult, InitializedParams,
        PublishDiagnosticsParams, TextDocumentContentChangeEvent,
    };
    use serde::de::DeserializeOwned;
    use serde::Serialize;
    use std::collections::HashMap;
    use std::str::FromStr;

    use crate::lsp_project::LspProject;
    use crate::project::{FileBackedProject, Project};

    use super::start_with_connection;

    struct TestServer {
        server_thread: Option<std::thread::JoinHandle<()>>,
        client_connection: Connection,
        request_id_counter: i32,

        responses: HashMap<RequestId, Response>,
        notifications: Vec<Notification>,
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            self.send_request::<request::Shutdown>(());
            self.send_notification::<notification::Exit>(());

            if let Some(server_thread) = self.server_thread.take() {
                server_thread.join().unwrap();
            }
        }
    }

    impl TestServer {
        #[allow(deprecated)]
        fn new(project: Box<dyn Project + Send>) -> Self {
            let project = LspProject::new(project);
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
                    notebook_document: None,
                },
                trace: None,
                workspace_folders: None,
                client_info: None,
                locale: None,
                work_done_progress_params: WorkDoneProgressParams {
                    work_done_token: None,
                },
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

        fn send_raw_request(&mut self, method: &str, params: serde_json::Value) -> RequestId {
            self.request_id_counter += 1;
            let message = lsp_server::Request {
                id: RequestId::from(self.request_id_counter),
                method: method.to_string(),
                params: serde_json::to_value(params).unwrap(),
            };
            self.client_connection
                .sender
                .send(Message::Request(message))
                .unwrap();
            RequestId::from(self.request_id_counter)
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
        let proj = Box::new(FileBackedProject::default());
        let mut server = TestServer::new(proj);
        server.send_notification::<notification::DidChangeTextDocument>(
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: Uri::from_str("file://example.net/a/b.html").unwrap(),
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

    #[test]
    fn disassemble_request_when_valid_iplc_file_then_returns_json() {
        use ironplc_container::ContainerBuilder;
        use std::io::Write;

        let proj = Box::new(FileBackedProject::default());
        let mut server = TestServer::new(proj);

        // Build a small .iplc file
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x01, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
            0x18, 0x00, 0x00,  // STORE_VAR_I32  var[0]
            0xB5,              // RET_VOID
        ];
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_i32_constant(42)
            .add_function(0, &bytecode, 1, 1)
            .build();
        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();

        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(&buf).unwrap();
        tmp.flush().unwrap();

        let uri = format!("file://{}", tmp.path().display());
        let params = serde_json::json!({"uri": uri});
        let req_id = server.send_raw_request("ironplc/disassemble", params);
        let result: serde_json::Value = server.receive_response(req_id);

        assert_eq!(result["header"]["numFunctions"], 1);
        assert_eq!(result["constants"][0]["value"], "42");
    }
}
