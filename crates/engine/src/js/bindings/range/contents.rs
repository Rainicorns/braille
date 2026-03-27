//! Heavy algorithms: clone_contents, delete/extract contents, range toString.

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{js_string, Context, JsResult, JsValue};

use crate::dom::{DomTree, NodeId};
use crate::js::bindings::mutation_observer;
use super::helpers::{
    ancestor_chain, child_index, collect_text_nodes_in_order,
    compare_boundary_points_impl, is_character_data, root_of,
};

/// Collect text content within range boundaries per spec.
/// Only Text nodes contribute to the result.
pub(super) fn range_to_string_impl(
    tree: &DomTree,
    start_node: NodeId,
    start_offset: usize,
    end_node: NodeId,
    end_offset: usize,
) -> String {
    // Walk all text nodes in tree order between the boundaries.
    // For each Text node, compute the portion within the range.
    let mut s = String::new();
    let start_root = root_of(tree, start_node);

    // Collect all text nodes under the root in document order
    let mut text_nodes = Vec::new();
    collect_text_nodes_in_order(tree, start_root, &mut text_nodes);

    for &text_id in &text_nodes {
        let text = tree.character_data_get(text_id).unwrap_or_default();
        let text_len = text.encode_utf16().count();

        // Compare text node boundaries against range boundaries:
        // start_of_text vs range_end: if text starts at or after range end, skip
        let text_start_vs_range_end = compare_boundary_points_impl(tree, text_id, 0, end_node, end_offset);
        if text_start_vs_range_end >= 0 {
            continue; // text node starts at or after range end
        }

        // end_of_text vs range_start: if text ends at or before range start, skip
        let text_end_vs_range_start = compare_boundary_points_impl(tree, text_id, text_len, start_node, start_offset);
        if text_end_vs_range_start <= 0 {
            continue; // text node ends at or before range start
        }

        // Compute the char offsets within this text node
        let char_start = if text_id == start_node {
            start_offset
        } else if compare_boundary_points_impl(tree, text_id, 0, start_node, start_offset) >= 0 {
            0 // text node starts at or after range start -> take from beginning
        } else {
            start_offset // text node contains the start boundary (shouldn't happen for non-start container)
        };

        let char_end = if text_id == end_node {
            end_offset
        } else if compare_boundary_points_impl(tree, text_id, text_len, end_node, end_offset) <= 0 {
            text_len // text node ends at or before range end -> take to the end
        } else {
            end_offset // text node extends past range end (shouldn't happen for non-end container)
        };

        let byte_start = DomTree::utf16_offset_to_byte_offset(&text, char_start).unwrap_or(text.len());
        let byte_end = DomTree::utf16_offset_to_byte_offset(&text, char_end.min(text_len)).unwrap_or(text.len());
        if byte_start < byte_end {
            s.push_str(&text[byte_start..byte_end]);
        }
    }

    s
}

pub(super) fn clone_contents_impl(
    _ctx: &mut Context,
    tree: &Rc<RefCell<DomTree>>,
    start_node: NodeId,
    start_offset: usize,
    end_node: NodeId,
    end_offset: usize,
) -> JsResult<NodeId> {
    let frag_id = tree.borrow_mut().create_document_fragment();

    // Empty range
    if start_node == end_node && start_offset == end_offset {
        return Ok(frag_id);
    }

    // Same container
    if start_node == end_node {
        let t = tree.borrow();
        if is_character_data(&t, start_node) {
            let text = t.character_data_get(start_node).unwrap_or_default();
            let byte_start = DomTree::utf16_offset_to_byte_offset(&text, start_offset).unwrap_or(text.len());
            let byte_end = DomTree::utf16_offset_to_byte_offset(&text, end_offset).unwrap_or(text.len());
            let cloned_text = text[byte_start..byte_end].to_string();
            drop(t);
            let clone_id = tree.borrow_mut().clone_node(start_node, false);
            tree.borrow_mut().character_data_set(clone_id, &cloned_text);
            tree.borrow_mut().append_child(frag_id, clone_id);
        } else {
            let children: Vec<NodeId> = t.get_node(start_node).children[start_offset..end_offset].to_vec();
            drop(t);
            for child in children {
                let clone = tree.borrow_mut().clone_node(child, true);
                tree.borrow_mut().append_child(frag_id, clone);
            }
        }
        return Ok(frag_id);
    }

    // Different containers — find common ancestor and clone structure
    let t = tree.borrow();

    // Find common ancestor
    let start_ancestors = ancestor_chain(&t, start_node);
    let end_ancestors = ancestor_chain(&t, end_node);
    let mut common = *start_ancestors.last().unwrap();
    for &a in &start_ancestors {
        if end_ancestors.contains(&a) {
            common = a;
            break;
        }
    }

    // Find first and last partially contained children of common ancestor
    let first_partial = if start_ancestors.contains(&common) && start_node != common {
        // Find child of common that is ancestor of start
        start_ancestors.iter().find(|&&a| t.get_node(a).parent == Some(common)).copied()
    } else {
        None
    };

    let last_partial = if end_ancestors.contains(&common) && end_node != common {
        end_ancestors.iter().find(|&&a| t.get_node(a).parent == Some(common)).copied()
    } else {
        None
    };

    drop(t);

    // Clone first partially contained child
    if let Some(fp) = first_partial {
        let t = tree.borrow();
        if is_character_data(&t, start_node) && start_node == fp {
            let text = t.character_data_get(start_node).unwrap_or_default();
            let byte_start = DomTree::utf16_offset_to_byte_offset(&text, start_offset).unwrap_or(text.len());
            let cloned_text = text[byte_start..].to_string();
            drop(t);
            let clone_id = tree.borrow_mut().clone_node(start_node, false);
            tree.borrow_mut().character_data_set(clone_id, &cloned_text);
            tree.borrow_mut().append_child(frag_id, clone_id);
        } else {
            drop(t);
            let clone = tree.borrow_mut().clone_node(fp, true);
            tree.borrow_mut().append_child(frag_id, clone);
        }
    }

    // Clone fully contained children between first_partial and last_partial
    {
        let t = tree.borrow();
        let common_children = &t.get_node(common).children;
        let start_idx = first_partial.map(|fp| child_index(&t, common, fp) + 1).unwrap_or(0);
        let end_idx = last_partial.map(|lp| child_index(&t, common, lp)).unwrap_or(common_children.len());
        let contained: Vec<NodeId> = common_children[start_idx..end_idx].to_vec();
        drop(t);

        for child in contained {
            let clone = tree.borrow_mut().clone_node(child, true);
            tree.borrow_mut().append_child(frag_id, clone);
        }
    }

    // Clone last partially contained child
    if let Some(lp) = last_partial {
        let t = tree.borrow();
        if is_character_data(&t, end_node) && end_node == lp {
            let text = t.character_data_get(end_node).unwrap_or_default();
            let byte_end = DomTree::utf16_offset_to_byte_offset(&text, end_offset).unwrap_or(text.len());
            let cloned_text = text[..byte_end].to_string();
            drop(t);
            let clone_id = tree.borrow_mut().clone_node(end_node, false);
            tree.borrow_mut().character_data_set(clone_id, &cloned_text);
            tree.borrow_mut().append_child(frag_id, clone_id);
        } else {
            drop(t);
            let clone = tree.borrow_mut().clone_node(lp, true);
            tree.borrow_mut().append_child(frag_id, clone);
        }
    }

    Ok(frag_id)
}

// ---------------------------------------------------------------------------
// Shared delete/extract implementation
// ---------------------------------------------------------------------------

pub(super) fn delete_or_extract_contents(
    ctx: &mut Context,
    tree: &Rc<RefCell<DomTree>>,
    start_node: NodeId,
    start_offset: usize,
    end_node: NodeId,
    end_offset: usize,
    extract: bool,
) -> JsResult<Option<NodeId>> {
    let frag_id = if extract {
        Some(tree.borrow_mut().create_document_fragment())
    } else {
        None
    };

    // Empty range
    if start_node == end_node && start_offset == end_offset {
        return Ok(frag_id);
    }

    // Case 1: Same container
    if start_node == end_node {
        let is_chardata = is_character_data(&tree.borrow(), start_node);
        if is_chardata {
            let count = end_offset - start_offset;
            if extract {
                let text = tree.borrow().character_data_get(start_node).unwrap_or_default();
                let byte_start = DomTree::utf16_offset_to_byte_offset(&text, start_offset).unwrap_or(text.len());
                let byte_end = DomTree::utf16_offset_to_byte_offset(&text, end_offset).unwrap_or(text.len());
                let extracted = &text[byte_start..byte_end];
                let text_id = tree.borrow_mut().create_text(extracted);
                tree.borrow_mut().append_child(frag_id.unwrap(), text_id);
            }
            mutation_observer::character_data_delete_with_observer(ctx, tree, start_node, start_offset, count)
                .map_err(|e| boa_engine::JsError::from_opaque(boa_engine::js_string!(e).into()))?;
        } else {
            let children_to_remove: Vec<NodeId> = {
                let t = tree.borrow();
                let node = t.get_node(start_node);
                node.children[start_offset..end_offset].to_vec()
            };

            let prev_sib = if start_offset > 0 {
                Some(tree.borrow().get_node(start_node).children[start_offset - 1])
            } else {
                None
            };
            let next_sib = {
                let t = tree.borrow();
                let node = t.get_node(start_node);
                if end_offset < node.children.len() {
                    Some(node.children[end_offset])
                } else {
                    None
                }
            };

            for &child_id in &children_to_remove {
                tree.borrow_mut().remove_child(start_node, child_id);
                if extract {
                    tree.borrow_mut().append_child(frag_id.unwrap(), child_id);
                }
            }

            if !children_to_remove.is_empty() {
                mutation_observer::queue_childlist_mutation(
                    ctx, tree, start_node, vec![], children_to_remove, prev_sib, next_sib,
                );
            }
        }
        return Ok(frag_id);
    }

    // Case 2: Different containers
    let start_is_chardata = is_character_data(&tree.borrow(), start_node);
    let end_is_chardata = is_character_data(&tree.borrow(), end_node);

    // Find common ancestor
    let (common, first_partial, last_partial) = {
        let t = tree.borrow();
        let start_ancestors = ancestor_chain(&t, start_node);
        let end_ancestors = ancestor_chain(&t, end_node);
        let mut common = *start_ancestors.last().unwrap();
        for &a in &start_ancestors {
            if end_ancestors.contains(&a) {
                common = a;
                break;
            }
        }
        let fp = start_ancestors.iter().find(|&&a| t.get_node(a).parent == Some(common)).copied();
        let lp = end_ancestors.iter().find(|&&a| t.get_node(a).parent == Some(common)).copied();
        (common, fp, lp)
    };

    // Truncate start
    if start_is_chardata {
        let text = tree.borrow().character_data_get(start_node).unwrap_or_default();
        let byte_off = DomTree::utf16_offset_to_byte_offset(&text, start_offset).unwrap_or(text.len());
        if extract {
            let extracted = &text[byte_off..];
            let text_id = tree.borrow_mut().create_text(extracted);
            tree.borrow_mut().append_child(frag_id.unwrap(), text_id);
        }
        let kept = text[..byte_off].to_string();
        mutation_observer::character_data_set_with_observer(ctx, tree, start_node, &kept);
    }

    // Collect and remove fully-contained children of common ancestor
    let contained: Vec<NodeId> = {
        let t = tree.borrow();
        let common_children = &t.get_node(common).children;
        let start_idx = first_partial.map(|fp| child_index(&t, common, fp) + 1).unwrap_or(0);
        let end_idx = last_partial.map(|lp| child_index(&t, common, lp)).unwrap_or(common_children.len());
        common_children[start_idx..end_idx].to_vec()
    };

    if !contained.is_empty() {
        let prev_sib = first_partial;
        let next_sib = last_partial;
        for &child_id in &contained {
            tree.borrow_mut().remove_child(common, child_id);
            if extract {
                tree.borrow_mut().append_child(frag_id.unwrap(), child_id);
            }
        }
        mutation_observer::queue_childlist_mutation(ctx, tree, common, vec![], contained, prev_sib, next_sib);
    }

    // Truncate end
    if end_is_chardata {
        let text = tree.borrow().character_data_get(end_node).unwrap_or_default();
        let byte_off = DomTree::utf16_offset_to_byte_offset(&text, end_offset).unwrap_or(text.len());
        if extract {
            let extracted = &text[..byte_off];
            let text_id = tree.borrow_mut().create_text(extracted);
            tree.borrow_mut().append_child(frag_id.unwrap(), text_id);
        }
        let kept = text[byte_off..].to_string();
        mutation_observer::character_data_set_with_observer(ctx, tree, end_node, &kept);
    }

    Ok(frag_id)
}
