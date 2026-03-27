//! Live range mutation hooks — called from mutation.rs and mutation_observer.rs.

use boa_engine::Context;

use crate::dom::{DomTree, NodeId};
use super::helpers::is_inclusive_descendant;

/// Called after a child node is inserted into `parent_id` at child index `new_index`.
/// Per spec section 4.2: "For each live range whose start node is parent and start offset
/// is greater than index, increase its start offset by count."
pub(crate) fn update_ranges_for_insert(ctx: &mut Context, parent_id: NodeId, new_index: usize, count: usize) {
    let registry = crate::js::realm_state::live_ranges(ctx);
    let reg = registry.borrow();
    for weak in reg.iter() {
        if let Some(inner) = weak.upgrade() {
            if inner.start_node.get() == parent_id && inner.start_offset.get() > new_index {
                inner.start_offset.set(inner.start_offset.get() + count);
            }
            if inner.end_node.get() == parent_id && inner.end_offset.get() > new_index {
                inner.end_offset.set(inner.end_offset.get() + count);
            }
        }
    }
}

/// Called before a child node at `old_index` is removed from `parent_id`.
/// Per spec section 4.2 removing steps: for each boundary, if node is removed_node
/// or descendant, set to (parent_id, old_index). If node == parent_id and
/// offset > old_index, decrement.
pub(crate) fn update_ranges_for_remove(
    ctx: &mut Context,
    parent_id: NodeId,
    old_index: usize,
    removed_node_id: NodeId,
    tree: &DomTree,
) {
    let registry = crate::js::realm_state::live_ranges(ctx);
    let reg = registry.borrow();
    for weak in reg.iter() {
        if let Some(inner) = weak.upgrade() {
            // Start boundary
            if is_inclusive_descendant(tree, inner.start_node.get(), removed_node_id) {
                inner.start_node.set(parent_id);
                inner.start_offset.set(old_index);
            } else if inner.start_node.get() == parent_id && inner.start_offset.get() > old_index {
                inner.start_offset.set(inner.start_offset.get() - 1);
            }

            // End boundary
            if is_inclusive_descendant(tree, inner.end_node.get(), removed_node_id) {
                inner.end_node.set(parent_id);
                inner.end_offset.set(old_index);
            } else if inner.end_node.get() == parent_id && inner.end_offset.get() > old_index {
                inner.end_offset.set(inner.end_offset.get() - 1);
            }
        }
    }
}

/// Called after character data replacement per spec section 4.2 "replace data" steps.
/// `node_id` is the CharacterData node. `offset` is where replacement starts.
/// `count` is how many UTF-16 code units were removed. `added_len` is how many
/// UTF-16 code units were inserted.
pub(crate) fn update_ranges_for_char_data(
    ctx: &mut Context,
    node_id: NodeId,
    offset: usize,
    count: usize,
    added_len: usize,
) {
    let registry = crate::js::realm_state::live_ranges(ctx);
    let reg = registry.borrow();
    for weak in reg.iter() {
        if let Some(inner) = weak.upgrade() {
            // Start boundary
            if inner.start_node.get() == node_id {
                let so = inner.start_offset.get();
                if so > offset && so <= offset + count {
                    inner.start_offset.set(offset);
                } else if so > offset + count {
                    inner.start_offset.set(so - count + added_len);
                }
            }
            // End boundary
            if inner.end_node.get() == node_id {
                let eo = inner.end_offset.get();
                if eo > offset && eo <= offset + count {
                    inner.end_offset.set(offset);
                } else if eo > offset + count {
                    inner.end_offset.set(eo - count + added_len);
                }
            }
        }
    }
}

/// Called after `Text.splitText(offset)` per spec section 4.2 "split a Text node" steps.
/// `old_node_id` is the original text node, `new_node_id` is the newly created
/// node (containing text after offset), `offset` is the split point (UTF-16),
/// `parent_id` is the parent (if any), `new_index` is the child index of new_node
/// in parent.
pub(crate) fn update_ranges_for_split_text(
    ctx: &mut Context,
    old_node_id: NodeId,
    new_node_id: NodeId,
    offset: usize,
    parent_id: Option<NodeId>,
    new_index: Option<usize>,
) {
    let registry = crate::js::realm_state::live_ranges(ctx);
    let reg = registry.borrow();
    for weak in reg.iter() {
        if let Some(inner) = weak.upgrade() {
            // Step 1 (from replaceData): already handled by update_ranges_for_char_data
            // Step 2: if boundary node == old_node and offset > split offset,
            //         move to new_node with offset adjusted
            if inner.start_node.get() == old_node_id && inner.start_offset.get() > offset {
                inner.start_node.set(new_node_id);
                inner.start_offset.set(inner.start_offset.get() - offset);
            }
            if inner.end_node.get() == old_node_id && inner.end_offset.get() > offset {
                inner.end_node.set(new_node_id);
                inner.end_offset.set(inner.end_offset.get() - offset);
            }

            // Step 3: if parent exists, adjust parent-based boundaries
            if let (Some(pid), Some(idx)) = (parent_id, new_index) {
                if inner.start_node.get() == pid && inner.start_offset.get() > idx {
                    inner.start_offset.set(inner.start_offset.get() + 1);
                }
                if inner.end_node.get() == pid && inner.end_offset.get() > idx {
                    inner.end_offset.set(inner.end_offset.get() + 1);
                }
            }
        }
    }
}
