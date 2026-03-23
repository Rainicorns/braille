use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use braille_wire::{DaemonRequest, DaemonResponse};

use crate::paths;

/// Ensure the daemon is running. If not, start it and wait for the socket.
pub fn ensure_daemon_running() {
    let socket = paths::socket_path();
    let pid_file = paths::pid_path();

    // Try connecting to existing socket.
    if socket.exists() {
        if UnixStream::connect(&socket).is_ok() {
            return; // Daemon is alive.
        }
        // Stale socket — remove it.
        std::fs::remove_file(&socket).ok();
    }

    // Check PID file for stale process.
    if pid_file.exists() {
        if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                if !is_process_alive(pid) {
                    std::fs::remove_file(&pid_file).ok();
                }
            }
        }
    }

    // Start daemon as a background process.
    let exe = std::env::current_exe().expect("cannot determine current executable path");
    let log = paths::log_path();

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .expect("cannot open daemon log file");

    let stderr_file = log_file.try_clone().expect("cannot clone log file handle");

    let mut child = std::process::Command::new(exe)
        .args(["daemon", "start"])
        .stdout(log_file)
        .stderr(stderr_file)
        .stdin(std::process::Stdio::null())
        .spawn()
        .expect("failed to start daemon process");

    // Detach: we don't want to wait for the daemon, but clippy
    // requires we handle the Child. The daemon runs indefinitely.
    std::thread::spawn(move || {
        let _ = child.wait();
    });

    // Wait for socket to appear (up to 5 seconds).
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if socket.exists() && UnixStream::connect(&socket).is_ok() {
            return;
        }
    }

    panic!("daemon did not start within 5 seconds");
}

/// Send a request to the daemon and return the response.
pub fn send_request(request: &DaemonRequest) -> DaemonResponse {
    send_request_to(&paths::socket_path(), request)
}

/// Send a request to a daemon at a specific socket path.
pub fn send_request_to(socket_path: &Path, request: &DaemonRequest) -> DaemonResponse {
    let mut stream =
        UnixStream::connect(socket_path).unwrap_or_else(|e| panic!("cannot connect to daemon: {e}"));

    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .ok();

    let json = serde_json::to_string(request).expect("failed to serialize request");
    stream.write_all(json.as_bytes()).expect("failed to write to daemon");
    stream.write_all(b"\n").expect("failed to write newline");
    stream.flush().expect("failed to flush");

    let mut reader = BufReader::new(&stream);
    let mut response_line = String::new();
    reader
        .read_line(&mut response_line)
        .expect("failed to read response from daemon");

    serde_json::from_str(&response_line).unwrap_or_else(|e| {
        DaemonResponse::err(format!("invalid daemon response: {e}"))
    })
}

#[cfg(unix)]
fn is_process_alive(pid: i32) -> bool {
    // kill(pid, 0) checks if process exists without sending a signal.
    unsafe { libc_kill(pid, 0) == 0 }
}

#[cfg(unix)]
extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

#[cfg(not(unix))]
fn is_process_alive(_pid: i32) -> bool {
    false
}
