use boa_engine::JsObject;

use super::super::element::JsElement;
use crate::dom::{DomTree, NodeData, NodeId};
use crate::js::realm_state;
use std::cell::RefCell;
use std::rc::Rc;

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
    // Iterative DFS with explicit stack of (src_id, Option<dst_parent_id>).
    // None means this is the root node whose new_id we return.
    let mut stack: Vec<(NodeId, Option<NodeId>)> = vec![(src_id, None)];
    let mut root_new_id: Option<NodeId> = None;

    while let Some((current_src, dst_parent)) = stack.pop() {
        // Borrow src_tree, clone needed data, then drop borrow before mutating dst_tree
        let (new_id, child_ids) = {
            let src = src_tree.borrow();
            let node = src.get_node(current_src);
            let child_ids: Vec<NodeId> = node.children.clone();
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
                    drop(src);
                    dst_tree.borrow_mut().create_element_ns(&tag, attrs, &ns)
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
                    drop(src);
                    dst_tree.borrow_mut().create_document_fragment()
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
            (new_id, child_ids)
        };

        // Append to parent if this isn't the root
        if let Some(parent_id) = dst_parent {
            dst_tree.borrow_mut().append_child(parent_id, new_id);
        } else {
            root_new_id = Some(new_id);
        }

        mapping.push((current_src, new_id));

        // Push children in reverse order for pre-order DFS
        for &child_id in child_ids.iter().rev() {
            stack.push((child_id, Some(new_id)));
        }
    }

    root_new_id.expect("adopt_node_recursive called with empty stack")
}
