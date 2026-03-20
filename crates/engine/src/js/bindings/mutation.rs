use boa_engine::{
    class::ClassBuilder, js_string, native_function::NativeFunction, Context, JsError, JsNativeError, JsResult, JsValue,
};

use boa_engine::JsObject;

use super::element::{get_or_create_js_element, JsElement};
use crate::dom::{DomTree, NodeData, NodeId};
use crate::js::realm_state;
use std::cell::RefCell;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

fn hierarchy_request_error(msg: &str) -> JsError {
    JsNativeError::typ()
        .with_message(format!("HierarchyRequestError: {}", msg))
        .into()
}

fn not_found_error(msg: &str) -> JsError {
    JsNativeError::typ()
        .with_message(format!("NotFoundError: {}", msg))
        .into()
}

// ---------------------------------------------------------------------------
// Cache update after cross-tree adoption
// ---------------------------------------------------------------------------

/// Update the NODE_CACHE after a cross-tree adoption:
/// - Remove the old entry (src_tree_ptr, src_node_id)
/// - Add a new entry (dst_tree_ptr, adopted_id) -> js_obj
pub(crate) fn update_node_cache_after_adoption(
    src_tree: &Rc<RefCell<DomTree>>,
    src_node_id: NodeId,
    dst_tree: &Rc<RefCell<DomTree>>,
    adopted_id: NodeId,
    js_obj: &JsObject,
    ctx: &boa_engine::Context,
) {
    let src_ptr = Rc::as_ptr(src_tree) as usize;
    let dst_ptr = Rc::as_ptr(dst_tree) as usize;

    let cache = realm_state::node_cache(ctx);
    let mut cache = cache.borrow_mut();
    cache.remove(&(src_ptr, src_node_id));
    cache.insert((dst_ptr, adopted_id), js_obj.clone());
}

/// Update the NODE_CACHE for all entries in a (src_id, dst_id) mapping.
/// For each pair, find the cached JsObject for (src_tree_ptr, src_id),
/// update its internal JsElement data to (dst_id, dst_tree), and re-cache
/// under (dst_tree_ptr, dst_id).
pub(crate) fn update_node_cache_for_adoption_mapping(
    src_tree: &Rc<RefCell<DomTree>>,
    dst_tree: &Rc<RefCell<DomTree>>,
    mapping: &[(NodeId, NodeId)],
    ctx: &boa_engine::Context,
) {
    let src_ptr = Rc::as_ptr(src_tree) as usize;
    let dst_ptr = Rc::as_ptr(dst_tree) as usize;

    let cache = realm_state::node_cache(ctx);
    let mut cache = cache.borrow_mut();

    for &(src_id, dst_id) in mapping {
        if let Some(js_obj) = cache.remove(&(src_ptr, src_id)) {
            // Update the JsElement inside the JsObject to point to the new tree/node
            if let Some(mut el_mut) = js_obj.downcast_mut::<JsElement>() {
                el_mut.node_id = dst_id;
                el_mut.tree = dst_tree.clone();
            }
            cache.insert((dst_ptr, dst_id), js_obj);
        }
    }
}

// ---------------------------------------------------------------------------
// Cross-tree node adoption
// ---------------------------------------------------------------------------

/// Adopt a node from one DomTree into another by creating a clone in the
/// destination tree and removing the original from its source parent.
/// Returns the new NodeId in the destination tree.
pub(crate) fn adopt_node(src_tree: &Rc<RefCell<DomTree>>, src_id: NodeId, dst_tree: &Rc<RefCell<DomTree>>) -> NodeId {
    let mut mapping = Vec::new();
    let new_id = adopt_node_recursive(src_tree, src_id, dst_tree, &mut mapping);

    // Remove the original node from its parent in the source tree
    src_tree.borrow_mut().remove_from_parent(src_id);

    new_id
}

/// Adopt a node cross-tree and collect a mapping of (src_id, dst_id) pairs
/// for the node and all its descendants. The caller is responsible for
/// updating cached JS objects using this mapping.
pub(crate) fn adopt_node_with_mapping(
    src_tree: &Rc<RefCell<DomTree>>,
    src_id: NodeId,
    dst_tree: &Rc<RefCell<DomTree>>,
) -> (NodeId, Vec<(NodeId, NodeId)>) {
    let mut mapping = Vec::new();
    let new_id = adopt_node_recursive(src_tree, src_id, dst_tree, &mut mapping);

    // Remove the original node from its parent in the source tree
    src_tree.borrow_mut().remove_from_parent(src_id);

    (new_id, mapping)
}

fn adopt_node_recursive(
    src_tree: &Rc<RefCell<DomTree>>,
    src_id: NodeId,
    dst_tree: &Rc<RefCell<DomTree>>,
    mapping: &mut Vec<(NodeId, NodeId)>,
) -> NodeId {
    let src = src_tree.borrow();
    let node = src.get_node(src_id);

    let new_id = match &node.data {
        NodeData::Text { content } => {
            let t = content.clone();
            drop(src);
            dst_tree.borrow_mut().create_text(&t)
        }
        NodeData::Comment { content } => {
            let t = content.clone();
            drop(src);
            dst_tree.borrow_mut().create_comment(&t)
        }
        NodeData::Element {
            tag_name,
            attributes,
            namespace,
            ..
        } => {
            let tag = tag_name.clone();
            let attrs = attributes.clone();
            let ns = namespace.clone();
            let child_ids: Vec<NodeId> = node.children.clone();
            drop(src);
            let id = dst_tree.borrow_mut().create_element_ns(&tag, attrs, &ns);
            for child_id in child_ids {
                let adopted_child = adopt_node_recursive(src_tree, child_id, dst_tree, mapping);
                dst_tree.borrow_mut().append_child(id, adopted_child);
            }
            id
        }
        NodeData::Doctype {
            name,
            public_id,
            system_id,
        } => {
            let n = name.clone();
            let p = public_id.clone();
            let s = system_id.clone();
            drop(src);
            dst_tree.borrow_mut().create_doctype(&n, &p, &s)
        }
        NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
            let child_ids: Vec<NodeId> = node.children.clone();
            drop(src);
            let id = dst_tree.borrow_mut().create_document_fragment();
            for child_id in child_ids {
                let adopted_child = adopt_node_recursive(src_tree, child_id, dst_tree, mapping);
                dst_tree.borrow_mut().append_child(id, adopted_child);
            }
            id
        }
        NodeData::ProcessingInstruction { target, data } => {
            let t = target.clone();
            let d = data.clone();
            drop(src);
            dst_tree.borrow_mut().create_processing_instruction(&t, &d)
        }
        NodeData::Attr {
            local_name,
            namespace,
            prefix,
            value,
        } => {
            let ln = local_name.clone();
            let ns = namespace.clone();
            let pfx = prefix.clone();
            let val = value.clone();
            drop(src);
            dst_tree.borrow_mut().create_attr(&ln, &ns, &pfx, &val)
        }
        NodeData::CDATASection { content } => {
            let t = content.clone();
            drop(src);
            dst_tree.borrow_mut().create_cdata_section(&t)
        }
        NodeData::Document => {
            drop(src);
            dst_tree.borrow_mut().create_document_fragment()
        }
    };

    mapping.push((src_id, new_id));
    new_id
}

// ---------------------------------------------------------------------------
// Pre-insertion validation (spec: https://dom.spec.whatwg.org/#concept-node-ensure-pre-insertion-validity)
// ---------------------------------------------------------------------------

/// Checks if `ancestor_id` is an inclusive ancestor of `node_id`.
fn is_inclusive_ancestor(tree: &DomTree, ancestor_id: NodeId, node_id: NodeId) -> bool {
    let mut current = node_id;
    loop {
        if current == ancestor_id {
            return true;
        }
        match tree.get_parent(current) {
            Some(parent) => current = parent,
            None => return false,
        }
    }
}

/// Pre-insertion validation for insertBefore/appendChild.
/// `child_ref` is the reference child (None means append).
/// `node_tree` is the tree for the node when it differs from the parent's tree (cross-tree insert).
pub(crate) fn validate_pre_insert(
    tree: &DomTree,
    parent_id: NodeId,
    node_id: NodeId,
    child_ref: Option<NodeId>,
    node_tree: Option<&DomTree>,
) -> JsResult<()> {
    let parent_data = &tree.get_node(parent_id).data;

    // Step 1: parent must be Document, DocumentFragment, or Element
    match parent_data {
        NodeData::Document | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } | NodeData::Element { .. } => {}
        _ => {
            return Err(hierarchy_request_error(
                "parent is not a Document, DocumentFragment, or Element",
            ));
        }
    }

    // Step 2: node must not be an inclusive ancestor of parent
    // If node is from a different tree, it can't be an ancestor of parent
    if node_tree.is_none() && is_inclusive_ancestor(tree, node_id, parent_id) {
        return Err(hierarchy_request_error("The new child is an ancestor of the parent"));
    }

    // Step 3: if child is not null, its parent must be parent
    // Note: ref_id must be from the same tree as parent (checked by caller)
    if let Some(ref_id) = child_ref {
        let ref_parent = tree.get_node(ref_id).parent;
        if ref_parent != Some(parent_id) {
            return Err(not_found_error(
                "The node before which the new node is to be inserted is not a child of this node",
            ));
        }
    }

    // Use the appropriate tree for the node
    let nt = node_tree.unwrap_or(tree);
    let node_data = &nt.get_node(node_id).data;

    // Step 4: node must be DocumentFragment, DocumentType, Element, Text, Comment, PI, CDATASection, or Attr
    match node_data {
        NodeData::DocumentFragment
        | NodeData::ShadowRoot { .. }
        | NodeData::Doctype { .. }
        | NodeData::Element { .. }
        | NodeData::Text { .. }
        | NodeData::Comment { .. }
        | NodeData::ProcessingInstruction { .. }
        | NodeData::CDATASection { .. } => {}
        NodeData::Attr { .. } => {
            return Err(hierarchy_request_error("Cannot insert an Attr node"));
        }
        NodeData::Document => {
            return Err(hierarchy_request_error("Cannot insert a Document node"));
        }
    }

    // Step 5: If node is Text and parent is Document, throw
    if matches!(node_data, NodeData::Text { .. }) && matches!(parent_data, NodeData::Document) {
        return Err(hierarchy_request_error("Cannot insert Text as a child of Document"));
    }

    // Step 5 (continued): If node is Doctype and parent is not Document, throw
    if matches!(node_data, NodeData::Doctype { .. }) && !matches!(parent_data, NodeData::Document) {
        return Err(hierarchy_request_error(
            "Cannot insert Doctype as a child of a non-Document node",
        ));
    }

    // Step 6: If parent is Document, additional constraints
    // For cross-tree nodes, some Document constraints use node_tree
    if matches!(parent_data, NodeData::Document) {
        validate_document_insert(tree, parent_id, node_id, child_ref, node_tree)?;
    }

    Ok(())
}

/// Additional validation when inserting into a Document node (step 6 of pre-insert).
/// `node_tree` is the tree for the node when it differs from the parent's tree.
fn validate_document_insert(
    tree: &DomTree,
    parent_id: NodeId,
    node_id: NodeId,
    child_ref: Option<NodeId>,
    node_tree: Option<&DomTree>,
) -> JsResult<()> {
    let nt = node_tree.unwrap_or(tree);
    let node_data = &nt.get_node(node_id).data;

    match node_data {
        NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => {
            // Count element children in the fragment
            let frag_children = &nt.get_node(node_id).children;
            let elem_count = frag_children
                .iter()
                .filter(|&&c| matches!(nt.get_node(c).data, NodeData::Element { .. }))
                .count();
            let has_text = frag_children
                .iter()
                .any(|&c| matches!(nt.get_node(c).data, NodeData::Text { .. }));

            // Fragment cannot have text children when inserting into Document
            if has_text {
                return Err(hierarchy_request_error(
                    "Cannot insert DocumentFragment containing Text into Document",
                ));
            }

            // Fragment cannot have more than one element child
            if elem_count > 1 {
                return Err(hierarchy_request_error(
                    "Cannot insert DocumentFragment with multiple elements into Document",
                ));
            }

            if elem_count == 1 {
                // If parent already has an element child (that isn't being replaced), throw
                let parent_children = &tree.get_node(parent_id).children;
                let has_existing_element = parent_children
                    .iter()
                    .any(|&c| matches!(tree.get_node(c).data, NodeData::Element { .. }));
                if has_existing_element {
                    return Err(hierarchy_request_error("Document already has an element child"));
                }

                // If child_ref is a doctype, or there's a doctype following child_ref, throw
                if let Some(ref_id) = child_ref {
                    if matches!(tree.get_node(ref_id).data, NodeData::Doctype { .. }) {
                        return Err(hierarchy_request_error("Cannot insert element before doctype"));
                    }
                    // Check if there's a doctype FOLLOWING the reference child
                    if has_doctype_after(tree, parent_id, ref_id) {
                        return Err(hierarchy_request_error("Cannot insert element before a doctype"));
                    }
                }
            }
        }
        NodeData::Element { .. } => {
            // If parent already has an element child, throw
            let parent_children = &tree.get_node(parent_id).children;
            let has_existing_element = parent_children
                .iter()
                .any(|&c| matches!(tree.get_node(c).data, NodeData::Element { .. }));
            if has_existing_element {
                return Err(hierarchy_request_error("Document already has an element child"));
            }

            // If child_ref is a doctype, or there's a doctype following child_ref, throw
            if let Some(ref_id) = child_ref {
                if matches!(tree.get_node(ref_id).data, NodeData::Doctype { .. }) {
                    return Err(hierarchy_request_error("Cannot insert element before doctype"));
                }
                if has_doctype_after(tree, parent_id, ref_id) {
                    return Err(hierarchy_request_error("Cannot insert element before a doctype"));
                }
            }
        }
        NodeData::Doctype { .. } => {
            // If parent already has a doctype child, throw
            let parent_children = &tree.get_node(parent_id).children;
            let has_existing_doctype = parent_children
                .iter()
                .any(|&c| matches!(tree.get_node(c).data, NodeData::Doctype { .. }));
            if has_existing_doctype {
                return Err(hierarchy_request_error("Document already has a doctype child"));
            }

            // If child_ref is non-null and there's an element before child_ref, throw
            // If child_ref is null (appending), and there's already an element child, throw
            if let Some(ref_id) = child_ref {
                if has_element_before(tree, parent_id, ref_id) {
                    return Err(hierarchy_request_error("Cannot insert doctype after an element"));
                }
            } else {
                // Appending: if there's already an element child, throw
                let has_element = parent_children
                    .iter()
                    .any(|&c| matches!(tree.get_node(c).data, NodeData::Element { .. }));
                if has_element {
                    return Err(hierarchy_request_error("Cannot insert doctype after an element"));
                }
            }
        }
        _ => {}
    }

    Ok(())
}

/// Pre-replace validation for replaceChild (spec: https://dom.spec.whatwg.org/#concept-node-replace)
/// `node_tree` is the tree for the node when it differs from the parent's tree.
fn validate_pre_replace(
    tree: &DomTree,
    parent_id: NodeId,
    node_id: NodeId,
    old_child_id: NodeId,
    node_tree: Option<&DomTree>,
) -> JsResult<()> {
    let parent_data = &tree.get_node(parent_id).data;

    // Step 1: parent must be Document, DocumentFragment, or Element
    match parent_data {
        NodeData::Document | NodeData::DocumentFragment | NodeData::ShadowRoot { .. } | NodeData::Element { .. } => {}
        _ => {
            return Err(hierarchy_request_error(
                "parent is not a Document, DocumentFragment, or Element",
            ));
        }
    }

    // Step 2: node must not be an inclusive ancestor of parent
    // If node is from a different tree, it can't be an ancestor of parent
    if node_tree.is_none() && is_inclusive_ancestor(tree, node_id, parent_id) {
        return Err(hierarchy_request_error("The new child is an ancestor of the parent"));
    }

    // Step 3: old child's parent must be parent
    let old_child_parent = tree.get_node(old_child_id).parent;
    if old_child_parent != Some(parent_id) {
        return Err(not_found_error("The node to be replaced is not a child of this node"));
    }

    let nt = node_tree.unwrap_or(tree);
    let node_data = &nt.get_node(node_id).data;

    // Step 4/5: node must be valid insertion type
    match node_data {
        NodeData::DocumentFragment
        | NodeData::ShadowRoot { .. }
        | NodeData::Doctype { .. }
        | NodeData::Element { .. }
        | NodeData::Text { .. }
        | NodeData::Comment { .. }
        | NodeData::ProcessingInstruction { .. }
        | NodeData::CDATASection { .. } => {}
        NodeData::Attr { .. } => {
            return Err(hierarchy_request_error("Cannot insert an Attr node"));
        }
        NodeData::Document => {
            return Err(hierarchy_request_error("Cannot insert a Document node"));
        }
    }

    // Step 5: If node is Text and parent is Document, throw
    if matches!(node_data, NodeData::Text { .. }) && matches!(parent_data, NodeData::Document) {
        return Err(hierarchy_request_error("Cannot insert Text as a child of Document"));
    }

    // If node is Doctype and parent is not Document, throw
    if matches!(node_data, NodeData::Doctype { .. }) && !matches!(parent_data, NodeData::Document) {
        return Err(hierarchy_request_error(
            "Cannot insert Doctype as a child of a non-Document node",
        ));
    }

    // Step 6: If parent is Document, additional constraints
    if matches!(parent_data, NodeData::Document) {
        validate_document_replace(tree, parent_id, node_id, old_child_id, node_tree)?;
    }

    Ok(())
}

/// Additional validation when replacing within a Document node (step 6 of replace).
/// `node_tree` is the tree for the node when it differs from the parent's tree.
fn validate_document_replace(
    tree: &DomTree,
    parent_id: NodeId,
    node_id: NodeId,
    old_child_id: NodeId,
    node_tree: Option<&DomTree>,
) -> JsResult<()> {
    let nt = node_tree.unwrap_or(tree);
    let node_data = &nt.get_node(node_id).data;

    match node_data {
        NodeData::DocumentFragment => {
            let frag_children = &nt.get_node(node_id).children;
            let elem_count = frag_children
                .iter()
                .filter(|&&c| matches!(nt.get_node(c).data, NodeData::Element { .. }))
                .count();
            let has_text = frag_children
                .iter()
                .any(|&c| matches!(nt.get_node(c).data, NodeData::Text { .. }));

            if has_text {
                return Err(hierarchy_request_error(
                    "Cannot insert DocumentFragment containing Text into Document",
                ));
            }

            if elem_count > 1 {
                return Err(hierarchy_request_error(
                    "Cannot insert DocumentFragment with multiple elements into Document",
                ));
            }

            if elem_count == 1 {
                // Check if parent has an element child that is NOT old_child
                let parent_children = &tree.get_node(parent_id).children;
                let has_other_element = parent_children
                    .iter()
                    .any(|&c| c != old_child_id && matches!(tree.get_node(c).data, NodeData::Element { .. }));
                if has_other_element {
                    return Err(hierarchy_request_error("Document already has an element child"));
                }

                // Check if there's a doctype following old_child
                if has_doctype_after(tree, parent_id, old_child_id) {
                    return Err(hierarchy_request_error("Cannot insert element before a doctype"));
                }
            }
        }
        NodeData::Element { .. } => {
            // Check if parent has an element child that is NOT old_child
            let parent_children = &tree.get_node(parent_id).children;
            let has_other_element = parent_children
                .iter()
                .any(|&c| c != old_child_id && matches!(tree.get_node(c).data, NodeData::Element { .. }));
            if has_other_element {
                return Err(hierarchy_request_error("Document already has an element child"));
            }

            // Check if there's a doctype following old_child
            if has_doctype_after(tree, parent_id, old_child_id) {
                return Err(hierarchy_request_error("Cannot insert element before a doctype"));
            }
        }
        NodeData::Doctype { .. } => {
            // Check if parent has a doctype child that is NOT old_child
            let parent_children = &tree.get_node(parent_id).children;
            let has_other_doctype = parent_children
                .iter()
                .any(|&c| c != old_child_id && matches!(tree.get_node(c).data, NodeData::Doctype { .. }));
            if has_other_doctype {
                return Err(hierarchy_request_error("Document already has a doctype child"));
            }

            // Check if there's an element BEFORE old_child in parent's children
            if has_element_before(tree, parent_id, old_child_id) {
                return Err(hierarchy_request_error("Cannot insert doctype after an element"));
            }
        }
        _ => {}
    }

    Ok(())
}

/// Returns true if there's a Doctype node after `ref_id` in `parent_id`'s children.
fn has_doctype_after(tree: &DomTree, parent_id: NodeId, ref_id: NodeId) -> bool {
    let parent_children = &tree.get_node(parent_id).children;
    let mut found_ref = false;
    for &c in parent_children {
        if c == ref_id {
            found_ref = true;
            continue;
        }
        if found_ref && matches!(tree.get_node(c).data, NodeData::Doctype { .. }) {
            return true;
        }
    }
    false
}

/// Returns true if there's an Element node before `ref_id` in `parent_id`'s children.
fn has_element_before(tree: &DomTree, parent_id: NodeId, ref_id: NodeId) -> bool {
    let parent_children = &tree.get_node(parent_id).children;
    for &c in parent_children {
        if c == ref_id {
            return false;
        }
        if matches!(tree.get_node(c).data, NodeData::Element { .. }) {
            return true;
        }
    }
    false
}

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
        super::mutation_observer::queue_childlist_mutation(
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
            super::range::update_ranges_for_insert(ctx, parent_id, first_idx, added_ids.len());
        }
    }

    // Queue addition to new parent
    if !added_ids.is_empty() {
        super::mutation_observer::queue_childlist_mutation(
            ctx,
            tree,
            parent_id,
            added_ids.to_vec(),
            vec![],
            prev_sib,
            next_sib,
        );
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
            super::range::update_ranges_for_remove(ctx, old_pid, old_idx, moved_node_id, &t);
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

fn do_replace(tree: &Rc<RefCell<DomTree>>, parent_id: NodeId, node_id: NodeId, old_child_id: NodeId) {
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

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

pub(crate) fn register_mutation(class: &mut ClassBuilder) -> JsResult<()> {
    class.method(
        js_string!("insertBefore"),
        2,
        NativeFunction::from_fn_ptr(insert_before),
    );
    class.method(
        js_string!("replaceChild"),
        2,
        NativeFunction::from_fn_ptr(replace_child),
    );
    class.method(js_string!("removeChild"), 1, NativeFunction::from_fn_ptr(remove_child));
    class.method(js_string!("cloneNode"), 1, NativeFunction::from_fn_ptr(clone_node));
    class.method(js_string!("append"), 0, NativeFunction::from_fn_ptr(append));
    class.method(js_string!("prepend"), 0, NativeFunction::from_fn_ptr(prepend));
    class.method(
        js_string!("replaceChildren"),
        0,
        NativeFunction::from_fn_ptr(replace_children),
    );
    class.method(js_string!("before"), 0, NativeFunction::from_fn_ptr(child_node_before));
    class.method(js_string!("after"), 0, NativeFunction::from_fn_ptr(child_node_after));
    class.method(
        js_string!("replaceWith"),
        0,
        NativeFunction::from_fn_ptr(child_node_replace_with),
    );
    class.method(
        js_string!("insertAdjacentElement"),
        2,
        NativeFunction::from_fn_ptr(insert_adjacent_element),
    );
    class.method(
        js_string!("insertAdjacentText"),
        2,
        NativeFunction::from_fn_ptr(insert_adjacent_text),
    );
    class.method(js_string!("normalize"), 0, NativeFunction::from_fn_ptr(normalize));
    Ok(())
}

fn insert_before(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(parent, this, "insertBefore");
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    // First argument: node (required, must be a Node)
    let new_node_arg = args
        .first()
        .ok_or_else(|| JsNativeError::typ().with_message("insertBefore: 1 argument required"))?;
    if new_node_arg.is_null() || new_node_arg.is_undefined() {
        return Err(JsNativeError::typ()
            .with_message("insertBefore: argument 1 is not a Node")
            .into());
    }
    let new_node_obj = new_node_arg
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("insertBefore: argument 1 is not a Node"))?;
    let new_node = new_node_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsNativeError::typ().with_message("insertBefore: argument 1 is not a Node"))?;

    // Check if node is a Document - must reject before adoption changes it
    {
        let node_tree_ref = new_node.tree.borrow();
        let node_data = &node_tree_ref.get_node(new_node.node_id).data;
        if matches!(node_data, NodeData::Document) {
            return Err(hierarchy_request_error("Cannot insert a Document node"));
        }
    }

    // Cross-tree adoption: if node is from a different tree, adopt it first
    let new_node_id = if !Rc::ptr_eq(&tree, &new_node.tree) {
        let src_tree = new_node.tree.clone();
        let src_id = new_node.node_id;
        let adopted_id = adopt_node(&src_tree, src_id, &tree);
        drop(new_node);
        let mut child_mut = new_node_obj.downcast_mut::<JsElement>().unwrap();
        child_mut.node_id = adopted_id;
        child_mut.tree = tree.clone();
        drop(child_mut);
        update_node_cache_after_adoption(&src_tree, src_id, &tree, adopted_id, &new_node_obj, ctx);
        adopted_id
    } else {
        new_node.node_id
    };

    // Second argument: reference child (required per spec — missing throws TypeError)
    let ref_arg = args
        .get(1)
        .ok_or_else(|| JsNativeError::typ().with_message("insertBefore: 2 arguments required"))?;

    let ref_id = if ref_arg.is_null() || ref_arg.is_undefined() {
        None
    } else {
        let ref_obj = ref_arg
            .as_object()
            .ok_or_else(|| JsNativeError::typ().with_message("insertBefore: argument 2 is not a Node or null"))?;
        let ref_el = ref_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsNativeError::typ().with_message("insertBefore: argument 2 is not a Node or null"))?;
        // If ref child is from a different tree, it can't be a child of parent -> NotFoundError
        if !Rc::ptr_eq(&tree, &ref_el.tree) {
            return Err(not_found_error(
                "The node before which the new node is to be inserted is not a child of this node",
            ));
        }
        Some(ref_el.node_id)
    };

    // Pre-insertion validation (node is now in same tree after adoption)
    validate_pre_insert(&tree.borrow(), parent_id, new_node_id, ref_id, None)?;

    // Capture pre-state for MutationObserver
    let (added_ids, removal_info, prev_sib, next_sib) = capture_insert_state(&tree, parent_id, new_node_id, ref_id);

    // Update live ranges for removal from old parent (before the move)
    fire_range_removal_for_move(ctx, &tree, &removal_info, new_node_id);

    do_insert(&tree, parent_id, new_node_id, ref_id);

    // Queue MutationObserver records + update live ranges for insertion
    fire_insert_records(ctx, &tree, parent_id, &added_ids, removal_info, prev_sib, next_sib);

    Ok(new_node_arg.clone())
}

fn replace_child(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(parent, this, "replaceChild");
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    // First arg: new child (required)
    let new_child_arg = args
        .first()
        .ok_or_else(|| JsNativeError::typ().with_message("replaceChild: 2 arguments required"))?;
    if new_child_arg.is_null() || new_child_arg.is_undefined() {
        return Err(JsNativeError::typ()
            .with_message("replaceChild: argument 1 is not a Node")
            .into());
    }
    let new_child_obj = new_child_arg
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("replaceChild: argument 1 is not a Node"))?;
    let new_child = new_child_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsNativeError::typ().with_message("replaceChild: argument 1 is not a Node"))?;

    // Check if node is a Document - must reject before adoption changes it
    {
        let node_tree_ref = new_child.tree.borrow();
        let node_data = &node_tree_ref.get_node(new_child.node_id).data;
        if matches!(node_data, NodeData::Document) {
            return Err(hierarchy_request_error("Cannot insert a Document node"));
        }
    }

    // Cross-tree adoption: if new child is from a different tree, adopt it first
    let new_child_id = if !Rc::ptr_eq(&tree, &new_child.tree) {
        let src_tree = new_child.tree.clone();
        let src_id = new_child.node_id;
        let adopted_id = adopt_node(&src_tree, src_id, &tree);
        drop(new_child);
        let mut child_mut = new_child_obj.downcast_mut::<JsElement>().unwrap();
        child_mut.node_id = adopted_id;
        child_mut.tree = tree.clone();
        drop(child_mut);
        update_node_cache_after_adoption(&src_tree, src_id, &tree, adopted_id, &new_child_obj, ctx);
        adopted_id
    } else {
        new_child.node_id
    };

    // Second arg: old child (required)
    let old_child_arg = args
        .get(1)
        .ok_or_else(|| JsNativeError::typ().with_message("replaceChild: 2 arguments required"))?;
    if old_child_arg.is_null() || old_child_arg.is_undefined() {
        return Err(JsNativeError::typ()
            .with_message("replaceChild: argument 2 is not a Node")
            .into());
    }
    let old_child_obj = old_child_arg
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("replaceChild: argument 2 is not a Node"))?;
    let old_child = old_child_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsNativeError::typ().with_message("replaceChild: argument 2 is not a Node"))?;
    // If old child is from a different tree, it can't be a child of parent -> NotFoundError
    if !Rc::ptr_eq(&tree, &old_child.tree) {
        return Err(not_found_error("The node to be replaced is not a child of this node"));
    }
    let old_child_id = old_child.node_id;

    // Pre-replace validation (new child is now in same tree after adoption)
    validate_pre_replace(&tree.borrow(), parent_id, new_child_id, old_child_id, None)?;

    // Capture pre-state for MutationObserver
    let (added_ids, removal_info, prev_sib, next_sib) = {
        let t = tree.borrow();
        let is_fragment = matches!(t.get_node(new_child_id).data, NodeData::DocumentFragment | NodeData::ShadowRoot { .. });
        let added = if is_fragment {
            t.get_node(new_child_id).children.clone()
        } else {
            vec![new_child_id]
        };

        // Capture removal from old parent if new_child is being moved
        let old_parent = t.get_node(new_child_id).parent;
        let removal = if let Some(old_pid) = old_parent {
            if !is_fragment && old_pid != parent_id {
                let old_children = &t.get_node(old_pid).children;
                let pos = old_children.iter().position(|&c| c == new_child_id);
                let old_prev = pos.and_then(|p| if p > 0 { Some(old_children[p - 1]) } else { None });
                let old_next = pos.and_then(|p| old_children.get(p + 1).copied());
                Some((old_pid, old_prev, old_next))
            } else {
                None
            }
        } else {
            None
        };

        // Siblings around the old_child being replaced
        let parent_children = &t.get_node(parent_id).children;
        let pos = parent_children.iter().position(|&c| c == old_child_id);
        let ps = pos.and_then(|p| if p > 0 { Some(parent_children[p - 1]) } else { None });
        let ns = pos.and_then(|p| parent_children.get(p + 1).copied());

        (added, removal, ps, ns)
    };

    // Update live ranges for the removal of old_child (must happen before do_replace)
    {
        let t = tree.borrow();
        let parent_children = &t.get_node(parent_id).children;
        if let Some(old_idx) = parent_children.iter().position(|&c| c == old_child_id) {
            super::range::update_ranges_for_remove(ctx, parent_id, old_idx, old_child_id, &t);
        }
    }

    do_replace(&tree, parent_id, new_child_id, old_child_id);

    // Update live ranges for the insertion of new child(ren)
    if !added_ids.is_empty() {
        let t = tree.borrow();
        let parent_children = &t.get_node(parent_id).children;
        if let Some(first_idx) = parent_children.iter().position(|&c| c == added_ids[0]) {
            drop(t);
            super::range::update_ranges_for_insert(ctx, parent_id, first_idx, added_ids.len());
        }
    }

    // Queue removal of new_child from old parent (if moved)
    if let Some((old_pid, old_prev, old_next)) = removal_info {
        super::mutation_observer::queue_childlist_mutation(
            ctx,
            &tree,
            old_pid,
            vec![],
            vec![new_child_id],
            old_prev,
            old_next,
        );
    }

    // Queue the replace record (both added and removed on the parent)
    super::mutation_observer::queue_childlist_mutation(
        ctx,
        &tree,
        parent_id,
        added_ids,
        vec![old_child_id],
        prev_sib,
        next_sib,
    );

    let js_obj = get_or_create_js_element(old_child_id, tree, ctx)?;
    Ok(js_obj.into())
}

fn remove_child(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(parent, this, "removeChild");
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    let child_arg = args
        .first()
        .ok_or_else(|| JsNativeError::typ().with_message("removeChild: 1 argument required"))?;
    if child_arg.is_null() || child_arg.is_undefined() {
        return Err(JsNativeError::typ()
            .with_message("removeChild: argument 1 is not a Node")
            .into());
    }
    let child_obj = child_arg
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("removeChild: argument 1 is not a Node"))?;
    let child = child_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsNativeError::typ().with_message("removeChild: argument 1 is not a Node"))?;

    // If child is from a different tree, it can't be a child of parent -> NotFoundError
    if !Rc::ptr_eq(&tree, &child.tree) {
        return Err(not_found_error("The node to be removed is not a child of this node"));
    }
    let child_id = child.node_id;

    let (prev_sib, next_sib, old_index) = {
        let t = tree.borrow();
        let parent_node = t.get_node(parent_id);
        if !parent_node.children.contains(&child_id) {
            return Err(not_found_error("The node to be removed is not a child of this node"));
        }
        let parent_children = &parent_node.children;
        let pos = parent_children.iter().position(|&c| c == child_id);
        let prev = pos.and_then(|p| if p > 0 { Some(parent_children[p - 1]) } else { None });
        let next = pos.and_then(|p| parent_children.get(p + 1).copied());
        (prev, next, pos.unwrap())
    };

    // Update live range boundaries before the actual removal
    super::range::update_ranges_for_remove(ctx, parent_id, old_index, child_id, &tree.borrow());

    tree.borrow_mut().remove_child(parent_id, child_id);

    super::mutation_observer::queue_childlist_mutation(
        ctx,
        &tree,
        parent_id,
        vec![],
        vec![child_id],
        prev_sib,
        next_sib,
    );

    let js_obj = get_or_create_js_element(child_id, tree, ctx)?;
    Ok(js_obj.into())
}

fn clone_node(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "cloneNode");
    let node_id = el.node_id;
    let tree = el.tree.clone();

    let deep = args.first().map(|v| v.to_boolean()).unwrap_or(false);

    // Special case: cloning a Document node creates a new DomTree
    let is_document = matches!(tree.borrow().get_node(node_id).data, NodeData::Document);
    if is_document {
        let is_html = tree.borrow().is_html_document();
        let new_tree = Rc::new(RefCell::new(if is_html { DomTree::new() } else { DomTree::new_xml() }));

        if deep {
            // Clone all children of the source document into the new document
            let child_ids: Vec<NodeId> = tree.borrow().get_node(node_id).children.clone();
            let new_doc_id = new_tree.borrow().document();
            for child_id in child_ids {
                let cloned_child = clone_node_cross_tree(&tree.borrow(), child_id, &mut new_tree.borrow_mut());
                new_tree.borrow_mut().append_child(new_doc_id, cloned_child);
            }
        }

        let doc_id = new_tree.borrow().document();
        let js_obj = get_or_create_js_element(doc_id, new_tree.clone(), ctx)?;
        let content_type = if is_html { "text/html" } else { "application/xml" };
        super::document::add_document_properties_to_element(&js_obj, new_tree, content_type.to_string(), ctx)?;
        return Ok(js_obj.into());
    }

    let cloned_id = tree.borrow_mut().clone_node(node_id, deep);

    let js_obj = get_or_create_js_element(cloned_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Recursively clone a node from one DomTree into another.
pub(crate) fn clone_node_cross_tree(src: &DomTree, src_id: NodeId, dst: &mut DomTree) -> NodeId {
    let src_node = src.get_node(src_id);
    let new_id = match &src_node.data {
        NodeData::Element {
            tag_name,
            attributes,
            namespace,
        } => {
            let id = dst.create_element(tag_name);
            if let NodeData::Element {
                attributes: ref mut dst_attrs,
                namespace: ref mut dst_ns,
                ..
            } = dst.get_node_mut(id).data
            {
                *dst_attrs = attributes.clone();
                *dst_ns = namespace.clone();
            }
            id
        }
        NodeData::Text { content } => dst.create_text(content),
        NodeData::Comment { content } => dst.create_comment(content),
        NodeData::CDATASection { content } => dst.create_cdata_section(content),
        NodeData::Doctype {
            name,
            public_id,
            system_id,
        } => dst.create_doctype(name, public_id, system_id),
        NodeData::ProcessingInstruction { target, data } => dst.create_processing_instruction(target, data),
        NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => dst.create_document_fragment(),
        NodeData::Document => unreachable!("nested Document nodes not supported"),
        NodeData::Attr { .. } => unreachable!("Attr nodes should not be children"),
    };

    // Recursively clone children
    let child_ids: Vec<NodeId> = src.get_node(src_id).children.clone();
    for child_id in child_ids {
        let cloned_child = clone_node_cross_tree(src, child_id, dst);
        dst.append_child(new_id, cloned_child);
    }

    new_id
}

/// Convert variadic args (nodes or strings) into a Vec<NodeId>.
/// String arguments become new Text nodes; JsElement arguments yield their node_id.
fn convert_nodes_from_args(args: &[JsValue], tree: &Rc<RefCell<DomTree>>, ctx: &mut Context) -> JsResult<Vec<NodeId>> {
    let mut node_ids = Vec::new();
    for arg in args {
        if let Some(s) = arg.as_string() {
            let text_id = tree.borrow_mut().create_text(&s.to_std_string_escaped());
            node_ids.push(text_id);
        } else if let Some(obj) = arg.as_object() {
            if let Some(el) = obj.downcast_ref::<JsElement>() {
                node_ids.push(el.node_id);
            } else {
                // Try converting to string
                let s = arg.to_string(ctx)?.to_std_string_escaped();
                let text_id = tree.borrow_mut().create_text(&s);
                node_ids.push(text_id);
            }
        } else {
            // Convert primitive to string and make text node
            let s = arg.to_string(ctx)?.to_std_string_escaped();
            let text_id = tree.borrow_mut().create_text(&s);
            node_ids.push(text_id);
        }
    }
    Ok(node_ids)
}

/// ParentNode.append(...nodes)
fn append(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(parent, this, "append");
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    let node_ids = convert_nodes_from_args(args, &tree, ctx)?;
    for nid in node_ids {
        validate_pre_insert(&tree.borrow(), parent_id, nid, None, None)?;
        let (added_ids, removal_info, prev_sib, next_sib) = capture_insert_state(&tree, parent_id, nid, None);
        fire_range_removal_for_move(ctx, &tree, &removal_info, nid);
        do_insert(&tree, parent_id, nid, None);
        fire_insert_records(ctx, &tree, parent_id, &added_ids, removal_info, prev_sib, next_sib);
    }
    Ok(JsValue::undefined())
}

/// ParentNode.prepend(...nodes)
fn prepend(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(parent, this, "prepend");
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    let node_ids = convert_nodes_from_args(args, &tree, ctx)?;
    let original_first_child = tree.borrow().first_child(parent_id);
    for nid in node_ids {
        validate_pre_insert(&tree.borrow(), parent_id, nid, original_first_child, None)?;
        let (added_ids, removal_info, prev_sib, next_sib) =
            capture_insert_state(&tree, parent_id, nid, original_first_child);
        fire_range_removal_for_move(ctx, &tree, &removal_info, nid);
        do_insert(&tree, parent_id, nid, original_first_child);
        fire_insert_records(ctx, &tree, parent_id, &added_ids, removal_info, prev_sib, next_sib);
    }
    Ok(JsValue::undefined())
}

/// ParentNode.replaceChildren(...nodes)
fn replace_children(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(parent, this, "replaceChildren");
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    let node_ids = convert_nodes_from_args(args, &tree, ctx)?;
    // Validate all nodes first before making changes
    for &nid in &node_ids {
        validate_pre_insert(&tree.borrow(), parent_id, nid, None, None)?;
    }
    // Capture removed children for MutationObserver and update live ranges
    let removed_children: Vec<NodeId> = tree.borrow().get_node(parent_id).children.clone();
    // Update live ranges for each removed child (in reverse order to keep indices valid)
    for (idx, &child_id) in removed_children.iter().enumerate().rev() {
        super::range::update_ranges_for_remove(ctx, parent_id, idx, child_id, &tree.borrow());
    }
    tree.borrow_mut().clear_children(parent_id);
    if !removed_children.is_empty() {
        super::mutation_observer::queue_childlist_mutation(ctx, &tree, parent_id, vec![], removed_children, None, None);
    }
    for nid in node_ids {
        let (added_ids, removal_info, prev_sib, next_sib) = capture_insert_state(&tree, parent_id, nid, None);
        fire_range_removal_for_move(ctx, &tree, &removal_info, nid);
        do_insert(&tree, parent_id, nid, None);
        fire_insert_records(ctx, &tree, parent_id, &added_ids, removal_info, prev_sib, next_sib);
    }
    Ok(JsValue::undefined())
}

/// ChildNode.before(...nodes)
fn child_node_before(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "before");
    let this_id = el.node_id;
    let tree = el.tree.clone();

    let parent_id = match tree.borrow().get_parent(this_id) {
        Some(p) => p,
        None => return Ok(JsValue::undefined()),
    };

    let node_ids = convert_nodes_from_args(args, &tree, ctx)?;
    if node_ids.is_empty() {
        return Ok(JsValue::undefined());
    }

    // Find viable previous sibling: first preceding sibling NOT in node_ids
    let viable_prev = {
        let t = tree.borrow();
        let parent_children = t.children(parent_id);
        let this_pos = parent_children.iter().position(|&c| c == this_id);
        match this_pos {
            Some(pos) => {
                let mut result = None;
                for i in (0..pos).rev() {
                    if !node_ids.contains(&parent_children[i]) {
                        result = Some(parent_children[i]);
                        break;
                    }
                }
                result
            }
            None => None,
        }
    };

    // Capture the previous sibling before the insertion point for MutationObserver
    let mo_prev_sib = viable_prev;

    for &nid in &node_ids {
        tree.borrow_mut().remove_from_parent(nid);
    }

    let reference = match viable_prev {
        Some(prev_id) => tree.borrow().next_sibling(prev_id),
        None => tree.borrow().first_child(parent_id),
    };

    for nid in &node_ids {
        match reference {
            Some(ref_id) => tree.borrow_mut().insert_child_before(parent_id, *nid, ref_id),
            None => tree.borrow_mut().append_child(parent_id, *nid),
        }
    }

    // Queue MutationObserver record for the batch insertion
    if !node_ids.is_empty() {
        super::mutation_observer::queue_childlist_mutation(
            ctx,
            &tree,
            parent_id,
            node_ids,
            vec![],
            mo_prev_sib,
            reference,
        );
    }

    Ok(JsValue::undefined())
}

/// ChildNode.after(...nodes)
fn child_node_after(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "after");
    let this_id = el.node_id;
    let tree = el.tree.clone();

    let parent_id = match tree.borrow().get_parent(this_id) {
        Some(p) => p,
        None => return Ok(JsValue::undefined()),
    };

    let node_ids = convert_nodes_from_args(args, &tree, ctx)?;
    if node_ids.is_empty() {
        return Ok(JsValue::undefined());
    }

    // Find viable next sibling: first following sibling NOT in node_ids
    let viable_next = {
        let t = tree.borrow();
        let parent_children = t.children(parent_id);
        let this_pos = parent_children.iter().position(|&c| c == this_id);
        match this_pos {
            Some(pos) => {
                let mut result = None;
                for &child in &parent_children[(pos + 1)..] {
                    if !node_ids.contains(&child) {
                        result = Some(child);
                        break;
                    }
                }
                result
            }
            None => None,
        }
    };

    // Capture MutationObserver siblings: prev is this_id, next is viable_next
    let mo_prev_sib = Some(this_id);
    let mo_next_sib = viable_next;

    for &nid in &node_ids {
        tree.borrow_mut().remove_from_parent(nid);
    }

    for nid in &node_ids {
        match viable_next {
            Some(ref_id) => tree.borrow_mut().insert_child_before(parent_id, *nid, ref_id),
            None => tree.borrow_mut().append_child(parent_id, *nid),
        }
    }

    // Queue MutationObserver record
    if !node_ids.is_empty() {
        super::mutation_observer::queue_childlist_mutation(
            ctx,
            &tree,
            parent_id,
            node_ids,
            vec![],
            mo_prev_sib,
            mo_next_sib,
        );
    }

    Ok(JsValue::undefined())
}

/// ChildNode.replaceWith(...nodes)
fn child_node_replace_with(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "replaceWith");
    let this_id = el.node_id;
    let tree = el.tree.clone();

    let node_ids = convert_nodes_from_args(args, &tree, ctx)?;

    let parent_id = match tree.borrow().get_parent(this_id) {
        Some(p) => p,
        None => return Ok(JsValue::undefined()),
    };

    // Find viable next sibling: first following sibling NOT in node_ids
    let viable_next = {
        let t = tree.borrow();
        let parent_children = t.children(parent_id);
        let this_pos = parent_children.iter().position(|&c| c == this_id);
        match this_pos {
            Some(pos) => {
                let mut result = None;
                for &child in &parent_children[(pos + 1)..] {
                    if !node_ids.contains(&child) {
                        result = Some(child);
                        break;
                    }
                }
                result
            }
            None => None,
        }
    };

    // Capture MutationObserver siblings around this_id
    let (mo_prev_sib, mo_next_sib) = {
        let t = tree.borrow();
        let parent_children = t.children(parent_id);
        let pos = parent_children.iter().position(|&c| c == this_id);
        let ps = pos.and_then(|p| if p > 0 { Some(parent_children[p - 1]) } else { None });
        let ns = viable_next;
        (ps, ns)
    };

    tree.borrow_mut().remove_from_parent(this_id);

    for &nid in &node_ids {
        tree.borrow_mut().remove_from_parent(nid);
    }

    for nid in &node_ids {
        match viable_next {
            Some(ref_id) => tree.borrow_mut().insert_child_before(parent_id, *nid, ref_id),
            None => tree.borrow_mut().append_child(parent_id, *nid),
        }
    }

    // Queue MutationObserver record: this_id removed, node_ids added
    super::mutation_observer::queue_childlist_mutation(
        ctx,
        &tree,
        parent_id,
        node_ids,
        vec![this_id],
        mo_prev_sib,
        mo_next_sib,
    );

    Ok(JsValue::undefined())
}

/// Parse an insertAdjacent position string (case-insensitive).
/// Returns the lowercase canonical form or a SyntaxError.
fn parse_adjacent_position(pos: &str) -> JsResult<&'static str> {
    match pos.to_ascii_lowercase().as_str() {
        "beforebegin" => Ok("beforebegin"),
        "afterbegin" => Ok("afterbegin"),
        "beforeend" => Ok("beforeend"),
        "afterend" => Ok("afterend"),
        _ => Err(JsNativeError::syntax()
            .with_message(format!(
                "The value provided ('{}') is not one of 'beforeBegin', 'afterBegin', 'beforeEnd', or 'afterEnd'.",
                pos
            ))
            .into()),
    }
}

/// Perform the insertion of `child_id` at `position` relative to `this_id`.
/// For "beforebegin"/"afterend", if the element has no parent, returns Ok(false).
/// For "beforebegin"/"afterend" where the parent is a Document node, throws HierarchyRequestError.
/// Returns Ok(true) if insertion was performed.
fn do_insert_adjacent(
    tree: &Rc<RefCell<DomTree>>,
    this_id: NodeId,
    child_id: NodeId,
    position: &str,
) -> JsResult<bool> {
    match position {
        "beforebegin" => {
            let parent_id = match tree.borrow().get_parent(this_id) {
                Some(p) => p,
                None => return Ok(false),
            };
            // If parent is a Document node, throw HierarchyRequestError
            if matches!(tree.borrow().get_node(parent_id).data, NodeData::Document) {
                return Err(JsNativeError::typ()
                    .with_message(
                        "HierarchyRequestError: Cannot insert before the document element's parent is a Document",
                    )
                    .into());
            }
            tree.borrow_mut().insert_before(this_id, child_id);
            Ok(true)
        }
        "afterbegin" => {
            let fc = tree.borrow().get_node(this_id).children.first().copied();
            match fc {
                Some(first_child) => tree.borrow_mut().insert_child_before(this_id, child_id, first_child),
                None => tree.borrow_mut().append_child(this_id, child_id),
            }
            Ok(true)
        }
        "beforeend" => {
            tree.borrow_mut().append_child(this_id, child_id);
            Ok(true)
        }
        "afterend" => {
            let parent_id = match tree.borrow().get_parent(this_id) {
                Some(p) => p,
                None => return Ok(false),
            };
            // If parent is a Document node, throw HierarchyRequestError
            if matches!(tree.borrow().get_node(parent_id).data, NodeData::Document) {
                return Err(JsNativeError::typ()
                    .with_message(
                        "HierarchyRequestError: Cannot insert after the document element's parent is a Document",
                    )
                    .into());
            }
            tree.borrow_mut().insert_after(this_id, child_id);
            Ok(true)
        }
        _ => unreachable!(),
    }
}

/// Element.insertAdjacentElement(position, element)
fn insert_adjacent_element(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "insertAdjacentElement");
    let this_id = el.node_id;
    let tree = el.tree.clone();

    let pos_str = args
        .first()
        .ok_or_else(|| JsNativeError::typ().with_message("insertAdjacentElement: missing position argument"))?
        .to_string(ctx)?
        .to_std_string_escaped();

    let position = parse_adjacent_position(&pos_str)?;

    let new_el_arg = args
        .get(1)
        .ok_or_else(|| JsNativeError::typ().with_message("insertAdjacentElement: missing element argument"))?;
    let new_el_obj = new_el_arg
        .as_object()
        .ok_or_else(|| JsNativeError::typ().with_message("insertAdjacentElement: second argument is not an object"))?;
    let new_el = new_el_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsNativeError::typ().with_message("insertAdjacentElement: second argument is not an Element"))?;
    let new_el_id = new_el.node_id;

    // Per spec, insertAdjacentElement only accepts Element nodes (nodeType 1).
    // DocumentType and other non-Element nodes must throw TypeError.
    {
        let t = tree.borrow();
        if t.node_type(new_el_id) != 1 {
            return Err(JsNativeError::typ()
                .with_message("insertAdjacentElement: second argument is not an Element")
                .into());
        }
    }

    let inserted = do_insert_adjacent(&tree, this_id, new_el_id, position)?;
    if inserted {
        Ok(new_el_arg.clone())
    } else {
        Ok(JsValue::null())
    }
}

/// Element.insertAdjacentText(position, text)
fn insert_adjacent_text(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "insertAdjacentText");
    let this_id = el.node_id;
    let tree = el.tree.clone();

    let pos_str = args
        .first()
        .ok_or_else(|| JsNativeError::typ().with_message("insertAdjacentText: missing position argument"))?
        .to_string(ctx)?
        .to_std_string_escaped();

    let position = parse_adjacent_position(&pos_str)?;

    let text_str = args
        .get(1)
        .ok_or_else(|| JsNativeError::typ().with_message("insertAdjacentText: missing text argument"))?
        .to_string(ctx)?
        .to_std_string_escaped();

    let text_id = tree.borrow_mut().create_text(&text_str);

    do_insert_adjacent(&tree, this_id, text_id, position)?;
    Ok(JsValue::undefined())
}

/// Node.normalize()
fn normalize(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    extract_element!(el, this, "normalize");
    let node_id = el.node_id;
    let tree = el.tree.clone();

    tree.borrow_mut().normalize(node_id);

    Ok(JsValue::undefined())
}

/// Standalone versions for document object (uses JsDocument instead of JsElement)
pub(crate) fn document_normalize(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("normalize: this is not an object").into()))?;
    let doc = obj
        .downcast_ref::<super::document::JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("normalize: this is not document").into()))?;
    let tree = doc.tree.clone();
    let doc_id = tree.borrow().document();

    tree.borrow_mut().normalize(doc_id);

    Ok(JsValue::undefined())
}

pub(crate) fn document_append(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("append: this is not an object").into()))?;
    let doc = obj
        .downcast_ref::<super::document::JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("append: this is not document").into()))?;
    let tree = doc.tree.clone();
    let doc_id = tree.borrow().document();

    let node_ids = convert_nodes_from_args(args, &tree, ctx)?;
    for nid in node_ids {
        validate_pre_insert(&tree.borrow(), doc_id, nid, None, None)?;
        let (added_ids, removal_info, prev_sib, next_sib) = capture_insert_state(&tree, doc_id, nid, None);
        fire_range_removal_for_move(ctx, &tree, &removal_info, nid);
        do_insert(&tree, doc_id, nid, None);
        fire_insert_records(ctx, &tree, doc_id, &added_ids, removal_info, prev_sib, next_sib);
    }
    Ok(JsValue::undefined())
}

pub(crate) fn document_prepend(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("prepend: this is not an object").into()))?;
    let doc = obj
        .downcast_ref::<super::document::JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("prepend: this is not document").into()))?;
    let tree = doc.tree.clone();
    let doc_id = tree.borrow().document();

    let node_ids = convert_nodes_from_args(args, &tree, ctx)?;
    let original_first_child = tree.borrow().first_child(doc_id);
    for nid in node_ids {
        validate_pre_insert(&tree.borrow(), doc_id, nid, original_first_child, None)?;
        let (added_ids, removal_info, prev_sib, next_sib) =
            capture_insert_state(&tree, doc_id, nid, original_first_child);
        fire_range_removal_for_move(ctx, &tree, &removal_info, nid);
        do_insert(&tree, doc_id, nid, original_first_child);
        fire_insert_records(ctx, &tree, doc_id, &added_ids, removal_info, prev_sib, next_sib);
    }
    Ok(JsValue::undefined())
}

pub(crate) fn document_replace_children(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChildren: this is not an object").into()))?;
    let doc = obj
        .downcast_ref::<super::document::JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChildren: this is not document").into()))?;
    let tree = doc.tree.clone();
    let doc_id = tree.borrow().document();

    let node_ids = convert_nodes_from_args(args, &tree, ctx)?;
    for &nid in &node_ids {
        validate_pre_insert(&tree.borrow(), doc_id, nid, None, None)?;
    }
    let removed_children: Vec<NodeId> = tree.borrow().get_node(doc_id).children.clone();
    tree.borrow_mut().clear_children(doc_id);
    if !removed_children.is_empty() {
        super::mutation_observer::queue_childlist_mutation(ctx, &tree, doc_id, vec![], removed_children, None, None);
    }
    for nid in node_ids {
        let (added_ids, removal_info, prev_sib, next_sib) = capture_insert_state(&tree, doc_id, nid, None);
        fire_range_removal_for_move(ctx, &tree, &removal_info, nid);
        do_insert(&tree, doc_id, nid, None);
        fire_insert_records(ctx, &tree, doc_id, &added_ids, removal_info, prev_sib, next_sib);
    }
    Ok(JsValue::undefined())
}

#[cfg(test)]
mod tests {
    use crate::dom::{DomTree, NodeData};
    use crate::js::runtime::JsRuntime;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_mutation_test_tree() -> Rc<RefCell<DomTree>> {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let div = t.create_element("div");
            let span_a = t.create_element("span");
            let span_b = t.create_element("span");
            let span_c = t.create_element("span");
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(div).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "parent"));
            }
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span_a).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "a"));
            }
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span_b).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "b"));
            }
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span_c).data {
                attributes.push(crate::dom::node::DomAttribute::new("id", "c"));
            }
            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
            t.append_child(body, div);
            t.append_child(div, span_a);
            t.append_child(div, span_b);
            t.append_child(div, span_c);
        }
        tree
    }

    #[test]
    fn insert_before_inserts_before_reference_node() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(
            r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            parent.insertBefore(newNode, b);
        "#,
        )
        .unwrap();
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        let children = &t.get_node(parent_id).children;
        assert_eq!(children.len(), 4);
        let new_id = t.get_element_by_id("new").unwrap();
        let a_id = t.get_element_by_id("a").unwrap();
        let b_id = t.get_element_by_id("b").unwrap();
        let c_id = t.get_element_by_id("c").unwrap();
        assert_eq!(children, &vec![a_id, new_id, b_id, c_id]);
    }

    #[test]
    fn insert_before_with_null_reference_appends() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(
            r#"
            var parent = document.getElementById("parent");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            parent.insertBefore(newNode, null);
        "#,
        )
        .unwrap();
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        let children = &t.get_node(parent_id).children;
        assert_eq!(children.len(), 4);
        let new_id = t.get_element_by_id("new").unwrap();
        assert_eq!(*children.last().unwrap(), new_id);
    }

    #[test]
    fn insert_before_detaches_from_old_parent() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(
            r#"
            var parent = document.getElementById("parent");
            var a = document.getElementById("a");
            var c = document.getElementById("c");
            parent.insertBefore(a, c);
        "#,
        )
        .unwrap();
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        let children = &t.get_node(parent_id).children;
        assert_eq!(children.len(), 3);
        let a_id = t.get_element_by_id("a").unwrap();
        let b_id = t.get_element_by_id("b").unwrap();
        let c_id = t.get_element_by_id("c").unwrap();
        assert_eq!(children, &vec![b_id, a_id, c_id]);
    }

    #[test]
    fn replace_child_swaps_nodes_correctly() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(
            r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            parent.replaceChild(newNode, b);
        "#,
        )
        .unwrap();
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        let children = &t.get_node(parent_id).children;
        assert_eq!(children.len(), 3);
        let new_id = t.get_element_by_id("new").unwrap();
        let a_id = t.get_element_by_id("a").unwrap();
        let c_id = t.get_element_by_id("c").unwrap();
        assert_eq!(children, &vec![a_id, new_id, c_id]);
        // After replacement, "b" is disconnected and should not be found via getElementById
        assert!(t.get_element_by_id("b").is_none());
    }

    #[test]
    fn replace_child_detaches_new_child() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(
            r#"
            var parent = document.getElementById("parent");
            var a = document.getElementById("a");
            var b = document.getElementById("b");
            parent.replaceChild(a, b);
        "#,
        )
        .unwrap();
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        let children = &t.get_node(parent_id).children;
        assert_eq!(children.len(), 2);
        let a_id = t.get_element_by_id("a").unwrap();
        let c_id = t.get_element_by_id("c").unwrap();
        assert_eq!(children, &vec![a_id, c_id]);
    }

    #[test]
    fn remove_child_removes_and_returns() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var removed = parent.removeChild(b);
            removed.getAttribute("id");
        "#,
            )
            .unwrap();
        let id_str = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(id_str, "b");
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        assert_eq!(t.get_node(parent_id).children.len(), 2);
        // After removal, "b" is disconnected and should not be found via getElementById
        assert!(t.get_element_by_id("b").is_none());
    }

    #[test]
    fn remove_child_on_non_child_returns_error() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var body = document.body;
            var a = document.getElementById("a");
            try { body.removeChild(a); "no error"; } catch(e) { "error"; }
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "error");
    }

    #[test]
    fn clone_node_shallow_copy() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(false);
            clone.hasChildNodes();
        "#,
            )
            .unwrap();
        assert!(!result.to_boolean());
    }

    #[test]
    fn clone_node_deep_copy_with_children() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(true);
            clone.childNodes.length;
        "#,
            )
            .unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 3);
    }

    #[test]
    fn clone_node_preserves_attributes() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(false);
            clone.getAttribute("id");
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "parent");
    }

    #[test]
    fn clone_node_has_no_parent() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(true);
            clone.parentNode === null;
        "#,
            )
            .unwrap();
        assert!(result.to_boolean());
    }

    #[test]
    fn insert_before_returns_new_node() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            var returned = parent.insertBefore(newNode, b);
            returned.getAttribute("id");
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "new");
    }

    #[test]
    fn replace_child_returns_old_child() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            var returned = parent.replaceChild(newNode, b);
            returned.getAttribute("id");
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "b");
    }

    #[test]
    fn doctype_into_element_throws() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var doc = document.implementation.createHTMLDocument("title");
            var doctype = doc.childNodes[0];
            var el = doc.createElement("a");
            var results = [];
            results.push("doctype.nodeType=" + doctype.nodeType);
            results.push("el has insertBefore=" + (typeof el.insertBefore));
            results.push("el.insertBefore.length=" + (el.insertBefore ? el.insertBefore.length : "N/A"));
            try { el.insertBefore(doctype, null); results.push("no error"); } catch(e) {
                results.push("error: " + e.message);
            }
            results.join(" | ");
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert!(s.contains("error:"), "Expected error but got: {}", s);
    }

    #[test]
    fn text_node_append_child_throws_hierarchy_request_error() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt
            .eval(
                r#"
            var text = document.createTextNode("foo");
            var result = 'no error';
            try { text.appendChild(document.createElement("div")); } catch(e) {
                result = 'error: ' + e.message;
            }
            result;
        "#,
            )
            .unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert!(
            s.contains("HierarchyRequestError") || s.contains("error:"),
            "Expected error but got: {}",
            s
        );
    }

    #[test]
    fn element_remove_with_siblings_via_engine_harness() {
        use crate::Engine;
        let mut engine = Engine::new();
        // Mimic the WPT pattern: test() wraps each check in try/catch
        let html = r#"<!DOCTYPE html>
<html><body>
<script>
var debug_log = [];
function test(fn, name) {
    try {
        fn();
        debug_log.push("PASS: " + name);
    } catch(e) {
        debug_log.push("FAIL: " + name + ": " + e.message);
    }
}
function assert_equals(a, b, msg) {
    if (a !== b) throw new Error(msg || "assert_equals: " + a + " !== " + b);
}
function assert_array_equals(a, b, msg) {
    var aLen = a ? a.length : undefined;
    var bLen = b ? b.length : undefined;
    if (aLen === undefined || bLen === undefined || aLen !== bLen) {
        throw new Error(msg || "assert_array_equals: length mismatch (" + aLen + " vs " + bLen + ")");
    }
    for (var i = 0; i < aLen; i++) {
        if (a[i] !== b[i]) throw new Error(msg || "assert_array_equals: index " + i);
    }
}
function assert_true(val, msg) {
    if (val !== true) throw new Error(msg || "assert_true: got " + val);
}

var node = document.createElement("div");
var parent = document.createElement("div");

test(function() {
    assert_true("remove" in node);
    assert_equals(typeof node.remove, "function");
    assert_equals(node.remove.length, 0);
}, "element should support remove()");

test(function() {
    assert_equals(node.parentNode, null, "Node should not have a parent");
    assert_equals(node.remove(), undefined);
    assert_equals(node.parentNode, null, "Removed new node should not have a parent");
}, "remove() should work if element doesn't have a parent");

test(function() {
    assert_equals(node.parentNode, null, "Node should not have a parent");
    parent.appendChild(node);
    assert_equals(node.parentNode, parent, "Appended node should have a parent");
    assert_equals(node.remove(), undefined);
    assert_equals(node.parentNode, null, "Removed node should not have a parent");
    assert_array_equals(parent.childNodes, [], "Parent should not have children");
}, "remove() should work if element does have a parent");

test(function() {
    assert_equals(node.parentNode, null, "Node should not have a parent");
    var before = parent.appendChild(document.createComment("before"));
    parent.appendChild(node);
    var after = parent.appendChild(document.createComment("after"));
    assert_equals(node.parentNode, parent, "Appended node should have a parent");
    assert_equals(node.remove(), undefined);
    assert_equals(node.parentNode, null, "Removed node should not have a parent");
    assert_array_equals(parent.childNodes, [before, after], "Parent should have two children left");
}, "remove() should work if element does have a parent and siblings");

window.__debug = debug_log.join("\n");
</script>
</body></html>"#;
        let _errors = engine.load_html_with_scripts_lossy(html, &std::collections::HashMap::new());
        let debug = engine.eval_js("window.__debug").unwrap_or_default();
        eprintln!("{}", debug);
        assert!(!debug.contains("FAIL"), "Element-remove harness test:\n{}", debug);
    }
}
