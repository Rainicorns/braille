use crate::dom::node::{NodeData, NodeId};
use crate::dom::DomTree;

/// Elements that are completely skipped (not recursed into).
pub(crate) const SKIP_ELEMENTS: &[&str] = &["head", "script", "style", "meta", "link", "noscript"];

/// Elements that are transparent containers: they don't produce a role line,
/// and their children appear at the same indentation level.
pub(crate) const TRANSPARENT_ELEMENTS: &[&str] = &["div", "span", "section", "article", "header", "footer", "html", "body"];

/// Elements that receive interactive `@eN` references.
pub(crate) const INTERACTIVE_ELEMENTS: &[&str] = &["a", "button", "input", "textarea", "select"];

/// Block-level elements that produce line breaks in the compact view.
pub(crate) const BLOCK_ELEMENTS: &[&str] = &[
    "p", "div", "h1", "h2", "h3", "h4", "h5", "h6", "li", "tr", "br", "hr", "section", "article",
    "header", "footer", "nav", "main", "blockquote", "pre", "table", "thead", "tbody", "tfoot",
    "ul", "ol", "dl", "dt", "dd", "form", "fieldset", "details", "summary", "figure", "figcaption",
];

pub(crate) fn trim_trailing_newlines(s: &mut String) {
    while s.ends_with('\n') {
        s.pop();
    }
}

pub(crate) fn is_display_none(node: &crate::dom::node::Node) -> bool {
    node.computed_style
        .as_ref()
        .and_then(|cs| cs.get("display"))
        .map(|v| v == "none")
        .unwrap_or(false)
}

pub(crate) fn is_visibility_hidden(node: &crate::dom::node::Node) -> bool {
    node.computed_style
        .as_ref()
        .and_then(|cs| cs.get("visibility"))
        .map(|v| v == "hidden")
        .unwrap_or(false)
}

/// Collect ALL text content recursively (deep), not just direct children.
pub(crate) fn collect_deep_text(tree: &DomTree, node_id: NodeId) -> String {
    let node = tree.get_node(node_id);
    let mut text = String::new();
    collect_deep_text_inner(tree, node, &mut text);
    text
}

fn collect_deep_text_inner(tree: &DomTree, node: &crate::dom::node::Node, text: &mut String) {
    match &node.data {
        NodeData::Text { content } | NodeData::CDATASection { content } => {
            text.push_str(content);
        }
        _ => {
            for &child_id in &node.children {
                let child = tree.get_node(child_id);
                collect_deep_text_inner(tree, child, text);
            }
        }
    }
}

/// Map an element tag to its accessibility role string.
pub(crate) fn element_role(tag: &str, attributes: &[crate::dom::node::DomAttribute]) -> String {
    match tag {
        "h1" => "heading[1]".to_string(),
        "h2" => "heading[2]".to_string(),
        "h3" => "heading[3]".to_string(),
        "h4" => "heading[4]".to_string(),
        "h5" => "heading[5]".to_string(),
        "h6" => "heading[6]".to_string(),
        "p" => "paragraph".to_string(),
        "a" => "link".to_string(),
        "button" => "button".to_string(),
        "input" => {
            if let Some(type_val) = attributes.iter().find(|a| a.local_name == "type").map(|a| &a.value) {
                format!("input[type={}]", type_val)
            } else {
                "input".to_string()
            }
        }
        "img" => "image".to_string(),
        "ul" | "ol" => "list".to_string(),
        "li" => "listitem".to_string(),
        "nav" => "navigation".to_string(),
        "main" => "main".to_string(),
        "form" => "form".to_string(),
        "textarea" => "textarea".to_string(),
        "select" => "select".to_string(),
        "table" => "table".to_string(),
        _ => tag.to_string(),
    }
}

/// Collect text content from immediate Text-node children only (not deeply nested).
pub(crate) fn collect_direct_text(tree: &DomTree, node_id: NodeId) -> String {
    let node = tree.get_node(node_id);
    let mut text = String::new();
    for &child_id in &node.children {
        let child = tree.get_node(child_id);
        match &child.data {
            NodeData::Text { content } | NodeData::CDATASection { content } => {
                text.push_str(content);
            }
            _ => {}
        }
    }
    text.trim().to_string()
}

/// For interactive elements (input, select), returns the display value.
/// - For "input": returns the "value" attribute if present.
/// - For "select": returns the text content of the selected <option>,
///   or the first <option> if none is explicitly selected.
/// - For all others: returns None.
pub(crate) fn get_interactive_value(
    tree: &DomTree,
    node_id: NodeId,
    tag: &str,
    attributes: &[crate::dom::node::DomAttribute],
) -> Option<String> {
    match tag {
        "input" => attributes
            .iter()
            .find(|a| a.local_name == "value")
            .map(|a| a.value.clone()),
        "select" => {
            let node = tree.get_node(node_id);
            let mut first_option_text: Option<String> = None;
            for &child_id in &node.children {
                let child = tree.get_node(child_id);
                if let NodeData::Element {
                    tag_name,
                    attributes: child_attrs,
                    ..
                } = &child.data
                {
                    if tag_name.eq_ignore_ascii_case("option") {
                        let text = tree.get_text_content(child_id);
                        let text = text.trim().to_string();
                        if child_attrs.iter().any(|a| a.local_name == "selected") {
                            return Some(text);
                        }
                        if first_option_text.is_none() {
                            first_option_text = Some(text);
                        }
                    }
                }
            }
            first_option_text
        }
        _ => None,
    }
}

/// Returns true if the subtree under `node_id` contains any renderable
/// (non-transparent, non-skipped) element descendants or text in non-transparent elements.
pub(crate) fn has_renderable_children(tree: &DomTree, node_id: NodeId) -> bool {
    let node = tree.get_node(node_id);
    for &child_id in &node.children {
        let child = tree.get_node(child_id);
        match &child.data {
            NodeData::Element { tag_name, .. } => {
                let tag = tag_name.to_ascii_lowercase();
                // Elements with display:none are not renderable.
                if let Some(ref computed) = child.computed_style {
                    if computed.get("display").map(|v| v.as_str()) == Some("none") {
                        continue;
                    }
                }
                if SKIP_ELEMENTS.contains(&tag.as_str()) {
                    continue;
                }
                if TRANSPARENT_ELEMENTS.contains(&tag.as_str()) {
                    // Check recursively inside transparent containers.
                    if has_renderable_children(tree, child_id) {
                        return true;
                    }
                } else {
                    return true;
                }
            }
            NodeData::Text { .. } => {
                // Text nodes are collected as direct text by the parent,
                // not as "renderable children" in terms of sub-elements.
            }
            _ => {}
        }
    }
    false
}

pub(crate) fn heading_level(tag: &str) -> Option<usize> {
    match tag {
        "h1" => Some(1),
        "h2" => Some(2),
        "h3" => Some(3),
        "h4" => Some(4),
        "h5" => Some(5),
        "h6" => Some(6),
        _ => None,
    }
}

/// Collapse runs of blank lines, trim each line, drop empties.
pub(crate) fn normalize_compact(raw: &str) -> String {
    let mut result = String::new();
    let mut blank_count = 0;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            blank_count += 1;
            continue;
        }
        if blank_count > 0 && !result.is_empty() {
            result.push('\n');
        }
        blank_count = 0;
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(trimmed);
    }
    result
}

pub(crate) fn collapse_whitespace(s: &str) -> String {
    let mut result = String::new();
    let mut prev_was_newline = false;
    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !prev_was_newline && !result.is_empty() {
                result.push('\n');
                prev_was_newline = true;
            }
        } else {
            if prev_was_newline {
                // Already have a newline separator
            } else if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(trimmed);
            prev_was_newline = false;
        }
    }
    result
}
