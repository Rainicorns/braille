use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::paths::runtime_dir;

/// Persistent metadata for a browser session stored on disk.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionMetadata {
    pub session_id: String,
    pub created_at: u64,
    pub last_accessed: u64,
    /// Container ID when running in container mode. None for local process sessions.
    pub container_id: Option<String>,
}

/// Directory for all sessions: `~/.braille/sessions/`
fn sessions_dir() -> PathBuf {
    runtime_dir().join("sessions")
}

/// Directory for a specific session: `~/.braille/sessions/<session-id>/`
fn session_dir(session_id: &str) -> PathBuf {
    sessions_dir().join(session_id)
}

/// Path to the metadata file: `~/.braille/sessions/<session-id>/metadata.json`
fn metadata_path(session_id: &str) -> PathBuf {
    session_dir(session_id).join("metadata.json")
}

fn now_epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Create a new session directory and write initial metadata.
/// Returns the metadata that was written.
pub fn create_session(session_id: &str) -> SessionMetadata {
    let dir = session_dir(session_id);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| {
        panic!("failed to create session directory {}: {e}", dir.display())
    });

    let now = now_epoch_secs();
    let metadata = SessionMetadata {
        session_id: session_id.to_string(),
        created_at: now,
        last_accessed: now,
        container_id: None,
    };

    write_metadata(&metadata);
    metadata
}

/// Read session metadata from disk.
pub fn read_metadata(session_id: &str) -> Option<SessionMetadata> {
    let path = metadata_path(session_id);
    let data = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

/// Write session metadata to disk.
pub fn write_metadata(metadata: &SessionMetadata) {
    let path = metadata_path(&metadata.session_id);
    let json = serde_json::to_string_pretty(metadata)
        .unwrap_or_else(|e| panic!("failed to serialize session metadata: {e}"));
    std::fs::write(&path, json)
        .unwrap_or_else(|e| panic!("failed to write metadata to {}: {e}", path.display()));
}

/// Update the last_accessed timestamp for a session.
pub fn touch_session(session_id: &str) {
    if let Some(mut metadata) = read_metadata(session_id) {
        metadata.last_accessed = now_epoch_secs();
        write_metadata(&metadata);
    }
}

/// Delete a session directory and all its contents.
pub fn delete_session(session_id: &str) {
    let dir = session_dir(session_id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap_or_else(|e| {
            panic!("failed to remove session directory {}: {e}", dir.display())
        });
    }
}

/// List all session IDs that have metadata on disk.
pub fn list_sessions() -> Vec<String> {
    let dir = sessions_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut ids = Vec::new();
    for entry in std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("failed to read sessions directory {}: {e}", dir.display()))
    {
        let entry = entry.unwrap_or_else(|e| panic!("failed to read directory entry: {e}"));
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                // Only include directories that have a metadata.json
                if metadata_path(name).exists() {
                    ids.push(name.to_string());
                }
            }
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::generate_session_id;
    use std::sync::Mutex;

    /// Mutex to serialize tests that modify HOME env var.
    static HOME_LOCK: Mutex<()> = Mutex::new(());

    /// Helper: override HOME to a unique temp dir so tests don't pollute real ~/.braille/
    /// Uses a mutex because env vars are process-global.
    fn with_temp_home<F: FnOnce()>(f: F) {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = std::env::temp_dir().join(format!(
            "braille-test-{}-{}",
            std::process::id(),
            n
        ));
        std::fs::create_dir_all(&tmp).unwrap();

        let _guard = HOME_LOCK.lock().unwrap();
        let old_home = std::env::var("HOME").ok();
        unsafe { std::env::set_var("HOME", &tmp); }
        f();
        unsafe { std::env::set_var("HOME", old_home.unwrap_or_default()); }
        drop(_guard);

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn create_and_read_session_metadata() {
        with_temp_home(|| {
            let sid = generate_session_id();
            let meta = create_session(&sid);

            assert_eq!(meta.session_id, sid);
            assert!(meta.container_id.is_none());
            assert!(meta.created_at > 0);
            assert_eq!(meta.created_at, meta.last_accessed);

            let read_back = read_metadata(&sid).expect("metadata should exist after create");
            assert_eq!(meta, read_back);
        });
    }

    #[test]
    fn delete_session_removes_directory() {
        with_temp_home(|| {
            let sid = generate_session_id();
            create_session(&sid);

            assert!(read_metadata(&sid).is_some());

            delete_session(&sid);

            assert!(read_metadata(&sid).is_none());
            assert!(!session_dir(&sid).exists());
        });
    }

    #[test]
    fn delete_nonexistent_session_is_noop() {
        with_temp_home(|| {
            delete_session("sess_nonexistent");
        });
    }

    #[test]
    fn list_sessions_returns_created_sessions() {
        with_temp_home(|| {
            let sid1 = generate_session_id();
            let sid2 = generate_session_id();
            create_session(&sid1);
            create_session(&sid2);

            let mut ids = list_sessions();
            ids.sort();
            let mut expected = vec![sid1.clone(), sid2.clone()];
            expected.sort();
            assert_eq!(ids, expected);
        });
    }

    #[test]
    fn list_sessions_empty_when_no_sessions() {
        with_temp_home(|| {
            let ids = list_sessions();
            assert!(ids.is_empty());
        });
    }

    #[test]
    fn touch_session_updates_last_accessed() {
        with_temp_home(|| {
            let sid = generate_session_id();
            let meta = create_session(&sid);
            let original_accessed = meta.last_accessed;

            // Sleep briefly so timestamp changes
            std::thread::sleep(std::time::Duration::from_millis(1100));
            touch_session(&sid);

            let updated = read_metadata(&sid).unwrap();
            assert!(
                updated.last_accessed >= original_accessed,
                "last_accessed should be >= original after touch"
            );
            assert_eq!(updated.created_at, meta.created_at);
        });
    }

    #[test]
    fn session_metadata_with_container_id() {
        with_temp_home(|| {
            let sid = generate_session_id();
            let mut meta = create_session(&sid);
            meta.container_id = Some("abc123def456".to_string());
            write_metadata(&meta);

            let read_back = read_metadata(&sid).unwrap();
            assert_eq!(read_back.container_id, Some("abc123def456".to_string()));
        });
    }
}
