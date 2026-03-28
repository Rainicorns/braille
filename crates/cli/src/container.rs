//! Container lifecycle management via podman.
//!
//! Each browser session maps to a podman container running braille-engine.
//! Session persistence uses CRIU checkpoint/restore via podman.
//!
//! Container lifecycle:
//!   1. Create + start on `braille new`
//!   2. Attach stdin/stdout for commands
//!   3. Checkpoint on session pause (podman container checkpoint --export)
//!   4. Restore on next command (podman container restore --import)

use std::path::PathBuf;
use std::process::Command;

use crate::paths::runtime_dir;

const IMAGE_NAME: &str = "braille-engine:latest";

/// Path to checkpoints directory: `~/.braille/sessions/<sid>/`
fn checkpoint_path(session_id: &str) -> PathBuf {
    runtime_dir()
        .join("sessions")
        .join(session_id)
        .join("checkpoint.tar.gz")
}

/// Create a new container for a session.
/// Returns the container ID.
pub fn create_container(session_id: &str) -> Result<String, String> {
    let container_name = format!("braille-sess-{session_id}");
    let output = Command::new("podman")
        .args([
            "create",
            "--network=none",
            "--name",
            &container_name,
            IMAGE_NAME,
        ])
        .output()
        .map_err(|e| format!("failed to run podman create: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "podman create failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(container_id)
}

/// Start a created container.
pub fn start_container(container_name: &str) -> Result<(), String> {
    let output = Command::new("podman")
        .args(["start", container_name])
        .output()
        .map_err(|e| format!("failed to run podman start: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "podman start failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

/// Checkpoint a running container to a tar.gz file.
pub fn checkpoint_container(session_id: &str) -> Result<PathBuf, String> {
    let container_name = format!("braille-sess-{session_id}");
    let path = checkpoint_path(session_id);

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create checkpoint directory: {e}"))?;
    }

    let output = Command::new("podman")
        .args([
            "container",
            "checkpoint",
            &container_name,
            "--export",
            path.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("failed to run podman checkpoint: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "podman checkpoint failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(path)
}

/// Restore a container from a checkpoint file.
pub fn restore_container(session_id: &str) -> Result<String, String> {
    let path = checkpoint_path(session_id);
    if !path.exists() {
        return Err(format!("no checkpoint found for session {session_id}"));
    }

    let container_name = format!("braille-sess-{session_id}");

    let output = Command::new("podman")
        .args([
            "container",
            "restore",
            "--import",
            path.to_str().unwrap(),
            "--name",
            &container_name,
        ])
        .output()
        .map_err(|e| format!("failed to run podman restore: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "podman restore failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(container_id)
}

/// Remove a container (used during cleanup).
pub fn remove_container(session_id: &str) -> Result<(), String> {
    let container_name = format!("braille-sess-{session_id}");
    let output = Command::new("podman")
        .args(["rm", "-f", &container_name])
        .output()
        .map_err(|e| format!("failed to run podman rm: {e}"))?;

    if !output.status.success() {
        // Not an error if container doesn't exist
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("no such container") {
            return Err(format!("podman rm failed: {stderr}"));
        }
    }
    Ok(())
}

/// Check if a checkpoint file exists for a session.
pub fn has_checkpoint(session_id: &str) -> bool {
    checkpoint_path(session_id).exists()
}
