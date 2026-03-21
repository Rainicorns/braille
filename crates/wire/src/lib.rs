use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub enum SnapMode {
    #[default]
    Accessibility,
    Interactive,
    Links,
    Forms,
    Headings,
    Text,
    Selector(String),
    Region(String),
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

/// A pending fetch request from the engine's JS runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FetchRequest {
    pub id: u64,
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

/// Response data to resolve a pending fetch request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FetchResponseData {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub url: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_roundtrip {
        ($val:expr, $ty:ty) => {
            let val = $val;
            let json = serde_json::to_string(&val).unwrap();
            let deserialized: $ty = serde_json::from_str(&json).unwrap();
            assert_eq!(val, deserialized);
        };
    }

    #[test]
    fn command_goto_roundtrip() {
        assert_roundtrip!(Command::Goto { url: "https://example.com".into() }, Command);
    }

    #[test]
    fn response_snapshot_roundtrip() {
        assert_roundtrip!(
            Response::Snapshot { content: "<h1>Hello</h1>".into(), url: "https://example.com".into() },
            Response
        );
    }

    #[test]
    fn snap_mode_accessibility_roundtrip() {
        assert_roundtrip!(SnapMode::Accessibility, SnapMode);
    }

    #[test]
    fn snap_mode_dom_roundtrip() {
        assert_roundtrip!(SnapMode::Dom, SnapMode);
    }

    #[test]
    fn snap_mode_markdown_roundtrip() {
        assert_roundtrip!(SnapMode::Markdown, SnapMode);
    }

    #[test]
    fn command_select_roundtrip() {
        assert_roundtrip!(Command::Select { selector: "#country".into(), value: "USA".into() }, Command);
    }

    #[test]
    fn command_focus_roundtrip() {
        assert_roundtrip!(Command::Focus { selector: "#search-input".into() }, Command);
    }

    #[test]
    fn http_method_get_roundtrip() {
        assert_roundtrip!(HttpMethod::Get, HttpMethod);
    }

    #[test]
    fn http_method_post_roundtrip() {
        assert_roundtrip!(HttpMethod::Post, HttpMethod);
    }

    #[test]
    fn navigate_request_get_roundtrip() {
        assert_roundtrip!(
            NavigateRequest { url: "https://example.com/page".into(), method: HttpMethod::Get, body: None, content_type: None },
            NavigateRequest
        );
    }

    #[test]
    fn navigate_request_post_roundtrip() {
        assert_roundtrip!(
            NavigateRequest {
                url: "https://example.com/submit".into(),
                method: HttpMethod::Post,
                body: Some("name=Alice&email=alice@example.com".into()),
                content_type: Some("application/x-www-form-urlencoded".into()),
            },
            NavigateRequest
        );
    }

    #[test]
    fn engine_action_none_roundtrip() {
        assert_roundtrip!(EngineAction::None, EngineAction);
    }

    #[test]
    fn engine_action_navigate_roundtrip() {
        assert_roundtrip!(
            EngineAction::Navigate(NavigateRequest {
                url: "https://example.com/next".into(),
                method: HttpMethod::Post,
                body: Some("data".into()),
                content_type: Some("text/plain".into()),
            }),
            EngineAction
        );
    }

    #[test]
    fn engine_action_error_roundtrip() {
        assert_roundtrip!(EngineAction::Error("Element not found".into()), EngineAction);
    }
}
