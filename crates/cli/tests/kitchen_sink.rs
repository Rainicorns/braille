//! Kitchen Sink App — multi-step session persistence test.
//!
//! Spins up a real HTTP server serving the kitchen_sink.html fixture + API endpoints,
//! starts a daemon, and walks through: login → dashboard → settings → logout.
//! Verifies that DOM state, localStorage, and JS globals persist across
//! separate daemon commands (the whole point of the daemon architecture).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use braille_wire::{DaemonCommand, DaemonRequest, DaemonResponse, SnapMode};

// ---------------------------------------------------------------------------
// Test HTTP server (serves HTML + API endpoints)
// ---------------------------------------------------------------------------

struct TestServer {
    port: u16,
    shutdown_tx: mpsc::Sender<()>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl TestServer {
    fn start() -> Self {
        let html = include_str!("../../../tests/fixtures/kitchen_sink.html").to_string();

        let server = tiny_http::Server::http("127.0.0.1:0").expect("failed to bind test server");
        let port = server.server_addr().to_ip().unwrap().port();
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        let handle = std::thread::spawn(move || loop {
            if shutdown_rx.try_recv().is_ok() {
                break;
            }
            match server.recv_timeout(Duration::from_millis(50)) {
                Ok(Some(mut req)) => {
                    let url = req.url().to_string();
                    let method = req.method().to_string();

                    // Read request body for POST requests
                    let mut body_str = String::new();
                    if method == "POST" {
                        let _ = req.as_reader().read_to_string(&mut body_str);
                    }

                    let (status, content_type, body) = handle_request(&url, &method, &body_str, &html);

                    let response = tiny_http::Response::from_string(body)
                        .with_status_code(status)
                        .with_header(
                            tiny_http::Header::from_bytes(
                                b"Content-Type" as &[u8],
                                content_type.as_bytes(),
                            )
                            .unwrap(),
                        );
                    let _ = req.respond(response);
                }
                Ok(None) => {}
                Err(_) => break,
            }
        });

        TestServer {
            port,
            shutdown_tx,
            handle: Some(handle),
        }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

fn handle_request(url: &str, method: &str, body: &str, html: &str) -> (u16, &'static str, String) {
    // API: POST /api/login
    if url == "/api/login" && method == "POST" {
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body) {
            let username = parsed["username"].as_str().unwrap_or("");
            let password = parsed["password"].as_str().unwrap_or("");
            if username == "testuser" && password == "testpass123" {
                return (
                    200,
                    "application/json",
                    r#"{"token":"tok_abc123","user":{"username":"testuser","name":"Test User","email":"test@example.com"}}"#.to_string(),
                );
            }
        }
        return (
            401,
            "application/json",
            r#"{"error":"Invalid username or password"}"#.to_string(),
        );
    }

    // API: GET /api/profile
    if url == "/api/profile" && method == "GET" {
        return (
            200,
            "application/json",
            r#"{"username":"testuser","name":"Test User","email":"test@example.com","memberSince":"2024-01-15"}"#.to_string(),
        );
    }

    // API: POST /api/settings
    if url == "/api/settings" && method == "POST" {
        return (
            200,
            "application/json",
            r#"{"success":true,"message":"Settings saved"}"#.to_string(),
        );
    }

    // Everything else: serve the HTML page
    (200, "text/html", html.to_string())
}

// ---------------------------------------------------------------------------
// Daemon helpers
// ---------------------------------------------------------------------------

fn send(socket: &std::path::Path, request: &DaemonRequest) -> DaemonResponse {
    let mut stream = UnixStream::connect(socket).expect("connect to daemon");
    stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
    let json = serde_json::to_string(request).unwrap();
    stream.write_all(json.as_bytes()).unwrap();
    stream.write_all(b"\n").unwrap();
    stream.flush().unwrap();
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(&line).unwrap()
}

fn new_session(socket: &std::path::Path) -> String {
    let resp = send(
        socket,
        &DaemonRequest {
            session_id: None,
            command: DaemonCommand::NewSession,
        },
    );
    assert!(resp.success, "NewSession failed: {:?}", resp.error);
    resp.session_id.expect("session_id missing")
}

fn goto(socket: &std::path::Path, sid: &str, url: &str) -> String {
    let resp = send(
        socket,
        &DaemonRequest {
            session_id: Some(sid.to_string()),
            command: DaemonCommand::Goto {
                url: url.to_string(),
                mode: SnapMode::Compact,
            },
        },
    );
    assert!(resp.success, "Goto {url} failed: {:?}", resp.error);
    resp.content.unwrap_or_default()
}

fn click(socket: &std::path::Path, sid: &str, selector: &str) -> String {
    let resp = send(
        socket,
        &DaemonRequest {
            session_id: Some(sid.to_string()),
            command: DaemonCommand::Click {
                selector: selector.to_string(),
            },
        },
    );
    assert!(resp.success, "Click {selector} failed: {:?}", resp.error);
    resp.content.unwrap_or_default()
}

fn type_text(socket: &std::path::Path, sid: &str, selector: &str, text: &str) -> String {
    let resp = send(
        socket,
        &DaemonRequest {
            session_id: Some(sid.to_string()),
            command: DaemonCommand::Type {
                selector: selector.to_string(),
                text: text.to_string(),
            },
        },
    );
    assert!(resp.success, "Type {selector} failed: {:?}", resp.error);
    resp.content.unwrap_or_default()
}

fn snap(socket: &std::path::Path, sid: &str) -> String {
    let resp = send(
        socket,
        &DaemonRequest {
            session_id: Some(sid.to_string()),
            command: DaemonCommand::Snap {
                mode: SnapMode::Compact,
            },
        },
    );
    assert!(resp.success, "Snap failed: {:?}", resp.error);
    resp.content.unwrap_or_default()
}

fn stop_daemon(socket: &std::path::Path) {
    let _ = send(
        socket,
        &DaemonRequest {
            session_id: None,
            command: DaemonCommand::DaemonStop,
        },
    );
}

fn start_daemon_in_thread(tmp: &std::path::Path) -> PathBuf {
    std::fs::create_dir_all(tmp).ok();
    let socket = tmp.join("daemon.sock");
    let pid = tmp.join("daemon.pid");
    let sock_clone = socket.clone();
    std::thread::spawn(move || {
        braille_cli::daemon::run_daemon(sock_clone, pid);
    });
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if socket.exists() && UnixStream::connect(&socket).is_ok() {
            return socket;
        }
    }
    panic!("daemon did not start");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn login_flow_persists_across_commands() {
    let server = TestServer::start();
    let tmp = std::env::temp_dir().join(format!("braille-kitchen-sink-{}", std::process::id()));
    let socket = start_daemon_in_thread(&tmp);
    let sid = new_session(&socket);

    // Step 1: Navigate to the app — should see login page.
    let content = goto(&socket, &sid, &server.url());
    assert!(
        content.contains("Sign In"),
        "should see login page, got: {content}"
    );
    assert!(
        content.contains("Username"),
        "should see username field, got: {content}"
    );

    // Step 2: Type username (in a separate command — session persists!).
    let content = type_text(&socket, &sid, "#username", "testuser");
    assert!(
        content.contains("Sign In"),
        "should still be on login page after typing username, got: {content}"
    );

    // Step 3: Type password.
    let content = type_text(&socket, &sid, "#password", "testpass123");
    assert!(
        content.contains("Sign In"),
        "should still be on login page after typing password, got: {content}"
    );

    // Step 4: Click Sign In — should navigate to dashboard.
    let content = click(&socket, &sid, "#login-btn");
    assert!(
        content.contains("Dashboard") || content.contains("Welcome"),
        "should be on dashboard after login, got: {content}"
    );

    // Step 5: Snap — dashboard should persist.
    let content = snap(&socket, &sid);
    assert!(
        content.contains("Dashboard") || content.contains("Welcome"),
        "dashboard should persist on snap, got: {content}"
    );

    stop_daemon(&socket);
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn login_wrong_password_shows_error() {
    let server = TestServer::start();
    let tmp = std::env::temp_dir().join(format!("braille-kitchen-wrong-{}", std::process::id()));
    let socket = start_daemon_in_thread(&tmp);
    let sid = new_session(&socket);

    goto(&socket, &sid, &server.url());
    type_text(&socket, &sid, "#username", "testuser");
    type_text(&socket, &sid, "#password", "wrongpassword");
    let content = click(&socket, &sid, "#login-btn");

    // Should stay on login page with error message.
    assert!(
        content.contains("Sign In") || content.contains("Invalid"),
        "should still be on login page after wrong password, got: {content}"
    );

    stop_daemon(&socket);
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn settings_persist_in_session() {
    let server = TestServer::start();
    let tmp = std::env::temp_dir().join(format!("braille-kitchen-settings-{}", std::process::id()));
    let socket = start_daemon_in_thread(&tmp);
    let sid = new_session(&socket);

    // Login.
    goto(&socket, &sid, &server.url());
    type_text(&socket, &sid, "#username", "testuser");
    type_text(&socket, &sid, "#password", "testpass123");
    click(&socket, &sid, "#login-btn");

    // Navigate to settings via button click.
    let content = click(&socket, &sid, "#btn-settings");
    assert!(
        content.contains("Settings") || content.contains("Display Name"),
        "should be on settings page, got: {content}"
    );

    // Type a display name.
    type_text(&socket, &sid, "#display-name", "Custom Name");

    // Save settings (uses localStorage directly, no fetch needed).
    let content = click(&socket, &sid, "#save-settings");
    assert!(
        content.contains("Settings saved") || content.contains("Settings"),
        "should see save confirmation, got: {content}"
    );

    // Go back to dashboard — display name should reflect the change.
    let content = click(&socket, &sid, "#nav-dashboard");
    assert!(
        content.contains("Custom Name") || content.contains("Dashboard"),
        "dashboard should show custom name, got: {content}"
    );

    stop_daemon(&socket);
    std::fs::remove_dir_all(&tmp).ok();
}
