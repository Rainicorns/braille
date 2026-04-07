pub mod worker_protocol;

use serde::{Deserialize, Serialize};

// --- Browser Events ---

/// A browser event that needs agent attention.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BrowserEvent {
    pub id: u64,
    pub kind: BrowserEventKind,
    pub timestamp_ms: u64,
}

/// The kind of browser event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BrowserEventKind {
    // Blocking — JS execution is paused until agent responds
    Alert { message: String },
    Confirm { message: String },
    Prompt { message: String, default_value: Option<String> },

    // Actionable — agent can choose to act on these
    Download { url: String, filename: String, mime_type: Option<String> },
    WindowOpen { url: String, target: String },
    GeolocationRequest,
    ClipboardWrite { text: String },
    NotificationRequest { title: String, body: Option<String> },
    FullscreenRequest,
    PrintRequest,
    MediaPlayAttempt { src: String },

    // Info-only — logged for agent visibility
    CspViolation { directive: String, blocked_uri: String },
    CorsViolation { url: String, origin: String },
    CertWarning { url: String, reason: String },
    ServiceWorkerRegister { url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Command {
    Goto { url: String },
    Click { selector: String },
    Type { selector: String, text: String },
    Select { selector: String, value: String },
    Focus { selector: String },
    Snap { mode: SnapMode },
    Back,
    Forward,
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum SnapMode {
    /// Compact text + interactive elements — token-efficient, the default for LLM agents.
    #[default]
    Compact,
    /// Full accessibility tree with roles, indentation, and element hierarchy.
    Accessibility,
    Interactive,
    Links,
    Forms,
    Headings,
    Text,
    Selector(String),
    Region(String),
    Dom,
    Markdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Response {
    SessionCreated { session_id: String },
    Snapshot { content: String, url: String },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NavigateRequest {
    pub url: String,
    pub method: HttpMethod,
    pub body: Option<String>,
    pub content_type: Option<String>,
    /// Custom HTTP headers (e.g., Authorization, User-Agent overrides).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EngineAction {
    None,
    Navigate(NavigateRequest),
    Error(String),
}

/// A pending fetch request from the engine's JS runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FetchRequest {
    pub id: u64,
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

/// Response data to resolve a pending fetch request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FetchResponseData {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub url: String,
    /// Redirect hops followed to reach this response. Empty if no redirects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub redirect_chain: Vec<RedirectHop>,
}

/// A single HTTP redirect hop within a fetch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RedirectHop {
    pub status: u16,
    pub url: String,
    pub location: String,
    pub set_cookies: Vec<String>,
}

// --- Engine REPL protocol types ---

/// Message sent from the host (CLI) to the engine process over stdin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HostMessage {
    /// Execute a command.
    Command(DaemonCommand),
    /// Here are the HTTP responses you asked for.
    FetchResults(Vec<FetchResult>),
    /// A worker process was successfully spawned.
    WorkerSpawned { worker_id: u64 },
    /// A message from a worker process to the main engine.
    WorkerMessage { worker_id: u64, data: String },
    /// A worker process encountered an error.
    WorkerError { worker_id: u64, error: String },
    /// A worker process has exited.
    WorkerExited { worker_id: u64 },
    /// Request the engine to prepare for checkpointing.
    PrepareCheckpoint,
    /// A worker has been restored after checkpoint (on session restore).
    WorkerRestored { worker_id: u64, url: String },
    /// Response to a blocking browser event from the host/agent.
    EventResponse { id: u64, value: String },
}

/// Message sent from the engine process to the host (CLI) over stdout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EngineMessage {
    /// I need these URLs fetched.
    NeedFetch(Vec<FetchRequest>),
    /// Here's the final result.
    CommandResult(DaemonResponse),
    /// Request the host to spawn a worker process.
    SpawnWorker { worker_id: u64, url: String },
    /// Post a message to a worker process.
    PostToWorker { worker_id: u64, data: String },
    /// Terminate a worker process.
    TerminateWorker { worker_id: u64 },
    /// Engine is ready for checkpointing; here are the active workers.
    CheckpointReady { active_workers: Vec<WorkerDescriptor> },
    /// Pending browser events for the agent.
    BrowserEvents(Vec<BrowserEvent>),
}

/// Descriptor for an active worker (used during checkpoint/restore).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerDescriptor {
    pub id: u64,
    pub url: String,
}

/// Result of a single fetch request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FetchResult {
    pub id: u64,
    pub outcome: FetchOutcome,
}

/// Whether a fetch succeeded or failed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FetchOutcome {
    Ok(FetchResponseData),
    Err(String),
}

// --- Daemon IPC types ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonRequest {
    pub session_id: Option<String>,
    pub command: DaemonCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DaemonCommand {
    NewSession,
    Goto {
        url: String,
        mode: SnapMode,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        record_path: Option<String>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        clean: bool,
    },
    Click { selector: String },
    Type { selector: String, text: String },
    Select { selector: String, value: String },
    Snap { mode: SnapMode },
    Back,
    Forward,
    Console,
    Eval { code: String },
    Mark { label: String },
    Close,
    Ping,
    DaemonStop,
    /// List pending browser events.
    Events,
    /// Respond to a blocking browser event (alert/confirm/prompt).
    RespondEvent { id: u64, value: String },
    /// Grant a permission.
    Permit { permission: String },
    /// Deny a permission.
    Deny { permission: String },
    /// Dismiss an info-only browser event.
    DismissEvent { id: u64 },
    /// Export all cookies from the session's cookie jar as JSON.
    ExportCookies,
    /// Import cookies into the session's cookie jar.
    ImportCookies { cookies: Vec<SerializableCookie> },
}

/// A cookie that can be serialized/deserialized over the wire protocol.
/// Mirrors the engine's internal StoredCookie but is serde-friendly for IPC.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerializableCookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    pub http_only: bool,
    pub secure: bool,
    /// Expiry as milliseconds since epoch, or None for session cookies.
    pub expires_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonResponse {
    pub success: bool,
    pub session_id: Option<String>,
    pub content: Option<String>,
    pub error: Option<String>,
    /// Console output (log/warn/error) captured since last command.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub console: Vec<String>,
    /// HTTP status code from the upstream response, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
}

impl DaemonResponse {
    pub fn ok(content: String) -> Self {
        DaemonResponse {
            success: true,
            session_id: None,
            content: Some(content),
            error: None,
            console: Vec::new(),
            status_code: None,
        }
    }

    pub fn ok_with_session(session_id: String, content: Option<String>) -> Self {
        DaemonResponse {
            success: true,
            session_id: Some(session_id),
            content,
            error: None,
            console: Vec::new(),
            status_code: None,
        }
    }

    pub fn err(message: String) -> Self {
        DaemonResponse {
            success: false,
            session_id: None,
            content: None,
            error: Some(message),
            console: Vec::new(),
            status_code: None,
        }
    }

    pub fn with_console(mut self, console: Vec<String>) -> Self {
        self.console = console;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_roundtrip {
        ($val:expr, $ty:ty) => {
            let val = $val;
            let json = serde_json::to_string(&val).unwrap();
            let deserialized: $ty = serde_json::from_str(&json).unwrap();
            assert_eq!(val, deserialized);
        };
    }

    #[test]
    fn command_goto_roundtrip() {
        assert_roundtrip!(Command::Goto { url: "https://example.com".into() }, Command);
    }

    #[test]
    fn response_snapshot_roundtrip() {
        assert_roundtrip!(
            Response::Snapshot { content: "<h1>Hello</h1>".into(), url: "https://example.com".into() },
            Response
        );
    }

    #[test]
    fn snap_mode_accessibility_roundtrip() {
        assert_roundtrip!(SnapMode::Accessibility, SnapMode);
    }

    #[test]
    fn snap_mode_dom_roundtrip() {
        assert_roundtrip!(SnapMode::Dom, SnapMode);
    }

    #[test]
    fn snap_mode_markdown_roundtrip() {
        assert_roundtrip!(SnapMode::Markdown, SnapMode);
    }

    #[test]
    fn command_select_roundtrip() {
        assert_roundtrip!(Command::Select { selector: "#country".into(), value: "USA".into() }, Command);
    }

    #[test]
    fn command_focus_roundtrip() {
        assert_roundtrip!(Command::Focus { selector: "#search-input".into() }, Command);
    }

    #[test]
    fn http_method_get_roundtrip() {
        assert_roundtrip!(HttpMethod::Get, HttpMethod);
    }

    #[test]
    fn http_method_post_roundtrip() {
        assert_roundtrip!(HttpMethod::Post, HttpMethod);
    }

    #[test]
    fn navigate_request_get_roundtrip() {
        assert_roundtrip!(
            NavigateRequest { url: "https://example.com/page".into(), method: HttpMethod::Get, body: None, content_type: None, headers: vec![] },
            NavigateRequest
        );
    }

    #[test]
    fn navigate_request_post_roundtrip() {
        assert_roundtrip!(
            NavigateRequest {
                url: "https://example.com/submit".into(),
                method: HttpMethod::Post,
                body: Some("name=Alice&email=alice@example.com".into()),
                content_type: Some("application/x-www-form-urlencoded".into()),
                headers: vec![],
            },
            NavigateRequest
        );
    }

    #[test]
    fn engine_action_none_roundtrip() {
        assert_roundtrip!(EngineAction::None, EngineAction);
    }

    #[test]
    fn engine_action_navigate_roundtrip() {
        assert_roundtrip!(
            EngineAction::Navigate(NavigateRequest {
                url: "https://example.com/next".into(),
                method: HttpMethod::Post,
                body: Some("data".into()),
                content_type: Some("text/plain".into()),
                headers: vec![],
            }),
            EngineAction
        );
    }

    #[test]
    fn engine_action_error_roundtrip() {
        assert_roundtrip!(EngineAction::Error("Element not found".into()), EngineAction);
    }

    #[test]
    fn daemon_request_new_session_roundtrip() {
        assert_roundtrip!(
            DaemonRequest { session_id: None, command: DaemonCommand::NewSession },
            DaemonRequest
        );
    }

    #[test]
    fn daemon_request_goto_roundtrip() {
        assert_roundtrip!(
            DaemonRequest {
                session_id: Some("sess_abc12345".into()),
                command: DaemonCommand::Goto { url: "https://example.com".into(), mode: SnapMode::Compact, record_path: None, clean: false },
            },
            DaemonRequest
        );
    }

    #[test]
    fn daemon_command_type_roundtrip() {
        assert_roundtrip!(
            DaemonCommand::Type { selector: "#email".into(), text: "test@example.com".into() },
            DaemonCommand
        );
    }

    #[test]
    fn daemon_response_ok_roundtrip() {
        assert_roundtrip!(DaemonResponse::ok("page content".into()), DaemonResponse);
    }

    #[test]
    fn daemon_response_err_roundtrip() {
        assert_roundtrip!(DaemonResponse::err("not found".into()), DaemonResponse);
    }

    #[test]
    fn daemon_response_with_session_roundtrip() {
        assert_roundtrip!(
            DaemonResponse::ok_with_session("sess_abc12345".into(), Some("content".into())),
            DaemonResponse
        );
    }

    // --- Auth support tests ---

    #[test]
    fn navigate_request_with_headers_roundtrip() {
        assert_roundtrip!(
            NavigateRequest {
                url: "https://api.example.com/data".into(),
                method: HttpMethod::Get,
                body: None,
                content_type: None,
                headers: vec![
                    ("Authorization".into(), "Bearer tok_abc123".into()),
                    ("User-Agent".into(), "Braille/1.0".into()),
                ],
            },
            NavigateRequest
        );
    }

    #[test]
    fn navigate_request_headers_omitted_when_empty() {
        let req = NavigateRequest {
            url: "https://example.com".into(),
            method: HttpMethod::Get,
            body: None,
            content_type: None,
            headers: vec![],
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("headers"), "empty headers should be skipped in JSON");
        let deserialized: NavigateRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, deserialized);
    }

    #[test]
    fn navigate_request_headers_default_from_old_json() {
        // Backward compat: JSON without a headers field deserializes to empty vec
        let json = r#"{"url":"https://example.com","method":"Get","body":null,"content_type":null}"#;
        let req: NavigateRequest = serde_json::from_str(json).unwrap();
        assert!(req.headers.is_empty());
    }

    #[test]
    fn daemon_response_with_status_code_roundtrip() {
        let resp = DaemonResponse {
            success: true,
            session_id: None,
            content: Some("page".into()),
            error: None,
            console: Vec::new(),
            status_code: Some(200),
        };
        assert_roundtrip!(resp, DaemonResponse);
    }

    #[test]
    fn daemon_response_status_code_omitted_when_none() {
        let resp = DaemonResponse::ok("content".into());
        let json = serde_json::to_string(&resp).unwrap();
        assert!(!json.contains("status_code"), "None status_code should be skipped in JSON");
        let deserialized: DaemonResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, deserialized);
    }

    #[test]
    fn daemon_response_status_code_default_from_old_json() {
        // Backward compat: JSON without status_code deserializes to None
        let json = r#"{"success":true,"session_id":null,"content":"ok","error":null}"#;
        let resp: DaemonResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.status_code, None);
    }

    // --- Engine REPL protocol tests ---

    #[test]
    fn host_message_command_roundtrip() {
        assert_roundtrip!(
            HostMessage::Command(DaemonCommand::Goto {
                url: "https://example.com".into(),
                mode: SnapMode::Compact,
                record_path: None,
                clean: false,
            }),
            HostMessage
        );
    }

    #[test]
    fn host_message_fetch_results_roundtrip() {
        assert_roundtrip!(
            HostMessage::FetchResults(vec![
                FetchResult {
                    id: 1,
                    outcome: FetchOutcome::Ok(FetchResponseData {
                        status: 200,
                        status_text: "OK".into(),
                        headers: vec![("content-type".into(), "text/html".into())],
                        body: "<html></html>".into(),
                        url: "https://example.com".into(),
                        redirect_chain: vec![],
                    }),
                },
                FetchResult {
                    id: 2,
                    outcome: FetchOutcome::Err("network error".into()),
                },
            ]),
            HostMessage
        );
    }

    #[test]
    fn engine_message_need_fetch_roundtrip() {
        assert_roundtrip!(
            EngineMessage::NeedFetch(vec![FetchRequest {
                id: 42,
                url: "https://example.com/api".into(),
                method: "GET".into(),
                headers: vec![],
                body: None,
            }]),
            EngineMessage
        );
    }

    #[test]
    fn engine_message_command_result_roundtrip() {
        assert_roundtrip!(
            EngineMessage::CommandResult(DaemonResponse::ok("snapshot content".into())),
            EngineMessage
        );
    }

    #[test]
    fn fetch_outcome_ok_roundtrip() {
        assert_roundtrip!(
            FetchOutcome::Ok(FetchResponseData {
                status: 404,
                status_text: "Not Found".into(),
                headers: vec![],
                body: "".into(),
                url: "https://example.com/missing".into(),
                redirect_chain: vec![],
            }),
            FetchOutcome
        );
    }

    #[test]
    fn fetch_outcome_err_roundtrip() {
        assert_roundtrip!(FetchOutcome::Err("timeout".into()), FetchOutcome);
    }

    // --- Worker and checkpoint protocol tests ---

    #[test]
    fn engine_message_spawn_worker_roundtrip() {
        assert_roundtrip!(
            EngineMessage::SpawnWorker { worker_id: 1, url: "https://example.com/worker.js".into() },
            EngineMessage
        );
    }

    #[test]
    fn engine_message_post_to_worker_roundtrip() {
        assert_roundtrip!(
            EngineMessage::PostToWorker { worker_id: 1, data: r#"{"nonce":42}"#.into() },
            EngineMessage
        );
    }

    #[test]
    fn engine_message_terminate_worker_roundtrip() {
        assert_roundtrip!(
            EngineMessage::TerminateWorker { worker_id: 3 },
            EngineMessage
        );
    }

    #[test]
    fn engine_message_checkpoint_ready_roundtrip() {
        assert_roundtrip!(
            EngineMessage::CheckpointReady {
                active_workers: vec![
                    WorkerDescriptor { id: 1, url: "https://example.com/w1.js".into() },
                    WorkerDescriptor { id: 2, url: "https://example.com/w2.js".into() },
                ],
            },
            EngineMessage
        );
    }

    #[test]
    fn engine_message_checkpoint_ready_empty_roundtrip() {
        assert_roundtrip!(
            EngineMessage::CheckpointReady { active_workers: vec![] },
            EngineMessage
        );
    }

    #[test]
    fn host_message_worker_spawned_roundtrip() {
        assert_roundtrip!(
            HostMessage::WorkerSpawned { worker_id: 1 },
            HostMessage
        );
    }

    #[test]
    fn host_message_worker_message_roundtrip() {
        assert_roundtrip!(
            HostMessage::WorkerMessage { worker_id: 1, data: "hello from worker".into() },
            HostMessage
        );
    }

    #[test]
    fn host_message_worker_error_roundtrip() {
        assert_roundtrip!(
            HostMessage::WorkerError { worker_id: 1, error: "ReferenceError: x is not defined".into() },
            HostMessage
        );
    }

    #[test]
    fn host_message_worker_exited_roundtrip() {
        assert_roundtrip!(
            HostMessage::WorkerExited { worker_id: 5 },
            HostMessage
        );
    }

    #[test]
    fn host_message_prepare_checkpoint_roundtrip() {
        assert_roundtrip!(HostMessage::PrepareCheckpoint, HostMessage);
    }

    #[test]
    fn host_message_worker_restored_roundtrip() {
        assert_roundtrip!(
            HostMessage::WorkerRestored { worker_id: 2, url: "https://example.com/solver.mjs".into() },
            HostMessage
        );
    }

    #[test]
    fn worker_descriptor_roundtrip() {
        assert_roundtrip!(
            WorkerDescriptor { id: 42, url: "https://example.com/worker.js".into() },
            WorkerDescriptor
        );
    }

    #[test]
    fn daemon_command_export_cookies_roundtrip() {
        assert_roundtrip!(DaemonCommand::ExportCookies, DaemonCommand);
    }

    #[test]
    fn daemon_command_import_cookies_roundtrip() {
        assert_roundtrip!(
            DaemonCommand::ImportCookies {
                cookies: vec![
                    SerializableCookie {
                        name: "session".into(),
                        value: "abc123".into(),
                        domain: "example.com".into(),
                        path: "/".into(),
                        http_only: true,
                        secure: true,
                        expires_ms: Some(1700000000000.0),
                    },
                    SerializableCookie {
                        name: "theme".into(),
                        value: "dark".into(),
                        domain: "example.com".into(),
                        path: "/".into(),
                        http_only: false,
                        secure: false,
                        expires_ms: None,
                    },
                ],
            },
            DaemonCommand
        );
    }

    #[test]
    fn serializable_cookie_roundtrip() {
        assert_roundtrip!(
            SerializableCookie {
                name: "auth".into(),
                value: "jwt_token_here".into(),
                domain: ".example.com".into(),
                path: "/api".into(),
                http_only: true,
                secure: true,
                expires_ms: Some(1700000000000.0),
            },
            SerializableCookie
        );
    }
}
