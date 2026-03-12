use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Command {
    Goto { url: String },
    Click { selector: String },
    Type { selector: String, text: String },
    Select { selector: String, value: String },
    Focus { selector: String },
    Snap { mode: SnapMode },
    Back,
    Forward,
    Close,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
pub enum SnapMode {
    #[default]
    Accessibility,
    Dom,
    Markdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Response {
    SessionCreated { session_id: String },
    Snapshot { content: String, url: String },
    Error { message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
}

// NOTE: Simplified version without headers. If more complex request handling
// is needed (e.g., custom headers, authentication), extend this struct.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NavigateRequest {
    pub url: String,
    pub method: HttpMethod,
    pub body: Option<String>,
    pub content_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EngineAction {
    None,
    Navigate(NavigateRequest),
    Error(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_goto_roundtrip() {
        let cmd = Command::Goto { url: "https://example.com".into() };
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn response_snapshot_roundtrip() {
        let resp = Response::Snapshot {
            content: "<h1>Hello</h1>".into(),
            url: "https://example.com".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let deserialized: Response = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, deserialized);
    }

    #[test]
    fn snap_mode_accessibility_roundtrip() {
        let mode = SnapMode::Accessibility;
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: SnapMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, deserialized);
    }

    #[test]
    fn snap_mode_dom_roundtrip() {
        let mode = SnapMode::Dom;
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: SnapMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, deserialized);
    }

    #[test]
    fn snap_mode_markdown_roundtrip() {
        let mode = SnapMode::Markdown;
        let json = serde_json::to_string(&mode).unwrap();
        let deserialized: SnapMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, deserialized);
    }

    #[test]
    fn command_select_roundtrip() {
        let cmd = Command::Select {
            selector: "#country".into(),
            value: "USA".into(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn command_focus_roundtrip() {
        let cmd = Command::Focus {
            selector: "#search-input".into(),
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn http_method_get_roundtrip() {
        let method = HttpMethod::Get;
        let json = serde_json::to_string(&method).unwrap();
        let deserialized: HttpMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(method, deserialized);
    }

    #[test]
    fn http_method_post_roundtrip() {
        let method = HttpMethod::Post;
        let json = serde_json::to_string(&method).unwrap();
        let deserialized: HttpMethod = serde_json::from_str(&json).unwrap();
        assert_eq!(method, deserialized);
    }

    #[test]
    fn navigate_request_get_roundtrip() {
        let req = NavigateRequest {
            url: "https://example.com/page".into(),
            method: HttpMethod::Get,
            body: None,
            content_type: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: NavigateRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, deserialized);
    }

    #[test]
    fn navigate_request_post_roundtrip() {
        let req = NavigateRequest {
            url: "https://example.com/submit".into(),
            method: HttpMethod::Post,
            body: Some("name=Alice&email=alice@example.com".into()),
            content_type: Some("application/x-www-form-urlencoded".into()),
        };
        let json = serde_json::to_string(&req).unwrap();
        let deserialized: NavigateRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, deserialized);
    }

    #[test]
    fn engine_action_none_roundtrip() {
        let action = EngineAction::None;
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: EngineAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }

    #[test]
    fn engine_action_navigate_roundtrip() {
        let action = EngineAction::Navigate(NavigateRequest {
            url: "https://example.com/next".into(),
            method: HttpMethod::Post,
            body: Some("data".into()),
            content_type: Some("text/plain".into()),
        });
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: EngineAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }

    #[test]
    fn engine_action_error_roundtrip() {
        let action = EngineAction::Error("Element not found".into());
        let json = serde_json::to_string(&action).unwrap();
        let deserialized: EngineAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, deserialized);
    }
}
