use serde::{Deserialize, Serialize};

/// State of a permission grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionState {
    Allow,
    Deny,
}

/// Engine-level permissions. Persists across page loads within a session.
#[derive(Debug, Clone)]
pub struct Permissions {
    pub geolocation: PermissionState,
    pub clipboard_read: PermissionState,
    pub clipboard_write: PermissionState,
    pub notifications: PermissionState,
    pub camera: PermissionState,
    pub microphone: PermissionState,
    /// When Allow: CORS violations are logged but requests go through (permissive).
    /// When Deny: CORS violations actually block the request.
    pub cors_enforcement: PermissionState,
    /// When Allow: CSP violations are logged but scripts run (permissive).
    /// When Deny: CSP violations block execution.
    pub csp_enforcement: PermissionState,
}

impl Default for Permissions {
    fn default() -> Self {
        Self {
            geolocation: PermissionState::Deny,
            clipboard_read: PermissionState::Deny,
            clipboard_write: PermissionState::Deny,
            notifications: PermissionState::Deny,
            camera: PermissionState::Deny,
            microphone: PermissionState::Deny,
            cors_enforcement: PermissionState::Allow,
            csp_enforcement: PermissionState::Allow,
        }
    }
}

impl Permissions {
    /// Look up a permission by name. Returns None if the name is unknown.
    pub fn get(&self, name: &str) -> Option<PermissionState> {
        match name {
            "geolocation" => Some(self.geolocation),
            "clipboard-read" => Some(self.clipboard_read),
            "clipboard-write" => Some(self.clipboard_write),
            "notifications" => Some(self.notifications),
            "camera" => Some(self.camera),
            "microphone" => Some(self.microphone),
            "cors" => Some(self.cors_enforcement),
            "csp" => Some(self.csp_enforcement),
            _ => None,
        }
    }

    /// Set a permission by name. Returns false if the name is unknown.
    pub fn set(&mut self, name: &str, state: PermissionState) -> bool {
        match name {
            "geolocation" => self.geolocation = state,
            "clipboard-read" => self.clipboard_read = state,
            "clipboard-write" => self.clipboard_write = state,
            "notifications" => self.notifications = state,
            "camera" => self.camera = state,
            "microphone" => self.microphone = state,
            "cors" => self.cors_enforcement = state,
            "csp" => self.csp_enforcement = state,
            _ => return false,
        }
        true
    }
}
