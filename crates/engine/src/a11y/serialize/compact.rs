use crate::dom::node::{NodeData, NodeId};
use crate::dom::DomTree;
use std::collections::HashMap;

use super::helpers::{
    collect_deep_text, element_role, get_interactive_value, heading_level, is_display_none,
    is_visibility_hidden, normalize_compact, BLOCK_ELEMENTS,
    INTERACTIVE_ELEMENTS, SKIP_ELEMENTS,
};
use super::refs::assign_refs;

// ---------------------------------------------------------------------------
// Compact view — text content + interactive elements, token-efficient
// ---------------------------------------------------------------------------

/// Compact serialization: text flows naturally, interactive elements annotated inline,
/// headings get # prefixes. The default view for LLM agents.
pub fn serialize_compact(tree: &DomTree, focused: Option<NodeId>) -> (String, HashMap<String, NodeId>) {
    let (ref_map, reverse) = assign_refs(tree);
    let mut output = String::new();
    walk_compact(tree, tree.document(), &reverse, &mut output, focused);
    let output = normalize_compact(&output);
    (output, ref_map)
}

fn walk_compact(
    tree: &DomTree,
    root: NodeId,
    reverse: &HashMap<NodeId, String>,
    output: &mut String,
    focused: Option<NodeId>,
) {
    enum Work {
        Open(NodeId),
        CloseBlock,
    }

    let mut stack: Vec<Work> = vec![Work::Open(root)];
    while let Some(item) = stack.pop() {
        match item {
            Work::CloseBlock => {
                output.push('\n');
            }
            Work::Open(node_id) => {
                let node = tree.get_node(node_id);
                match &node.data {
                    NodeData::Text { content } | NodeData::CDATASection { content } => {
                        let trimmed = content.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !trimmed.is_empty() {
                            if !output.is_empty() && !output.ends_with('\n') && !output.ends_with(' ') {
                                output.push(' ');
                            }
                            output.push_str(&trimmed);
                        }
                    }
                    NodeData::Element {
                        tag_name, attributes, ..
                    } => {
                        let tag = tag_name.to_ascii_lowercase();

                        if is_display_none(node) || is_visibility_hidden(node) {
                            continue;
                        }
                        if SKIP_ELEMENTS.contains(&tag.as_str()) {
                            continue;
                        }

                        let is_block = BLOCK_ELEMENTS.contains(&tag.as_str());
                        let is_interactive = INTERACTIVE_ELEMENTS.contains(&tag.as_str());
                        let eref = reverse.get(&node_id);

                        // Headings: # prefix
                        if let Some(level) = heading_level(&tag) {
                            if !output.is_empty() && !output.ends_with('\n') {
                                output.push('\n');
                            }
                            for _ in 0..level {
                                output.push('#');
                            }
                            output.push(' ');
                            let text = collect_deep_text(tree, node_id).trim().to_string();
                            output.push_str(&text);
                            if let Some(r) = eref {
                                output.push_str(&format!(" [{}]", r));
                            }
                            output.push('\n');
                            continue;
                        }

                        // Images: alt text
                        if tag == "img" {
                            let alt = attributes
                                .iter()
                                .find(|a| a.local_name == "alt")
                                .map(|a| a.value.as_str())
                                .unwrap_or("");
                            if !alt.is_empty() {
                                output.push_str(&format!("[image: {}]", alt));
                            }
                            continue;
                        }

                        // Interactive elements: [ref type "text" ="value"]
                        if is_interactive {
                            let role = element_role(&tag, attributes);
                            let text = collect_deep_text(tree, node_id)
                                .split_whitespace()
                                .collect::<Vec<_>>()
                                .join(" ");
                            let ref_str = eref.map(|r| r.as_str()).unwrap_or("?");

                            let mut token = format!("[{} {}", ref_str, role);
                            if let Some(val) = get_interactive_value(tree, node_id, &tag, attributes) {
                                token.push_str(&format!(" =\"{}\"", val));
                            }
                            if !text.is_empty() && tag != "input" {
                                token.push_str(&format!(" \"{}\"", text));
                            }
                            if focused == Some(node_id) {
                                token.push_str(" focused");
                            }
                            token.push(']');
                            output.push_str(&token);
                            continue;
                        }

                        // List items: bullet prefix
                        if tag == "li" {
                            if !output.is_empty() && !output.ends_with('\n') {
                                output.push('\n');
                            }
                            let parent_tag = node.parent.and_then(|pid| {
                                if let NodeData::Element { tag_name, .. } =
                                    &tree.get_node(pid).data
                                {
                                    Some(tag_name.to_ascii_lowercase())
                                } else {
                                    None
                                }
                            });
                            if parent_tag.as_deref() == Some("ol") {
                                let parent_id = node.parent.unwrap();
                                let parent_node = tree.get_node(parent_id);
                                let li_index = parent_node
                                    .children
                                    .iter()
                                    .filter(|&&cid| {
                                        matches!(
                                            &tree.get_node(cid).data,
                                            NodeData::Element { tag_name, .. }
                                                if tag_name.eq_ignore_ascii_case("li")
                                        )
                                    })
                                    .position(|&cid| cid == node_id)
                                    .unwrap_or(0);
                                output.push_str(&format!("{}. ", li_index + 1));
                            } else {
                                output.push_str("- ");
                            }
                            stack.push(Work::CloseBlock);
                            let children: Vec<NodeId> = node.children.clone();
                            for child_id in children.into_iter().rev() {
                                stack.push(Work::Open(child_id));
                            }
                            continue;
                        }

                        // Block elements: newline before, push close marker
                        if is_block {
                            if !output.is_empty() && !output.ends_with('\n') {
                                output.push('\n');
                            }
                            stack.push(Work::CloseBlock);
                        }

                        let children: Vec<NodeId> = node.children.clone();
                        for child_id in children.into_iter().rev() {
                            stack.push(Work::Open(child_id));
                        }
                    }
                    NodeData::Document | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
                        let children: Vec<NodeId> = node.children.clone();
                        for child_id in children.into_iter().rev() {
                            stack.push(Work::Open(child_id));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
