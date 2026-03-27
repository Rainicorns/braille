//! Range prototype method implementations (setStart, setEnd, collapse, etc.).

use boa_engine::{js_string, Context, JsResult, JsValue};

use crate::dom::{NodeData, NodeId};
use crate::js::bindings::mutation_observer;
use super::helpers::{
    get_range, extract_node, child_index, validate_boundary,
    compare_boundary_points_impl, root_of, node_length,
};
use super::contents::{range_to_string_impl, clone_contents_impl, delete_or_extract_contents};
use super::registration::create_range_with_bounds;
use super::types::JsRange;

// ---------------------------------------------------------------------------
// Range prototype methods
// ---------------------------------------------------------------------------

pub(super) fn range_set_start(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let offset = args.get(1).map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as usize;

    // Validate: doctype -> InvalidNodeTypeError, offset > nodeLength -> IndexSizeError
    validate_boundary(node_id, offset, &range.tree.borrow())?;

    range.inner.start_node.set(node_id);
    range.inner.start_offset.set(offset);

    // If start is after end (same root), collapse end to start
    let tree = range.tree.borrow();
    let same_root = root_of(&tree, node_id) == root_of(&tree, range.inner.end_node.get());
    if same_root && compare_boundary_points_impl(&tree, node_id, offset, range.inner.end_node.get(), range.inner.end_offset.get()) > 0 {
        drop(tree);
        range.inner.end_node.set(node_id);
        range.inner.end_offset.set(offset);
    }
    Ok(JsValue::undefined())
}

pub(super) fn range_set_end(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let offset = args.get(1).map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as usize;

    // Validate: doctype -> InvalidNodeTypeError, offset > nodeLength -> IndexSizeError
    validate_boundary(node_id, offset, &range.tree.borrow())?;

    range.inner.end_node.set(node_id);
    range.inner.end_offset.set(offset);

    // If end is before start (same root), collapse start to end
    let tree = range.tree.borrow();
    let same_root = root_of(&tree, node_id) == root_of(&tree, range.inner.start_node.get());
    if same_root && compare_boundary_points_impl(&tree, range.inner.start_node.get(), range.inner.start_offset.get(), node_id, offset) > 0 {
        drop(tree);
        range.inner.start_node.set(node_id);
        range.inner.start_offset.set(offset);
    }
    Ok(JsValue::undefined())
}

pub(super) fn range_set_start_before(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let tree = range.tree.borrow();
    let parent = tree.get_node(node_id).parent.expect("setStartBefore: node has no parent");
    let idx = child_index(&tree, parent, node_id);
    drop(tree);
    range.inner.start_node.set(parent);
    range.inner.start_offset.set(idx);
    Ok(JsValue::undefined())
}

pub(super) fn range_set_start_after(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let tree = range.tree.borrow();
    let parent = tree.get_node(node_id).parent.expect("setStartAfter: node has no parent");
    let idx = child_index(&tree, parent, node_id);
    drop(tree);
    range.inner.start_node.set(parent);
    range.inner.start_offset.set(idx + 1);
    Ok(JsValue::undefined())
}

pub(super) fn range_set_end_before(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let tree = range.tree.borrow();
    let parent = tree.get_node(node_id).parent.expect("setEndBefore: node has no parent");
    let idx = child_index(&tree, parent, node_id);
    drop(tree);
    range.inner.end_node.set(parent);
    range.inner.end_offset.set(idx);
    Ok(JsValue::undefined())
}

pub(super) fn range_set_end_after(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let tree = range.tree.borrow();
    let parent = tree.get_node(node_id).parent.expect("setEndAfter: node has no parent");
    let idx = child_index(&tree, parent, node_id);
    drop(tree);
    range.inner.end_node.set(parent);
    range.inner.end_offset.set(idx + 1);
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// collapsed getter
// ---------------------------------------------------------------------------

pub(super) fn range_collapsed(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let collapsed = range.inner.start_node.get() == range.inner.end_node.get()
        && range.inner.start_offset.get() == range.inner.end_offset.get();
    Ok(JsValue::from(collapsed))
}

// ---------------------------------------------------------------------------
// commonAncestorContainer getter
// ---------------------------------------------------------------------------

pub(super) fn range_common_ancestor_container(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let start = range.inner.start_node.get();
    let end = range.inner.end_node.get();
    let tree = range.tree.clone();
    drop(range);

    let t = tree.borrow();
    let start_ancestors = super::helpers::ancestor_chain(&t, start);
    let end_ancestors = super::helpers::ancestor_chain(&t, end);

    // Find lowest common ancestor: walk start chain, find first that's in end chain
    let mut common = start_ancestors.last().copied().unwrap_or(start);
    for &ancestor in &start_ancestors {
        if end_ancestors.contains(&ancestor) {
            common = ancestor;
            break;
        }
    }
    drop(t);

    let js_el = crate::js::bindings::element::get_or_create_js_element(common, tree, ctx)?;
    Ok(js_el.into())
}

// ---------------------------------------------------------------------------
// detach — no-op per spec
// ---------------------------------------------------------------------------

pub(super) fn range_detach(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// collapse(toStart)
// ---------------------------------------------------------------------------

pub(super) fn range_collapse(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let to_start = args.first().map(|v| v.to_boolean()).unwrap_or(false);
    if to_start {
        range.inner.end_node.set(range.inner.start_node.get());
        range.inner.end_offset.set(range.inner.start_offset.get());
    } else {
        range.inner.start_node.set(range.inner.end_node.get());
        range.inner.start_offset.set(range.inner.end_offset.get());
    }
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// cloneRange
// ---------------------------------------------------------------------------

pub(super) fn range_clone_range(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let tree = range.tree.clone();
    let start_node = range.inner.start_node.get();
    let start_offset = range.inner.start_offset.get();
    let end_node = range.inner.end_node.get();
    let end_offset = range.inner.end_offset.get();
    drop(range);
    let obj = create_range_with_bounds(tree, start_node, start_offset, end_node, end_offset, ctx)?;
    Ok(obj.into())
}

// ---------------------------------------------------------------------------
// selectNode / selectNodeContents
// ---------------------------------------------------------------------------

pub(super) fn range_select_node(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let tree = range.tree.borrow();
    let parent = tree.get_node(node_id).parent.ok_or_else(|| {
        boa_engine::JsError::from_opaque(js_string!("InvalidNodeTypeError").into())
    })?;
    let idx = child_index(&tree, parent, node_id);
    drop(tree);
    range.inner.start_node.set(parent);
    range.inner.start_offset.set(idx);
    range.inner.end_node.set(parent);
    range.inner.end_offset.set(idx + 1);
    Ok(JsValue::undefined())
}

pub(super) fn range_select_node_contents(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let tree = range.tree.borrow();
    // Doctype throws InvalidNodeTypeError
    if matches!(tree.get_node(node_id).data, NodeData::Doctype { .. }) {
        return Err(boa_engine::JsError::from_opaque(js_string!("InvalidNodeTypeError").into()));
    }
    let len = node_length(&tree, node_id);
    drop(tree);
    range.inner.start_node.set(node_id);
    range.inner.start_offset.set(0);
    range.inner.end_node.set(node_id);
    range.inner.end_offset.set(len);
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// toString — text content within range
// ---------------------------------------------------------------------------

pub(super) fn range_to_string(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let start_node = range.inner.start_node.get();
    let start_offset = range.inner.start_offset.get();
    let end_node = range.inner.end_node.get();
    let end_offset = range.inner.end_offset.get();
    let tree = range.tree.clone();
    drop(range);

    let t = tree.borrow();
    let result = range_to_string_impl(&t, start_node, start_offset, end_node, end_offset);
    Ok(JsValue::from(js_string!(result)))
}

// ---------------------------------------------------------------------------
// compareBoundaryPoints(how, sourceRange)
// ---------------------------------------------------------------------------

pub(super) fn range_compare_boundary_points(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);

    let how = args.first().unwrap_or(&JsValue::undefined()).to_number(ctx)? as u16;
    if how > 3 {
        return Err(boa_engine::JsError::from_opaque(js_string!("NotSupportedError").into()));
    }

    let source_obj = args
        .get(1)
        .and_then(|v| v.as_object())
        .ok_or_else(|| boa_engine::JsError::from_opaque(js_string!("compareBoundaryPoints: argument is not a Range").into()))?;
    let source = source_obj
        .downcast_ref::<JsRange>()
        .ok_or_else(|| boa_engine::JsError::from_opaque(js_string!("compareBoundaryPoints: argument is not a Range").into()))?;

    // Check roots are the same
    let tree = range.tree.borrow();
    let this_root = root_of(&tree, range.inner.start_node.get());
    let source_root = root_of(&tree, source.inner.start_node.get());
    if this_root != source_root {
        drop(tree);
        drop(source);
        drop(range);
        return Err(boa_engine::JsError::from_opaque(js_string!("WrongDocumentError").into()));
    }

    let (node_a, offset_a, node_b, offset_b) = match how {
        0 => {
            // START_TO_START: this.start vs source.start
            (range.inner.start_node.get(), range.inner.start_offset.get(), source.inner.start_node.get(), source.inner.start_offset.get())
        }
        1 => {
            // START_TO_END: this.end vs source.start
            (range.inner.end_node.get(), range.inner.end_offset.get(), source.inner.start_node.get(), source.inner.start_offset.get())
        }
        2 => {
            // END_TO_END: this.end vs source.end
            (range.inner.end_node.get(), range.inner.end_offset.get(), source.inner.end_node.get(), source.inner.end_offset.get())
        }
        3 => {
            // END_TO_START: this.start vs source.end
            (range.inner.start_node.get(), range.inner.start_offset.get(), source.inner.end_node.get(), source.inner.end_offset.get())
        }
        _ => unreachable!(),
    };

    let result = compare_boundary_points_impl(&tree, node_a, offset_a, node_b, offset_b);
    Ok(JsValue::from(result as i32))
}

// ---------------------------------------------------------------------------
// comparePoint(node, offset) -> -1, 0, 1
// ---------------------------------------------------------------------------

pub(super) fn range_compare_point(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let offset = args.get(1).map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as usize;

    let tree = range.tree.borrow();

    // Check same root
    let range_root = root_of(&tree, range.inner.start_node.get());
    let node_root = root_of(&tree, node_id);
    if range_root != node_root {
        drop(tree);
        drop(range);
        return Err(boa_engine::JsError::from_opaque(js_string!("WrongDocumentError").into()));
    }

    // If point is before start, return -1
    let vs_start = compare_boundary_points_impl(&tree, node_id, offset, range.inner.start_node.get(), range.inner.start_offset.get());
    if vs_start < 0 {
        return Ok(JsValue::from(-1));
    }

    // If point is after end, return 1
    let vs_end = compare_boundary_points_impl(&tree, node_id, offset, range.inner.end_node.get(), range.inner.end_offset.get());
    if vs_end > 0 {
        return Ok(JsValue::from(1));
    }

    Ok(JsValue::from(0))
}

// ---------------------------------------------------------------------------
// isPointInRange(node, offset) -> boolean
// ---------------------------------------------------------------------------

pub(super) fn range_is_point_in_range(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let offset = args.get(1).map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as usize;

    let tree = range.tree.borrow();

    // Different roots -> false (not an error)
    let range_root = root_of(&tree, range.inner.start_node.get());
    let node_root = root_of(&tree, node_id);
    if range_root != node_root {
        return Ok(JsValue::from(false));
    }

    let vs_start = compare_boundary_points_impl(&tree, node_id, offset, range.inner.start_node.get(), range.inner.start_offset.get());
    let vs_end = compare_boundary_points_impl(&tree, node_id, offset, range.inner.end_node.get(), range.inner.end_offset.get());
    Ok(JsValue::from(vs_start >= 0 && vs_end <= 0))
}

// ---------------------------------------------------------------------------
// intersectsNode(node) -> boolean
// ---------------------------------------------------------------------------

pub(super) fn range_intersects_node(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;

    let tree = range.tree.borrow();

    // Different roots -> false
    let range_root = root_of(&tree, range.inner.start_node.get());
    let node_root = root_of(&tree, node_id);
    if range_root != node_root {
        return Ok(JsValue::from(false));
    }

    let parent = match tree.get_node(node_id).parent {
        Some(p) => p,
        None => return Ok(JsValue::from(true)), // root node always intersects
    };

    let idx = child_index(&tree, parent, node_id);

    // Per spec: node intersects range if:
    // (parent, offset+1) is after range start AND (parent, offset) is before range end
    let after_start = compare_boundary_points_impl(
        &tree,
        parent,
        idx + 1,
        range.inner.start_node.get(),
        range.inner.start_offset.get(),
    ) > 0;
    let before_end = compare_boundary_points_impl(
        &tree,
        parent,
        idx,
        range.inner.end_node.get(),
        range.inner.end_offset.get(),
    ) < 0;

    Ok(JsValue::from(after_start && before_end))
}

// ---------------------------------------------------------------------------
// cloneContents
// ---------------------------------------------------------------------------

pub(super) fn range_clone_contents(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (tree, start_node, start_offset, end_node, end_offset) = {
        get_range!(_this, range);
        (
            range.tree.clone(),
            range.inner.start_node.get(),
            range.inner.start_offset.get(),
            range.inner.end_node.get(),
            range.inner.end_offset.get(),
        )
    };

    let frag_id = clone_contents_impl(ctx, &tree, start_node, start_offset, end_node, end_offset)?;
    let js_frag = crate::js::bindings::element::get_or_create_js_element(frag_id, tree, ctx)?;
    Ok(js_frag.into())
}

// ---------------------------------------------------------------------------
// deleteContents
// ---------------------------------------------------------------------------

pub(super) fn range_delete_contents(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (tree, start_node, start_offset, end_node, end_offset) = {
        get_range!(_this, range);
        (
            range.tree.clone(),
            range.inner.start_node.get(),
            range.inner.start_offset.get(),
            range.inner.end_node.get(),
            range.inner.end_offset.get(),
        )
    };
    delete_or_extract_contents(ctx, &tree, start_node, start_offset, end_node, end_offset, false)?;
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// extractContents
// ---------------------------------------------------------------------------

pub(super) fn range_extract_contents(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (tree, start_node, start_offset, end_node, end_offset) = {
        get_range!(_this, range);
        (
            range.tree.clone(),
            range.inner.start_node.get(),
            range.inner.start_offset.get(),
            range.inner.end_node.get(),
            range.inner.end_offset.get(),
        )
    };
    let frag_id = delete_or_extract_contents(ctx, &tree, start_node, start_offset, end_node, end_offset, true)?;
    let frag_id = frag_id.expect("extractContents must return a fragment");
    let js_frag = crate::js::bindings::element::get_or_create_js_element(frag_id, tree, ctx)?;
    Ok(js_frag.into())
}

// ---------------------------------------------------------------------------
// insertNode
// ---------------------------------------------------------------------------

pub(super) fn range_insert_node(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (tree, start_node, start_offset) = {
        get_range!(_this, range);
        (range.tree.clone(), range.inner.start_node.get(), range.inner.start_offset.get())
    };

    let new_node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let is_text = matches!(tree.borrow().get_node(start_node).data, NodeData::Text { .. });

    if is_text {
        let parent = tree.borrow().get_node(start_node).parent.expect("insertNode: text has no parent");
        let text_content = tree.borrow().character_data_get(start_node).unwrap_or_default();
        let utf16_len = text_content.encode_utf16().count();

        if start_offset > 0 && start_offset < utf16_len {
            let split_id = tree
                .borrow_mut()
                .split_text(start_node, start_offset)
                .map_err(|e| boa_engine::JsError::from_opaque(js_string!(e).into()))?;

            mutation_observer::queue_childlist_mutation(
                ctx, &tree, parent, vec![split_id], vec![], Some(start_node),
                {
                    let t = tree.borrow();
                    let pidx = child_index(&t, parent, split_id);
                    if pidx + 1 < t.get_node(parent).children.len() {
                        Some(t.get_node(parent).children[pidx + 1])
                    } else {
                        None
                    }
                },
            );

            let prev_sib = Some(start_node);
            let next_sib = Some(split_id);
            tree.borrow_mut().insert_before(split_id, new_node_id);
            mutation_observer::queue_childlist_mutation(
                ctx, &tree, parent, vec![new_node_id], vec![], prev_sib, next_sib,
            );
        } else if start_offset == 0 {
            let prev_sib = {
                let t = tree.borrow();
                let idx = child_index(&t, parent, start_node);
                if idx > 0 { Some(t.get_node(parent).children[idx - 1]) } else { None }
            };
            tree.borrow_mut().insert_before(start_node, new_node_id);
            mutation_observer::queue_childlist_mutation(
                ctx, &tree, parent, vec![new_node_id], vec![], prev_sib, Some(start_node),
            );
        } else {
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
                ctx, &tree, parent, vec![new_node_id], vec![], Some(start_node), next_sib,
            );
        }
    } else {
        let children_len = tree.borrow().get_node(start_node).children.len();
        if start_offset >= children_len {
            let prev_sib = if children_len > 0 {
                Some(tree.borrow().get_node(start_node).children[children_len - 1])
            } else {
                None
            };
            tree.borrow_mut().append_child(start_node, new_node_id);
            mutation_observer::queue_childlist_mutation(
                ctx, &tree, start_node, vec![new_node_id], vec![], prev_sib, None,
            );
        } else {
            let ref_child = tree.borrow().get_node(start_node).children[start_offset];
            let prev_sib = if start_offset > 0 {
                Some(tree.borrow().get_node(start_node).children[start_offset - 1])
            } else {
                None
            };
            tree.borrow_mut().insert_before(ref_child, new_node_id);
            mutation_observer::queue_childlist_mutation(
                ctx, &tree, start_node, vec![new_node_id], vec![], prev_sib, Some(ref_child),
            );
        }
    }

    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// surroundContents
// ---------------------------------------------------------------------------

pub(super) fn range_surround_contents(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let (tree, start_node, start_offset, end_node, end_offset) = {
        get_range!(_this, range);
        (
            range.tree.clone(),
            range.inner.start_node.get(),
            range.inner.start_offset.get(),
            range.inner.end_node.get(),
            range.inner.end_offset.get(),
        )
    };

    let wrapper_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;

    assert_eq!(start_node, end_node, "surroundContents: partial ranges not supported");

    let children_to_wrap: Vec<NodeId> = {
        let t = tree.borrow();
        t.get_node(start_node).children[start_offset..end_offset].to_vec()
    };

    for &child_id in &children_to_wrap {
        let (prev_sib, next_sib) = {
            let t = tree.borrow();
            let idx = child_index(&t, start_node, child_id);
            let ps = if idx > 0 { Some(t.get_node(start_node).children[idx - 1]) } else { None };
            let ns = if idx + 1 < t.get_node(start_node).children.len() {
                Some(t.get_node(start_node).children[idx + 1])
            } else {
                None
            };
            (ps, ns)
        };
        tree.borrow_mut().remove_child(start_node, child_id);
        mutation_observer::queue_childlist_mutation(
            ctx, &tree, start_node, vec![], vec![child_id], prev_sib, next_sib,
        );
    }

    for &child_id in &children_to_wrap {
        tree.borrow_mut().append_child(wrapper_id, child_id);
    }

    let parent_children_len = tree.borrow().get_node(start_node).children.len();
    let prev_sib = if start_offset > 0 && !tree.borrow().get_node(start_node).children.is_empty() {
        let t = tree.borrow();
        let actual_idx = start_offset.min(t.get_node(start_node).children.len());
        if actual_idx > 0 { Some(t.get_node(start_node).children[actual_idx - 1]) } else { None }
    } else {
        None
    };
    let next_sib = if start_offset < parent_children_len {
        Some(tree.borrow().get_node(start_node).children[start_offset])
    } else {
        None
    };

    if let Some(ref_child) = next_sib {
        tree.borrow_mut().insert_before(ref_child, wrapper_id);
    } else {
        tree.borrow_mut().append_child(start_node, wrapper_id);
    }

    mutation_observer::queue_childlist_mutation(
        ctx, &tree, start_node, vec![wrapper_id], vec![], prev_sib, next_sib,
    );

    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// Getters: startContainer, startOffset, endContainer, endOffset
// ---------------------------------------------------------------------------

pub(super) fn range_start_container(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = range.inner.start_node.get();
    let tree = range.tree.clone();
    drop(range);
    let js_el = crate::js::bindings::element::get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_el.into())
}

pub(super) fn range_start_offset(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    Ok(JsValue::from(range.inner.start_offset.get() as u32))
}

pub(super) fn range_end_container(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = range.inner.end_node.get();
    let tree = range.tree.clone();
    drop(range);
    let js_el = crate::js::bindings::element::get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_el.into())
}

pub(super) fn range_end_offset(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    Ok(JsValue::from(range.inner.end_offset.get() as u32))
}
