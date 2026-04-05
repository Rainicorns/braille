use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use braille_wire::{DaemonRequest, DaemonResponse};

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

pub fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
}

pub fn runtime_dir() -> PathBuf {
    let dir = home_dir().join(".braille");
    if !dir.exists() {
        #[cfg(unix)]
        {
            use std::fs::DirBuilder;
            use std::os::unix::fs::DirBuilderExt;
            DirBuilder::new()
                .mode(0o700)
                .recursive(true)
                .create(&dir)
                .ok();
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir_all(&dir).ok();
        }
    }
    dir
}

pub fn socket_path() -> PathBuf {
    runtime_dir().join("daemon.sock")
}

pub fn pid_path() -> PathBuf {
    runtime_dir().join("daemon.pid")
}

pub fn log_path() -> PathBuf {
    runtime_dir().join("daemon.log")
}

pub fn mtime_path() -> PathBuf {
    runtime_dir().join("daemon.mtime")
}

// ---------------------------------------------------------------------------
// Process helpers
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub fn is_process_alive(pid: i32) -> bool {
    unsafe { libc_kill(pid, 0) == 0 }
}

#[cfg(unix)]
extern "C" {
    #[link_name = "kill"]
    fn libc_kill(pid: i32, sig: i32) -> i32;
}

#[cfg(not(unix))]
pub fn is_process_alive(_pid: i32) -> bool {
    false
}

pub fn is_daemon_stale() -> bool {
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

pub fn kill_daemon(pid_file: &Path) {
    if let Ok(pid_str) = std::fs::read_to_string(pid_file) {
        if let Ok(pid) = pid_str.trim().parse::<i32>() {
            if is_process_alive(pid) {
                #[cfg(unix)]
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

// ---------------------------------------------------------------------------
// Daemon lifecycle
// ---------------------------------------------------------------------------

/// Ensure the daemon is running, starting it if needed.
///
/// `find_binary` is called to locate the `braille` binary when we need to spawn.
/// Returns `Ok(())` if the daemon is reachable, or `Err` with a message.
pub fn ensure_daemon_running<F>(find_binary: F) -> Result<(), String>
where
    F: FnOnce() -> Result<PathBuf, String>,
{
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

    let exe = find_binary()?;
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

    std::thread::spawn(move || {
        let _ = child.wait();
    });

    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if socket.exists() && UnixStream::connect(&socket).is_ok() {
            return Ok(());
        }
    }

    Err("daemon did not start within 5 seconds".into())
}

// ---------------------------------------------------------------------------
// IPC
// ---------------------------------------------------------------------------

/// Send a request to the daemon at the default socket path.
pub fn send_request_blocking(request: &DaemonRequest) -> Result<DaemonResponse, String> {
    send_request_to(&socket_path(), request)
}

/// Send a request to a daemon at a specific socket path.
pub fn send_request_to(socket_path: &Path, request: &DaemonRequest) -> Result<DaemonResponse, String> {
    let mut stream =
        UnixStream::connect(socket_path).map_err(|e| format!("cannot connect to daemon: {e}"))?;

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
