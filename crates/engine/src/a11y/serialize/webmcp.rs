use crate::dom::node::NodeData;
use crate::dom::DomTree;

/// Scan the DOM tree for `<meta name="webmcp:tool">` tags and return a formatted
/// section string. Returns an empty string if no valid WebMCP tool declarations found.
pub fn collect_webmcp_section(tree: &DomTree) -> String {
    let mut tools: Vec<WebMcpTool> = Vec::new();

    for nid in 0..tree.node_count() {
        let node = tree.get_node(nid);
        if let NodeData::Element {
            tag_name,
            attributes,
            ..
        } = &node.data
        {
            if !tag_name.eq_ignore_ascii_case("meta") {
                continue;
            }

            let name_attr = attributes
                .iter()
                .find(|a| a.local_name.eq_ignore_ascii_case("name"));
            let content_attr = attributes
                .iter()
                .find(|a| a.local_name.eq_ignore_ascii_case("content"));

            if let (Some(name), Some(content)) = (name_attr, content_attr) {
                if *name.value == *"webmcp:tool" {
                    if let Some(tool) = parse_webmcp_tool(&content.value) {
                        tools.push(tool);
                    }
                }
            }
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
