use crate::dom::node::{NodeData, NodeId};
use crate::dom::DomTree;
use std::collections::HashMap;

use super::helpers::{
    collect_deep_text, collect_direct_text, collapse_whitespace, element_role, heading_level,
    is_display_none, is_visibility_hidden, trim_trailing_newlines, SKIP_ELEMENTS,
};

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
    super::accessibility::serialize_a11y_rooted(tree, target, reverse, focused)
}
