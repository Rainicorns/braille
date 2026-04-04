use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use braille_wire::{DaemonRequest, DaemonResponse};

/// Returns `~/.braille/`, creating it if needed.
fn runtime_dir() -> PathBuf {
    let dir = home_dir().join(".braille");
    if !dir.exists() {
        use std::fs::DirBuilder;
        use std::os::unix::fs::DirBuilderExt;
        DirBuilder::new()
            .mode(0o700)
            .recursive(true)
            .create(&dir)
            .ok();
    }
    dir
}

fn socket_path() -> PathBuf {
    runtime_dir().join("daemon.sock")
}

fn pid_path() -> PathBuf {
    runtime_dir().join("daemon.pid")
}

fn log_path() -> PathBuf {
    runtime_dir().join("daemon.log")
}

fn mtime_path() -> PathBuf {
    runtime_dir().join("daemon.mtime")
}

fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

/// Check if a process is alive.
fn is_process_alive(pid: i32) -> bool {
    unsafe { libc_kill(pid, 0) == 0 }
}

extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

/// Check if the running daemon was built from an older binary.
fn is_daemon_stale() -> bool {
    let mtime_file = mtime_path();
    let recorded = match std::fs::read_to_string(&mtime_file) {
        Ok(s) => match s.trim().parse::<u128>() {
            Ok(n) => n,
            Err(_) => return false,
        },
        Err(_) => return false,
    };

    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(_) => return false,
    };
    let current = match exe.metadata().and_then(|m| m.modified()) {
        Ok(mtime) => mtime
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos(),
        Err(_) => return false,
    };

    current > recorded
}

/// Kill the daemon process via its PID file.
fn kill_daemon(pid_file: &std::path::Path) {
    if let Ok(pid_str) = std::fs::read_to_string(pid_file) {
        if let Ok(pid) = pid_str.trim().parse::<i32>() {
            if is_process_alive(pid) {
                unsafe {
                    libc_kill(pid, 15); // SIGTERM
                }
                for _ in 0..20 {
                    std::thread::sleep(Duration::from_millis(50));
                    if !is_process_alive(pid) {
                        break;
                    }
                }
            }
        }
    }
    std::fs::remove_file(pid_file).ok();
}

/// Ensure the daemon is running, starting it if necessary.
fn ensure_daemon_running() -> Result<(), String> {
    let socket = socket_path();
    let pid_file = pid_path();

    if socket.exists() {
        if UnixStream::connect(&socket).is_ok() {
            if is_daemon_stale() {
                eprintln!("braille binary updated — restarting daemon");
                kill_daemon(&pid_file);
                std::fs::remove_file(&socket).ok();
            } else {
                return Ok(());
            }
        } else {
            std::fs::remove_file(&socket).ok();
        }
    }

    if pid_file.exists() {
        if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                if !is_process_alive(pid) {
                    std::fs::remove_file(&pid_file).ok();
                }
            }
        }
    }

    // Find the braille binary. Prefer one next to our binary, fall back to PATH.
    let exe = find_braille_binary()?;
    let log = log_path();

    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log)
        .map_err(|e| format!("cannot open daemon log: {e}"))?;

    let stderr_file = log_file
        .try_clone()
        .map_err(|e| format!("cannot clone log handle: {e}"))?;

    let mut child = std::process::Command::new(exe)
        .args(["daemon", "start"])
        .stdout(log_file)
        .stderr(stderr_file)
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to start daemon: {e}"))?;

    // Detach the child — the daemon runs indefinitely.
    std::thread::spawn(move || {
        let _ = child.wait();
    });

    // Wait for socket to appear (up to 5 seconds).
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if socket.exists() && UnixStream::connect(&socket).is_ok() {
            return Ok(());
        }
    }

    Err("daemon did not start within 5 seconds".into())
}

/// Find the `braille` binary. Check next to our own binary first, then PATH.
fn find_braille_binary() -> Result<PathBuf, String> {
    // Check next to our own binary.
    if let Ok(self_exe) = std::env::current_exe() {
        if let Some(dir) = self_exe.parent() {
            let sibling = dir.join("braille");
            if sibling.exists() {
                return Ok(sibling);
            }
        }
    }

    // Fall back to PATH lookup via `which`.
    let output = std::process::Command::new("which")
        .arg("braille")
        .output()
        .map_err(|e| format!("cannot search PATH for braille: {e}"))?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    Err("cannot find 'braille' binary — build it first with `cargo build -p braille-cli`".into())
}

/// Send a request to the daemon (blocking I/O).
fn send_request_blocking(request: &DaemonRequest) -> Result<DaemonResponse, String> {
    ensure_daemon_running()?;

    let socket = socket_path();
    let mut stream =
        UnixStream::connect(&socket).map_err(|e| format!("cannot connect to daemon: {e}"))?;

    stream.set_read_timeout(Some(Duration::from_secs(60))).ok();

    let json = serde_json::to_string(request).map_err(|e| format!("serialize error: {e}"))?;
    stream
        .write_all(json.as_bytes())
        .map_err(|e| format!("write error: {e}"))?;
    stream
        .write_all(b"\n")
        .map_err(|e| format!("write error: {e}"))?;
    stream.flush().map_err(|e| format!("flush error: {e}"))?;

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("read error: {e}"))?;

    serde_json::from_str(&line).map_err(|e| format!("invalid daemon response: {e}"))
}

/// Send a request to the daemon, running blocking I/O on a spawn_blocking thread.
pub async fn send_request(request: &DaemonRequest) -> Result<DaemonResponse, String> {
    let request = request.clone();
    tokio::task::spawn_blocking(move || send_request_blocking(&request))
        .await
        .map_err(|e| format!("task join error: {e}"))?
}
