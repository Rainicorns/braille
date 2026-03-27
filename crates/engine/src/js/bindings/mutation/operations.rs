use boa_engine::Context;

use crate::dom::{DomTree, NodeData, NodeId};
use std::cell::RefCell;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// Perform the actual insertion after validation
// ---------------------------------------------------------------------------

/// Info about removal from old parent: (parent_id, prev_sibling, next_sibling).
pub(crate) type RemovalInfo = Option<(NodeId, Option<NodeId>, Option<NodeId>)>;

/// Capture the pre-state needed to fire MutationObserver childList records
/// for an insertion (insertBefore, appendChild, append, prepend, etc.).
///
/// Returns (added_ids, removal_info, prev_sibling, next_sibling).
pub(crate) fn capture_insert_state(
    tree: &Rc<RefCell<DomTree>>,
    parent_id: NodeId,
    node_id: NodeId,
    child_ref: Option<NodeId>,
) -> (Vec<NodeId>, RemovalInfo, Option<NodeId>, Option<NodeId>) {
    let t = tree.borrow();
    let is_fragment = matches!(t.get_node(node_id).data, NodeData::DocumentFragment | NodeData::ShadowRoot { .. });
    let added = if is_fragment {
        t.get_node(node_id).children.clone()
    } else {
        vec![node_id]
    };

    // Capture removal from old parent if node is being moved
    let old_parent = t.get_node(node_id).parent;
    let removal_info = if let Some(old_pid) = old_parent {
        if !is_fragment {
            let old_children = &t.get_node(old_pid).children;
            let pos = old_children.iter().position(|&c| c == node_id);
            let old_prev = pos.and_then(|p| if p > 0 { Some(old_children[p - 1]) } else { None });
            let old_next = pos.and_then(|p| old_children.get(p + 1).copied());
            Some((old_pid, old_prev, old_next))
        } else {
            None
        }
    } else {
        None
    };

    // Capture siblings at insertion point
    let parent_children = &t.get_node(parent_id).children;
    let (ps, ns) = if let Some(ref_child) = child_ref {
        let pos = parent_children.iter().position(|&c| c == ref_child);
        let prev = pos.and_then(|p| if p > 0 { Some(parent_children[p - 1]) } else { None });
        // Filter out the node being moved (if it's the prev sibling)
        let prev = prev.filter(|&p| p != node_id);
        (prev, Some(ref_child))
    } else {
        // Appending at end
        let prev = parent_children.last().copied();
        // Filter out the node being moved
        let prev = prev.filter(|&p| p != node_id);
        (prev, None)
    };

    (added, removal_info, ps, ns)
}

/// Fire MutationObserver childList records after an insertion,
/// and update live range boundaries.
pub(crate) fn fire_insert_records(
    ctx: &mut Context,
    tree: &Rc<RefCell<DomTree>>,
    parent_id: NodeId,
    added_ids: &[NodeId],
    removal_info: RemovalInfo,
    prev_sib: Option<NodeId>,
    next_sib: Option<NodeId>,
) {
    // Queue removal from old parent
    if let Some((old_pid, old_prev, old_next)) = removal_info {
        super::super::mutation_observer::queue_childlist_mutation(
            ctx,
            tree,
            old_pid,
            vec![],
            vec![added_ids[0]],
            old_prev,
            old_next,
        );
    }

    // Update live range boundaries for the insertion
    if !added_ids.is_empty() {
        let t = tree.borrow();
        let parent_children = &t.get_node(parent_id).children;
        // Find the index of the first added node in the parent's children
        if let Some(first_idx) = parent_children.iter().position(|&c| c == added_ids[0]) {
            drop(t);
            super::super::range::update_ranges_for_insert(ctx, parent_id, first_idx, added_ids.len());
        }
    }

    // Queue addition to new parent
    if !added_ids.is_empty() {
        super::super::mutation_observer::queue_childlist_mutation(
            ctx,
            tree,
            parent_id,
            added_ids.to_vec(),
            vec![],
            prev_sib,
            next_sib,
        );
    }

    // Invoke connectedCallback for custom elements that were inserted
    for &nid in added_ids {
        super::super::custom_elements::invoke_connected_callback(tree, nid, ctx);
    }
}

/// Update live range boundaries for the removal of a node from its old parent
/// (called BEFORE `do_insert` when moving a node).
pub(crate) fn fire_range_removal_for_move(
    ctx: &mut Context,
    tree: &Rc<RefCell<DomTree>>,
    removal_info: &RemovalInfo,
    moved_node_id: NodeId,
) {
    if let Some((old_pid, _, _)) = *removal_info {
        let t = tree.borrow();
        let old_children = &t.get_node(old_pid).children;
        if let Some(old_idx) = old_children.iter().position(|&c| c == moved_node_id) {
            super::super::range::update_ranges_for_remove(ctx, old_pid, old_idx, moved_node_id, &t);
            super::super::node_iterator::update_iterators_for_removal(ctx, moved_node_id, &t);
        }
    }
}

pub(crate) fn do_insert(tree: &Rc<RefCell<DomTree>>, parent_id: NodeId, node_id: NodeId, child_ref: Option<NodeId>) {
    let is_fragment = matches!(tree.borrow().get_node(node_id).data, NodeData::DocumentFragment | NodeData::ShadowRoot { .. });

    if is_fragment {
        let children: Vec<NodeId> = tree.borrow().get_node(node_id).children.clone();
        for frag_child in children {
            match child_ref {
                Some(ref_id) => tree.borrow_mut().insert_child_before(parent_id, frag_child, ref_id),
                None => tree.borrow_mut().append_child(parent_id, frag_child),
            }
        }
    } else {
        // Special case: if node == child_ref, it's already in the right place
        if child_ref == Some(node_id) {
            return;
        }
        match child_ref {
            Some(ref_id) => tree.borrow_mut().insert_child_before(parent_id, node_id, ref_id),
            None => tree.borrow_mut().append_child(parent_id, node_id),
        }
    }
}

pub(super) fn do_replace(tree: &Rc<RefCell<DomTree>>, parent_id: NodeId, node_id: NodeId, old_child_id: NodeId) {
    let is_fragment = matches!(tree.borrow().get_node(node_id).data, NodeData::DocumentFragment | NodeData::ShadowRoot { .. });

    if node_id == old_child_id {
        // Replacing a node with itself is a no-op
        return;
    }

    if is_fragment {
        let frag_children: Vec<NodeId> = tree.borrow().get_node(node_id).children.clone();
        // Find the position of old_child, insert fragment children there, then remove old_child
        let next_sibling = tree.borrow().next_sibling(old_child_id);
        tree.borrow_mut().remove_child(parent_id, old_child_id);
        for frag_child in frag_children {
            match next_sibling {
                Some(ref_id) => tree.borrow_mut().insert_child_before(parent_id, frag_child, ref_id),
                None => tree.borrow_mut().append_child(parent_id, frag_child),
            }
        }
    } else {
        tree.borrow_mut().replace_child(parent_id, node_id, old_child_id);
    }
}
