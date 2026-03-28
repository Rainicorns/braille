pub mod worker_protocol;

use serde::{Deserialize, Serialize};

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

// NOTE: Simplified version without headers. If more complex request handling
// is needed (e.g., custom headers, authentication), extend this struct.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NavigateRequest {
    pub url: String,
    pub method: HttpMethod,
    pub body: Option<String>,
    pub content_type: Option<String>,
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
    },
    Click { selector: String },
    Type { selector: String, text: String },
    Select { selector: String, value: String },
    Snap { mode: SnapMode },
    Back,
    Forward,
    Console,
    Mark { label: String },
    Close,
    DaemonStop,
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
}

impl DaemonResponse {
    pub fn ok(content: String) -> Self {
        DaemonResponse {
            success: true,
            session_id: None,
            content: Some(content),
            error: None,
            console: Vec::new(),
        }
    }

    pub fn ok_with_session(session_id: String, content: Option<String>) -> Self {
        DaemonResponse {
            success: true,
            session_id: Some(session_id),
            content,
            error: None,
            console: Vec::new(),
        }
    }

    pub fn err(message: String) -> Self {
        DaemonResponse {
            success: false,
            session_id: None,
            content: None,
            error: Some(message),
            console: Vec::new(),
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
            NavigateRequest { url: "https://example.com/page".into(), method: HttpMethod::Get, body: None, content_type: None },
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
                command: DaemonCommand::Goto { url: "https://example.com".into(), mode: SnapMode::Compact, record_path: None },
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

    // --- Engine REPL protocol tests ---

    #[test]
    fn host_message_command_roundtrip() {
        assert_roundtrip!(
            HostMessage::Command(DaemonCommand::Goto {
                url: "https://example.com".into(),
                mode: SnapMode::Compact,
                record_path: None,
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
}
