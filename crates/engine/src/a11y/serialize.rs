use crate::dom::node::{NodeData, NodeId};
use crate::dom::DomTree;
use std::collections::HashMap;

/// Elements that are completely skipped (not recursed into).
const SKIP_ELEMENTS: &[&str] = &["head", "script", "style", "meta", "link", "noscript"];

/// Elements that are transparent containers: they don't produce a role line,
/// and their children appear at the same indentation level.
const TRANSPARENT_ELEMENTS: &[&str] = &["div", "span", "section", "article", "header", "footer", "html", "body"];

/// Elements that receive interactive `@eN` references.
const INTERACTIVE_ELEMENTS: &[&str] = &["a", "button", "input", "textarea", "select"];

// ---------------------------------------------------------------------------
// Phase V1: Ref assignment — always walks the full tree so @eN is stable
// ---------------------------------------------------------------------------

/// Assign `@eN` refs to all interactive elements in document order.
/// Returns `(ref_map, reverse_ref_map)` where:
/// - `ref_map`: "@eN" → NodeId  (for resolving user input)
/// - `reverse_ref_map`: NodeId → "@eN"  (for emitting refs in views)
pub fn assign_refs(tree: &DomTree) -> (HashMap<String, NodeId>, HashMap<NodeId, String>) {
    let mut ref_counter: usize = 0;
    let mut ref_map = HashMap::new();
    let mut reverse = HashMap::new();
    assign_refs_walk(tree, tree.document(), &mut ref_counter, &mut ref_map, &mut reverse);
    (ref_map, reverse)
}

fn assign_refs_walk(
    tree: &DomTree,
    node_id: NodeId,
    ref_counter: &mut usize,
    ref_map: &mut HashMap<String, NodeId>,
    reverse: &mut HashMap<NodeId, String>,
) {
    let node = tree.get_node(node_id);
    match &node.data {
        NodeData::Element { tag_name, .. } => {
            let tag = tag_name.to_ascii_lowercase();

            // Skip display:none subtrees — they don't get refs
            if let Some(ref computed) = node.computed_style {
                if computed.get("display").map(|v| v.as_str()) == Some("none") {
                    return;
                }
            }

            if SKIP_ELEMENTS.contains(&tag.as_str()) {
                return;
            }

            if INTERACTIVE_ELEMENTS.contains(&tag.as_str()) {
                *ref_counter += 1;
                let ref_str = format!("@e{}", *ref_counter);
                ref_map.insert(ref_str.clone(), node_id);
                reverse.insert(node_id, ref_str);
            }

            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                assign_refs_walk(tree, child_id, ref_counter, ref_map, reverse);
            }
        }
        NodeData::Document | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                assign_refs_walk(tree, child_id, ref_counter, ref_map, reverse);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Accessibility view (full tree)
// ---------------------------------------------------------------------------

/// Walk a `DomTree` and produce a compact accessibility-tree text
/// representation suitable for LLM agents.
/// Returns a tuple of (text_output, ref_map) where ref_map maps "@eN" strings to NodeIds.
pub fn serialize_a11y(tree: &DomTree, focused: Option<NodeId>) -> (String, HashMap<String, NodeId>) {
    let (ref_map, reverse) = assign_refs(tree);
    let mut output = String::new();
    walk_a11y(tree, tree.document(), 0, &reverse, &mut output, focused);
    trim_trailing_newlines(&mut output);
    (output, ref_map)
}

/// Serialize a11y tree rooted at a specific node (for Region view).
/// Uses the provided reverse_ref_map so refs remain stable.
pub fn serialize_a11y_rooted(
    tree: &DomTree,
    root: NodeId,
    reverse: &HashMap<NodeId, String>,
    focused: Option<NodeId>,
) -> String {
    let mut output = String::new();
    walk_a11y(tree, root, 0, reverse, &mut output, focused);
    trim_trailing_newlines(&mut output);
    output
}

fn walk_a11y(
    tree: &DomTree,
    node_id: NodeId,
    indent: usize,
    reverse: &HashMap<NodeId, String>,
    output: &mut String,
    focused: Option<NodeId>,
) {
    let node = tree.get_node(node_id);

    match &node.data {
        NodeData::Document | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_a11y(tree, child_id, indent, reverse, output, focused);
            }
        }
        NodeData::Text { .. } | NodeData::Comment { .. } | NodeData::CDATASection { .. } => {}
        NodeData::Doctype { .. }
        | NodeData::ProcessingInstruction { .. }
        | NodeData::Attr { .. } => {}
        NodeData::Element {
            tag_name, attributes, ..
        } => {
            let tag = tag_name.to_ascii_lowercase();

            if let Some(ref computed) = node.computed_style {
                if computed.get("display").map(|v| v.as_str()) == Some("none") {
                    return;
                }
            }

            if SKIP_ELEMENTS.contains(&tag.as_str()) {
                return;
            }

            if TRANSPARENT_ELEMENTS.contains(&tag.as_str()) {
                let children: Vec<NodeId> = node.children.clone();
                for child_id in children {
                    walk_a11y(tree, child_id, indent, reverse, output, focused);
                }
                return;
            }

            let role = element_role(&tag, attributes);
            let is_visibility_hidden = is_visibility_hidden(node);

            let direct_text = if is_visibility_hidden {
                String::new()
            } else if tag == "img" {
                attributes
                    .iter()
                    .find(|a| a.local_name == "alt")
                    .map(|a| a.value.clone())
                    .unwrap_or_default()
            } else {
                collect_direct_text(tree, node_id)
            };

            let eref = reverse.get(&node_id);
            let has_child_output = has_renderable_children(tree, node_id);

            if direct_text.is_empty() && !has_child_output && eref.is_none() && !is_visibility_hidden {
                return;
            }

            let indent_str = " ".repeat(indent);
            let mut line = format!("{}{}", indent_str, role);

            if let Some(r) = eref {
                line.push(' ');
                line.push_str(r);
            }

            if let Some(val) = get_interactive_value(tree, node_id, &tag, attributes) {
                line.push_str(&format!(" value=\"{}\"", val));
            }

            if !direct_text.is_empty() {
                line.push_str(&format!(" \"{}\"", direct_text));
            }

            if focused == Some(node_id) {
                line.push_str(" [focused]");
            }

            output.push_str(&line);
            output.push('\n');

            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_a11y(tree, child_id, indent + 2, reverse, output, focused);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Interactive view — just clickable/typeable elements
// ---------------------------------------------------------------------------

pub fn serialize_interactive(tree: &DomTree, reverse: &HashMap<NodeId, String>, focused: Option<NodeId>) -> String {
    let mut output = String::new();
    walk_interactive(tree, tree.document(), reverse, &mut output, focused);
    trim_trailing_newlines(&mut output);
    output
}

fn walk_interactive(
    tree: &DomTree,
    node_id: NodeId,
    reverse: &HashMap<NodeId, String>,
    output: &mut String,
    focused: Option<NodeId>,
) {
    let node = tree.get_node(node_id);
    match &node.data {
        NodeData::Element { tag_name, attributes, .. } => {
            let tag = tag_name.to_ascii_lowercase();

            if is_display_none(node) {
                return;
            }
            if SKIP_ELEMENTS.contains(&tag.as_str()) {
                return;
            }

            if INTERACTIVE_ELEMENTS.contains(&tag.as_str()) {
                if let Some(eref) = reverse.get(&node_id) {
                    let role = element_role(&tag, attributes);
                    let mut line = format!("{} {}", eref, role);

                    if let Some(val) = get_interactive_value(tree, node_id, &tag, attributes) {
                        line.push_str(&format!(" value=\"{}\"", val));
                    }

                    let text = collect_direct_text(tree, node_id);
                    if !text.is_empty() {
                        line.push_str(&format!(" \"{}\"", text));
                    }

                    if focused == Some(node_id) {
                        line.push_str(" [focused]");
                    }

                    output.push_str(&line);
                    output.push('\n');
                }
            }

            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_interactive(tree, child_id, reverse, output, focused);
            }
        }
        NodeData::Document | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_interactive(tree, child_id, reverse, output, focused);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Links view — just <a> elements with href
// ---------------------------------------------------------------------------

pub fn serialize_links(tree: &DomTree, reverse: &HashMap<NodeId, String>) -> String {
    let mut output = String::new();
    walk_links(tree, tree.document(), reverse, &mut output);
    trim_trailing_newlines(&mut output);
    output
}

fn walk_links(
    tree: &DomTree,
    node_id: NodeId,
    reverse: &HashMap<NodeId, String>,
    output: &mut String,
) {
    let node = tree.get_node(node_id);
    match &node.data {
        NodeData::Element { tag_name, attributes, .. } => {
            let tag = tag_name.to_ascii_lowercase();

            if is_display_none(node) {
                return;
            }
            if SKIP_ELEMENTS.contains(&tag.as_str()) {
                return;
            }

            if tag == "a" {
                if let Some(href) = attributes.iter().find(|a| a.local_name == "href") {
                    let eref = reverse.get(&node_id).map(|s| s.as_str()).unwrap_or("???");
                    let text = collect_deep_text(tree, node_id);
                    output.push_str(&format!("{} \"{}\" -> {}\n", eref, text.trim(), href.value));
                }
            }

            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_links(tree, child_id, reverse, output);
            }
        }
        NodeData::Document | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_links(tree, child_id, reverse, output);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Forms view — form structure with input values
// ---------------------------------------------------------------------------

pub fn serialize_forms(tree: &DomTree, reverse: &HashMap<NodeId, String>) -> String {
    let mut output = String::new();
    walk_forms(tree, tree.document(), reverse, &mut output);
    trim_trailing_newlines(&mut output);
    output
}

fn walk_forms(
    tree: &DomTree,
    node_id: NodeId,
    reverse: &HashMap<NodeId, String>,
    output: &mut String,
) {
    let node = tree.get_node(node_id);
    match &node.data {
        NodeData::Element { tag_name, attributes, .. } => {
            let tag = tag_name.to_ascii_lowercase();

            if is_display_none(node) {
                return;
            }
            if SKIP_ELEMENTS.contains(&tag.as_str()) {
                return;
            }

            if tag == "form" {
                let action = attributes.iter().find(|a| a.local_name == "action").map(|a| a.value.as_str());
                let method = attributes.iter().find(|a| a.local_name == "method").map(|a| a.value.as_str());
                let mut header = "form".to_string();
                if let Some(action) = action {
                    header.push_str(&format!(" action=\"{}\"", action));
                }
                if let Some(method) = method {
                    header.push_str(&format!(" method=\"{}\"", method));
                }
                output.push_str(&header);
                output.push('\n');

                // Emit form's interactive children
                walk_form_inputs(tree, node_id, reverse, output);
                return; // Don't recurse further for nested forms
            }

            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_forms(tree, child_id, reverse, output);
            }
        }
        NodeData::Document | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_forms(tree, child_id, reverse, output);
            }
        }
        _ => {}
    }
}

fn walk_form_inputs(
    tree: &DomTree,
    node_id: NodeId,
    reverse: &HashMap<NodeId, String>,
    output: &mut String,
) {
    let node = tree.get_node(node_id);
    match &node.data {
        NodeData::Element { tag_name, attributes, .. } => {
            let tag = tag_name.to_ascii_lowercase();

            if is_display_none(node) {
                return;
            }

            if INTERACTIVE_ELEMENTS.contains(&tag.as_str()) {
                let eref = reverse.get(&node_id).map(|s| s.as_str()).unwrap_or("???");
                let role = element_role(&tag, attributes);
                let mut line = format!("  {} {}", eref, role);

                if let Some(val) = get_interactive_value(tree, node_id, &tag, attributes) {
                    line.push_str(&format!(" value=\"{}\"", val));
                }

                let text = collect_direct_text(tree, node_id);
                if !text.is_empty() {
                    line.push_str(&format!(" \"{}\"", text));
                }

                output.push_str(&line);
                output.push('\n');
            }

            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_form_inputs(tree, child_id, reverse, output);
            }
        }
        _ => {
            let node = tree.get_node(node_id);
            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_form_inputs(tree, child_id, reverse, output);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Headings view — h1-h6 outline
// ---------------------------------------------------------------------------

pub fn serialize_headings(tree: &DomTree) -> String {
    let mut output = String::new();
    walk_headings(tree, tree.document(), &mut output);
    trim_trailing_newlines(&mut output);
    output
}

fn walk_headings(tree: &DomTree, node_id: NodeId, output: &mut String) {
    let node = tree.get_node(node_id);
    match &node.data {
        NodeData::Element { tag_name, .. } => {
            let tag = tag_name.to_ascii_lowercase();

            if is_display_none(node) {
                return;
            }
            if SKIP_ELEMENTS.contains(&tag.as_str()) {
                return;
            }

            if let Some(level) = heading_level(&tag) {
                let indent = "  ".repeat(level - 1);
                let text = collect_deep_text(tree, node_id);
                output.push_str(&format!("{}h{} \"{}\"\n", indent, level, text.trim()));
            }

            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_headings(tree, child_id, output);
            }
        }
        NodeData::Document | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_headings(tree, child_id, output);
            }
        }
        _ => {}
    }
}

fn heading_level(tag: &str) -> Option<usize> {
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

// ---------------------------------------------------------------------------
// Text view — readable content, no structure, no refs
// ---------------------------------------------------------------------------

pub fn serialize_text(tree: &DomTree) -> String {
    let mut output = String::new();
    walk_text(tree, tree.document(), &mut output);
    // Normalize: collapse multiple newlines, trim
    let normalized = collapse_whitespace(&output);
    normalized.trim().to_string()
}

fn walk_text(tree: &DomTree, node_id: NodeId, output: &mut String) {
    let node = tree.get_node(node_id);
    match &node.data {
        NodeData::Text { content } | NodeData::CDATASection { content } => {
            output.push_str(content);
        }
        NodeData::Element { tag_name, .. } => {
            let tag = tag_name.to_ascii_lowercase();

            if is_display_none(node) || is_visibility_hidden(node) {
                return;
            }
            if SKIP_ELEMENTS.contains(&tag.as_str()) {
                return;
            }

            // Block-level elements get newlines for readability
            let is_block = matches!(
                tag.as_str(),
                "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "tr" | "br" | "hr"
                    | "section" | "article" | "header" | "footer" | "nav" | "main" | "blockquote"
            );

            if is_block {
                output.push('\n');
            }

            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_text(tree, child_id, output);
            }

            if is_block {
                output.push('\n');
            }
        }
        NodeData::Document | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
            let children: Vec<NodeId> = node.children.clone();
            for child_id in children {
                walk_text(tree, child_id, output);
            }
        }
        _ => {}
    }
}

fn collapse_whitespace(s: &str) -> String {
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

// ---------------------------------------------------------------------------
// Selector view — CSS selector query
// ---------------------------------------------------------------------------

pub fn serialize_selector(
    tree: &DomTree,
    selector: &str,
    reverse: &HashMap<NodeId, String>,
) -> String {
    let matches = crate::css::matching::query_selector_all(tree, tree.document(), selector, None);
    let mut output = String::new();
    for nid in matches {
        let node = tree.get_node(nid);
        if let NodeData::Element { tag_name, attributes, .. } = &node.data {
            let tag = tag_name.to_ascii_lowercase();
            let role = element_role(&tag, attributes);
            let eref = reverse.get(&nid).map(|s| format!(" {}", s)).unwrap_or_default();
            let text = collect_direct_text(tree, nid);
            let text_part = if text.is_empty() { String::new() } else { format!(" \"{}\"", text) };
            output.push_str(&format!("{}{}{}\n", role, eref, text_part));
        }
    }
    trim_trailing_newlines(&mut output);
    output
}

// ---------------------------------------------------------------------------
// Region view — subtree snapshot rooted at target
// ---------------------------------------------------------------------------

pub fn serialize_region(
    tree: &DomTree,
    target: NodeId,
    reverse: &HashMap<NodeId, String>,
    focused: Option<NodeId>,
) -> String {
    serialize_a11y_rooted(tree, target, reverse, focused)
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn trim_trailing_newlines(s: &mut String) {
    while s.ends_with('\n') {
        s.pop();
    }
}

fn is_display_none(node: &crate::dom::node::Node) -> bool {
    node.computed_style
        .as_ref()
        .and_then(|cs| cs.get("display"))
        .map(|v| v == "none")
        .unwrap_or(false)
}

fn is_visibility_hidden(node: &crate::dom::node::Node) -> bool {
    node.computed_style
        .as_ref()
        .and_then(|cs| cs.get("visibility"))
        .map(|v| v == "hidden")
        .unwrap_or(false)
}

/// Collect ALL text content recursively (deep), not just direct children.
fn collect_deep_text(tree: &DomTree, node_id: NodeId) -> String {
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
fn element_role(tag: &str, attributes: &[crate::dom::node::DomAttribute]) -> String {
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
fn collect_direct_text(tree: &DomTree, node_id: NodeId) -> String {
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
fn get_interactive_value(
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
fn has_renderable_children(tree: &DomTree, node_id: NodeId) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::DomTree;

    /// Helper: set an attribute on an element node.
    fn set_attr(tree: &mut DomTree, node_id: NodeId, key: &str, value: &str) {
        if let NodeData::Element { ref mut attributes, .. } = tree.get_node_mut(node_id).data {
            attributes.push(crate::dom::node::DomAttribute::new(key, value));
        }
    }

    #[test]
    fn simple_h1_and_p() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let html = tree.create_element("html");
        tree.append_child(doc, html);

        let body = tree.create_element("body");
        tree.append_child(html, body);

        let h1 = tree.create_element("h1");
        tree.append_child(body, h1);
        let h1_text = tree.create_text("Hello");
        tree.append_child(h1, h1_text);

        let p = tree.create_element("p");
        tree.append_child(body, p);
        let p_text = tree.create_text("World");
        tree.append_child(p, p_text);

        let (result, ref_map) = serialize_a11y(&tree, None);
        assert_eq!(result, "heading[1] \"Hello\"\nparagraph \"World\"");
        assert_eq!(ref_map.len(), 0); // No interactive elements
    }

    #[test]
    fn skips_script_style_head() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let html = tree.create_element("html");
        tree.append_child(doc, html);

        let head = tree.create_element("head");
        tree.append_child(html, head);
        let title = tree.create_element("title");
        tree.append_child(head, title);
        let title_text = tree.create_text("Page Title");
        tree.append_child(title, title_text);

        let body = tree.create_element("body");
        tree.append_child(html, body);

        let script = tree.create_element("script");
        tree.append_child(body, script);
        let script_text = tree.create_text("alert('hi')");
        tree.append_child(script, script_text);

        let style = tree.create_element("style");
        tree.append_child(body, style);
        let style_text = tree.create_text("body { color: red; }");
        tree.append_child(style, style_text);

        let p = tree.create_element("p");
        tree.append_child(body, p);
        let p_text = tree.create_text("Visible");
        tree.append_child(p, p_text);

        let (result, ref_map) = serialize_a11y(&tree, None);
        // Only the paragraph should appear; head, script, style are all skipped.
        assert_eq!(result, "paragraph \"Visible\"");
        assert_eq!(ref_map.len(), 0);
    }

    #[test]
    fn interactive_elements_get_refs() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let a = tree.create_element("a");
        set_attr(&mut tree, a, "href", "/home");
        tree.append_child(body, a);
        let a_text = tree.create_text("Home");
        tree.append_child(a, a_text);

        let btn = tree.create_element("button");
        tree.append_child(body, btn);
        let btn_text = tree.create_text("Submit");
        tree.append_child(btn, btn_text);

        let input = tree.create_element("input");
        set_attr(&mut tree, input, "type", "text");
        tree.append_child(body, input);

        let (result, ref_map) = serialize_a11y(&tree, None);
        let expected = "link @e1 \"Home\"\nbutton @e2 \"Submit\"\ninput[type=text] @e3";
        assert_eq!(result, expected);
        assert_eq!(ref_map.len(), 3);
        assert_eq!(ref_map.get("@e1"), Some(&a));
        assert_eq!(ref_map.get("@e2"), Some(&btn));
        assert_eq!(ref_map.get("@e3"), Some(&input));
    }

    #[test]
    fn transparent_containers_dont_appear() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let div = tree.create_element("div");
        tree.append_child(body, div);

        let span = tree.create_element("span");
        tree.append_child(div, span);

        let p = tree.create_element("p");
        tree.append_child(span, p);
        let p_text = tree.create_text("Inside div>span>p");
        tree.append_child(p, p_text);

        let (result, ref_map) = serialize_a11y(&tree, None);
        // div and span are transparent, so the paragraph appears at indent 0.
        assert_eq!(result, "paragraph \"Inside div>span>p\"");
        assert_eq!(ref_map.len(), 0);
    }

    #[test]
    fn spike_test_case() {
        // The exact example from the spike plan:
        // <html><body>
        //   <h1>Hello</h1>
        //   <div id="app"><p>Created by JavaScript</p></div>
        // </body></html>
        let mut tree = DomTree::new();
        let doc = tree.document();

        let html = tree.create_element("html");
        tree.append_child(doc, html);

        let body = tree.create_element("body");
        tree.append_child(html, body);

        let h1 = tree.create_element("h1");
        tree.append_child(body, h1);
        let h1_text = tree.create_text("Hello");
        tree.append_child(h1, h1_text);

        let div = tree.create_element("div");
        set_attr(&mut tree, div, "id", "app");
        tree.append_child(body, div);

        let p = tree.create_element("p");
        tree.append_child(div, p);
        let p_text = tree.create_text("Created by JavaScript");
        tree.append_child(p, p_text);

        let (result, ref_map) = serialize_a11y(&tree, None);
        assert_eq!(result, "heading[1] \"Hello\"\nparagraph \"Created by JavaScript\"");
        assert_eq!(ref_map.len(), 0);
    }

    #[test]
    fn nav_and_main_with_nested_elements() {
        // The second example from the task description.
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let nav = tree.create_element("nav");
        tree.append_child(body, nav);

        let a1 = tree.create_element("a");
        set_attr(&mut tree, a1, "href", "/home");
        tree.append_child(nav, a1);
        let a1_text = tree.create_text("Home");
        tree.append_child(a1, a1_text);

        let a2 = tree.create_element("a");
        set_attr(&mut tree, a2, "href", "/about");
        tree.append_child(nav, a2);
        let a2_text = tree.create_text("About");
        tree.append_child(a2, a2_text);

        let main = tree.create_element("main");
        tree.append_child(body, main);

        let h1 = tree.create_element("h1");
        tree.append_child(main, h1);
        let h1_text = tree.create_text("Welcome");
        tree.append_child(h1, h1_text);

        let p = tree.create_element("p");
        tree.append_child(main, p);
        let p_text = tree.create_text("Some text");
        tree.append_child(p, p_text);

        let btn = tree.create_element("button");
        tree.append_child(main, btn);
        let btn_text = tree.create_text("Click me");
        tree.append_child(btn, btn_text);

        let (result, ref_map) = serialize_a11y(&tree, None);
        let expected = "\
navigation
  link @e1 \"Home\"
  link @e2 \"About\"
main
  heading[1] \"Welcome\"
  paragraph \"Some text\"
  button @e3 \"Click me\"";
        assert_eq!(result, expected);
        assert_eq!(ref_map.len(), 3);
        assert_eq!(ref_map.get("@e1"), Some(&a1));
        assert_eq!(ref_map.get("@e2"), Some(&a2));
        assert_eq!(ref_map.get("@e3"), Some(&btn));
    }

    #[test]
    fn image_with_alt_text() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let img = tree.create_element("img");
        set_attr(&mut tree, img, "src", "logo.png");
        set_attr(&mut tree, img, "alt", "Company Logo");
        tree.append_child(body, img);

        let (result, ref_map) = serialize_a11y(&tree, None);
        assert_eq!(result, "image \"Company Logo\"");
        assert_eq!(ref_map.len(), 0);
    }

    #[test]
    fn list_elements() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let ul = tree.create_element("ul");
        tree.append_child(body, ul);

        let li1 = tree.create_element("li");
        tree.append_child(ul, li1);
        let li1_text = tree.create_text("First");
        tree.append_child(li1, li1_text);

        let li2 = tree.create_element("li");
        tree.append_child(ul, li2);
        let li2_text = tree.create_text("Second");
        tree.append_child(li2, li2_text);

        let (result, ref_map) = serialize_a11y(&tree, None);
        let expected = "\
list
  listitem \"First\"
  listitem \"Second\"";
        assert_eq!(result, expected);
        assert_eq!(ref_map.len(), 0);
    }

    #[test]
    fn empty_elements_are_skipped() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        // Empty div (transparent, no children) should produce nothing.
        let div = tree.create_element("div");
        tree.append_child(body, div);

        // Empty p (non-transparent, no text, no children) should be skipped.
        let p = tree.create_element("p");
        tree.append_child(body, p);

        // p with text should appear.
        let p2 = tree.create_element("p");
        tree.append_child(body, p2);
        let p2_text = tree.create_text("Visible");
        tree.append_child(p2, p2_text);

        let (result, ref_map) = serialize_a11y(&tree, None);
        assert_eq!(result, "paragraph \"Visible\"");
        assert_eq!(ref_map.len(), 0);
    }

    #[test]
    fn textarea_and_select_are_interactive() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let form = tree.create_element("form");
        tree.append_child(body, form);

        let textarea = tree.create_element("textarea");
        tree.append_child(form, textarea);
        let ta_text = tree.create_text("Default text");
        tree.append_child(textarea, ta_text);

        let select = tree.create_element("select");
        tree.append_child(form, select);

        let (result, ref_map) = serialize_a11y(&tree, None);
        let expected = "\
form
  textarea @e1 \"Default text\"
  select @e2";
        assert_eq!(result, expected);
        assert_eq!(ref_map.len(), 2);
        assert_eq!(ref_map.get("@e1"), Some(&textarea));
        assert_eq!(ref_map.get("@e2"), Some(&select));
    }

    #[test]
    fn ref_map_returns_correct_node_ids() {
        // Test that the ref map contains correct mappings
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let link = tree.create_element("a");
        set_attr(&mut tree, link, "href", "/test");
        tree.append_child(body, link);
        let link_text = tree.create_text("Test Link");
        tree.append_child(link, link_text);

        let (output, ref_map) = serialize_a11y(&tree, None);

        assert_eq!(output, "link @e1 \"Test Link\"");
        assert_eq!(ref_map.len(), 1);
        assert_eq!(ref_map.get("@e1"), Some(&link));
        assert_eq!(ref_map.get("@e2"), None);
    }

    #[test]
    fn ref_map_with_no_interactive_elements() {
        // Test that ref map is empty when there are no interactive elements
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let p = tree.create_element("p");
        tree.append_child(body, p);
        let p_text = tree.create_text("Static text");
        tree.append_child(p, p_text);

        let (output, ref_map) = serialize_a11y(&tree, None);

        assert_eq!(output, "paragraph \"Static text\"");
        assert_eq!(ref_map.len(), 0);
    }

    #[test]
    fn ref_map_with_multiple_interactive_elements() {
        // Test that ref map contains all interactive elements with correct refs
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let input1 = tree.create_element("input");
        set_attr(&mut tree, input1, "type", "text");
        tree.append_child(body, input1);

        let button = tree.create_element("button");
        tree.append_child(body, button);
        let btn_text = tree.create_text("Click");
        tree.append_child(button, btn_text);

        let input2 = tree.create_element("input");
        set_attr(&mut tree, input2, "type", "submit");
        tree.append_child(body, input2);

        let (_output, ref_map) = serialize_a11y(&tree, None);

        assert_eq!(ref_map.len(), 3);
        assert_eq!(ref_map.get("@e1"), Some(&input1));
        assert_eq!(ref_map.get("@e2"), Some(&button));
        assert_eq!(ref_map.get("@e3"), Some(&input2));
    }

    #[test]
    fn focused_element_shows_focused_marker() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let input1 = tree.create_element("input");
        set_attr(&mut tree, input1, "type", "text");
        tree.append_child(body, input1);

        let input2 = tree.create_element("input");
        set_attr(&mut tree, input2, "type", "text");
        tree.append_child(body, input2);

        // Serialize with input2 focused
        let (output, _ref_map) = serialize_a11y(&tree, Some(input2));

        assert!(
            output.contains("input[type=text] @e1\n"),
            "first input should not be focused: {}",
            output
        );
        assert!(
            output.contains("input[type=text] @e2 [focused]"),
            "second input should be focused: {}",
            output
        );
    }

    #[test]
    fn focused_non_interactive_element_shows_marker() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let div = tree.create_element("div");
        set_attr(&mut tree, div, "tabindex", "0");
        tree.append_child(body, div);
        let div_text = tree.create_text("Focusable div");
        tree.append_child(div, div_text);

        // Even though div is transparent, if it has tabindex and is focused, it should show the marker
        // Note: Currently div is in TRANSPARENT_ELEMENTS, so it won't produce a role line.
        // This test documents current behavior - focused state only shows on non-transparent elements.
        let (output, _ref_map) = serialize_a11y(&tree, Some(div));

        // div is transparent, so no output expected
        assert_eq!(
            output, "",
            "transparent elements don't produce output even when focused: {}",
            output
        );
    }

    #[test]
    fn no_focused_marker_when_none() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let input = tree.create_element("input");
        set_attr(&mut tree, input, "type", "text");
        tree.append_child(body, input);

        let (output, _ref_map) = serialize_a11y(&tree, None);

        assert!(
            !output.contains("[focused]"),
            "should not contain focused marker when None: {}",
            output
        );
        assert_eq!(output, "input[type=text] @e1");
    }

    // ==================== A-2D: Interactive value display tests ====================

    #[test]
    fn input_with_value_shows_value() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let input = tree.create_element("input");
        set_attr(&mut tree, input, "type", "text");
        set_attr(&mut tree, input, "value", "username");
        tree.append_child(body, input);

        let (result, _) = serialize_a11y(&tree, None);
        assert_eq!(result, "input[type=text] @e1 value=\"username\"");
    }

    #[test]
    fn input_without_value_shows_no_value() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let input = tree.create_element("input");
        set_attr(&mut tree, input, "type", "text");
        tree.append_child(body, input);

        let (result, _) = serialize_a11y(&tree, None);
        assert_eq!(result, "input[type=text] @e1");
        assert!(
            !result.contains("value="),
            "should not contain value= when no value attribute"
        );
    }

    #[test]
    fn select_with_selected_option_shows_that_option_text() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let select = tree.create_element("select");
        tree.append_child(body, select);

        let opt1 = tree.create_element("option");
        tree.append_child(select, opt1);
        let opt1_text = tree.create_text("Apple");
        tree.append_child(opt1, opt1_text);

        let opt2 = tree.create_element("option");
        set_attr(&mut tree, opt2, "selected", "");
        tree.append_child(select, opt2);
        let opt2_text = tree.create_text("Banana");
        tree.append_child(opt2, opt2_text);

        let opt3 = tree.create_element("option");
        tree.append_child(select, opt3);
        let opt3_text = tree.create_text("Cherry");
        tree.append_child(opt3, opt3_text);

        let (result, _) = serialize_a11y(&tree, None);
        // The select line should show value="Banana" (the selected option)
        assert!(
            result.contains("select @e1 value=\"Banana\""),
            "expected selected option text, got: {}",
            result
        );
    }

    #[test]
    fn select_with_no_selected_option_shows_first_option_text() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let select = tree.create_element("select");
        tree.append_child(body, select);

        let opt1 = tree.create_element("option");
        tree.append_child(select, opt1);
        let opt1_text = tree.create_text("Red");
        tree.append_child(opt1, opt1_text);

        let opt2 = tree.create_element("option");
        tree.append_child(select, opt2);
        let opt2_text = tree.create_text("Green");
        tree.append_child(opt2, opt2_text);

        let (result, _) = serialize_a11y(&tree, None);
        assert!(
            result.contains("select @e1 value=\"Red\""),
            "expected first option text when none selected, got: {}",
            result
        );
    }

    #[test]
    fn select_with_no_options_shows_no_value() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let select = tree.create_element("select");
        tree.append_child(body, select);

        let (result, _) = serialize_a11y(&tree, None);
        assert_eq!(result, "select @e1");
        assert!(!result.contains("value="), "should not contain value= with no options");
    }

    #[test]
    fn textarea_shows_text_content() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let textarea = tree.create_element("textarea");
        tree.append_child(body, textarea);
        let ta_text = tree.create_text("some text content");
        tree.append_child(textarea, ta_text);

        let (result, _) = serialize_a11y(&tree, None);
        assert_eq!(result, "textarea @e1 \"some text content\"");
    }

    #[test]
    fn input_value_with_special_chars() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let input = tree.create_element("input");
        set_attr(&mut tree, input, "type", "text");
        set_attr(&mut tree, input, "value", "hello world & <test>");
        tree.append_child(body, input);

        let (result, _) = serialize_a11y(&tree, None);
        assert_eq!(result, "input[type=text] @e1 value=\"hello world & <test>\"");
    }

    #[test]
    fn form_with_multiple_inputs_showing_different_values() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let form = tree.create_element("form");
        tree.append_child(body, form);

        // Text input with value
        let input1 = tree.create_element("input");
        set_attr(&mut tree, input1, "type", "text");
        set_attr(&mut tree, input1, "value", "john");
        tree.append_child(form, input1);

        // Password input with value
        let input2 = tree.create_element("input");
        set_attr(&mut tree, input2, "type", "password");
        set_attr(&mut tree, input2, "value", "secret123");
        tree.append_child(form, input2);

        // Select with selected option
        let select = tree.create_element("select");
        tree.append_child(form, select);

        let opt1 = tree.create_element("option");
        tree.append_child(select, opt1);
        let opt1_text = tree.create_text("US");
        tree.append_child(opt1, opt1_text);

        let opt2 = tree.create_element("option");
        set_attr(&mut tree, opt2, "selected", "");
        tree.append_child(select, opt2);
        let opt2_text = tree.create_text("UK");
        tree.append_child(opt2, opt2_text);

        // Textarea with content
        let textarea = tree.create_element("textarea");
        tree.append_child(form, textarea);
        let ta_text = tree.create_text("Additional notes here");
        tree.append_child(textarea, ta_text);

        // Empty input (no value)
        let input3 = tree.create_element("input");
        set_attr(&mut tree, input3, "type", "email");
        tree.append_child(form, input3);

        // Submit button
        let btn = tree.create_element("button");
        tree.append_child(form, btn);
        let btn_text = tree.create_text("Submit");
        tree.append_child(btn, btn_text);

        let (result, ref_map) = serialize_a11y(&tree, None);

        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines[0], "form");
        assert_eq!(lines[1], "  input[type=text] @e1 value=\"john\"");
        assert_eq!(lines[2], "  input[type=password] @e2 value=\"secret123\"");
        assert_eq!(lines[3], "  select @e3 value=\"UK\"");
        // option children of select are rendered as sub-lines
        assert_eq!(lines[4], "    option \"US\"");
        assert_eq!(lines[5], "    option \"UK\"");
        // textarea shows text via direct_text as "..."
        assert_eq!(lines[6], "  textarea @e4 \"Additional notes here\"");
        assert_eq!(lines[7], "  input[type=email] @e5");
        assert_eq!(lines[8], "  button @e6 \"Submit\"");
        assert_eq!(ref_map.len(), 6);
    }

    #[test]
    fn input_with_empty_value_shows_empty_value() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let input = tree.create_element("input");
        set_attr(&mut tree, input, "type", "text");
        set_attr(&mut tree, input, "value", "");
        tree.append_child(body, input);

        let (result, _) = serialize_a11y(&tree, None);
        // Even empty string value should show value=""
        assert_eq!(result, "input[type=text] @e1 value=\"\"");
    }

    #[test]
    fn input_value_with_focused() {
        let mut tree = DomTree::new();
        let doc = tree.document();

        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let input = tree.create_element("input");
        set_attr(&mut tree, input, "type", "text");
        set_attr(&mut tree, input, "value", "hello");
        tree.append_child(body, input);

        let (result, _) = serialize_a11y(&tree, Some(input));
        assert_eq!(result, "input[type=text] @e1 value=\"hello\" [focused]");
    }

    // ==================== C-3B: CSS computed style integration tests ====================

    #[test]
    fn display_none_skips_element_and_descendants() {
        let mut tree = DomTree::new();
        let doc = tree.document();
        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let p1 = tree.create_element("p");
        tree.append_child(body, p1);
        let p1_text = tree.create_text("Visible");
        tree.append_child(p1, p1_text);

        let div = tree.create_element("div");
        tree.append_child(body, div);
        // Set display: none on the div
        let mut style = HashMap::new();
        style.insert("display".to_string(), "none".to_string());
        tree.get_node_mut(div).computed_style = Some(style);

        let p2 = tree.create_element("p");
        tree.append_child(div, p2);
        let p2_text = tree.create_text("Hidden");
        tree.append_child(p2, p2_text);

        let (result, _) = serialize_a11y(&tree, None);
        assert!(result.contains("Visible"));
        assert!(
            !result.contains("Hidden"),
            "display:none should hide descendants: {}",
            result
        );
    }

    #[test]
    fn visibility_hidden_keeps_structure_hides_text() {
        let mut tree = DomTree::new();
        let doc = tree.document();
        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let p = tree.create_element("p");
        tree.append_child(body, p);
        let p_text = tree.create_text("Hidden text");
        tree.append_child(p, p_text);

        let mut style = HashMap::new();
        style.insert("visibility".to_string(), "hidden".to_string());
        tree.get_node_mut(p).computed_style = Some(style);

        let (result, _) = serialize_a11y(&tree, None);
        // The paragraph role should still appear, but without text
        assert!(
            result.contains("paragraph"),
            "structure should be preserved: {}",
            result
        );
        assert!(!result.contains("Hidden text"), "text should be hidden: {}", result);
    }

    #[test]
    fn display_none_hides_interactive_elements_no_refs() {
        let mut tree = DomTree::new();
        let doc = tree.document();
        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let btn = tree.create_element("button");
        tree.append_child(body, btn);
        let btn_text = tree.create_text("Click me");
        tree.append_child(btn, btn_text);

        let mut style = HashMap::new();
        style.insert("display".to_string(), "none".to_string());
        tree.get_node_mut(btn).computed_style = Some(style);

        let (result, ref_map) = serialize_a11y(&tree, None);
        assert!(
            !result.contains("button"),
            "hidden button should not appear: {}",
            result
        );
        assert!(ref_map.is_empty(), "hidden elements should not get refs");
    }

    #[test]
    fn no_computed_style_falls_back_to_normal_behavior() {
        // Same as existing tests -- when computed_style is None, everything works as before
        let mut tree = DomTree::new();
        let doc = tree.document();
        let body = tree.create_element("body");
        tree.append_child(doc, body);
        let p = tree.create_element("p");
        tree.append_child(body, p);
        let text = tree.create_text("Normal");
        tree.append_child(p, text);

        let (result, _) = serialize_a11y(&tree, None);
        assert_eq!(result, "paragraph \"Normal\"");
    }

    #[test]
    fn visibility_hidden_interactive_still_gets_ref() {
        let mut tree = DomTree::new();
        let doc = tree.document();
        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let input = tree.create_element("input");
        tree.append_child(body, input);
        // Add type attribute manually
        if let crate::dom::node::NodeData::Element { ref mut attributes, .. } = tree.get_node_mut(input).data {
            attributes.push(crate::dom::node::DomAttribute::new("type", "text"));
        }
        let mut style = HashMap::new();
        style.insert("visibility".to_string(), "hidden".to_string());
        tree.get_node_mut(input).computed_style = Some(style);

        let (result, ref_map) = serialize_a11y(&tree, None);
        assert!(
            result.contains("@e1"),
            "hidden-visibility input should still get ref: {}",
            result
        );
        assert_eq!(ref_map.len(), 1);
    }

    #[test]
    fn display_none_on_parent_skips_entire_subtree() {
        let mut tree = DomTree::new();
        let doc = tree.document();
        let body = tree.create_element("body");
        tree.append_child(doc, body);

        let nav = tree.create_element("nav");
        tree.append_child(body, nav);
        let mut style = HashMap::new();
        style.insert("display".to_string(), "none".to_string());
        tree.get_node_mut(nav).computed_style = Some(style);

        let a = tree.create_element("a");
        tree.append_child(nav, a);
        // Add href
        if let crate::dom::node::NodeData::Element { ref mut attributes, .. } = tree.get_node_mut(a).data {
            attributes.push(crate::dom::node::DomAttribute::new("href", "/home"));
        }
        let a_text = tree.create_text("Home");
        tree.append_child(a, a_text);

        let p = tree.create_element("p");
        tree.append_child(body, p);
        let p_text = tree.create_text("Visible");
        tree.append_child(p, p_text);

        let (result, ref_map) = serialize_a11y(&tree, None);
        assert!(!result.contains("Home"), "nav content should be hidden: {}", result);
        assert!(
            !result.contains("link"),
            "link inside display:none should be hidden: {}",
            result
        );
        assert!(result.contains("Visible"), "visible content should appear: {}", result);
        assert!(ref_map.is_empty(), "no refs for hidden elements");
    }
}
