use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::time::Instant;

use braille_wire::{DaemonCommand, DaemonRequest, DaemonResponse};

use crate::engine_process::EngineProcess;
use crate::network::NetworkClient;
use crate::session::generate_session_id;

struct SessionHandle {
    engine: EngineProcess,
    net: NetworkClient,
    last_activity: Instant,
}

const IDLE_TIMEOUT_SECS: u64 = 30 * 60; // 30 minutes

/// Run the daemon. Binds to the Unix domain socket at `socket_path`,
/// writes PID to `pid_path`. Blocks forever until DaemonStop or signal.
pub fn run_daemon(socket_path: PathBuf, pid_path: PathBuf) {
    // Clean up stale socket if it exists.
    if socket_path.exists() {
        std::fs::remove_file(&socket_path).ok();
    }

    // Write PID file.
    std::fs::write(&pid_path, std::process::id().to_string()).ok();

    // Record the binary's mtime so the client can detect stale daemons.
    if let Ok(exe) = std::env::current_exe() {
        if let Ok(meta) = exe.metadata() {
            if let Ok(mtime) = meta.modified() {
                let nanos = mtime
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                std::fs::write(crate::paths::mtime_path(), nanos.to_string()).ok();
            }
        }
    }

    let listener = UnixListener::bind(&socket_path).unwrap_or_else(|e| {
        eprintln!("failed to bind socket {}: {e}", socket_path.display());
        std::process::exit(1);
    });

    // Set a timeout on accept so we can periodically reap idle sessions.
    listener.set_nonblocking(false).ok();

    let mut sessions: HashMap<String, SessionHandle> = HashMap::new();

    // Install signal handler for clean shutdown.
    let running = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    {
        let running = running.clone();
        ctrlc_handler(move || {
            running.store(false, std::sync::atomic::Ordering::SeqCst);
        });
    }

    eprintln!("braille daemon started (pid {})", std::process::id());

    for stream in listener.incoming() {
        if !running.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };

        // Read one JSON line.
        let mut reader = BufReader::new(&stream);
        let mut line = String::new();
        if reader.read_line(&mut line).is_err() || line.is_empty() {
            continue;
        }

        let request: DaemonRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = DaemonResponse::err(format!("invalid request: {e}"));
                write_response(&mut stream, &resp);
                continue;
            }
        };

        let response = dispatch(&mut sessions, request);

        // Check if we should stop.
        let should_stop = matches!(response.content.as_deref(), Some("daemon stopped"));
        write_response(&mut stream, &response);

        if should_stop {
            break;
        }

        // Reap idle sessions.
        reap_idle_sessions(&mut sessions);
    }

    // Cleanup.
    drop(sessions);
    std::fs::remove_file(&socket_path).ok();
    std::fs::remove_file(&pid_path).ok();
    eprintln!("braille daemon stopped");
}

fn dispatch(sessions: &mut HashMap<String, SessionHandle>, request: DaemonRequest) -> DaemonResponse {
    match request.command {
        DaemonCommand::NewSession => {
            let session_id = generate_session_id();
            let engine = EngineProcess::spawn();
            let net = NetworkClient::new();
            sessions.insert(
                session_id.clone(),
                SessionHandle {
                    engine,
                    net,
                    last_activity: Instant::now(),
                },
            );
            DaemonResponse::ok_with_session(session_id, None)
        }
        DaemonCommand::DaemonStop => {
            sessions.clear();
            DaemonResponse::ok("daemon stopped".to_string())
        }
        cmd => {
            let session_id = match &request.session_id {
                Some(id) => id.clone(),
                None => return DaemonResponse::err("session_id required".to_string()),
            };

            let is_close = matches!(cmd, DaemonCommand::Close);

            let handle = match sessions.get_mut(&session_id) {
                Some(h) => h,
                None => return DaemonResponse::err(format!("session not found: {session_id}")),
            };

            handle.last_activity = Instant::now();

            let response = handle.engine.send_command(cmd, &mut handle.net);

            if is_close {
                sessions.remove(&session_id);
            }

            response
        }
    }
}

fn write_response(stream: &mut std::os::unix::net::UnixStream, response: &DaemonResponse) {
    if let Ok(json) = serde_json::to_string(response) {
        let _ = stream.write_all(json.as_bytes());
        let _ = stream.write_all(b"\n");
        let _ = stream.flush();
    }
}

fn reap_idle_sessions(sessions: &mut HashMap<String, SessionHandle>) {
    let now = Instant::now();
    let idle: Vec<String> = sessions
        .iter()
        .filter(|(_, h)| now.duration_since(h.last_activity).as_secs() > IDLE_TIMEOUT_SECS)
        .map(|(id, _)| id.clone())
        .collect();
    for id in idle {
        eprintln!("reaping idle session: {id}");
        sessions.remove(&id);
    }
}

fn ctrlc_handler(f: impl Fn() + Send + Sync + 'static) {
    #[cfg(unix)]
    {
        use std::sync::OnceLock;
        static HANDLER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();
        HANDLER.get_or_init(|| Box::new(f));

        unsafe {
            libc_signal(libc::SIGTERM, signal_handler as *const () as libc::sighandler_t);
            libc_signal(libc::SIGINT, signal_handler as *const () as libc::sighandler_t);
        }

        extern "C" fn signal_handler(_: libc::c_int) {
            if let Some(handler) = HANDLER.get() {
                handler();
            }
        }

        unsafe fn libc_signal(sig: libc::c_int, handler: libc::sighandler_t) {
            libc::signal(sig, handler);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = f;
    }
}

#[cfg(unix)]
#[allow(non_camel_case_types)]
mod libc {
    pub use std::os::raw::c_int;
    pub type sighandler_t = usize;
    pub const SIGTERM: c_int = 15;
    pub const SIGINT: c_int = 2;

    extern "C" {
        pub fn signal(sig: c_int, handler: sighandler_t) -> sighandler_t;
    }
}
