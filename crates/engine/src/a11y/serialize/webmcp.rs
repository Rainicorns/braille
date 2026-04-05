use crate::dom::DomTree;

/// Read cached `<meta name="webmcp:tool">` entries and return a formatted
/// section string. Returns an empty string if no valid WebMCP tool declarations found.
pub fn collect_webmcp_section(tree: &DomTree) -> String {
    let entries = tree.get_meta("webmcp:tool");
    if entries.is_empty() {
        return String::new();
    }

    let mut tools: Vec<WebMcpTool> = Vec::new();
    for entry in entries {
        if let Some(tool) = parse_webmcp_tool(&entry.content) {
            tools.push(tool);
        }
    }

    if tools.is_empty() {
        return String::new();
    }

    let mut section = String::from("\n[WebMCP Tools]\n");
    for tool in &tools {
        section.push_str("- ");
        section.push_str(&tool.name);
        section.push_str(": ");
        section.push_str(&tool.description);
        if !tool.params.is_empty() {
            section.push_str(" (params: ");
            section.push_str(&tool.params.join(", "));
            section.push(')');
        }
        section.push('\n');
    }

    section
}

struct WebMcpTool {
    name: String,
    description: String,
    params: Vec<String>,
}

fn parse_webmcp_tool(content: &str) -> Option<WebMcpTool> {
    let parsed: serde_json::Value = serde_json::from_str(content).ok()?;
    let obj = parsed.as_object()?;

    let name = obj.get("name")?.as_str()?.to_string();
    let description = obj.get("description")?.as_str()?.to_string();

    let mut params = Vec::new();
    if let Some(parameters) = obj.get("parameters") {
        if let Some(param_obj) = parameters.as_object() {
            let mut keys: Vec<&String> = param_obj.keys().collect();
            keys.sort();
            params = keys.into_iter().cloned().collect();
        }
    }

    Some(WebMcpTool {
        name,
        description,
        params,
    })
}
