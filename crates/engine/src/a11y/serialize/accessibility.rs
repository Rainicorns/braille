use crate::dom::node::{NodeData, NodeId};
use crate::dom::DomTree;
use std::collections::HashMap;

use super::helpers::{
    collect_direct_text, element_role, get_interactive_value, has_renderable_children,
    is_visibility_hidden, trim_trailing_newlines, SKIP_ELEMENTS, TRANSPARENT_ELEMENTS,
};
use super::refs::assign_refs;

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
            let is_vis_hidden = is_visibility_hidden(node);

            let direct_text = if is_vis_hidden {
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

            if direct_text.is_empty() && !has_child_output && eref.is_none() && !is_vis_hidden {
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
