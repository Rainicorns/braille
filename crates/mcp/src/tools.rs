use rmcp::model::{CallToolResult, Content, Tool};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;

use braille_wire::{DaemonCommand, DaemonRequest, DaemonResponse, SnapMode};

use crate::client;

// ---------------------------------------------------------------------------
// Tool parameter structs (derive JsonSchema for automatic schema generation)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
struct BrowseParams {
    /// The URL to navigate to.
    url: String,
    /// Snapshot mode: compact (default), accessibility, interactive, links, forms,
    /// headings, text, dom, markdown, or selector:CSS / region:LABEL.
    mode: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GotoParams {
    /// Session ID returned by braille_browse.
    session_id: String,
    /// The URL to navigate to.
    url: String,
    /// Snapshot mode (see braille_browse for options).
    mode: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ClickParams {
    /// Session ID.
    session_id: String,
    /// CSS selector of the element to click.
    selector: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TypeParams {
    /// Session ID.
    session_id: String,
    /// CSS selector of the input element.
    selector: String,
    /// Text to type into the element.
    text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SelectParams {
    /// Session ID.
    session_id: String,
    /// CSS selector of the select element.
    selector: String,
    /// Value to select.
    value: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SnapParams {
    /// Session ID.
    session_id: String,
    /// Snapshot mode (see braille_browse for options).
    mode: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct EvalParams {
    /// Session ID.
    session_id: String,
    /// JavaScript code to evaluate in the page context.
    code: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CloseParams {
    /// Session ID.
    session_id: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Return the list of MCP tools this server exposes.
pub fn tool_list() -> Vec<Tool> {
    vec![
        Tool::new(
            "braille_browse",
            "Create a new browser session and navigate to a URL. Returns the session ID and a page snapshot.",
            rmcp::handler::server::tool::schema_for_type::<BrowseParams>(),
        ),
        Tool::new(
            "braille_goto",
            "Navigate an existing session to a new URL.",
            rmcp::handler::server::tool::schema_for_type::<GotoParams>(),
        ),
        Tool::new(
            "braille_click",
            "Click an element by CSS selector.",
            rmcp::handler::server::tool::schema_for_type::<ClickParams>(),
        ),
        Tool::new(
            "braille_type",
            "Type text into an input element.",
            rmcp::handler::server::tool::schema_for_type::<TypeParams>(),
        ),
        Tool::new(
            "braille_select",
            "Select a dropdown option by value.",
            rmcp::handler::server::tool::schema_for_type::<SelectParams>(),
        ),
        Tool::new(
            "braille_snap",
            "Take a snapshot of the current page.",
            rmcp::handler::server::tool::schema_for_type::<SnapParams>(),
        ),
        Tool::new(
            "braille_eval",
            "Evaluate JavaScript in the page context.",
            rmcp::handler::server::tool::schema_for_type::<EvalParams>(),
        ),
        Tool::new(
            "braille_close",
            "Close a browser session.",
            rmcp::handler::server::tool::schema_for_type::<CloseParams>(),
        ),
    ]
}

/// Dispatch a tool call by name.
pub async fn call_tool(name: &str, args: serde_json::Value) -> CallToolResult {
    match name {
        "braille_browse" => handle_browse(args).await,
        "braille_goto" => handle_goto(args).await,
        "braille_click" => handle_click(args).await,
        "braille_type" => handle_type(args).await,
        "braille_select" => handle_select(args).await,
        "braille_snap" => handle_snap(args).await,
        "braille_eval" => handle_eval(args).await,
        "braille_close" => handle_close(args).await,
        _ => tool_error(format!("unknown tool: {name}")),
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn handle_browse(args: serde_json::Value) -> CallToolResult {
    let params: BrowseParams = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return tool_error(format!("invalid parameters: {e}")),
    };

    // Step 1: create a new session.
    let new_req = DaemonRequest {
        session_id: None,
        command: DaemonCommand::NewSession,
    };
    let new_resp = match client::send_request(&new_req).await {
        Ok(r) => r,
        Err(e) => return tool_error(e),
    };
    if !new_resp.success {
        return tool_error(
            new_resp
                .error
                .unwrap_or_else(|| "failed to create session".into()),
        );
    }
    let session_id = match new_resp.session_id {
        Some(id) => id,
        None => return tool_error("no session_id in response".into()),
    };

    // Step 2: navigate.
    let mode = parse_snap_mode(params.mode.as_deref());
    let goto_req = DaemonRequest {
        session_id: Some(session_id.clone()),
        command: DaemonCommand::Goto {
            url: params.url,
            mode,
            record_path: None,
            clean: false,
        },
    };
    let goto_resp = match client::send_request(&goto_req).await {
        Ok(r) => r,
        Err(e) => return tool_error(e),
    };

    format_response(&session_id, &goto_resp)
}

async fn handle_goto(args: serde_json::Value) -> CallToolResult {
    let params: GotoParams = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return tool_error(format!("invalid parameters: {e}")),
    };
    let mode = parse_snap_mode(params.mode.as_deref());
    let req = DaemonRequest {
        session_id: Some(params.session_id.clone()),
        command: DaemonCommand::Goto {
            url: params.url,
            mode,
            record_path: None,
            clean: false,
        },
    };
    let resp = match client::send_request(&req).await {
        Ok(r) => r,
        Err(e) => return tool_error(e),
    };
    format_response(&params.session_id, &resp)
}

async fn handle_click(args: serde_json::Value) -> CallToolResult {
    let params: ClickParams = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return tool_error(format!("invalid parameters: {e}")),
    };
    let req = DaemonRequest {
        session_id: Some(params.session_id.clone()),
        command: DaemonCommand::Click {
            selector: params.selector,
        },
    };
    let resp = match client::send_request(&req).await {
        Ok(r) => r,
        Err(e) => return tool_error(e),
    };
    format_response(&params.session_id, &resp)
}

async fn handle_type(args: serde_json::Value) -> CallToolResult {
    let params: TypeParams = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return tool_error(format!("invalid parameters: {e}")),
    };
    let req = DaemonRequest {
        session_id: Some(params.session_id.clone()),
        command: DaemonCommand::Type {
            selector: params.selector,
            text: params.text,
        },
    };
    let resp = match client::send_request(&req).await {
        Ok(r) => r,
        Err(e) => return tool_error(e),
    };
    format_response(&params.session_id, &resp)
}

async fn handle_select(args: serde_json::Value) -> CallToolResult {
    let params: SelectParams = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return tool_error(format!("invalid parameters: {e}")),
    };
    let req = DaemonRequest {
        session_id: Some(params.session_id.clone()),
        command: DaemonCommand::Select {
            selector: params.selector,
            value: params.value,
        },
    };
    let resp = match client::send_request(&req).await {
        Ok(r) => r,
        Err(e) => return tool_error(e),
    };
    format_response(&params.session_id, &resp)
}

async fn handle_snap(args: serde_json::Value) -> CallToolResult {
    let params: SnapParams = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return tool_error(format!("invalid parameters: {e}")),
    };
    let mode = parse_snap_mode(params.mode.as_deref());
    let req = DaemonRequest {
        session_id: Some(params.session_id.clone()),
        command: DaemonCommand::Snap { mode },
    };
    let resp = match client::send_request(&req).await {
        Ok(r) => r,
        Err(e) => return tool_error(e),
    };
    format_response(&params.session_id, &resp)
}

async fn handle_eval(args: serde_json::Value) -> CallToolResult {
    let params: EvalParams = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return tool_error(format!("invalid parameters: {e}")),
    };
    let req = DaemonRequest {
        session_id: Some(params.session_id.clone()),
        command: DaemonCommand::Eval { code: params.code },
    };
    let resp = match client::send_request(&req).await {
        Ok(r) => r,
        Err(e) => return tool_error(e),
    };
    format_response(&params.session_id, &resp)
}

async fn handle_close(args: serde_json::Value) -> CallToolResult {
    let params: CloseParams = match serde_json::from_value(args) {
        Ok(p) => p,
        Err(e) => return tool_error(format!("invalid parameters: {e}")),
    };
    let req = DaemonRequest {
        session_id: Some(params.session_id.clone()),
        command: DaemonCommand::Close,
    };
    let resp = match client::send_request(&req).await {
        Ok(r) => r,
        Err(e) => return tool_error(e),
    };
    format_response(&params.session_id, &resp)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_snap_mode(mode: Option<&str>) -> SnapMode {
    match mode {
        None | Some("compact") => SnapMode::Compact,
        Some("accessibility") => SnapMode::Accessibility,
        Some("interactive") => SnapMode::Interactive,
        Some("links") => SnapMode::Links,
        Some("forms") => SnapMode::Forms,
        Some("headings") => SnapMode::Headings,
        Some("text") => SnapMode::Text,
        Some("dom") => SnapMode::Dom,
        Some("markdown") => SnapMode::Markdown,
        Some(other) => {
            if let Some(sel) = other.strip_prefix("selector:") {
                SnapMode::Selector(sel.to_string())
            } else if let Some(region) = other.strip_prefix("region:") {
                SnapMode::Region(region.to_string())
            } else {
                SnapMode::Compact
            }
        }
    }
}

fn format_response(session_id: &str, resp: &DaemonResponse) -> CallToolResult {
    if resp.success {
        let mut text = format!("[session: {session_id}]\n");
        if let Some(content) = &resp.content {
            text.push_str(content);
        } else {
            text.push_str("OK");
        }
        if !resp.console.is_empty() {
            text.push_str("\n\n[console]\n");
            for line in &resp.console {
                text.push_str(line);
                text.push('\n');
            }
        }
        CallToolResult::success(vec![Content::text(text)])
    } else {
        let msg = resp
            .error
            .clone()
            .unwrap_or_else(|| "unknown error".into());
        tool_error(msg)
    }
}

fn tool_error(message: String) -> CallToolResult {
    CallToolResult::error(vec![Content::text(message)])
}
