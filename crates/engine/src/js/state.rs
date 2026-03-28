use std::collections::HashMap;

/// A pending fetch request from JS fetch() API.
pub struct PendingFetch {
    pub id: u64,
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    /// JS-side ID for the resolve callback (stored in global __braille_fetch_resolvers)
    pub resolve_id: u32,
    /// JS-side ID for the reject callback (stored in global __braille_fetch_rejecters)
    pub reject_id: u32,
}

/// A timer entry (setTimeout/setInterval).
pub struct TimerEntry {
    pub id: u32,
    pub callback_code: String,
    pub delay_ms: u64,
    pub registered_at: u64,
    pub is_interval: bool,
}

/// A pending worker spawn request from JS `new Worker(url)`.
pub struct PendingWorkerSpawn {
    pub url: String,
}

/// A pending message to post to a worker.
pub struct PendingWorkerMessage {
    pub worker_id: u64,
    pub data: String,
}

/// A pending worker termination request.
pub struct PendingWorkerTerminate {
    pub worker_id: u64,
}

/// Engine-side state shared across all JS operations.
/// This replaces Boa's RealmState — all state that was stored in
/// Realm::host_defined() is now stored here in plain Rust.
pub struct EngineState {
    pub console_buffer: Vec<String>,
    pub pending_fetches: Vec<PendingFetch>,
    pub next_fetch_id: u64,
    pub timer_entries: HashMap<u32, TimerEntry>,
    pub next_timer_id: u32,
    pub timer_current_time_ms: u64,
    pub location_url: String,
    pub iframe_src_content: HashMap<String, String>,
    pub pending_worker_spawns: Vec<PendingWorkerSpawn>,
    pub pending_worker_messages: Vec<PendingWorkerMessage>,
    pub pending_worker_terminates: Vec<PendingWorkerTerminate>,
}

impl EngineState {
    pub fn new() -> Self {
        Self {
            console_buffer: Vec::new(),
            pending_fetches: Vec::new(),
            next_fetch_id: 1,
            timer_entries: HashMap::new(),
            next_timer_id: 1,
            timer_current_time_ms: 0,
            location_url: String::from("about:blank"),
            iframe_src_content: HashMap::new(),
            pending_worker_spawns: Vec::new(),
            pending_worker_messages: Vec::new(),
            pending_worker_terminates: Vec::new(),
        }
    }
}
