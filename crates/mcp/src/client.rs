use braille_wire::{DaemonRequest, DaemonResponse};

/// Find the `braille` binary. Check next to our own binary first, then PATH.
fn find_braille_binary() -> Result<std::path::PathBuf, String> {
    if let Ok(self_exe) = std::env::current_exe() {
        if let Some(dir) = self_exe.parent() {
            let sibling = dir.join("braille");
            if sibling.exists() {
                return Ok(sibling);
            }
        }
    }

    let output = std::process::Command::new("which")
        .arg("braille")
        .output()
        .map_err(|e| format!("cannot search PATH for braille: {e}"))?;

    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(std::path::PathBuf::from(path));
        }
    }

    Err("cannot find 'braille' binary — build it first with `cargo build -p braille-cli`".into())
}

/// Send a request to the daemon, running blocking I/O on a spawn_blocking thread.
pub async fn send_request(request: &DaemonRequest) -> Result<DaemonResponse, String> {
    let request = request.clone();
    tokio::task::spawn_blocking(move || {
        braille_client::ensure_daemon_running(find_braille_binary)?;
        braille_client::send_request_blocking(&request)
    })
    .await
    .map_err(|e| format!("task join error: {e}"))?
}
