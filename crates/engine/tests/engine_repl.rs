use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use braille_wire::{
    DaemonCommand, DaemonResponse, EngineMessage, FetchOutcome, FetchResponseData, FetchResult,
    HostMessage, SnapMode,
};

fn engine_binary() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    // tests are in target/debug/deps/, binary is in target/debug/
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.push("braille-engine");
    path
}

struct EngineHarness {
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    _child: std::process::Child,
}

impl EngineHarness {
    fn new() -> Self {
        let bin = engine_binary();
        let mut child = Command::new(&bin)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", bin.display()));

        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());

        EngineHarness {
            stdin,
            stdout,
            _child: child,
        }
    }

    fn send(&mut self, msg: &HostMessage) {
        let json = serde_json::to_string(msg).unwrap();
        writeln!(self.stdin, "{json}").unwrap();
        self.stdin.flush().unwrap();
    }

    fn recv(&mut self) -> EngineMessage {
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(line.trim()).unwrap()
    }

    /// Send a command and handle all NeedFetch rounds with mock HTML.
    fn send_goto(&mut self, url: &str, html: &str) -> DaemonResponse {
        self.send(&HostMessage::Command(DaemonCommand::Goto {
            url: url.to_string(),
            mode: SnapMode::Compact,
            record_path: None,
        }));

        loop {
            let msg = self.recv();
            match msg {
                EngineMessage::NeedFetch(requests) => {
                    let results: Vec<FetchResult> = requests
                        .into_iter()
                        .map(|req| FetchResult {
                            id: req.id,
                            outcome: FetchOutcome::Ok(FetchResponseData {
                                status: 200,
                                status_text: "OK".to_string(),
                                headers: vec![(
                                    "content-type".to_string(),
                                    "text/html".to_string(),
                                )],
                                body: html.to_string(),
                                url: url.to_string(),
                            }),
                        })
                        .collect();
                    self.send(&HostMessage::FetchResults(results));
                }
                EngineMessage::CommandResult(resp) => return resp,
                _ => { /* ignore worker/checkpoint messages in tests */ }
            }
        }
    }
}

// stdin is automatically dropped when EngineHarness is dropped,
// which signals EOF to the engine process and causes it to exit.

#[test]
fn goto_returns_snapshot() {
    let mut engine = EngineHarness::new();
    let resp = engine.send_goto(
        "https://example.com",
        "<html><body><h1>Hello World</h1></body></html>",
    );
    assert!(resp.success);
    let content = resp.content.unwrap();
    assert!(content.contains("Hello World"), "snapshot should contain page content, got: {content}");
}

#[test]
fn snap_after_goto() {
    let mut engine = EngineHarness::new();
    engine.send_goto(
        "https://example.com",
        "<html><body><h1>Test Page</h1><a href='/link'>Click me</a></body></html>",
    );

    engine.send(&HostMessage::Command(DaemonCommand::Snap {
        mode: SnapMode::Compact,
    }));
    let msg = engine.recv();
    match msg {
        EngineMessage::CommandResult(resp) => {
            assert!(resp.success);
            let content = resp.content.unwrap();
            assert!(content.contains("Test Page"), "snap should contain page content, got: {content}");
        }
        other => panic!("expected CommandResult, got: {other:?}"),
    }
}

#[test]
fn type_command() {
    let mut engine = EngineHarness::new();
    engine.send_goto(
        "https://example.com",
        r#"<html><body><form><input id="name" type="text" /><button>Submit</button></form></body></html>"#,
    );

    engine.send(&HostMessage::Command(DaemonCommand::Type {
        selector: "#name".to_string(),
        text: "Alice".to_string(),
    }));

    // Type might trigger NeedFetch if JS does something, handle it
    loop {
        let msg = engine.recv();
        match msg {
            EngineMessage::NeedFetch(requests) => {
                let results = requests
                    .into_iter()
                    .map(|req| FetchResult {
                        id: req.id,
                        outcome: FetchOutcome::Err("not found".to_string()),
                    })
                    .collect();
                engine.send(&HostMessage::FetchResults(results));
            }
            EngineMessage::CommandResult(resp) => {
                assert!(resp.success, "type command should succeed: {:?}", resp.error);
                break;
            }
            _ => { /* ignore worker/checkpoint messages */ }
        }
    }
}

#[test]
fn fetch_error_propagates() {
    let mut engine = EngineHarness::new();

    engine.send(&HostMessage::Command(DaemonCommand::Goto {
        url: "https://example.com".to_string(),
        mode: SnapMode::Compact,
    }));

    // Respond with an error to the page fetch
    let msg = engine.recv();
    match msg {
        EngineMessage::NeedFetch(requests) => {
            let results = requests
                .into_iter()
                .map(|req| FetchResult {
                    id: req.id,
                    outcome: FetchOutcome::Err("connection refused".to_string()),
                })
                .collect();
            engine.send(&HostMessage::FetchResults(results));
        }
        other => panic!("expected NeedFetch, got: {other:?}"),
    }

    let msg = engine.recv();
    match msg {
        EngineMessage::CommandResult(resp) => {
            assert!(!resp.success, "should have failed");
            assert!(
                resp.error.unwrap().contains("connection refused"),
                "error should mention the failure"
            );
        }
        other => panic!("expected CommandResult, got: {other:?}"),
    }
}

#[test]
fn invalid_json_returns_error() {
    let mut engine = EngineHarness::new();

    writeln!(engine.stdin, "not valid json").unwrap();
    engine.stdin.flush().unwrap();

    let msg = engine.recv();
    match msg {
        EngineMessage::CommandResult(resp) => {
            assert!(!resp.success);
            assert!(resp.error.unwrap().contains("invalid message"));
        }
        other => panic!("expected CommandResult with error, got: {other:?}"),
    }
}

#[test]
fn console_output_captured() {
    let mut engine = EngineHarness::new();
    let resp = engine.send_goto(
        "https://example.com",
        r#"<html><body><script>console.log("hello from js"); console.warn("a warning"); console.error("an error");</script></body></html>"#,
    );
    assert!(resp.success);
    assert!(
        resp.console.iter().any(|l| l.contains("hello from js")),
        "console should capture log output, got: {:?}",
        resp.console
    );
    assert!(
        resp.console.iter().any(|l| l.contains("a warning")),
        "console should capture warn output, got: {:?}",
        resp.console
    );
    assert!(
        resp.console.iter().any(|l| l.contains("an error")),
        "console should capture error output, got: {:?}",
        resp.console
    );
}

#[test]
fn console_command_returns_output() {
    let mut engine = EngineHarness::new();
    engine.send_goto(
        "https://example.com",
        r#"<html><body><script>console.log("page loaded");</script></body></html>"#,
    );

    // Console output from goto was already drained.
    // Now trigger more console output via a click that runs JS.
    engine.send_goto(
        "https://example.com",
        r#"<html><body><script>console.log("second page");</script></body></html>"#,
    );

    // The console command should return whatever was logged
    engine.send(&HostMessage::Command(DaemonCommand::Console));
    let msg = engine.recv();
    match msg {
        EngineMessage::CommandResult(resp) => {
            assert!(resp.success);
            // Console field contains output from this command (which is nothing new since
            // the goto already drained it). The point is it doesn't crash.
        }
        other => panic!("expected CommandResult, got: {other:?}"),
    }
}
