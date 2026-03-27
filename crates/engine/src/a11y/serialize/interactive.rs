use crate::dom::node::{NodeData, NodeId};
use crate::dom::DomTree;
use std::collections::HashMap;

use super::helpers::{
    collect_deep_text, collect_direct_text, element_role, get_interactive_value, is_display_none,
    trim_trailing_newlines, INTERACTIVE_ELEMENTS, SKIP_ELEMENTS,
};

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
