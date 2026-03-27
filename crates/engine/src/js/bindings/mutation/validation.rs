use boa_engine::JsResult;

use super::errors::{hierarchy_request_error, not_found_error};
use crate::dom::{DomTree, NodeData, NodeId};

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
pub(super) fn validate_pre_replace(
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
