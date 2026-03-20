//! Range API — full implementation for WPT dom/ranges tests.
//!
//! Implements: new Range(), createRange, setStart/End, setStart/EndBefore/After,
//! collapsed, commonAncestorContainer, detach, collapse, cloneRange,
//! selectNode, selectNodeContents, toString, compareBoundaryPoints,
//! comparePoint, isPointInRange, intersectsNode, cloneContents,
//! deleteContents, extractContents, insertNode, surroundContents.

use std::cell::Cell;
use std::rc::Rc;
use std::cell::RefCell;

use boa_engine::{
    js_string, native_function::NativeFunction,
    Context, JsError, JsObject, JsResult, JsValue,
};

use crate::dom::{DomTree, NodeId, NodeData};
use crate::js::prop_desc;
use super::element::JsElement;
use super::mutation_observer;

// ---------------------------------------------------------------------------
// RangeInner — shared boundary state for live range tracking
// ---------------------------------------------------------------------------

/// Shared interior state for a Range, referenced by both the JsRange (on the
/// JS object) and the live-range registry in RealmState. Using `Rc` + `Cell`
/// lets mutation hooks update boundaries without holding a borrow on the JS
/// object.
#[derive(Debug)]
pub(crate) struct RangeInner {
    pub(crate) start_node: Cell<NodeId>,
    pub(crate) start_offset: Cell<usize>,
    pub(crate) end_node: Cell<NodeId>,
    pub(crate) end_offset: Cell<usize>,
}

// ---------------------------------------------------------------------------
// JsRange — native data stored on the Range JsObject
// ---------------------------------------------------------------------------

#[derive(Debug, boa_engine::JsData, boa_gc::Trace, boa_gc::Finalize)]
pub(crate) struct JsRange {
    #[unsafe_ignore_trace]
    tree: Rc<RefCell<DomTree>>,
    #[unsafe_ignore_trace]
    inner: Rc<RangeInner>,
}

// ---------------------------------------------------------------------------
// Helpers
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

fn child_index(tree: &DomTree, parent: NodeId, child: NodeId) -> usize {
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

/// Validate a boundary point: doctype → InvalidNodeTypeError, offset > nodeLength → IndexSizeError
fn validate_boundary(node_id: NodeId, offset: usize, tree: &DomTree) -> JsResult<()> {
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
/// - DocumentType → 0
/// - CharacterData (Text/Comment/PI/CDATA) → UTF-16 code unit count
/// - Everything else → number of children
fn node_length(tree: &DomTree, node_id: NodeId) -> usize {
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
fn ancestor_chain(tree: &DomTree, node_id: NodeId) -> Vec<NodeId> {
    let mut chain = vec![node_id];
    let mut current = node_id;
    while let Some(parent) = tree.get_node(current).parent {
        chain.push(parent);
        current = parent;
    }
    chain
}

/// Compare two boundary points per DOM spec §4.2.
/// Returns -1, 0, or 1.
fn compare_boundary_points_impl(
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
    // FOLLOWING (0x04): node_b follows node_a → A is before B in tree order
    // PRECEDING (0x02): node_b precedes node_a → A is after B in tree order

    // Step 2: "If node A is after node B in tree order" → PRECEDING flag
    if pos & PRECEDING != 0 {
        let result = compare_boundary_points_impl(tree, node_b, offset_b, node_a, offset_a);
        return -result;
    }

    // Step 3: "If node A is an ancestor of node B"
    // CONTAINED_BY (0x10): node_b is contained by node_a → A is ancestor of B
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

// ---------------------------------------------------------------------------
// Range prototype methods
// ---------------------------------------------------------------------------

fn range_set_start(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let offset = args.get(1).map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as usize;

    // Validate: doctype → InvalidNodeTypeError, offset > nodeLength → IndexSizeError
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

fn range_set_end(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let offset = args.get(1).map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as usize;

    // Validate: doctype → InvalidNodeTypeError, offset > nodeLength → IndexSizeError
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

fn range_set_start_before(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
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

fn range_set_start_after(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
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

fn range_set_end_before(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
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

fn range_set_end_after(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
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

fn range_collapsed(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let collapsed = range.inner.start_node.get() == range.inner.end_node.get()
        && range.inner.start_offset.get() == range.inner.end_offset.get();
    Ok(JsValue::from(collapsed))
}

// ---------------------------------------------------------------------------
// commonAncestorContainer getter
// ---------------------------------------------------------------------------

fn range_common_ancestor_container(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let start = range.inner.start_node.get();
    let end = range.inner.end_node.get();
    let tree = range.tree.clone();
    drop(range);

    let t = tree.borrow();
    let start_ancestors = ancestor_chain(&t, start);
    let end_ancestors = ancestor_chain(&t, end);

    // Find lowest common ancestor: walk start chain, find first that's in end chain
    let mut common = start_ancestors.last().copied().unwrap_or(start);
    for &ancestor in &start_ancestors {
        if end_ancestors.contains(&ancestor) {
            common = ancestor;
            break;
        }
    }
    drop(t);

    let js_el = super::element::get_or_create_js_element(common, tree, ctx)?;
    Ok(js_el.into())
}

// ---------------------------------------------------------------------------
// detach — no-op per spec
// ---------------------------------------------------------------------------

fn range_detach(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// collapse(toStart)
// ---------------------------------------------------------------------------

fn range_collapse(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
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

fn range_clone_range(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

fn range_select_node(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let tree = range.tree.borrow();
    let parent = tree.get_node(node_id).parent.ok_or_else(|| {
        JsError::from_opaque(js_string!("InvalidNodeTypeError").into())
    })?;
    let idx = child_index(&tree, parent, node_id);
    drop(tree);
    range.inner.start_node.set(parent);
    range.inner.start_offset.set(idx);
    range.inner.end_node.set(parent);
    range.inner.end_offset.set(idx + 1);
    Ok(JsValue::undefined())
}

fn range_select_node_contents(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let tree = range.tree.borrow();
    // Doctype throws InvalidNodeTypeError
    if matches!(tree.get_node(node_id).data, NodeData::Doctype { .. }) {
        return Err(JsError::from_opaque(js_string!("InvalidNodeTypeError").into()));
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

fn range_to_string(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
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

/// Collect text content within range boundaries per spec.
/// Only Text nodes contribute to the result.
fn range_to_string_impl(
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
            0 // text node starts at or after range start → take from beginning
        } else {
            start_offset // text node contains the start boundary (shouldn't happen for non-start container)
        };

        let char_end = if text_id == end_node {
            end_offset
        } else if compare_boundary_points_impl(tree, text_id, text_len, end_node, end_offset) <= 0 {
            text_len // text node ends at or before range end → take to the end
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

fn collect_text_nodes_in_order(tree: &DomTree, node_id: NodeId, result: &mut Vec<NodeId>) {
    if matches!(tree.get_node(node_id).data, NodeData::Text { .. }) {
        result.push(node_id);
    }
    for &child in &tree.get_node(node_id).children {
        collect_text_nodes_in_order(tree, child, result);
    }
}

fn is_character_data(tree: &DomTree, node_id: NodeId) -> bool {
    matches!(
        tree.get_node(node_id).data,
        NodeData::Text { .. } | NodeData::Comment { .. } | NodeData::ProcessingInstruction { .. } | NodeData::CDATASection { .. }
    )
}

fn root_of(tree: &DomTree, node_id: NodeId) -> NodeId {
    let mut current = node_id;
    while let Some(parent) = tree.get_node(current).parent {
        current = parent;
    }
    current
}

// ---------------------------------------------------------------------------
// compareBoundaryPoints(how, sourceRange)
// ---------------------------------------------------------------------------

fn range_compare_boundary_points(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);

    let how = args.first().unwrap_or(&JsValue::undefined()).to_number(ctx)? as u16;
    if how > 3 {
        return Err(JsError::from_opaque(js_string!("NotSupportedError").into()));
    }

    let source_obj = args
        .get(1)
        .and_then(|v| v.as_object())
        .ok_or_else(|| JsError::from_opaque(js_string!("compareBoundaryPoints: argument is not a Range").into()))?;
    let source = source_obj
        .downcast_ref::<JsRange>()
        .ok_or_else(|| JsError::from_opaque(js_string!("compareBoundaryPoints: argument is not a Range").into()))?;

    // Check roots are the same
    let tree = range.tree.borrow();
    let this_root = root_of(&tree, range.inner.start_node.get());
    let source_root = root_of(&tree, source.inner.start_node.get());
    if this_root != source_root {
        drop(tree);
        drop(source);
        drop(range);
        return Err(JsError::from_opaque(js_string!("WrongDocumentError").into()));
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
// comparePoint(node, offset) → -1, 0, 1
// ---------------------------------------------------------------------------

fn range_compare_point(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
        return Err(JsError::from_opaque(js_string!("WrongDocumentError").into()));
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
// isPointInRange(node, offset) → boolean
// ---------------------------------------------------------------------------

fn range_is_point_in_range(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;
    let offset = args.get(1).map(|v| v.to_number(ctx)).transpose()?.unwrap_or(0.0) as usize;

    let tree = range.tree.borrow();

    // Different roots → false (not an error)
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
// intersectsNode(node) → boolean
// ---------------------------------------------------------------------------

fn range_intersects_node(_this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = extract_node(args.first().unwrap_or(&JsValue::undefined()))?;

    let tree = range.tree.borrow();

    // Different roots → false
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

fn range_clone_contents(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
    let js_frag = super::element::get_or_create_js_element(frag_id, tree, ctx)?;
    Ok(js_frag.into())
}

fn clone_contents_impl(
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
// deleteContents
// ---------------------------------------------------------------------------

fn range_delete_contents(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

fn range_extract_contents(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
    let js_frag = super::element::get_or_create_js_element(frag_id, tree, ctx)?;
    Ok(js_frag.into())
}

// ---------------------------------------------------------------------------
// Shared delete/extract implementation
// ---------------------------------------------------------------------------

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
                .map_err(|e| JsError::from_opaque(js_string!(e).into()))?;
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

// ---------------------------------------------------------------------------
// insertNode
// ---------------------------------------------------------------------------

fn range_insert_node(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
                .map_err(|e| JsError::from_opaque(js_string!(e).into()))?;

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

fn range_surround_contents(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

fn range_start_container(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = range.inner.start_node.get();
    let tree = range.tree.clone();
    drop(range);
    let js_el = super::element::get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_el.into())
}

fn range_start_offset(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    Ok(JsValue::from(range.inner.start_offset.get() as u32))
}

fn range_end_container(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    let node_id = range.inner.end_node.get();
    let tree = range.tree.clone();
    drop(range);
    let js_el = super::element::get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_el.into())
}

fn range_end_offset(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    get_range!(_this, range);
    Ok(JsValue::from(range.inner.end_offset.get() as u32))
}

// ---------------------------------------------------------------------------
// Range.prototype — shared across all Range instances
// ---------------------------------------------------------------------------

pub(crate) fn create_range_prototype(ctx: &mut Context) -> JsObject {
    let realm = ctx.realm().clone();
    let proto = JsObject::with_null_proto();

    use prop_desc::{data_prop, readonly_accessor, readonly_constant};

    let method = |f: fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>, _name: &str, _len: usize| {
        NativeFunction::from_fn_ptr(f).to_js_function(&realm)
    };

    // Methods
    type MethodEntry = (&'static str, fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>, usize);
    let methods: &[MethodEntry] = &[
        ("setStart", range_set_start, 2),
        ("setEnd", range_set_end, 2),
        ("setStartBefore", range_set_start_before, 1),
        ("setStartAfter", range_set_start_after, 1),
        ("setEndBefore", range_set_end_before, 1),
        ("setEndAfter", range_set_end_after, 1),
        ("collapse", range_collapse, 0),
        ("cloneRange", range_clone_range, 0),
        ("selectNode", range_select_node, 1),
        ("selectNodeContents", range_select_node_contents, 1),
        ("deleteContents", range_delete_contents, 0),
        ("extractContents", range_extract_contents, 0),
        ("cloneContents", range_clone_contents, 0),
        ("insertNode", range_insert_node, 1),
        ("surroundContents", range_surround_contents, 1),
        ("compareBoundaryPoints", range_compare_boundary_points, 2),
        ("comparePoint", range_compare_point, 2),
        ("isPointInRange", range_is_point_in_range, 2),
        ("intersectsNode", range_intersects_node, 1),
        ("detach", range_detach, 0),
        ("toString", range_to_string, 0),
    ];

    for &(name, f, len) in methods {
        proto
            .define_property_or_throw(js_string!(name), data_prop(method(f, name, len)), ctx)
            .expect("define Range method");
    }

    // Readonly accessor properties
    type AccessorEntry = (&'static str, fn(&JsValue, &[JsValue], &mut Context) -> JsResult<JsValue>);
    let accessors: &[AccessorEntry] = &[
        ("startContainer", range_start_container),
        ("startOffset", range_start_offset),
        ("endContainer", range_end_container),
        ("endOffset", range_end_offset),
        ("collapsed", range_collapsed),
        ("commonAncestorContainer", range_common_ancestor_container),
    ];

    for &(name, f) in accessors {
        let getter = NativeFunction::from_fn_ptr(f).to_js_function(&realm);
        proto
            .define_property_or_throw(js_string!(name), readonly_accessor(getter), ctx)
            .expect("define Range accessor");
    }

    // Constants on prototype
    proto.define_property_or_throw(js_string!("START_TO_START"), readonly_constant(0), ctx).expect("const");
    proto.define_property_or_throw(js_string!("START_TO_END"), readonly_constant(1), ctx).expect("const");
    proto.define_property_or_throw(js_string!("END_TO_END"), readonly_constant(2), ctx).expect("const");
    proto.define_property_or_throw(js_string!("END_TO_START"), readonly_constant(3), ctx).expect("const");

    proto
}

// ---------------------------------------------------------------------------
// Register Range global constructor
// ---------------------------------------------------------------------------

pub(crate) fn register_range_global(ctx: &mut Context) {
    use boa_engine::object::FunctionObjectBuilder;
    use boa_engine::property::Attribute;

    let tree = crate::js::realm_state::dom_tree(ctx);
    let proto = create_range_prototype(ctx);

    // Store prototype in RealmState
    crate::js::realm_state::set_range_proto(ctx, proto.clone());

    // Range constructor: new Range() creates range at (document, 0)
    let tree_for_ctor = tree.clone();
    let proto_for_ctor = proto.clone();
    let ctor = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let doc_id = tree_for_ctor.borrow().document();
            let obj = create_range_with_bounds(tree_for_ctor.clone(), doc_id, 0, doc_id, 0, ctx2)?;
            obj.set_prototype(Some(proto_for_ctor.clone()));
            Ok(obj.into())
        })
    };

    let ctor_fn = FunctionObjectBuilder::new(ctx.realm(), ctor)
        .name(js_string!("Range"))
        .length(0)
        .constructor(true)
        .build();

    // Set Range.prototype
    ctor_fn
        .define_property_or_throw(js_string!("prototype"), prop_desc::prototype_on_ctor(proto.clone()), ctx)
        .expect("Range.prototype");

    // Constants on constructor
    ctor_fn.define_property_or_throw(js_string!("START_TO_START"), prop_desc::readonly_constant(0), ctx).expect("const");
    ctor_fn.define_property_or_throw(js_string!("START_TO_END"), prop_desc::readonly_constant(1), ctx).expect("const");
    ctor_fn.define_property_or_throw(js_string!("END_TO_END"), prop_desc::readonly_constant(2), ctx).expect("const");
    ctor_fn.define_property_or_throw(js_string!("END_TO_START"), prop_desc::readonly_constant(3), ctx).expect("const");

    // Set constructor on prototype
    proto
        .define_property_or_throw(js_string!("constructor"), prop_desc::constructor_on_proto(ctor_fn.clone()), ctx)
        .expect("proto.constructor");

    // Register as global
    ctx.register_global_property(js_string!("Range"), ctor_fn, Attribute::WRITABLE | Attribute::CONFIGURABLE)
        .expect("register Range global");
}

// ---------------------------------------------------------------------------
// Factory: create_range() — used by document.createRange()
// ---------------------------------------------------------------------------

pub(crate) fn create_range(
    tree: Rc<RefCell<DomTree>>,
    document_id: NodeId,
    ctx: &mut Context,
) -> JsResult<JsObject> {
    let obj = create_range_with_bounds(tree, document_id, 0, document_id, 0, ctx)?;

    // Set prototype if available
    if let Some(proto) = crate::js::realm_state::range_proto(ctx) {
        obj.set_prototype(Some(proto));
    }

    Ok(obj)
}

// ---------------------------------------------------------------------------
// Live range mutation hooks — called from mutation.rs and mutation_observer.rs
// ---------------------------------------------------------------------------

/// Helper: check if `descendant` is an inclusive descendant of `ancestor` in the given tree.
fn is_inclusive_descendant(tree: &DomTree, descendant: NodeId, ancestor: NodeId) -> bool {
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

/// Called after a child node is inserted into `parent_id` at child index `new_index`.
/// Per spec §4.2: "For each live range whose start node is parent and start offset
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
/// Per spec §4.2 removing steps: for each boundary, if node is removed_node
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

/// Called after character data replacement per spec §4.2 "replace data" steps.
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

/// Called after `Text.splitText(offset)` per spec §4.2 "split a Text node" steps.
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

// ---------------------------------------------------------------------------
// Factory: create_range_with_bounds
// ---------------------------------------------------------------------------

/// Creates a Range JsObject with specified boundaries.
/// Registers the range's inner state in the live-range registry so that
/// DOM mutations can update boundaries automatically.
fn create_range_with_bounds(
    tree: Rc<RefCell<DomTree>>,
    start_node: NodeId,
    start_offset: usize,
    end_node: NodeId,
    end_offset: usize,
    ctx: &mut Context,
) -> JsResult<JsObject> {
    let inner = Rc::new(RangeInner {
        start_node: Cell::new(start_node),
        start_offset: Cell::new(start_offset),
        end_node: Cell::new(end_node),
        end_offset: Cell::new(end_offset),
    });

    // Register in live-range registry (weak ref so JS GC can collect)
    let registry = crate::js::realm_state::live_ranges(ctx);
    let mut reg = registry.borrow_mut();
    // Periodic cleanup: drop dead weak refs
    if reg.len() > 64 {
        reg.retain(|w| w.strong_count() > 0);
    }
    reg.push(Rc::downgrade(&inner));
    drop(reg);

    let range_data = JsRange {
        tree,
        inner,
    };

    let obj = boa_engine::object::ObjectInitializer::with_native_data(range_data, ctx).build();

    // Set prototype if available
    if let Some(proto) = crate::js::realm_state::range_proto(ctx) {
        obj.set_prototype(Some(proto));
    }

    Ok(obj)
}

