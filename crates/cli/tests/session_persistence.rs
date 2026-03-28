use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use braille_wire::{DaemonCommand, DaemonRequest, DaemonResponse, SnapMode};

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

fn stop_daemon(socket: &std::path::Path) {
    let _ = send(
        socket,
        &DaemonRequest {
            session_id: None,
            command: DaemonCommand::DaemonStop,
        },
    );
}

/// Start a daemon in a background thread, returning the socket path.
fn start_daemon_in_thread(tmp: &std::path::Path) -> PathBuf {
    std::fs::create_dir_all(tmp).ok();
    let socket = tmp.join("daemon.sock");
    let pid = tmp.join("daemon.pid");
    let sock_clone = socket.clone();
    std::thread::spawn(move || {
        braille_cli::daemon::run_daemon(sock_clone, pid);
    });
    // Wait for socket.
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if socket.exists() && UnixStream::connect(&socket).is_ok() {
            return socket;
        }
    }
    panic!("daemon did not start");
}

#[test]
fn session_persists_across_commands() {
    let tmp = std::env::temp_dir().join(format!("braille-test-persist-{}", std::process::id()));
    let socket = start_daemon_in_thread(&tmp);

    let sid = new_session(&socket);

    // Goto example.com
    let resp = send(
        &socket,
        &DaemonRequest {
            session_id: Some(sid.clone()),
            command: DaemonCommand::Goto {
                url: "https://example.com".to_string(),
                mode: SnapMode::Compact,
                record_path: None,
                clean: false,
            },
        },
    );
    assert!(resp.success, "Goto failed: {:?}", resp.error);
    let content1 = resp.content.expect("no content from goto");
    assert!(
        content1.contains("Example Domain") || content1.contains("example"),
        "goto content should contain Example Domain: {content1}"
    );

    // Snap — same session should return same page.
    let resp2 = send(
        &socket,
        &DaemonRequest {
            session_id: Some(sid.clone()),
            command: DaemonCommand::Snap {
                mode: SnapMode::Compact,
            },
        },
    );
    assert!(resp2.success, "Snap failed: {:?}", resp2.error);
    let content2 = resp2.content.expect("no content from snap");
    assert!(
        content2.contains("Example Domain") || content2.contains("example"),
        "snap should return same page content: {content2}"
    );

    // Close session.
    let resp3 = send(
        &socket,
        &DaemonRequest {
            session_id: Some(sid.clone()),
            command: DaemonCommand::Close,
        },
    );
    assert!(resp3.success);

    // Session should be gone now.
    let resp4 = send(
        &socket,
        &DaemonRequest {
            session_id: Some(sid),
            command: DaemonCommand::Snap {
                mode: SnapMode::Compact,
            },
        },
    );
    assert!(!resp4.success, "snap on closed session should fail");

    stop_daemon(&socket);
    std::fs::remove_dir_all(&tmp).ok();
}

#[test]
fn two_sessions_are_isolated() {
    let tmp = std::env::temp_dir().join(format!("braille-test-isolated-{}", std::process::id()));
    let socket = start_daemon_in_thread(&tmp);

    let sid1 = new_session(&socket);
    let sid2 = new_session(&socket);
    assert_ne!(sid1, sid2);

    // Load different pages in each session.
    let resp1 = send(
        &socket,
        &DaemonRequest {
            session_id: Some(sid1.clone()),
            command: DaemonCommand::Goto {
                url: "https://example.com".to_string(),
                mode: SnapMode::Compact,
                record_path: None,
                clean: false,
            },
        },
    );
    assert!(resp1.success);

    // Session 2: snap without loading anything — should return empty doc, not example.com.
    let resp2 = send(
        &socket,
        &DaemonRequest {
            session_id: Some(sid2.clone()),
            command: DaemonCommand::Snap {
                mode: SnapMode::Compact,
            },
        },
    );
    assert!(resp2.success);
    let content2 = resp2.content.unwrap_or_default();
    assert!(
        !content2.contains("Example Domain"),
        "session 2 should NOT have session 1's content: {content2}"
    );

    stop_daemon(&socket);
    std::fs::remove_dir_all(&tmp).ok();
}
