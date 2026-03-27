use crate::dom::node::{NodeData, NodeId};
use crate::dom::DomTree;
use std::collections::HashMap;

use super::helpers::{INTERACTIVE_ELEMENTS, SKIP_ELEMENTS};

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
