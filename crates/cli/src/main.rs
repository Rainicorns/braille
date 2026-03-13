use clap::{Parser, Subcommand, ValueEnum};
use braille_wire::{SnapMode, EngineAction};
use braille_engine::ScriptDescriptor;
use std::collections::HashMap;

mod session;
pub mod network;

use session::SessionManager;
use network::NetworkClient;

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
    /// Run a command in an existing session
    #[command(external_subcommand)]
    Session(Vec<String>),
}

#[derive(Subcommand)]
enum SessionAction {
    /// Navigate to a URL
    Goto {
        url: String,
    },
    /// Click an element matching the selector
    Click {
        selector: String,
    },
    /// Type text into an element matching the selector
    Type {
        selector: String,
        text: String,
    },
    /// Select an option in a <select> element
    Select {
        selector: String,
        value: String,
    },
    /// Take a snapshot of the current page
    Snap {
        /// Output mode for the snapshot
        #[arg(long, default_value = "accessibility")]
        mode: SnapModeArg,
    },
    /// Go back in history
    Back,
    /// Go forward in history
    Forward,
    /// Close the session
    Close,
}

#[derive(Clone, ValueEnum)]
enum SnapModeArg {
    Accessibility,
    Dom,
    Markdown,
}

impl From<SnapModeArg> for SnapMode {
    fn from(arg: SnapModeArg) -> Self {
        match arg {
            SnapModeArg::Accessibility => SnapMode::Accessibility,
            SnapModeArg::Dom => SnapMode::Dom,
            SnapModeArg::Markdown => SnapMode::Markdown,
        }
    }
}

fn parse_session_action(args: &[String]) -> SessionAction {
    // Build a clap command for session actions and parse from the raw tokens.
    // We strip the session ID before calling this, so args is just the verb + its args.
    use clap::FromArgMatches;

    let cmd = clap::Command::new("braille-session")
        .subcommand_required(true)
        .no_binary_name(true);
    let cmd = SessionAction::augment_subcommands(cmd);
    let matches = cmd.get_matches_from(args);
    SessionAction::from_arg_matches(&matches).unwrap()
}

/// Fetch a URL using the NetworkClient, load HTML into the session's engine
/// using two-phase script loading, and return an accessibility snapshot.
fn fetch_and_load(net: &mut NetworkClient, session: &mut session::Session, url: &str) -> Result<String, String> {
    let resp = net.fetch(url)?;
    let html = &resp.body;

    // Two-phase script loading:
    // 1. Parse HTML and collect script descriptors
    let descriptors = session.engine.parse_and_collect_scripts(html);

    // 2. Fetch external scripts
    let mut fetched = HashMap::new();
    for desc in &descriptors {
        if let ScriptDescriptor::External(src_url) = desc {
            let resolved = net.resolve_url(src_url);
            match net.fetch(&resolved) {
                Ok(script_resp) => {
                    fetched.insert(src_url.clone(), script_resp.body);
                }
                Err(_) => {
                    // Skip failed external scripts (engine will also skip missing ones)
                }
            }
        }
    }

    // 3. Execute all scripts in document order
    session.engine.execute_scripts(&descriptors, &fetched);

    // 4. Record navigation in session history
    session.navigate(resp.url);

    // 5. Return accessibility snapshot
    Ok(session.engine.snapshot(SnapMode::Accessibility))
}

fn run(cli: Cli) -> String {
    match cli.command {
        TopLevel::New => {
            let mut manager = SessionManager::new();
            manager.new_session()
        }
        TopLevel::Session(args) => {
            if args.is_empty() {
                return "error: session ID required".to_string();
            }
            let _sid = &args[0];
            let action_args = &args[1..];
            if action_args.is_empty() {
                return "error: session command required (goto, click, type, select, snap, back, forward, close)".to_string();
            }
            let action = parse_session_action(action_args);
            match action {
                SessionAction::Goto { url } => {
                    let mut manager = SessionManager::new();
                    let session_id = manager.new_session();
                    let session = manager.get_session(&session_id).unwrap();
                    let mut net = NetworkClient::new();
                    match fetch_and_load(&mut net, session, &url) {
                        Ok(snapshot) => snapshot,
                        Err(e) => format!("error: {e}"),
                    }
                }
                SessionAction::Click { selector } => {
                    // Without a persistent session, we have no loaded page to click on.
                    // This will work once the daemon architecture is in place.
                    let mut manager = SessionManager::new();
                    let session_id = manager.new_session();
                    let session = manager.get_session(&session_id).unwrap();
                    // Engine has no page loaded, so snapshot will produce empty tree.
                    // Need a snapshot first to populate ref_map before click can work.
                    session.engine.snapshot(SnapMode::Accessibility);
                    let action = session.engine.handle_click(&selector);
                    match action {
                        EngineAction::Navigate(nav_req) => {
                            let mut net = NetworkClient::new();
                            let resolved = net.resolve_url(&nav_req.url);
                            match fetch_and_load(&mut net, session, &resolved) {
                                Ok(snapshot) => snapshot,
                                Err(e) => format!("error: {e}"),
                            }
                        }
                        EngineAction::Error(msg) => format!("error: {msg}"),
                        EngineAction::None => {
                            session.engine.snapshot(SnapMode::Accessibility)
                        }
                    }
                }
                SessionAction::Type { selector, text } => {
                    let mut manager = SessionManager::new();
                    let session_id = manager.new_session();
                    let session = manager.get_session(&session_id).unwrap();
                    session.engine.snapshot(SnapMode::Accessibility);
                    match session.engine.handle_type(&selector, &text) {
                        Ok(()) => session.engine.snapshot(SnapMode::Accessibility),
                        Err(e) => format!("error: {e}"),
                    }
                }
                SessionAction::Select { selector, value } => {
                    let mut manager = SessionManager::new();
                    let session_id = manager.new_session();
                    let session = manager.get_session(&session_id).unwrap();
                    session.engine.snapshot(SnapMode::Accessibility);
                    match session.engine.handle_select(&selector, &value) {
                        Ok(()) => session.engine.snapshot(SnapMode::Accessibility),
                        Err(e) => format!("error: {e}"),
                    }
                }
                SessionAction::Snap { mode } => {
                    let mut manager = SessionManager::new();
                    let session_id = manager.new_session();
                    let session = manager.get_session(&session_id).unwrap();
                    session.engine.snapshot(mode.into())
                }
                SessionAction::Back => {
                    let mut manager = SessionManager::new();
                    let session_id = manager.new_session();
                    let session = manager.get_session(&session_id).unwrap();
                    match session.go_back() {
                        Some(url) => {
                            let url = url.to_string();
                            let mut net = NetworkClient::new();
                            match fetch_and_load(&mut net, session, &url) {
                                Ok(snapshot) => snapshot,
                                Err(e) => format!("error: {e}"),
                            }
                        }
                        None => "error: no previous page in history".to_string(),
                    }
                }
                SessionAction::Forward => {
                    let mut manager = SessionManager::new();
                    let session_id = manager.new_session();
                    let session = manager.get_session(&session_id).unwrap();
                    match session.go_forward() {
                        Some(url) => {
                            let url = url.to_string();
                            let mut net = NetworkClient::new();
                            match fetch_and_load(&mut net, session, &url) {
                                Ok(snapshot) => snapshot,
                                Err(e) => format!("error: {e}"),
                            }
                        }
                        None => "error: no forward page in history".to_string(),
                    }
                }
                SessionAction::Close => {
                    let mut manager = SessionManager::new();
                    let session_id = manager.new_session();
                    manager.close_session(&session_id);
                    "session closed".to_string()
                }
            }
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let output = run(cli);
    println!("{output}");
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
    fn cmd_new() {
        let output = parse(&["braille", "new"]);
        assert!(output.starts_with("sess_"), "new should return a session ID starting with 'sess_', got: {}", output);
        assert_eq!(output.len(), 13, "session ID should be 13 chars (sess_ + 8 hex), got: {}", output);
    }

    #[test]
    fn cmd_goto() {
        // goto now performs a real HTTP fetch + two-phase script loading via Session + NetworkClient.
        let output = parse(&["braille", "abc123", "goto", "https://example.com"]);
        assert!(!output.is_empty(), "goto should produce output");
        assert!(!output.starts_with("error:"), "goto should not error for example.com, got: {}", output);
    }

    #[test]
    fn cmd_click_no_page() {
        // Without a loaded page, click on a selector finds nothing.
        let output = parse(&["braille", "abc123", "click", "button.submit"]);
        assert!(output.contains("error:"), "click without loaded page should return an error, got: {}", output);
    }

    #[test]
    fn cmd_type_no_page() {
        // Without a loaded page, type into a selector finds nothing.
        let output = parse(&["braille", "abc123", "type", "input#email", "hello@test.com"]);
        assert!(output.contains("error:"), "type without loaded page should return an error, got: {}", output);
    }

    #[test]
    fn cmd_select_no_page() {
        // Without a loaded page, select finds nothing.
        let output = parse(&["braille", "abc123", "select", "#country", "us"]);
        assert!(output.contains("error:"), "select without loaded page should return an error, got: {}", output);
    }

    #[test]
    fn cmd_snap_default() {
        // With a fresh session (no page loaded), snapshot returns an empty tree.
        let output = parse(&["braille", "abc123", "snap"]);
        // A fresh engine with no HTML loaded will produce some output (empty doc).
        assert!(!output.starts_with("error:"), "snap should not error, got: {}", output);
    }

    #[test]
    fn cmd_snap_dom() {
        let output = parse(&["braille", "abc123", "snap", "--mode", "dom"]);
        // DOM mode returns a placeholder for now.
        assert!(!output.is_empty(), "snap --mode dom should produce output");
    }

    #[test]
    fn cmd_snap_markdown() {
        let output = parse(&["braille", "abc123", "snap", "--mode", "markdown"]);
        assert!(!output.is_empty(), "snap --mode markdown should produce output");
    }

    #[test]
    fn cmd_back_no_history() {
        let output = parse(&["braille", "s1", "back"]);
        assert!(output.contains("error:"), "back with no history should return an error, got: {}", output);
        assert!(output.contains("no previous page"), "back error should mention no previous page, got: {}", output);
    }

    #[test]
    fn cmd_forward_no_history() {
        let output = parse(&["braille", "s1", "forward"]);
        assert!(output.contains("error:"), "forward with no history should return an error, got: {}", output);
        assert!(output.contains("no forward page"), "forward error should mention no forward page, got: {}", output);
    }

    #[test]
    fn cmd_close() {
        let output = parse(&["braille", "s1", "close"]);
        assert_eq!(output, "session closed");
    }

    #[test]
    fn cmd_missing_session_id() {
        // external_subcommand with no args
        // This case is handled by the empty args check
        let cli = Cli::parse_from(&["braille", ""]);
        let output = run(cli);
        assert!(output.contains("error:"), "empty session ID should produce an error, got: {}", output);
    }

    #[test]
    fn cmd_missing_action() {
        let output = parse(&["braille", "abc123"]);
        assert!(output.contains("error:"), "missing action should produce an error, got: {}", output);
        assert!(output.contains("session command required"), "error should mention session command required, got: {}", output);
    }

    /// Integration test: create a SessionManager, create a session, navigate, and verify snapshot.
    #[test]
    fn session_goto_and_snapshot() {
        let mut manager = SessionManager::new();
        let session_id = manager.new_session();
        let session = manager.get_session(&session_id).unwrap();
        let mut net = NetworkClient::new();

        let result = fetch_and_load(&mut net, session, "https://example.com");
        assert!(result.is_ok(), "fetch_and_load should succeed for example.com: {:?}", result);

        let snapshot = result.unwrap();
        assert!(!snapshot.is_empty(), "snapshot should not be empty");
        // example.com has a heading
        assert!(snapshot.contains("Example Domain") || snapshot.contains("example"),
            "snapshot should contain content from example.com: {}", snapshot);

        // Verify the session recorded the navigation
        let session = manager.get_session(&session_id).unwrap();
        assert!(session.current_url().is_some(), "session should have a current URL after goto");
        assert!(session.history.len() == 1, "session should have 1 history entry");
    }
}
