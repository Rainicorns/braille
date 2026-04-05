
#[cfg(test)]
use braille_engine::Engine;
#[cfg(test)]
use std::collections::HashMap;

/// Metadata and state for a browser session.
#[cfg(test)]
pub struct Session {
    pub engine: Engine,
    pub current_url: Option<String>,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
}

#[cfg(test)]
impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl Session {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine.include_hidden_content = true;
        Session {
            engine,
            current_url: None,
            history: Vec::new(),
            history_index: None,
        }
    }

    /// Push a URL onto history, truncating any forward history.
    /// Sets history_index to point at the new entry.
    /// Also updates the current_url field for backward compatibility.
    pub fn navigate(&mut self, url: String) {
        // If we have a history index, truncate everything after it (forward history)
        if let Some(idx) = self.history_index {
            self.history.truncate(idx + 1);
        }
        self.history.push(url.clone());
        self.history_index = Some(self.history.len() - 1);
        self.current_url = Some(url);
    }

    /// True if history_index > 0 (there is a previous page to go back to).
    pub fn can_go_back(&self) -> bool {
        matches!(self.history_index, Some(idx) if idx > 0)
    }

    /// True if history_index < history.len() - 1 (there is a forward page).
    pub fn can_go_forward(&self) -> bool {
        match self.history_index {
            Some(idx) => !self.history.is_empty() && idx < self.history.len() - 1,
            None => false,
        }
    }

    /// Decrements history_index and returns the URL to navigate to.
    /// Returns None if can't go back.
    pub fn go_back(&mut self) -> Option<&str> {
        if !self.can_go_back() {
            return None;
        }
        let idx = self.history_index.unwrap() - 1;
        self.history_index = Some(idx);
        self.current_url = Some(self.history[idx].clone());
        Some(&self.history[idx])
    }

    /// Increments history_index and returns the URL to navigate to.
    /// Returns None if can't go forward.
    pub fn go_forward(&mut self) -> Option<&str> {
        if !self.can_go_forward() {
            return None;
        }
        let idx = self.history_index.unwrap() + 1;
        self.history_index = Some(idx);
        self.current_url = Some(self.history[idx].clone());
        Some(&self.history[idx])
    }

    /// Returns current URL from history, or None if history is empty.
    pub fn current_url(&self) -> Option<&str> {
        match self.history_index {
            Some(idx) => Some(&self.history[idx]),
            None => None,
        }
    }
}

/// Manages multiple browser sessions in memory.
/// Used in tests; the daemon uses per-thread Sessions directly.
#[cfg(test)]
pub struct SessionManager {
    sessions: HashMap<String, Session>,
}

#[cfg(test)]
impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            sessions: HashMap::new(),
        }
    }

    pub fn new_session(&mut self) -> String {
        let session_id = generate_session_id();
        let session = Session::new();
        self.sessions.insert(session_id.clone(), session);
        session_id
    }

    pub fn get_session(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(id)
    }

    pub fn close_session(&mut self, id: &str) -> bool {
        self.sessions.remove(id).is_some()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

/// Generate a unique session ID with 128 bits of cryptographic randomness.
///
/// Format: "sess_" + 32 hex characters (128 bits from OS CSPRNG).
pub fn generate_session_id() -> String {
    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).expect("failed to generate random bytes for session ID");
    let hex: String = buf.iter().map(|b| format!("{:02x}", b)).collect();
    format!("sess_{}", hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_session_id() {
        let id1 = generate_session_id();
        let id2 = generate_session_id();

        assert!(id1.starts_with("sess_"));
        assert!(id2.starts_with("sess_"));
        assert_ne!(id1, id2, "consecutive IDs should be different");
    }

    #[test]
    fn session_id_has_sufficient_entropy() {
        let id = generate_session_id();
        let hex_part = &id["sess_".len()..];
        // 128 bits of entropy = 32 hex chars minimum
        assert!(
            hex_part.len() >= 32,
            "session ID hex portion should be at least 32 chars (128 bits) but got {} chars: {}",
            hex_part.len(),
            hex_part
        );
        // Verify it's actually valid hex
        assert!(
            hex_part.chars().all(|c| c.is_ascii_hexdigit()),
            "session ID hex portion should be valid hex: {}",
            hex_part
        );
    }

    #[test]
    fn test_session_manager_new_session() {
        let mut manager = SessionManager::new();
        assert_eq!(manager.session_count(), 0);

        let id = manager.new_session();
        assert_eq!(manager.session_count(), 1);
        assert!(id.starts_with("sess_"));

        let session = manager.get_session(&id);
        assert!(session.is_some());
        let s = session.unwrap();
        assert!(s.current_url.is_none());
        assert!(s.history.is_empty());
        assert!(s.history_index.is_none());
    }

    #[test]
    fn test_session_manager_get_session() {
        let mut manager = SessionManager::new();
        let id = manager.new_session();

        let session = manager.get_session(&id).unwrap();
        session.current_url = Some("https://example.com".to_string());

        let session_again = manager.get_session(&id).unwrap();
        assert_eq!(session_again.current_url, Some("https://example.com".to_string()));
    }

    #[test]
    fn test_session_manager_get_nonexistent_session() {
        let mut manager = SessionManager::new();
        let session = manager.get_session("nonexistent");
        assert!(session.is_none());
    }

    #[test]
    fn test_session_manager_close_session() {
        let mut manager = SessionManager::new();
        let id = manager.new_session();
        assert_eq!(manager.session_count(), 1);

        let closed = manager.close_session(&id);
        assert!(closed);
        assert_eq!(manager.session_count(), 0);

        let session = manager.get_session(&id);
        assert!(session.is_none());
    }

    #[test]
    fn test_session_manager_close_nonexistent_session() {
        let mut manager = SessionManager::new();
        let closed = manager.close_session("nonexistent");
        assert!(!closed);
    }

    #[test]
    fn test_session_manager_multiple_sessions() {
        let mut manager = SessionManager::new();
        let id1 = manager.new_session();
        let id2 = manager.new_session();
        let id3 = manager.new_session();

        assert_eq!(manager.session_count(), 3);
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);

        manager.get_session(&id1).unwrap().current_url = Some("url1".to_string());
        manager.get_session(&id2).unwrap().current_url = Some("url2".to_string());
        manager.get_session(&id3).unwrap().current_url = Some("url3".to_string());

        assert_eq!(manager.get_session(&id1).unwrap().current_url, Some("url1".to_string()));
        assert_eq!(manager.get_session(&id2).unwrap().current_url, Some("url2".to_string()));
        assert_eq!(manager.get_session(&id3).unwrap().current_url, Some("url3".to_string()));

        manager.close_session(&id2);
        assert_eq!(manager.session_count(), 2);
        assert!(manager.get_session(&id1).is_some());
        assert!(manager.get_session(&id2).is_none());
        assert!(manager.get_session(&id3).is_some());
    }

    #[test]
    fn test_session_new() {
        let session = Session::new();
        assert!(session.current_url.is_none());
        assert!(session.history.is_empty());
        assert!(session.history_index.is_none());
    }

    #[test]
    fn test_session_can_modify_engine() {
        let mut session = Session::new();
        session.engine.load_html("<html><body><h1>Test</h1></body></html>");
        let snapshot = session.engine.snapshot(braille_wire::SnapMode::Accessibility);
        assert!(snapshot.contains("Test"));
    }

    // --- Navigation history tests ---

    #[test]
    fn test_navigate_adds_to_history() {
        let mut session = Session::new();
        session.navigate("https://a.com".to_string());
        assert_eq!(session.history.len(), 1);
        assert_eq!(session.history[0], "https://a.com");
        assert_eq!(session.history_index, Some(0));
    }

    #[test]
    fn test_navigate_after_go_back_truncates_forward_history() {
        let mut session = Session::new();
        session.navigate("https://a.com".to_string());
        session.navigate("https://b.com".to_string());
        session.navigate("https://c.com".to_string());

        session.go_back(); // now at b.com
        session.navigate("https://d.com".to_string()); // should truncate c.com

        assert_eq!(session.history.len(), 3);
        assert_eq!(session.history[0], "https://a.com");
        assert_eq!(session.history[1], "https://b.com");
        assert_eq!(session.history[2], "https://d.com");
        assert_eq!(session.history_index, Some(2));
    }

    #[test]
    fn test_go_back_returns_previous_url() {
        let mut session = Session::new();
        session.navigate("https://a.com".to_string());
        session.navigate("https://b.com".to_string());

        let url = session.go_back().map(|s| s.to_string());
        assert_eq!(url.as_deref(), Some("https://a.com"));
        assert_eq!(session.history_index, Some(0));
    }

    #[test]
    fn test_go_forward_returns_next_url() {
        let mut session = Session::new();
        session.navigate("https://a.com".to_string());
        session.navigate("https://b.com".to_string());
        session.go_back();

        let url = session.go_forward().map(|s| s.to_string());
        assert_eq!(url.as_deref(), Some("https://b.com"));
        assert_eq!(session.history_index, Some(1));
    }

    #[test]
    fn test_go_back_at_start_returns_none() {
        let mut session = Session::new();
        session.navigate("https://a.com".to_string());

        let url = session.go_back();
        assert!(url.is_none());
        assert_eq!(session.history_index, Some(0));
    }

    #[test]
    fn test_go_forward_at_end_returns_none() {
        let mut session = Session::new();
        session.navigate("https://a.com".to_string());

        let url = session.go_forward();
        assert!(url.is_none());
        assert_eq!(session.history_index, Some(0));
    }

    #[test]
    fn test_can_go_back_and_forward_states() {
        let mut session = Session::new();

        // Empty history: can't go anywhere
        assert!(!session.can_go_back());
        assert!(!session.can_go_forward());

        // One entry: can't go anywhere
        session.navigate("https://a.com".to_string());
        assert!(!session.can_go_back());
        assert!(!session.can_go_forward());

        // Two entries, at the end: can go back, can't go forward
        session.navigate("https://b.com".to_string());
        assert!(session.can_go_back());
        assert!(!session.can_go_forward());

        // Go back: can't go back, can go forward
        session.go_back();
        assert!(!session.can_go_back());
        assert!(session.can_go_forward());

        // Three entries with middle position
        session.go_forward();
        session.navigate("https://c.com".to_string());
        session.go_back(); // at b.com
        assert!(session.can_go_back());
        assert!(session.can_go_forward());
    }

    #[test]
    fn test_multiple_navigations_build_full_history() {
        let mut session = Session::new();
        let urls = vec![
            "https://a.com",
            "https://b.com",
            "https://c.com",
            "https://d.com",
            "https://e.com",
        ];
        for url in &urls {
            session.navigate(url.to_string());
        }
        assert_eq!(session.history.len(), 5);
        for (i, url) in urls.iter().enumerate() {
            assert_eq!(session.history[i], *url);
        }
        assert_eq!(session.history_index, Some(4));
    }

    #[test]
    fn test_navigate_updates_current_url_field() {
        let mut session = Session::new();
        session.navigate("https://a.com".to_string());
        assert_eq!(session.current_url, Some("https://a.com".to_string()));

        session.navigate("https://b.com".to_string());
        assert_eq!(session.current_url, Some("https://b.com".to_string()));
    }

    #[test]
    fn test_empty_history_defaults() {
        let session = Session::new();
        assert!(session.history.is_empty());
        assert_eq!(session.history_index, None);
        assert!(session.current_url().is_none());
        assert!(session.current_url.is_none());
    }

    #[test]
    fn test_go_back_on_empty_history_returns_none() {
        let mut session = Session::new();
        assert!(session.go_back().is_none());
    }

    #[test]
    fn test_go_forward_on_empty_history_returns_none() {
        let mut session = Session::new();
        assert!(session.go_forward().is_none());
    }

    #[test]
    fn test_current_url_method_returns_current() {
        let mut session = Session::new();
        assert!(session.current_url().is_none());

        session.navigate("https://a.com".to_string());
        assert_eq!(session.current_url(), Some("https://a.com"));

        session.navigate("https://b.com".to_string());
        assert_eq!(session.current_url(), Some("https://b.com"));

        session.go_back();
        assert_eq!(session.current_url(), Some("https://a.com"));
    }

    #[test]
    fn test_go_back_updates_current_url_field() {
        let mut session = Session::new();
        session.navigate("https://a.com".to_string());
        session.navigate("https://b.com".to_string());

        session.go_back();
        assert_eq!(session.current_url, Some("https://a.com".to_string()));
    }

    #[test]
    fn test_go_forward_updates_current_url_field() {
        let mut session = Session::new();
        session.navigate("https://a.com".to_string());
        session.navigate("https://b.com".to_string());
        session.go_back();

        session.go_forward();
        assert_eq!(session.current_url, Some("https://b.com".to_string()));
    }
}
