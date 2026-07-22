//! Implements the language server protocol for integration with an IDE such
//! as Visual Studio Code.

use crossbeam_channel::{Receiver, Sender};
use ironplc_parser::options::{CompilerOptions, Dialect};
use log::{debug, trace};
use lsp_server::{Connection, ExtractError, Message, RequestId};
use lsp_types::{
    notification::{self, Notification, PublishDiagnostics},
    request::{self, Request},
    InitializeParams, OneOf, PublishDiagnosticsParams, SemanticTokens, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, SemanticTokensResult,
    SemanticTokensServerCapabilities, ServerCapabilities, TextDocumentSyncCapability,
    TextDocumentSyncKind, Uri, WorkDoneProgressOptions, WorkspaceFoldersServerCapabilities,
    WorkspaceServerCapabilities,
};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashSet;
use std::str::FromStr;

use crate::lsp_project::{LspProject, UriKey, TOKEN_TYPE_LEGEND};
use ironplc_project::disassemble;
use ironplc_project::FileBackedProject;

/// Start the LSP server.
///
/// The project is constructed after receiving `initializationOptions` from the
/// client so that parse options (e.g. the IEC 61131-3 standard version) can be
/// applied.
pub fn start() -> Result<(), String> {
    let (connection, io_threads) = Connection::stdio();
    let result = start_with_connection(connection, None);

    io_threads.join().map_err(|e| e.to_string())?;

    result
}

/// Extract parse options from LSP initialization options.
///
/// Reads `"dialect"` to select the base preset, then overlays individual
/// `--allow-*` flags.  Recognised dialect values come from
/// [`Dialect::ALL`] via `FromStr`; unknown or missing values fall back to
/// the default ([`Dialect::Iec61131_3Ed2`]).
fn extract_compiler_options(initialize_params: &InitializeParams) -> CompilerOptions {
    if let Some(ref opts) = initialize_params.initialization_options {
        let dialect = opts
            .get("dialect")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Dialect>().ok())
            .unwrap_or_default();
        debug!(
            "Using {} dialect from initializationOptions",
            dialect.display_name()
        );

        let mut options = CompilerOptions::from_dialect(dialect);

        // Individual flags override (can only enable, never disable).
        let flag = |key: &str| opts.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
        options.allow_missing_semicolon |= flag("allowMissingSemicolon");
        options.allow_empty_var_blocks |= flag("allowEmptyVarBlocks");
        options.allow_time_as_function_name |= flag("allowTimeAsFunctionName");
        options.allow_c_style_comments |= flag("allowCStyleComments");
        options.allow_top_level_var_global |= flag("allowTopLevelVarGlobal");
        options.allow_constant_type_params |= flag("allowConstantTypeParams");
        options.allow_ref_to |= flag("allowRefTo");
        options.allow_reference_to |= flag("allowReferenceTo");
        options.allow_ref_stack_variables |= flag("allowRefStackVariables");
        options.allow_ref_type_punning |= flag("allowRefTypePunning");
        options.allow_int_to_bool_initializer |= flag("allowIntToBoolInitializer");
        options.allow_sizeof |= flag("allowSizeof");
        options.allow_system_uptime_global |= flag("allowSystemUptimeGlobal");
        options.allow_cross_family_widening |= flag("allowCrossFamilyWidening");
        options.allow_partial_access_syntax |= flag("allowPartialAccessSyntax");
        options.allow_pragmas |= flag("allowPragmas");
        options
    } else {
        CompilerOptions::default()
    }
}

/// Start the LSP server using the connection for communication.
///
/// When `project_override` is `None`, the project is constructed from
/// `initializationOptions` received from the client. When `Some`, the
/// provided project is used directly (for testing).
fn start_with_connection(
    connection: Connection,
    project_override: Option<LspProject>,
) -> Result<(), String> {
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

    // Build the project — use override if provided, otherwise construct from
    // initializationOptions
    let project = match project_override {
        Some(project) => project,
        None => {
            let compiler_options = extract_compiler_options(&initialize_params);
            LspProject::with_options(
                Box::new(FileBackedProject::with_options(compiler_options)),
                compiler_options,
            )
        }
    };

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
    /// URIs for which the server has an outstanding non-empty
    /// `publishDiagnostics`. Tracking these lets the server send an
    /// empty-diagnostics notification when previously-failing files
    /// no longer have errors, so stale squiggles get cleared instead
    /// of lingering forever. Stored as `UriKey` (a `String` newtype)
    /// rather than `Uri` so the set is independent of `lsp_types::Uri`'s
    /// interior `Cell`s.
    published_uris: HashSet<UriKey>,
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
        Self {
            sender,
            project,
            published_uris: HashSet::new(),
        }
    }

    /// Run semantic analysis on the entire workspace and publish
    /// diagnostics for every affected file. Sends an empty-clear
    /// notification for every URI that previously had diagnostics
    /// but no longer does. The version field is attached only to the
    /// notification for `edited_uri` (the document that triggered
    /// this round of analysis); other URIs receive `version: None`,
    /// which the LSP spec permits and signals "out of band" to the
    /// client.
    fn publish_workspace_diagnostics(&mut self, edited_uri: &Uri, edited_version: Option<i32>) {
        let edited_key = UriKey::from_uri(edited_uri);
        let by_key = self.project.semantic_all();
        let mut new_published: HashSet<UriKey> = HashSet::new();

        for (key, diagnostics) in by_key {
            if diagnostics.is_empty() {
                continue;
            }
            let version = if key == edited_key {
                edited_version
            } else {
                None
            };
            let uri = key.to_uri();
            new_published.insert(key);
            self.send_notification::<PublishDiagnostics>(PublishDiagnosticsParams {
                uri,
                diagnostics,
                version,
            });
        }

        // Clear stale diagnostics: every URI we previously published
        // that did not appear in this round.
        let stale: Vec<UriKey> = self
            .published_uris
            .iter()
            .filter(|key| !new_published.contains(*key))
            .cloned()
            .collect();
        for key in stale {
            let version = if key == edited_key {
                edited_version
            } else {
                None
            };
            self.send_notification::<PublishDiagnostics>(PublishDiagnosticsParams {
                uri: key.to_uri(),
                diagnostics: vec![],
                version,
            });
        }

        // The edited URI must always receive a notification, even
        // when it is currently error-free and was never previously
        // published. Without this the editor would never see the
        // initial "no problems" state for a freshly-opened file and
        // the LSP test harness — which expects one notification per
        // edit — would block.
        if !new_published.contains(&edited_key) && !self.published_uris.contains(&edited_key) {
            self.send_notification::<PublishDiagnostics>(PublishDiagnosticsParams {
                uri: edited_uri.clone(),
                diagnostics: vec![],
                version: edited_version,
            });
        }

        self.published_uris = new_published;
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

    fn handle_request(&mut self, req: lsp_server::Request) -> &'static str {
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
                    let path_str = uri_to_file_path(&uri);
                    let path = std::path::Path::new(&path_str);
                    disassemble::disassemble_file(path)
                }
                Err(_) => serde_json::json!({"error": "Invalid URI"}),
            };

            let response = lsp_server::Response::new_ok(req_id, result);
            self.sender
                .send(lsp_server::Message::Response(response))
                .unwrap();
            return "ironplc/disassemble";
        }

        if _req.method == "ironplc/run" {
            let params: serde_json::Value = serde_json::from_value(_req.params).unwrap_or_default();
            let source = params["source"].as_str().unwrap_or("");
            let cycle_time_us = params["cycleTimeUs"].as_u64().unwrap_or(100_000);

            let result = self.project.run_load(source, cycle_time_us);
            let response = lsp_server::Response::new_ok(req_id, result);
            self.sender
                .send(lsp_server::Message::Response(response))
                .unwrap();
            return "ironplc/run";
        }

        if _req.method == "ironplc/step" {
            let params: serde_json::Value = serde_json::from_value(_req.params).unwrap_or_default();
            let scans = params["scans"].as_u64().unwrap_or(1) as u32;

            let result = self.project.run_step(scans);
            let response = lsp_server::Response::new_ok(req_id, result);
            self.sender
                .send(lsp_server::Message::Response(response))
                .unwrap();
            return "ironplc/step";
        }

        if _req.method == "ironplc/stop" {
            let result = self.project.run_stop();
            let response = lsp_server::Response::new_ok(req_id, result);
            self.sender
                .send(lsp_server::Message::Response(response))
                .unwrap();
            return "ironplc/stop";
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
                    trace!("DidOpenTextDocument {}", params.text_document.uri.as_str());
                    let contents = params.text_document.text;
                    let uri = params.text_document.uri;
                    let version = params.text_document.version;

                    self.project
                        .change_text_document(&uri, contents.as_str().to_string());
                    self.publish_workspace_diagnostics(&uri, Some(version));

                    return notification::DidOpenTextDocument::METHOD;
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
                    self.publish_workspace_diagnostics(&uri, Some(version));

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

/// Converts a `file:` URI to a filesystem path string.
///
/// On Windows, file URIs have the form `file:///C:/path` where the URI path
/// component is `/C:/path`. This function strips the leading `/` so the
/// result is a valid Windows path like `C:/path`.
fn uri_to_file_path(uri: &lsp_types::Uri) -> String {
    let path = uri.path().as_str();
    // On Windows, strip the leading / before the drive letter (e.g. /C:/foo -> C:/foo)
    #[cfg(windows)]
    if path.len() >= 3 && path.as_bytes()[0] == b'/' && path.as_bytes()[2] == b':' {
        return path[1..].to_string();
    }
    path.to_string()
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

    use crate::lsp_project::{LspProject, UriKey};
    use ironplc_project::{FileBackedProject, Project};

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
                start_with_connection(server_connection, Some(project)).unwrap();
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
            // `response_result` is `Ok(value)` for a successful response and
            // `Err(..)` for a failure, so `expect` asserts success here.
            let result = response
                .response_result
                .as_ref()
                .expect("Expected successful response")
                .clone();
            serde_json::from_value::<T>(result).unwrap()
        }

        fn receive_notification<T: DeserializeOwned>(&mut self) -> T {
            self.receive();
            let notification = self.notifications.pop().expect("Must have notification");
            serde_json::from_value::<T>(notification.params).unwrap()
        }

        /// Receive `n` `publishDiagnostics` notifications from the
        /// server and return them keyed by URI. The server emits one
        /// notification per affected file plus one per stale URI it
        /// is clearing, so callers know exactly how many to expect.
        /// HashMap iteration order in `semantic_all` is non-deterministic,
        /// so tests must look notifications up by URI rather than position.
        fn receive_publishes(&mut self, n: usize) -> HashMap<UriKey, PublishDiagnosticsParams> {
            let mut out = HashMap::new();
            for _ in 0..n {
                let p = self.receive_notification::<PublishDiagnosticsParams>();
                out.insert(UriKey::from_uri(&p.uri), p);
            }
            out
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
            0x00, 0x00, 0x00,  // LOAD_CONST_I32 pool[0]
            0x10, 0x00, 0x00,  // STORE_VAR_I32  var[0]
            0x8C,              // RET_VOID
        ];
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_i32_constant(42)
            .add_function(ironplc_container::FunctionId::new(0), &bytecode, 1, 1, 0)
            .build();
        let mut buf = Vec::new();
        container.write_to(&mut buf).unwrap();

        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(&buf).unwrap();
        tmp.flush().unwrap();

        let path_str = tmp.path().display().to_string().replace('\\', "/");
        let uri = if path_str.starts_with('/') {
            format!("file://{path_str}")
        } else {
            format!("file:///{path_str}")
        };
        let params = serde_json::json!({"uri": uri});
        let req_id = server.send_raw_request("ironplc/disassemble", params);
        let result: serde_json::Value = server.receive_response(req_id);

        assert_eq!(result["header"]["numFunctions"], 1);
        assert_eq!(result["constants"][0]["value"], "42");
    }

    #[test]
    fn extract_compiler_options_when_ed3_dialect_then_enables_edition_3() {
        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: None,
            root_path: None,
            root_uri: None,
            initialization_options: Some(serde_json::json!({"dialect": "iec61131-3-ed3"})),
            capabilities: ClientCapabilities::default(),
            trace: None,
            workspace_folders: None,
            client_info: None,
            locale: None,
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let options = super::extract_compiler_options(&params);
        assert!(options.allow_iec_61131_3_2013);
    }

    #[test]
    fn extract_compiler_options_when_ed2_dialect_then_uses_default() {
        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: None,
            root_path: None,
            root_uri: None,
            initialization_options: Some(serde_json::json!({"dialect": "iec61131-3-ed2"})),
            capabilities: ClientCapabilities::default(),
            trace: None,
            workspace_folders: None,
            client_info: None,
            locale: None,
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let options = super::extract_compiler_options(&params);
        assert!(!options.allow_iec_61131_3_2013);
    }

    #[test]
    fn extract_compiler_options_when_rusty_dialect_then_enables_ref_to_and_vendor_flags() {
        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: None,
            root_path: None,
            root_uri: None,
            initialization_options: Some(serde_json::json!({"dialect": "rusty"})),
            capabilities: ClientCapabilities::default(),
            trace: None,
            workspace_folders: None,
            client_info: None,
            locale: None,
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let options = super::extract_compiler_options(&params);
        assert!(!options.allow_iec_61131_3_2013);
        assert!(options.allow_ref_to);
        assert!(options.allow_c_style_comments);
        assert!(options.allow_missing_semicolon);
    }

    #[test]
    fn extract_compiler_options_when_codesys_dialect_then_enables_ref_to_without_uptime_global() {
        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: None,
            root_path: None,
            root_uri: None,
            initialization_options: Some(serde_json::json!({"dialect": "codesys"})),
            capabilities: ClientCapabilities::default(),
            trace: None,
            workspace_folders: None,
            client_info: None,
            locale: None,
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let options = super::extract_compiler_options(&params);
        assert!(!options.allow_iec_61131_3_2013);
        assert!(options.allow_ref_to);
        assert!(options.allow_c_style_comments);
        assert!(options.allow_sizeof);
        // CODESYS dialect does not pre-bind the IronPLC uptime globals.
        assert!(!options.allow_system_uptime_global);
    }

    #[test]
    fn extract_compiler_options_when_no_options_then_uses_default() {
        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: None,
            root_path: None,
            root_uri: None,
            initialization_options: None,
            capabilities: ClientCapabilities::default(),
            trace: None,
            workspace_folders: None,
            client_info: None,
            locale: None,
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let options = super::extract_compiler_options(&params);
        assert!(!options.allow_iec_61131_3_2013);
    }

    #[test]
    fn extract_compiler_options_when_allow_missing_semicolon_then_enables_flag() {
        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: None,
            root_path: None,
            root_uri: None,
            initialization_options: Some(serde_json::json!({"allowMissingSemicolon": true})),
            capabilities: ClientCapabilities::default(),
            trace: None,
            workspace_folders: None,
            client_info: None,
            locale: None,
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let options = super::extract_compiler_options(&params);
        assert!(options.allow_missing_semicolon);
    }

    #[test]
    fn extract_compiler_options_when_allow_empty_var_blocks_then_enables_flag() {
        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: None,
            root_path: None,
            root_uri: None,
            initialization_options: Some(serde_json::json!({"allowEmptyVarBlocks": true})),
            capabilities: ClientCapabilities::default(),
            trace: None,
            workspace_folders: None,
            client_info: None,
            locale: None,
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let options = super::extract_compiler_options(&params);
        assert!(options.allow_empty_var_blocks);
    }

    #[test]
    fn extract_compiler_options_when_allow_sizeof_then_enables_flag() {
        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: None,
            root_path: None,
            root_uri: None,
            initialization_options: Some(serde_json::json!({"allowSizeof": true})),
            capabilities: ClientCapabilities::default(),
            trace: None,
            workspace_folders: None,
            client_info: None,
            locale: None,
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let options = super::extract_compiler_options(&params);
        assert!(options.allow_sizeof);
    }

    #[test]
    fn extract_compiler_options_when_allow_pragmas_then_enables_flag() {
        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: None,
            root_path: None,
            root_uri: None,
            initialization_options: Some(serde_json::json!({"allowPragmas": true})),
            capabilities: ClientCapabilities::default(),
            trace: None,
            workspace_folders: None,
            client_info: None,
            locale: None,
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let options = super::extract_compiler_options(&params);
        assert!(options.allow_pragmas);
    }

    #[test]
    fn extract_compiler_options_when_allow_reference_to_then_enables_flag() {
        #[allow(deprecated)]
        let params = InitializeParams {
            process_id: None,
            root_path: None,
            root_uri: None,
            initialization_options: Some(serde_json::json!({"allowReferenceTo": true})),
            capabilities: ClientCapabilities::default(),
            trace: None,
            workspace_folders: None,
            client_info: None,
            locale: None,
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        let options = super::extract_compiler_options(&params);
        assert!(options.allow_reference_to);
    }

    #[test]
    fn lsp_when_ed3_dialect_then_accepts_ltime() {
        use ironplc_parser::options::{CompilerOptions, Dialect};

        let compiler_options = CompilerOptions::from_dialect(Dialect::Iec61131_3Ed3);
        let proj = Box::new(FileBackedProject::with_options(compiler_options));
        let mut server = TestServer::new(proj);

        // Send a document with LTIME literal (Edition 3 feature)
        server.send_notification::<notification::DidChangeTextDocument>(
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: Uri::from_str("file://example.net/test.st").unwrap(),
                    version: 1,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: String::from("PROGRAM Main\nVAR\nx : LTIME;\nEND_VAR\nEND_PROGRAM"),
                }],
            },
        );

        let diagnostics = server.receive_notification::<PublishDiagnosticsParams>();
        // LTIME should be accepted with 2013 options — no parse errors
        assert!(
            diagnostics.diagnostics.is_empty(),
            "Expected no diagnostics for LTIME with 2013 options, got: {:?}",
            diagnostics.diagnostics
        );
    }

    #[test]
    fn run_request_when_valid_source_then_returns_ok() {
        let proj = Box::new(FileBackedProject::default());
        let mut server = TestServer::new(proj);

        let params = serde_json::json!({
            "source": "PROGRAM main\nVAR\nx : DINT;\nEND_VAR\nx := 42;\nEND_PROGRAM",
            "cycleTimeUs": 100000
        });
        let req_id = server.send_raw_request("ironplc/run", params);
        let result: serde_json::Value = server.receive_response(req_id);

        assert_eq!(result["ok"], true);
        assert_eq!(result["total_scans"], 0);
    }

    #[test]
    fn run_request_when_invalid_source_then_returns_error() {
        let proj = Box::new(FileBackedProject::default());
        let mut server = TestServer::new(proj);

        let params = serde_json::json!({
            "source": "INVALID CODE",
            "cycleTimeUs": 100000
        });
        let req_id = server.send_raw_request("ironplc/run", params);
        let result: serde_json::Value = server.receive_response(req_id);

        assert_eq!(result["ok"], false);
        assert!(result["error"].as_str().is_some());
    }

    #[test]
    fn step_request_when_program_loaded_then_returns_variables() {
        let proj = Box::new(FileBackedProject::default());
        let mut server = TestServer::new(proj);

        // Load program first
        let params = serde_json::json!({
            "source": "PROGRAM main\nVAR\nx : DINT;\nEND_VAR\nx := 42;\nEND_PROGRAM",
            "cycleTimeUs": 100000
        });
        let req_id = server.send_raw_request("ironplc/run", params);
        let _: serde_json::Value = server.receive_response(req_id);

        // Step one scan
        let step_params = serde_json::json!({"scans": 1});
        let step_id = server.send_raw_request("ironplc/step", step_params);
        let result: serde_json::Value = server.receive_response(step_id);

        assert_eq!(result["ok"], true);
        assert_eq!(result["total_scans"], 1);
        let vars = result["variables"].as_array().unwrap();
        assert!(!vars.is_empty());
        assert_eq!(vars[0]["name"], "x");
        assert_eq!(vars[0]["value"], "42");
    }

    #[test]
    fn step_request_when_no_program_then_returns_error() {
        let proj = Box::new(FileBackedProject::default());
        let mut server = TestServer::new(proj);

        let params = serde_json::json!({"scans": 1});
        let req_id = server.send_raw_request("ironplc/step", params);
        let result: serde_json::Value = server.receive_response(req_id);

        assert_eq!(result["ok"], false);
        assert!(result["error"]
            .as_str()
            .unwrap()
            .contains("No program loaded"));
    }

    #[test]
    fn stop_request_when_program_running_then_clears_session() {
        let proj = Box::new(FileBackedProject::default());
        let mut server = TestServer::new(proj);

        // Load and step
        let params = serde_json::json!({
            "source": "PROGRAM main\nVAR\nx : DINT;\nEND_VAR\nx := 42;\nEND_PROGRAM",
            "cycleTimeUs": 100000
        });
        let req_id = server.send_raw_request("ironplc/run", params);
        let _: serde_json::Value = server.receive_response(req_id);

        // Stop
        let stop_id = server.send_raw_request("ironplc/stop", serde_json::json!({}));
        let result: serde_json::Value = server.receive_response(stop_id);
        assert_eq!(result["ok"], true);

        // Step should fail now
        let step_id = server.send_raw_request("ironplc/step", serde_json::json!({"scans": 1}));
        let result: serde_json::Value = server.receive_response(step_id);
        assert_eq!(result["ok"], false);
    }

    #[test]
    fn lsp_when_default_options_then_demotes_ltime_to_identifier() {
        let proj = Box::new(FileBackedProject::default());
        let mut server = TestServer::new(proj);

        // Send a document with LTIME used as an identifier (Edition 3 keyword demoted)
        server.send_notification::<notification::DidChangeTextDocument>(
            DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier {
                    uri: Uri::from_str("file://example.net/test.st").unwrap(),
                    version: 1,
                },
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text: String::from("PROGRAM Main\nVAR\nx : LTIME;\nEND_VAR\nEND_PROGRAM"),
                }],
            },
        );

        let diagnostics = server.receive_notification::<PublishDiagnosticsParams>();
        // LTIME should be demoted to an identifier with default (2003) options,
        // so no tokenization-level diagnostics are produced
        assert!(
            diagnostics.diagnostics.is_empty(),
            "Expected no diagnostics for LTIME with default options (demoted to identifier)"
        );
    }

    fn change_doc(uri: &Uri, version: i32, text: &str) -> DidChangeTextDocumentParams {
        DidChangeTextDocumentParams {
            text_document: VersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version,
            },
            content_changes: vec![TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: text.to_string(),
            }],
        }
    }

    #[test]
    fn multi_file_when_two_broken_files_opened_then_publishes_for_each_uri() {
        // Reproduces the user-reported bug: with two files in the
        // workspace, both broken, the LSP must publish diagnostics for
        // each URI rather than dropping the unedited file's errors.
        let proj = Box::new(FileBackedProject::default());
        let mut server = TestServer::new(proj);

        let uri_a = Uri::from_str("file:///workspace/a.st").unwrap();
        let uri_b = Uri::from_str("file:///workspace/b.st").unwrap();
        let key_a = UriKey::from_uri(&uri_a);
        let key_b = UriKey::from_uri(&uri_b);

        // Open file A — broken.
        server.send_notification::<notification::DidChangeTextDocument>(change_doc(
            &uri_a,
            1,
            "this is not valid IEC 61131-3 in file a",
        ));
        // After this, only A is in the project so we expect one publish for A.
        let n = server.receive_publishes(1);
        assert!(n.contains_key(&key_a));
        assert!(!n[&key_a].diagnostics.is_empty());

        // Open file B — also broken. Server now re-analyses the
        // workspace and publishes per-file: one for A, one for B.
        server.send_notification::<notification::DidChangeTextDocument>(change_doc(
            &uri_b,
            1,
            "this is not valid IEC 61131-3 in file b",
        ));
        let n = server.receive_publishes(2);
        assert!(
            n.contains_key(&key_a),
            "expected publish for url_a, got {:?}",
            n.keys().collect::<Vec<_>>()
        );
        assert!(
            n.contains_key(&key_b),
            "expected publish for url_b, got {:?}",
            n.keys().collect::<Vec<_>>()
        );
        assert!(
            !n[&key_a].diagnostics.is_empty(),
            "url_a should still report its errors after url_b is opened"
        );
        assert!(!n[&key_b].diagnostics.is_empty());

        // Version is attached only to the URI we actually edited.
        assert_eq!(n[&key_b].version, Some(1));
        assert!(n[&key_a].version.is_none());
    }

    #[test]
    fn multi_file_when_error_fixed_in_second_file_then_clearing_diagnostics_published() {
        // Open A (broken) and B (broken), then fix B. The server must
        // emit a clearing publish (empty diagnostics) for B so the IDE
        // erases the stale squiggle, while still re-publishing A's
        // outstanding errors.
        let proj = Box::new(FileBackedProject::default());
        let mut server = TestServer::new(proj);

        let uri_a = Uri::from_str("file:///workspace/a.st").unwrap();
        let uri_b = Uri::from_str("file:///workspace/b.st").unwrap();
        let key_a = UriKey::from_uri(&uri_a);
        let key_b = UriKey::from_uri(&uri_b);

        server.send_notification::<notification::DidChangeTextDocument>(change_doc(
            &uri_a,
            1,
            "this is not valid IEC 61131-3 a",
        ));
        let _ = server.receive_publishes(1);

        server.send_notification::<notification::DidChangeTextDocument>(change_doc(
            &uri_b,
            1,
            "this is not valid IEC 61131-3 b",
        ));
        let _ = server.receive_publishes(2);

        // Fix B by replacing it with a valid program.
        server.send_notification::<notification::DidChangeTextDocument>(change_doc(
            &uri_b,
            2,
            "PROGRAM Main\nVAR\n  x : BOOL;\nEND_VAR\nEND_PROGRAM",
        ));

        // Expect one publish for A (still broken) plus one clearing
        // publish for B.
        let n = server.receive_publishes(2);
        assert!(n.contains_key(&key_a), "expected publish for url_a");
        assert!(n.contains_key(&key_b), "expected clear for url_b");
        assert!(!n[&key_a].diagnostics.is_empty());
        assert!(
            n[&key_b].diagnostics.is_empty(),
            "url_b's clear notification must carry an empty diagnostics list, got {:?}",
            n[&key_b].diagnostics
        );
    }

    #[test]
    fn single_file_when_error_introduced_then_cleared_then_clearing_notification_emitted() {
        // Regression guard for the same-file case: introducing then
        // fixing an error in one file should still produce a clearing
        // publish so the squiggle goes away.
        let proj = Box::new(FileBackedProject::default());
        let mut server = TestServer::new(proj);

        let uri = Uri::from_str("file:///workspace/only.st").unwrap();
        let key = UriKey::from_uri(&uri);

        // Broken first.
        server.send_notification::<notification::DidChangeTextDocument>(change_doc(
            &uri,
            1,
            "this is not valid IEC 61131-3 source",
        ));
        let n = server.receive_publishes(1);
        assert!(!n[&key].diagnostics.is_empty());

        // Fix it.
        server.send_notification::<notification::DidChangeTextDocument>(change_doc(
            &uri,
            2,
            "PROGRAM Main\nVAR\n  x : BOOL;\nEND_VAR\nEND_PROGRAM",
        ));
        let n = server.receive_publishes(1);
        assert!(
            n[&key].diagnostics.is_empty(),
            "fixing the only file should emit a clear publish, got {:?}",
            n[&key].diagnostics
        );
        // The clear is in response to the edit, so the version should
        // match the edit that produced it.
        assert_eq!(n[&key].version, Some(2));
    }

    #[test]
    fn multi_file_when_clean_file_opened_after_broken_file_then_each_publish_targets_its_uri() {
        // After a broken file is open, opening a *clean* second file
        // must not silently inherit or hide the broken one. The clean
        // file gets an empty publish; the broken one gets a re-publish
        // of its errors.
        let proj = Box::new(FileBackedProject::default());
        let mut server = TestServer::new(proj);

        let broken = Uri::from_str("file:///workspace/broken.st").unwrap();
        let clean = Uri::from_str("file:///workspace/clean.st").unwrap();
        let broken_key = UriKey::from_uri(&broken);
        let clean_key = UriKey::from_uri(&clean);

        server.send_notification::<notification::DidChangeTextDocument>(change_doc(
            &broken,
            1,
            "this is not valid IEC 61131-3 syntax",
        ));
        let _ = server.receive_publishes(1);

        server.send_notification::<notification::DidChangeTextDocument>(change_doc(
            &clean,
            1,
            "PROGRAM Clean\nVAR\n  x : BOOL;\nEND_VAR\nEND_PROGRAM",
        ));

        // Expect one publish for the broken file (re-published), and
        // one empty publish for the clean file (so the editor knows
        // the clean file has no problems).
        let n = server.receive_publishes(2);
        assert!(n.contains_key(&broken_key));
        assert!(n.contains_key(&clean_key));
        assert!(!n[&broken_key].diagnostics.is_empty());
        assert!(n[&clean_key].diagnostics.is_empty());
    }
}
