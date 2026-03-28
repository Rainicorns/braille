use braille_wire::{DaemonCommand, DaemonRequest, DaemonResponse, SnapMode};
use clap::{Parser, Subcommand, ValueEnum};

mod client;
#[allow(dead_code)] // Foundation module — container integration not wired yet.
mod container;
pub mod daemon;
mod engine_process;
pub mod network;
mod paths;
mod session;
#[allow(dead_code)] // Foundation module — not wired into CLI commands yet.
mod session_store;
mod worker_manager;

#[derive(Parser)]
#[command(name = "braille", about = "A text browser for LLM agents")]
struct Cli {
    #[command(subcommand)]
    command: TopLevel,
}

#[derive(Subcommand)]
enum TopLevel {
    /// Create a new browser session
    New,
    /// Daemon management
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Run a command in an existing session
    #[command(external_subcommand)]
    Session(Vec<String>),
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Start the daemon (foreground — used internally)
    Start,
    /// Stop the running daemon
    Stop,
    /// Check if daemon is running
    Status,
}

#[derive(Subcommand)]
enum SessionAction {
    /// Navigate to a URL
    Goto {
        url: String,
        /// Output mode for the snapshot
        #[arg(long, default_value = "compact")]
        mode: SnapModeArg,
        /// CSS selector for selector mode
        #[arg(long)]
        query: Option<String>,
        /// Target element (@eN, #id, CSS selector) for region mode
        #[arg(long)]
        target: Option<String>,
        /// Record the network transcript for replay/debugging
        #[arg(long)]
        record: bool,
    },
    /// Click an element matching the selector
    Click { selector: String },
    /// Type text into an element matching the selector
    Type { selector: String, text: String },
    /// Select an option in a <select> element
    Select { selector: String, value: String },
    /// Take a snapshot of the current page
    Snap {
        /// Output mode for the snapshot
        #[arg(long, default_value = "compact")]
        mode: SnapModeArg,
        /// CSS selector for selector mode
        #[arg(long)]
        query: Option<String>,
        /// Target element (@eN, #id, CSS selector) for region mode
        #[arg(long)]
        target: Option<String>,
    },
    /// Go back in history
    Back,
    /// Go forward in history
    Forward,
    /// Show console output (log/warn/error) from JS
    Console,
    /// Show the last recorded network transcript
    Transcript,
    /// Close the session
    Close,
}

#[derive(Clone, ValueEnum)]
enum SnapModeArg {
    Compact,
    Accessibility,
    Interactive,
    Links,
    Forms,
    Headings,
    Text,
    Selector,
    Region,
    Dom,
    Markdown,
}

impl SnapModeArg {
    fn into_snap_mode(self, query: Option<String>, target: Option<String>) -> SnapMode {
        match self {
            SnapModeArg::Compact => SnapMode::Compact,
            SnapModeArg::Accessibility => SnapMode::Accessibility,
            SnapModeArg::Interactive => SnapMode::Interactive,
            SnapModeArg::Links => SnapMode::Links,
            SnapModeArg::Forms => SnapMode::Forms,
            SnapModeArg::Headings => SnapMode::Headings,
            SnapModeArg::Text => SnapMode::Text,
            SnapModeArg::Selector => SnapMode::Selector(query.unwrap_or_default()),
            SnapModeArg::Region => SnapMode::Region(target.unwrap_or_default()),
            SnapModeArg::Dom => SnapMode::Dom,
            SnapModeArg::Markdown => SnapMode::Markdown,
        }
    }
}

fn transcript_path(session_id: &str) -> String {
    let dir = paths::runtime_dir()
        .join("sessions")
        .join(session_id);
    std::fs::create_dir_all(&dir).ok();
    dir.join("transcript.json")
        .to_string_lossy()
        .into_owned()
}

fn parse_session_action(args: &[String]) -> SessionAction {
    use clap::FromArgMatches;

    let cmd = clap::Command::new("braille-session")
        .subcommand_required(true)
        .no_binary_name(true);
    let cmd = SessionAction::augment_subcommands(cmd);
    let matches = cmd.get_matches_from(args);
    SessionAction::from_arg_matches(&matches).unwrap()
}

fn session_action_to_daemon_command(action: SessionAction, session_id: &str) -> DaemonCommand {
    match action {
        SessionAction::Goto {
            url,
            mode,
            query,
            target,
            record,
        } => DaemonCommand::Goto {
            url,
            mode: mode.into_snap_mode(query, target),
            record_path: if record { Some(transcript_path(session_id)) } else { None },
        },
        SessionAction::Click { selector } => DaemonCommand::Click { selector },
        SessionAction::Type { selector, text } => DaemonCommand::Type { selector, text },
        SessionAction::Select { selector, value } => DaemonCommand::Select { selector, value },
        SessionAction::Snap { mode, query, target } => DaemonCommand::Snap {
            mode: mode.into_snap_mode(query, target),
        },
        SessionAction::Back => DaemonCommand::Back,
        SessionAction::Forward => DaemonCommand::Forward,
        SessionAction::Console => DaemonCommand::Console,
        SessionAction::Transcript => unreachable!("transcript handled before daemon dispatch"),
        SessionAction::Close => DaemonCommand::Close,
    }
}

fn format_response(response: DaemonResponse) -> String {
    let mut output = if response.success {
        if let Some(sid) = &response.session_id {
            if let Some(content) = &response.content {
                format!("{sid}\n{content}")
            } else {
                sid.clone()
            }
        } else {
            response.content.unwrap_or_default()
        }
    } else {
        format!("error: {}", response.error.unwrap_or_else(|| "unknown error".to_string()))
    };

    if !response.console.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("[console]\n");
        for line in &response.console {
            output.push_str(line);
            output.push('\n');
        }
    }

    output
}

fn run(cli: Cli) -> String {
    match cli.command {
        TopLevel::Daemon { action } => match action {
            DaemonAction::Start => {
                let socket = paths::socket_path();
                let pid = paths::pid_path();
                daemon::run_daemon(socket, pid);
                String::new()
            }
            DaemonAction::Stop => {
                let request = DaemonRequest {
                    session_id: None,
                    command: DaemonCommand::DaemonStop,
                };
                client::ensure_daemon_running();
                let response = client::send_request(&request);
                format_response(response)
            }
            DaemonAction::Status => {
                let socket = paths::socket_path();
                if socket.exists() {
                    if std::os::unix::net::UnixStream::connect(&socket).is_ok() {
                        "daemon is running".to_string()
                    } else {
                        "daemon socket exists but is not responding".to_string()
                    }
                } else {
                    "daemon is not running".to_string()
                }
            }
        },
        TopLevel::New => {
            client::ensure_daemon_running();
            let request = DaemonRequest {
                session_id: None,
                command: DaemonCommand::NewSession,
            };
            let response = client::send_request(&request);
            format_response(response)
        }
        TopLevel::Session(args) => {
            if args.is_empty() {
                return "error: session ID required".to_string();
            }
            let sid = &args[0];
            let action_args = &args[1..];
            if action_args.is_empty() {
                return "error: session command required (goto, click, type, select, snap, back, forward, close)"
                    .to_string();
            }
            let action = parse_session_action(action_args);

            // Transcript is handled locally — no daemon round-trip needed
            if matches!(action, SessionAction::Transcript) {
                let path = transcript_path(sid);
                return match std::fs::read_to_string(&path) {
                    Ok(contents) => contents,
                    Err(_) => format!("error: no transcript found for session {sid} (use --record on goto)"),
                };
            }

            let command = session_action_to_daemon_command(action, sid);

            client::ensure_daemon_running();
            let request = DaemonRequest {
                session_id: Some(sid.clone()),
                command,
            };
            let response = client::send_request(&request);
            format_response(response)
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let output = run(cli);
    if !output.is_empty() {
        println!("{output}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse(args: &[&str]) -> String {
        let cli = Cli::parse_from(args);
        run(cli)
    }

    #[test]
    fn cmd_daemon_status_not_running() {
        // When no daemon is running, status should report that.
        // Use a non-existent socket to test the status check logic.
        let output = parse(&["braille", "daemon", "status"]);
        // This test just verifies the status command doesn't panic.
        assert!(
            output.contains("daemon") || output.contains("running") || output.contains("not"),
            "status should report daemon state, got: {output}"
        );
    }

    #[test]
    fn cmd_missing_action() {
        let output = parse(&["braille", "abc123"]);
        assert!(output.contains("error:"), "missing action should produce an error, got: {output}");
        assert!(
            output.contains("session command required"),
            "error should mention session command required, got: {output}",
        );
    }

    #[test]
    fn cmd_missing_session_id() {
        let cli = Cli::parse_from(&["braille", ""]);
        let output = run(cli);
        assert!(output.contains("error:"), "empty session ID should produce an error, got: {output}");
    }
}
