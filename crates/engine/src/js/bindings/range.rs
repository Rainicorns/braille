//! Range API — enough to pass MutationObserver-characterData and MutationObserver-childList tests.
//!
//! Implements: createRange, setStart/End, setStart/EndBefore/After,
//! deleteContents, extractContents, insertNode, surroundContents.

use std::cell::Cell;
use std::rc::Rc;
use std::cell::RefCell;

use boa_engine::{
    js_string, native_function::NativeFunction, object::ObjectInitializer,
    property::Attribute, Context, JsError, JsObject, JsResult, JsValue,
};

use crate::dom::{DomTree, NodeId, NodeData};
use super::element::JsElement;
use super::mutation_observer;

// ---------------------------------------------------------------------------
// JsRange — native data stored on the Range JsObject
// ---------------------------------------------------------------------------

#[derive(Debug, boa_engine::JsData, boa_gc::Trace, boa_gc::Finalize)]
pub(crate) struct JsRange {
    #[unsafe_ignore_trace]
    tree: Rc<RefCell<DomTree>>,
    #[unsafe_ignore_trace]
    start_node: Cell<NodeId>,
    #[unsafe_ignore_trace]
    start_offset: Cell<usize>,
    #[unsafe_ignore_trace]
    end_node: Cell<NodeId>,
    #[unsafe_ignore_trace]
    end_offset: Cell<usize>,
}

// ---------------------------------------------------------------------------
// Helper: extract (tree, node_id) from a JsValue arg
// ---------------------------------------------------------------------------

fn extract_node(val: &JsValue) -> JsResult<NodeId> {
    let obj = val
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("Range: argument is not a node").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("Range: argument is not a node").into()))?;
    Ok(el.node_id)
}

/// Find position of `child` in `parent`'s children list.
fn child_index(tree: &DomTree, parent: NodeId, child: NodeId) -> usize {
    tree.get_node(parent)
        .children
        .iter()
        .position(|&c| c == child)
        .expect("child_index: child not found in parent")
}

// ---------------------------------------------------------------------------
// Range prototype methods
// ---------------------------------------------------------------------------

fn range_set_start(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("setStart: not a Range").into()))?;
    let range = this_obj
        .downcast_ref::<JsRange>()
        .ok_or_else(|| JsError::from_opaque(js_string!("setStart: not a Range").into()))?;

    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let offset = args
        .get(1)
        .map(|v| v.to_number(_ctx))
        .transpose()?
        .unwrap_or(0.0) as usize;

    range.start_node.set(node_id);
    range.start_offset.set(offset);
    Ok(JsValue::undefined())
}

fn range_set_end(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("setEnd: not a Range").into()))?;
    let range = this_obj
        .downcast_ref::<JsRange>()
        .ok_or_else(|| JsError::from_opaque(js_string!("setEnd: not a Range").into()))?;

    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let offset = args
        .get(1)
        .map(|v| v.to_number(_ctx))
        .transpose()?
        .unwrap_or(0.0) as usize;

    range.end_node.set(node_id);
    range.end_offset.set(offset);
    Ok(JsValue::undefined())
}

fn range_set_start_before(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("setStartBefore: not a Range").into()))?;
    let range = this_obj
        .downcast_ref::<JsRange>()
        .ok_or_else(|| JsError::from_opaque(js_string!("setStartBefore: not a Range").into()))?;

    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let tree = range.tree.borrow();
    let parent = tree.get_node(node_id).parent.expect("setStartBefore: node has no parent");
    let idx = child_index(&tree, parent, node_id);
    drop(tree);

    range.start_node.set(parent);
    range.start_offset.set(idx);
    Ok(JsValue::undefined())
}

fn range_set_start_after(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("setStartAfter: not a Range").into()))?;
    let range = this_obj
        .downcast_ref::<JsRange>()
        .ok_or_else(|| JsError::from_opaque(js_string!("setStartAfter: not a Range").into()))?;

    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let tree = range.tree.borrow();
    let parent = tree.get_node(node_id).parent.expect("setStartAfter: node has no parent");
    let idx = child_index(&tree, parent, node_id);
    drop(tree);

    range.start_node.set(parent);
    range.start_offset.set(idx + 1);
    Ok(JsValue::undefined())
}

fn range_set_end_before(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("setEndBefore: not a Range").into()))?;
    let range = this_obj
        .downcast_ref::<JsRange>()
        .ok_or_else(|| JsError::from_opaque(js_string!("setEndBefore: not a Range").into()))?;

    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let tree = range.tree.borrow();
    let parent = tree.get_node(node_id).parent.expect("setEndBefore: node has no parent");
    let idx = child_index(&tree, parent, node_id);
    drop(tree);

    range.end_node.set(parent);
    range.end_offset.set(idx);
    Ok(JsValue::undefined())
}

fn range_set_end_after(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("setEndAfter: not a Range").into()))?;
    let range = this_obj
        .downcast_ref::<JsRange>()
        .ok_or_else(|| JsError::from_opaque(js_string!("setEndAfter: not a Range").into()))?;

    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let tree = range.tree.borrow();
    let parent = tree.get_node(node_id).parent.expect("setEndAfter: node has no parent");
    let idx = child_index(&tree, parent, node_id);
    drop(tree);

    range.end_node.set(parent);
    range.end_offset.set(idx + 1);
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// deleteContents
// ---------------------------------------------------------------------------

fn range_delete_contents(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("deleteContents: not a Range").into()))?;
    let (tree, start_node, start_offset, end_node, end_offset) = {
        let range = this_obj
            .downcast_ref::<JsRange>()
            .ok_or_else(|| JsError::from_opaque(js_string!("deleteContents: not a Range").into()))?;
        (
            range.tree.clone(),
            range.start_node.get(),
            range.start_offset.get(),
            range.end_node.get(),
            range.end_offset.get(),
        )
    };

    delete_or_extract_contents(ctx, &tree, start_node, start_offset, end_node, end_offset, false)?;
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// extractContents
// ---------------------------------------------------------------------------

fn range_extract_contents(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("extractContents: not a Range").into()))?;
    let (tree, start_node, start_offset, end_node, end_offset) = {
        let range = this_obj
            .downcast_ref::<JsRange>()
            .ok_or_else(|| JsError::from_opaque(js_string!("extractContents: not a Range").into()))?;
        (
            range.tree.clone(),
            range.start_node.get(),
            range.start_offset.get(),
            range.end_node.get(),
            range.end_offset.get(),
        )
    };

    let frag_id = delete_or_extract_contents(ctx, &tree, start_node, start_offset, end_node, end_offset, true)?;
    let frag_id = frag_id.expect("extractContents must return a fragment");
    let js_frag = super::element::get_or_create_js_element(frag_id, tree, ctx)?;
    Ok(js_frag.into())
}

// ---------------------------------------------------------------------------
// Shared delete/extract implementation
// ---------------------------------------------------------------------------

/// Performs the delete (extract=false) or extract (extract=true) operation.
/// Returns Some(frag_id) if extracting, None if deleting.
fn delete_or_extract_contents(
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

    // Case 1: Same container
    if start_node == end_node {
        let is_text = matches!(tree.borrow().get_node(start_node).data, NodeData::Text { .. });
        if is_text {
            // Delete chars [start_offset..end_offset]
            let count = end_offset - start_offset;
            if extract {
                // Clone the removed portion into fragment
                let text = tree.borrow().character_data_get(start_node).unwrap_or_default();
                let byte_start =
                    DomTree::utf16_offset_to_byte_offset(&text, start_offset).unwrap_or(text.len());
                let byte_end =
                    DomTree::utf16_offset_to_byte_offset(&text, end_offset).unwrap_or(text.len());
                let extracted = &text[byte_start..byte_end];
                let text_id = tree.borrow_mut().create_text(extracted);
                tree.borrow_mut().append_child(frag_id.unwrap(), text_id);
            }
            mutation_observer::character_data_delete_with_observer(ctx, tree, start_node, start_offset, count)
                .map_err(|e| JsError::from_opaque(js_string!(e).into()))?;
        } else {
            // Element container: remove children [start_offset..end_offset]
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
                    ctx,
                    tree,
                    start_node,
                    vec![],
                    children_to_remove,
                    prev_sib,
                    next_sib,
                );
            }
        }
        return Ok(frag_id);
    }

    // Case 2: Different containers (text nodes in same parent — the MO test pattern)
    // Start node is text: truncate to data[..start_offset]
    // End node is text: truncate to data[end_offset..]
    // Remove fully-contained nodes between them

    let start_is_text = matches!(tree.borrow().get_node(start_node).data, NodeData::Text { .. });
    let end_is_text = matches!(tree.borrow().get_node(end_node).data, NodeData::Text { .. });

    let start_parent = tree.borrow().get_node(start_node).parent;
    let end_parent = tree.borrow().get_node(end_node).parent;

    // For the MO tests, start and end are always children of the same parent
    let parent = start_parent.expect("deleteContents: start node has no parent");
    assert_eq!(
        parent,
        end_parent.expect("deleteContents: end node has no parent"),
        "deleteContents: cross-parent ranges not yet supported"
    );

    // Find the children between start_node and end_node (exclusive)
    let (start_idx, end_idx) = {
        let t = tree.borrow();
        let si = child_index(&t, parent, start_node);
        let ei = child_index(&t, parent, end_node);
        (si, ei)
    };

    let nodes_between: Vec<NodeId> = {
        let t = tree.borrow();
        t.get_node(parent).children[start_idx + 1..end_idx].to_vec()
    };

    // Truncate start text node
    if start_is_text && start_offset > 0 {
        let text = tree.borrow().character_data_get(start_node).unwrap_or_default();
        let byte_off = DomTree::utf16_offset_to_byte_offset(&text, start_offset).unwrap_or(text.len());
        let kept = &text[..byte_off];
        if extract {
            let extracted = &text[byte_off..];
            let text_id = tree.borrow_mut().create_text(extracted);
            tree.borrow_mut().append_child(frag_id.unwrap(), text_id);
        }
        mutation_observer::character_data_set_with_observer(ctx, tree, start_node, kept);
    } else if start_is_text {
        // start_offset == 0 — whole text node content is in range
        if extract {
            let text = tree.borrow().character_data_get(start_node).unwrap_or_default();
            let text_id = tree.borrow_mut().create_text(&text);
            tree.borrow_mut().append_child(frag_id.unwrap(), text_id);
        }
        mutation_observer::character_data_set_with_observer(ctx, tree, start_node, "");
    }

    // Truncate end text node
    if end_is_text && end_offset > 0 {
        let text = tree.borrow().character_data_get(end_node).unwrap_or_default();
        let byte_off = DomTree::utf16_offset_to_byte_offset(&text, end_offset).unwrap_or(text.len());
        let kept = &text[byte_off..];
        if extract {
            let extracted = &text[..byte_off];
            let text_id = tree.borrow_mut().create_text(extracted);
            tree.borrow_mut().append_child(frag_id.unwrap(), text_id);
        }
        mutation_observer::character_data_set_with_observer(ctx, tree, end_node, kept);
    } else if end_is_text {
        // end_offset == 0 — nothing to extract from end node
    }

    // Remove fully-contained nodes between start and end
    if !nodes_between.is_empty() {
        let prev_sib = Some(start_node);
        let next_sib = Some(end_node);

        for &child_id in &nodes_between {
            tree.borrow_mut().remove_child(parent, child_id);
            if extract {
                tree.borrow_mut().append_child(frag_id.unwrap(), child_id);
            }
        }

        mutation_observer::queue_childlist_mutation(ctx, tree, parent, vec![], nodes_between, prev_sib, next_sib);
    }

    Ok(frag_id)
}

// ---------------------------------------------------------------------------
// insertNode
// ---------------------------------------------------------------------------

fn range_insert_node(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertNode: not a Range").into()))?;
    let (tree, start_node, start_offset) = {
        let range = this_obj
            .downcast_ref::<JsRange>()
            .ok_or_else(|| JsError::from_opaque(js_string!("insertNode: not a Range").into()))?;
        (range.tree.clone(), range.start_node.get(), range.start_offset.get())
    };

    let new_node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;

    let is_text = matches!(tree.borrow().get_node(start_node).data, NodeData::Text { .. });

    if is_text {
        // Split the text node at start_offset, then insert new_node before the split portion
        let parent = tree.borrow().get_node(start_node).parent.expect("insertNode: text has no parent");
        let text_content = tree.borrow().character_data_get(start_node).unwrap_or_default();
        let utf16_len = text_content.encode_utf16().count();

        if start_offset > 0 && start_offset < utf16_len {
            // Split: keeps [..offset] in original, creates new node with [offset..]
            let split_id = tree
                .borrow_mut()
                .split_text(start_node, start_offset)
                .map_err(|e| JsError::from_opaque(js_string!(e).into()))?;

            // Fire characterData MO for the truncation of the original node
            mutation_observer::queue_childlist_mutation(
                ctx,
                &tree,
                parent,
                vec![split_id],
                vec![],
                Some(start_node),
                {
                    // next sibling of split_id
                    let t = tree.borrow();
                    let pidx = child_index(&t, parent, split_id);
                    if pidx + 1 < t.get_node(parent).children.len() {
                        Some(t.get_node(parent).children[pidx + 1])
                    } else {
                        None
                    }
                },
            );

            // Now insert new_node before the split portion
            let prev_sib = Some(start_node);
            let next_sib = Some(split_id);
            tree.borrow_mut().insert_before(split_id, new_node_id);
            mutation_observer::queue_childlist_mutation(
                ctx,
                &tree,
                parent,
                vec![new_node_id],
                vec![],
                prev_sib,
                next_sib,
            );
        } else if start_offset == 0 {
            // Insert before the text node itself
            let prev_sib = {
                let t = tree.borrow();
                let idx = child_index(&t, parent, start_node);
                if idx > 0 {
                    Some(t.get_node(parent).children[idx - 1])
                } else {
                    None
                }
            };
            tree.borrow_mut().insert_before(start_node, new_node_id);
            mutation_observer::queue_childlist_mutation(
                ctx,
                &tree,
                parent,
                vec![new_node_id],
                vec![],
                prev_sib,
                Some(start_node),
            );
        } else {
            // offset == length — insert after the text node
            let next_sib = {
                let t = tree.borrow();
                let idx = child_index(&t, parent, start_node);
                if idx + 1 < t.get_node(parent).children.len() {
                    Some(t.get_node(parent).children[idx + 1])
                } else {
                    None
                }
            };
            tree.borrow_mut().insert_after(start_node, new_node_id);
            mutation_observer::queue_childlist_mutation(
                ctx,
                &tree,
                parent,
                vec![new_node_id],
                vec![],
                Some(start_node),
                next_sib,
            );
        }
    } else {
        // Element container: insert at child position start_offset
        let children_len = tree.borrow().get_node(start_node).children.len();
        if start_offset >= children_len {
            // Append
            let prev_sib = if children_len > 0 {
                Some(tree.borrow().get_node(start_node).children[children_len - 1])
            } else {
                None
            };
            tree.borrow_mut().append_child(start_node, new_node_id);
            mutation_observer::queue_childlist_mutation(
                ctx,
                &tree,
                start_node,
                vec![new_node_id],
                vec![],
                prev_sib,
                None,
            );
        } else {
            // Insert before child at start_offset
            let ref_child = tree.borrow().get_node(start_node).children[start_offset];
            let prev_sib = if start_offset > 0 {
                Some(tree.borrow().get_node(start_node).children[start_offset - 1])
            } else {
                None
            };
            tree.borrow_mut().insert_before(ref_child, new_node_id);
            mutation_observer::queue_childlist_mutation(
                ctx,
                &tree,
                start_node,
                vec![new_node_id],
                vec![],
                prev_sib,
                Some(ref_child),
            );
        }
    }

    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// surroundContents
// ---------------------------------------------------------------------------

fn range_surround_contents(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("surroundContents: not a Range").into()))?;
    let (tree, start_node, start_offset, end_node, end_offset) = {
        let range = this_obj
            .downcast_ref::<JsRange>()
            .ok_or_else(|| JsError::from_opaque(js_string!("surroundContents: not a Range").into()))?;
        (
            range.tree.clone(),
            range.start_node.get(),
            range.start_offset.get(),
            range.end_node.get(),
            range.end_offset.get(),
        )
    };

    let wrapper_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;

    // The test pattern: start_node == end_node == parent element,
    // range covers children[start_offset..end_offset]
    assert_eq!(start_node, end_node, "surroundContents: partial ranges not supported");

    let children_to_wrap: Vec<NodeId> = {
        let t = tree.borrow();
        t.get_node(start_node).children[start_offset..end_offset].to_vec()
    };

    // Remove each child individually, firing separate MO records per the spec
    for &child_id in &children_to_wrap {
        let (prev_sib, next_sib) = {
            let t = tree.borrow();
            let idx = child_index(&t, start_node, child_id);
            let ps = if idx > 0 {
                Some(t.get_node(start_node).children[idx - 1])
            } else {
                None
            };
            let ns = if idx + 1 < t.get_node(start_node).children.len() {
                Some(t.get_node(start_node).children[idx + 1])
            } else {
                None
            };
            (ps, ns)
        };

        tree.borrow_mut().remove_child(start_node, child_id);
        mutation_observer::queue_childlist_mutation(
            ctx,
            &tree,
            start_node,
            vec![],
            vec![child_id],
            prev_sib,
            next_sib,
        );
    }

    // Append extracted children to wrapper
    for &child_id in &children_to_wrap {
        tree.borrow_mut().append_child(wrapper_id, child_id);
    }

    // Insert wrapper at the original position
    let parent_children_len = tree.borrow().get_node(start_node).children.len();
    let prev_sib = if start_offset > 0 && !tree.borrow().get_node(start_node).children.is_empty() {
        let t = tree.borrow();
        let actual_idx = start_offset.min(t.get_node(start_node).children.len());
        if actual_idx > 0 {
            Some(t.get_node(start_node).children[actual_idx - 1])
        } else {
            None
        }
    } else {
        None
    };
    let next_sib = if start_offset < parent_children_len {
        Some(tree.borrow().get_node(start_node).children[start_offset])
    } else {
        None
    };

    // Insert wrapper: if there's a reference child at start_offset, insert before it
    if let Some(ref_child) = next_sib {
        tree.borrow_mut().insert_before(ref_child, wrapper_id);
    } else {
        tree.borrow_mut().append_child(start_node, wrapper_id);
    }

    mutation_observer::queue_childlist_mutation(
        ctx,
        &tree,
        start_node,
        vec![wrapper_id],
        vec![],
        prev_sib,
        next_sib,
    );

    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// Getters: startContainer, startOffset, endContainer, endOffset
// ---------------------------------------------------------------------------

fn range_start_container(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("not a Range").into()))?;
    let range = this_obj
        .downcast_ref::<JsRange>()
        .ok_or_else(|| JsError::from_opaque(js_string!("not a Range").into()))?;
    let node_id = range.start_node.get();
    let tree = range.tree.clone();
    drop(range);
    let js_el = super::element::get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_el.into())
}

fn range_start_offset(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("not a Range").into()))?;
    let range = this_obj
        .downcast_ref::<JsRange>()
        .ok_or_else(|| JsError::from_opaque(js_string!("not a Range").into()))?;
    Ok(JsValue::from(range.start_offset.get() as u32))
}

fn range_end_container(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("not a Range").into()))?;
    let range = this_obj
        .downcast_ref::<JsRange>()
        .ok_or_else(|| JsError::from_opaque(js_string!("not a Range").into()))?;
    let node_id = range.end_node.get();
    let tree = range.tree.clone();
    drop(range);
    let js_el = super::element::get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_el.into())
}

fn range_end_offset(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = _this.as_object().ok_or_else(|| JsError::from_opaque(js_string!("not a Range").into()))?;
    let range = this_obj
        .downcast_ref::<JsRange>()
        .ok_or_else(|| JsError::from_opaque(js_string!("not a Range").into()))?;
    Ok(JsValue::from(range.end_offset.get() as u32))
}

// ---------------------------------------------------------------------------
// Factory: create_range()
// ---------------------------------------------------------------------------

/// Creates a new Range JsObject with boundaries at (document, 0).
pub(crate) fn create_range(
    tree: Rc<RefCell<DomTree>>,
    document_id: NodeId,
    ctx: &mut Context,
) -> JsResult<JsObject> {
    let range_data = JsRange {
        tree,
        start_node: Cell::new(document_id),
        start_offset: Cell::new(0),
        end_node: Cell::new(document_id),
        end_offset: Cell::new(0),
    };

    let obj = ObjectInitializer::with_native_data(range_data, ctx)
        .function(
            NativeFunction::from_fn_ptr(range_set_start),
            js_string!("setStart"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(range_set_end),
            js_string!("setEnd"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(range_set_start_before),
            js_string!("setStartBefore"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(range_set_start_after),
            js_string!("setStartAfter"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(range_set_end_before),
            js_string!("setEndBefore"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(range_set_end_after),
            js_string!("setEndAfter"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(range_delete_contents),
            js_string!("deleteContents"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(range_extract_contents),
            js_string!("extractContents"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(range_insert_node),
            js_string!("insertNode"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(range_surround_contents),
            js_string!("surroundContents"),
            1,
        )
        .property(
            js_string!("startContainer"),
            JsValue::undefined(),
            Attribute::CONFIGURABLE | Attribute::WRITABLE,
        )
        .property(
            js_string!("startOffset"),
            JsValue::from(0),
            Attribute::CONFIGURABLE | Attribute::WRITABLE,
        )
        .property(
            js_string!("endContainer"),
            JsValue::undefined(),
            Attribute::CONFIGURABLE | Attribute::WRITABLE,
        )
        .property(
            js_string!("endOffset"),
            JsValue::from(0),
            Attribute::CONFIGURABLE | Attribute::WRITABLE,
        )
        .build();

    // Define getters for startContainer, startOffset, endContainer, endOffset
    let realm = ctx.realm().clone();

    obj.define_property_or_throw(
        js_string!("startContainer"),
        boa_engine::property::PropertyDescriptor::builder()
            .get(NativeFunction::from_fn_ptr(range_start_container).to_js_function(&realm))
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;
    obj.define_property_or_throw(
        js_string!("startOffset"),
        boa_engine::property::PropertyDescriptor::builder()
            .get(NativeFunction::from_fn_ptr(range_start_offset).to_js_function(&realm))
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;
    obj.define_property_or_throw(
        js_string!("endContainer"),
        boa_engine::property::PropertyDescriptor::builder()
            .get(NativeFunction::from_fn_ptr(range_end_container).to_js_function(&realm))
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;
    obj.define_property_or_throw(
        js_string!("endOffset"),
        boa_engine::property::PropertyDescriptor::builder()
            .get(NativeFunction::from_fn_ptr(range_end_offset).to_js_function(&realm))
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    Ok(obj)
}
