//! Shared helper functions, macros, and utilities used across the Range module.

use boa_engine::{js_string, JsError, JsResult, JsValue};

use crate::dom::{DomTree, NodeData, NodeId};
use super::types::JsRange;
use crate::js::bindings::element::JsElement;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(super) fn extract_node(val: &JsValue) -> JsResult<NodeId> {
    let obj = val
        .as_object()
        .ok_or_else(|| JsError::from_native(boa_engine::JsNativeError::typ().with_message("Value is not a Node")))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_native(boa_engine::JsNativeError::typ().with_message("Value is not a Node")))?;
    Ok(el.node_id)
}

pub(super) fn child_index(tree: &DomTree, parent: NodeId, child: NodeId) -> usize {
    tree.get_node(parent)
        .children
        .iter()
        .position(|&c| c == child)
        .expect("child_index: child not found in parent")
}

/// Macro to extract JsRange from `this`. Binds to local variable named `$name`.
macro_rules! get_range {
    ($this:expr, $name:ident) => {
        let __obj = $this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("not a Range").into()))?;
        let $name = __obj
            .downcast_ref::<JsRange>()
            .ok_or_else(|| JsError::from_opaque(js_string!("not a Range").into()))?;
    };
}
pub(super) use get_range;

/// Validate a boundary point: doctype -> InvalidNodeTypeError, offset > nodeLength -> IndexSizeError
pub(super) fn validate_boundary(node_id: NodeId, offset: usize, tree: &DomTree) -> JsResult<()> {
    if matches!(tree.get_node(node_id).data, NodeData::Doctype { .. }) {
        return Err(JsError::from_opaque(js_string!("InvalidNodeTypeError").into()));
    }
    let len = node_length(tree, node_id);
    if offset > len {
        return Err(JsError::from_opaque(js_string!("IndexSizeError").into()));
    }
    Ok(())
}

/// Compute the "length" of a node per DOM spec:
/// - DocumentType -> 0
/// - CharacterData (Text/Comment/PI/CDATA) -> UTF-16 code unit count
/// - Everything else -> number of children
pub(super) fn node_length(tree: &DomTree, node_id: NodeId) -> usize {
    let node = tree.get_node(node_id);
    match &node.data {
        NodeData::Doctype { .. } => 0,
        NodeData::Text { .. } | NodeData::Comment { .. } | NodeData::ProcessingInstruction { .. } | NodeData::CDATASection { .. } => {
            tree.character_data_get(node_id)
                .map(|s| s.encode_utf16().count())
                .unwrap_or(0)
        }
        _ => node.children.len(),
    }
}

/// Collect ancestor chain from node to root (inclusive), returning [node, parent, grandparent, ...].
pub(super) fn ancestor_chain(tree: &DomTree, node_id: NodeId) -> Vec<NodeId> {
    let mut chain = vec![node_id];
    let mut current = node_id;
    while let Some(parent) = tree.get_node(current).parent {
        chain.push(parent);
        current = parent;
    }
    chain
}

/// Compare two boundary points per DOM spec section 4.2.
/// Returns -1, 0, or 1.
pub(super) fn compare_boundary_points_impl(
    tree: &DomTree,
    node_a: NodeId,
    offset_a: usize,
    node_b: NodeId,
    offset_b: usize,
) -> i8 {
    if node_a == node_b {
        return if offset_a == offset_b {
            0
        } else if offset_a < offset_b {
            -1
        } else {
            1
        };
    }

    let pos = tree.compare_document_position(node_a, node_b);
    const DISCONNECTED: u16 = 0x01;
    const PRECEDING: u16 = 0x02;
    #[allow(dead_code)]
    const FOLLOWING: u16 = 0x04;
    #[allow(dead_code)]
    const CONTAINS: u16 = 0x08;
    const CONTAINED_BY: u16 = 0x10;

    // Disconnected nodes: arbitrary but consistent ordering
    if pos & DISCONNECTED != 0 {
        return if node_a < node_b { -1 } else { 1 };
    }

    // pos = compare_document_position(reference=node_a, other=node_b)
    // FOLLOWING (0x04): node_b follows node_a -> A is before B in tree order
    // PRECEDING (0x02): node_b precedes node_a -> A is after B in tree order

    // Step 2: "If node A is after node B in tree order" -> PRECEDING flag
    if pos & PRECEDING != 0 {
        let result = compare_boundary_points_impl(tree, node_b, offset_b, node_a, offset_a);
        return -result;
    }

    // Step 3: "If node A is an ancestor of node B"
    // CONTAINED_BY (0x10): node_b is contained by node_a -> A is ancestor of B
    if pos & CONTAINED_BY != 0 {
        // Walk up from B to find the child of A
        let mut child = node_b;
        while tree.get_node(child).parent != Some(node_a) {
            match tree.get_node(child).parent {
                Some(p) => child = p,
                None => return -1,
            }
        }
        let child_idx = child_index(tree, node_a, child);
        if child_idx < offset_a {
            return 1; // "after"
        }
    }

    // Step 4: "Return before."
    -1
}

pub(super) fn collect_text_nodes_in_order(tree: &DomTree, node_id: NodeId, result: &mut Vec<NodeId>) {
    if matches!(tree.get_node(node_id).data, NodeData::Text { .. }) {
        result.push(node_id);
    }
    for &child in &tree.get_node(node_id).children {
        collect_text_nodes_in_order(tree, child, result);
    }
}

pub(super) fn is_character_data(tree: &DomTree, node_id: NodeId) -> bool {
    matches!(
        tree.get_node(node_id).data,
        NodeData::Text { .. } | NodeData::Comment { .. } | NodeData::ProcessingInstruction { .. } | NodeData::CDATASection { .. }
    )
}

pub(super) fn root_of(tree: &DomTree, node_id: NodeId) -> NodeId {
    let mut current = node_id;
    while let Some(parent) = tree.get_node(current).parent {
        current = parent;
    }
    current
}

/// Helper: check if `descendant` is an inclusive descendant of `ancestor` in the given tree.
pub(super) fn is_inclusive_descendant(tree: &DomTree, descendant: NodeId, ancestor: NodeId) -> bool {
    let mut current = descendant;
    loop {
        if current == ancestor {
            return true;
        }
        match tree.get_parent(current) {
            Some(p) => current = p,
            None => return false,
        }
    }
}
