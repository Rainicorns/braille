//! SPA Dynamic Loading — tests the full pipeline:
//!   dynamic <script> chunk loading, parallel fetches, API calls from components,
//!   and version polling that shouldn't starve the fetch loop.
//!
//! Exercises the same patterns as ProtonMail: webpack-style chunk loading via
//! script tag insertion, React-like rendering, and API calls triggered by
//! component mount.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use braille_wire::{DaemonCommand, DaemonRequest, DaemonResponse, SnapMode};

// ---------------------------------------------------------------------------
// Chunk JS content (served by the test server)
// ---------------------------------------------------------------------------

const CHUNK_FRAMEWORK: &str = r#"
__registerChunk('framework', function() {
    // Mini React-like framework
    return {
        createElement: function(tag, props, children) {
            var el = document.createElement(tag);
            if (props) {
                for (var k in props) {
                    if (k === 'textContent') el.textContent = props[k];
                    else if (k === 'className') el.className = props[k];
                    else el.setAttribute(k, props[k]);
                }
            }
            if (Array.isArray(children)) {
                children.forEach(function(c) {
                    if (typeof c === 'string') el.appendChild(document.createTextNode(c));
                    else if (c) el.appendChild(c);
                });
            } else if (typeof children === 'string') {
                el.appendChild(document.createTextNode(children));
            }
            return el;
        },
        render: function(root, el) {
            root.innerHTML = '';
            root.appendChild(el);
        }
    };
});
"#;

const CHUNK_APP: &str = r#"
__registerChunk('app', function(framework) {
    var h = framework.createElement;

    return {
        mount: function(root) {
            // Show loading while we fetch config + user
            framework.render(root, h('div', {id: 'loading', className: 'loading'}, 'Initializing...'));

            // Fetch config and user in parallel (like a real SPA)
            Promise.all([
                fetch('/api/config').then(function(r) { return r.json(); }),
                fetch('/api/user').then(function(r) { return r.json(); })
            ]).then(function(results) {
                var config = results[0];
                var user = results[1];

                // Render the actual app
                var form = h('div', {id: 'signup-page'}, [
                    h('h1', null, config.appName + ' - Signup'),
                    h('p', {id: 'welcome'}, 'Welcome, ' + (user.name || 'guest') + '!'),
                    h('div', {id: 'signup-form'}, [
                        h('label', {'for': 'email'}, 'Email:'),
                        h('input', {type: 'email', id: 'email', name: 'email', placeholder: 'you@example.com'}),
                        h('label', {'for': 'password'}, 'Password:'),
                        h('input', {type: 'password', id: 'password', name: 'password'}),
                        h('button', {id: 'submit-btn', type: 'button'}, 'Create Account'),
                        h('p', {id: 'status'}, '')
                    ]),
                    h('footer', null, 'Version: ' + config.version)
                ]);
                framework.render(root, form);

                // Wire up the submit button
                document.getElementById('submit-btn').addEventListener('click', function() {
                    var email = document.getElementById('email').value;
                    var password = document.getElementById('password').value;
                    document.getElementById('status').textContent = 'Creating account...';
                    fetch('/api/signup', {
                        method: 'POST',
                        headers: {'Content-Type': 'application/json'},
                        body: JSON.stringify({email: email, password: password})
                    }).then(function(r) { return r.json(); }).then(function(data) {
                        if (data.success) {
                            document.getElementById('status').textContent = 'Account created! ID: ' + data.userId;
                        } else {
                            document.getElementById('status').textContent = 'Error: ' + data.error;
                        }
                    });
                });
            }).catch(function(err) {
                framework.render(root, h('div', {id: 'error'}, 'Init failed: ' + err.message));
            });
        }
    };
});
"#;

const CHUNK_VENDOR: &str = r#"
__registerChunk('vendor', function() {
    // Third-party libs — no side effects, just registers
    window.__vendorLoaded = true;
    return {};
});
"#;

// ---------------------------------------------------------------------------
// Test HTTP server
// ---------------------------------------------------------------------------

static API_CONFIG_CALLS: AtomicUsize = AtomicUsize::new(0);
static API_USER_CALLS: AtomicUsize = AtomicUsize::new(0);
static API_VERSION_CALLS: AtomicUsize = AtomicUsize::new(0);
static API_SIGNUP_CALLS: AtomicUsize = AtomicUsize::new(0);

struct TestServer {
    port: u16,
    shutdown_tx: mpsc::Sender<()>,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl TestServer {
    fn start() -> Self {
        let html = include_str!("../../../tests/fixtures/spa_dynamic.html").to_string();

        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind test server");
        let port = server.server_addr().to_ip().unwrap().port();
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

        // Reset counters
        API_CONFIG_CALLS.store(0, Ordering::SeqCst);
        API_USER_CALLS.store(0, Ordering::SeqCst);
        API_VERSION_CALLS.store(0, Ordering::SeqCst);
        API_SIGNUP_CALLS.store(0, Ordering::SeqCst);

        let handle = std::thread::spawn(move || loop {
            if shutdown_rx.try_recv().is_ok() {
                break;
            }
            match server.recv_timeout(Duration::from_millis(50)) {
                Ok(Some(mut req)) => {
                    let url = req.url().to_string();
                    let method = req.method().to_string();
                    let mut body_str = String::new();
                    if method == "POST" {
                        let _ = req.as_reader().read_to_string(&mut body_str);
                    }

                    let (status, content_type, body) = handle_request(&url, &method, &body_str, &html);

                    let response = tiny_http::Response::from_string(body)
                        .with_status_code(status)
                        .with_header(
                            tiny_http::Header::from_bytes(b"Content-Type" as &[u8], content_type.as_bytes()).unwrap(),
                        );
                    let _ = req.respond(response);
                }
                Ok(None) => {}
                Err(_) => break,
            }
        });

        TestServer { port, shutdown_tx, handle: Some(handle) }
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
    // Chunk scripts
    if url == "/chunks/framework.js" {
        return (200, "application/javascript", CHUNK_FRAMEWORK.to_string());
    }
    if url == "/chunks/app.js" {
        return (200, "application/javascript", CHUNK_APP.to_string());
    }
    if url == "/chunks/vendor.js" {
        return (200, "application/javascript", CHUNK_VENDOR.to_string());
    }

    // API endpoints
    if url == "/api/config" && method == "GET" {
        API_CONFIG_CALLS.fetch_add(1, Ordering::SeqCst);
        return (200, "application/json", r#"{"appName":"TestApp","version":"1.0.0","features":["signup","login"]}"#.to_string());
    }
    if url == "/api/user" && method == "GET" {
        API_USER_CALLS.fetch_add(1, Ordering::SeqCst);
        return (200, "application/json", r#"{"name":"Guest","role":"anonymous"}"#.to_string());
    }
    if url == "/api/version" {
        API_VERSION_CALLS.fetch_add(1, Ordering::SeqCst);
        return (200, "application/json", r#"{"version":"1.0.0"}"#.to_string());
    }
    if url == "/api/signup" && method == "POST" {
        API_SIGNUP_CALLS.fetch_add(1, Ordering::SeqCst);
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body) {
            let email = parsed["email"].as_str().unwrap_or("");
            if email.contains('@') {
                return (200, "application/json", r#"{"success":true,"userId":"usr_42"}"#.to_string());
            }
        }
        return (400, "application/json", r#"{"success":false,"error":"Invalid email"}"#.to_string());
    }

    // Default: HTML page
    (200, "text/html", html.to_string())
}

// ---------------------------------------------------------------------------
// Daemon helpers (same pattern as kitchen_sink.rs)
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
    let resp = send(socket, &DaemonRequest { session_id: None, command: DaemonCommand::NewSession });
    assert!(resp.success, "NewSession failed: {:?}", resp.error);
    resp.session_id.expect("session_id missing")
}

fn goto(socket: &std::path::Path, sid: &str, url: &str) -> String {
    let resp = send(socket, &DaemonRequest {
        session_id: Some(sid.to_string()),
        command: DaemonCommand::Goto { url: url.to_string(), mode: SnapMode::Compact, record_path: None, clean: false },
    });
    assert!(resp.success, "Goto {url} failed: {:?}", resp.error);
    resp.content.unwrap_or_default()
}

fn snap(socket: &std::path::Path, sid: &str, mode: SnapMode) -> String {
    let resp = send(socket, &DaemonRequest {
        session_id: Some(sid.to_string()),
        command: DaemonCommand::Snap { mode },
    });
    assert!(resp.success, "Snap failed: {:?}", resp.error);
    resp.content.unwrap_or_default()
}

fn click(socket: &std::path::Path, sid: &str, selector: &str) -> String {
    let resp = send(socket, &DaemonRequest {
        session_id: Some(sid.to_string()),
        command: DaemonCommand::Click { selector: selector.to_string() },
    });
    assert!(resp.success, "Click {selector} failed: {:?}", resp.error);
    resp.content.unwrap_or_default()
}

fn type_text(socket: &std::path::Path, sid: &str, selector: &str, text: &str) -> String {
    let resp = send(socket, &DaemonRequest {
        session_id: Some(sid.to_string()),
        command: DaemonCommand::Type { selector: selector.to_string(), text: text.to_string() },
    });
    assert!(resp.success, "Type {selector} failed: {:?}", resp.error);
    resp.content.unwrap_or_default()
}

fn stop_daemon(socket: &std::path::Path) {
    let _ = send(socket, &DaemonRequest { session_id: None, command: DaemonCommand::DaemonStop });
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

fn setup() -> (TestServer, PathBuf) {
    use std::sync::atomic::AtomicU32;
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);

    let server = TestServer::start();
    let socket = std::env::temp_dir().join(format!("braille-spa-test-{}-{n}.sock", std::process::id()));
    let pid = std::env::temp_dir().join(format!("braille-spa-test-{}-{n}.pid", std::process::id()));

    // Clean up stale socket
    let _ = std::fs::remove_file(&socket);

    let socket_for_daemon = socket.clone();
    std::thread::spawn(move || {
        braille_cli::daemon::run_daemon(socket_for_daemon, pid);
    });

    // Wait for daemon to be ready
    for _ in 0..50 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(socket.exists(), "daemon socket not created");

    (server, socket)
}

/// Core test: dynamic chunks load, API calls fire, signup form renders.
#[test]
fn dynamic_chunks_load_and_api_calls_fire() {
    let (server, socket) = setup();

    let sid = new_session(&socket);
    let snapshot = goto(&socket, &sid, &server.url());

    eprintln!("snapshot:\n{snapshot}");

    // The app should have loaded all 3 chunks, called /api/config + /api/user,
    // and rendered the signup form.
    assert!(snapshot.contains("Signup"), "should show signup page, got: {snapshot}");
    assert!(snapshot.contains("TestApp"), "should show app name from config API");

    // Verify the API calls were actually made
    assert!(API_CONFIG_CALLS.load(Ordering::SeqCst) >= 1, "config API should have been called");
    assert!(API_USER_CALLS.load(Ordering::SeqCst) >= 1, "user API should have been called");

    // Version polling should have fired at least once
    assert!(API_VERSION_CALLS.load(Ordering::SeqCst) >= 1, "version API should have been called");

    // Version polling should NOT have starved the loop (shouldn't be called 20+ times)
    let version_calls = API_VERSION_CALLS.load(Ordering::SeqCst);
    eprintln!("version API calls: {version_calls}");
    assert!(version_calls < 10, "version polling should not starve fetch loop, got {version_calls} calls");

    stop_daemon(&socket);
}

/// Test the full signup flow: load page, fill form, submit.
#[test]
fn signup_flow_end_to_end() {
    let (server, socket) = setup();

    let sid = new_session(&socket);
    let snapshot = goto(&socket, &sid, &server.url());

    assert!(snapshot.contains("Signup"), "should show signup form, got: {snapshot}");

    // Type email and password
    type_text(&socket, &sid, "#email", "alice@example.com");
    type_text(&socket, &sid, "#password", "hunter2");

    // Click submit
    let result = click(&socket, &sid, "#submit-btn");
    eprintln!("after submit:\n{result}");

    // The status should show account created
    let text = snap(&socket, &sid, SnapMode::Text);
    eprintln!("text after submit:\n{text}");
    assert!(text.contains("Account created") || text.contains("usr_42"),
        "should show account created, got: {text}");
    assert!(API_SIGNUP_CALLS.load(Ordering::SeqCst) >= 1, "signup API should have been called");

    stop_daemon(&socket);
}
