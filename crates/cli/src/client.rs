use std::path::Path;

use braille_wire::{DaemonRequest, DaemonResponse};

use crate::paths;

/// Ensure the daemon is running. If not, start it and wait for the socket.
/// If the binary has been rebuilt since the daemon started, kill and restart it.
pub fn ensure_daemon_running() {
    braille_client::ensure_daemon_running(|| Ok(std::env::current_exe().expect("cannot determine current executable path")))
        .unwrap_or_else(|e| panic!("{e}"));
}

/// Send a request to the daemon and return the response.
pub fn send_request(request: &DaemonRequest) -> DaemonResponse {
    send_request_to(&paths::socket_path(), request)
}

/// Send a request to a daemon at a specific socket path.
pub fn send_request_to(socket_path: &Path, request: &DaemonRequest) -> DaemonResponse {
    braille_client::send_request_to(socket_path, request)
        .unwrap_or_else(|e| DaemonResponse::err(format!("invalid daemon response: {e}")))
}
