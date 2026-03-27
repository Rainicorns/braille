use boa_engine::{
    class::ClassBuilder, js_string, native_function::NativeFunction, Context, JsError, JsNativeError, JsResult, JsValue,
};

use super::super::element::{get_or_create_js_element, JsElement};
use super::adoption::{adopt_node, update_node_cache_after_adoption};
use super::errors::{hierarchy_request_error, not_found_error};
use super::operations::{
    capture_insert_state, do_insert, do_replace, fire_insert_records, fire_range_removal_for_move, RemovalInfo,
};
use super::validation::{validate_pre_insert, validate_pre_replace};
use crate::dom::{DomTree, NodeData, NodeId};
use std::cell::RefCell;
use std::rc::Rc;

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

    // Update live ranges and iterators for the removal of old_child (must happen before do_replace)
    {
        let t = tree.borrow();
        let parent_children = &t.get_node(parent_id).children;
        if let Some(old_idx) = parent_children.iter().position(|&c| c == old_child_id) {
            super::super::range::update_ranges_for_remove(ctx, parent_id, old_idx, old_child_id, &t);
        }
        super::super::node_iterator::update_iterators_for_removal(ctx, old_child_id, &t);
    }

    do_replace(&tree, parent_id, new_child_id, old_child_id);

    // Update live ranges for the insertion of new child(ren)
    if !added_ids.is_empty() {
        let t = tree.borrow();
        let parent_children = &t.get_node(parent_id).children;
        if let Some(first_idx) = parent_children.iter().position(|&c| c == added_ids[0]) {
            drop(t);
            super::super::range::update_ranges_for_insert(ctx, parent_id, first_idx, added_ids.len());
        }
    }

    // Queue removal of new_child from old parent (if moved)
    if let Some((old_pid, old_prev, old_next)) = removal_info {
        super::super::mutation_observer::queue_childlist_mutation(
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
    super::super::mutation_observer::queue_childlist_mutation(
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

    // Update live range boundaries and iterators before the actual removal
    {
        let t = tree.borrow();
        super::super::range::update_ranges_for_remove(ctx, parent_id, old_index, child_id, &t);
        super::super::node_iterator::update_iterators_for_removal(ctx, child_id, &t);
    }

    tree.borrow_mut().remove_child(parent_id, child_id);

    super::super::mutation_observer::queue_childlist_mutation(
        ctx,
        &tree,
        parent_id,
        vec![],
        vec![child_id],
        prev_sib,
        next_sib,
    );

    // Invoke disconnectedCallback for custom elements
    super::super::custom_elements::invoke_disconnected_callback(&tree, child_id, ctx);

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
                let cloned_child = super::clone_node_cross_tree(&tree.borrow(), child_id, &mut new_tree.borrow_mut());
                new_tree.borrow_mut().append_child(new_doc_id, cloned_child);
            }
        }

        let doc_id = new_tree.borrow().document();
        let js_obj = get_or_create_js_element(doc_id, new_tree.clone(), ctx)?;
        let content_type = if is_html { "text/html" } else { "application/xml" };
        super::super::document::add_document_properties_to_element(&js_obj, new_tree, content_type.to_string(), ctx)?;
        return Ok(js_obj.into());
    }

    let cloned_id = tree.borrow_mut().clone_node(node_id, deep);

    let js_obj = get_or_create_js_element(cloned_id, tree, ctx)?;
    Ok(js_obj.into())
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
    // Capture removed children for MutationObserver and update live ranges/iterators
    let removed_children: Vec<NodeId> = tree.borrow().get_node(parent_id).children.clone();
    // Update live ranges and iterators for each removed child (in reverse order to keep indices valid)
    for (idx, &child_id) in removed_children.iter().enumerate().rev() {
        let t = tree.borrow();
        super::super::range::update_ranges_for_remove(ctx, parent_id, idx, child_id, &t);
        super::super::node_iterator::update_iterators_for_removal(ctx, child_id, &t);
    }
    tree.borrow_mut().clear_children(parent_id);
    if !removed_children.is_empty() {
        super::super::mutation_observer::queue_childlist_mutation(ctx, &tree, parent_id, vec![], removed_children, None, None);
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
        super::super::mutation_observer::queue_childlist_mutation(
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
        super::super::mutation_observer::queue_childlist_mutation(
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
    super::super::mutation_observer::queue_childlist_mutation(
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
        .downcast_ref::<super::super::document::JsDocument>()
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
        .downcast_ref::<super::super::document::JsDocument>()
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
        .downcast_ref::<super::super::document::JsDocument>()
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
        .downcast_ref::<super::super::document::JsDocument>()
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
        super::super::mutation_observer::queue_childlist_mutation(ctx, &tree, doc_id, vec![], removed_children, None, None);
    }
    for nid in node_ids {
        let (added_ids, removal_info, prev_sib, next_sib) = capture_insert_state(&tree, doc_id, nid, None);
        fire_range_removal_for_move(ctx, &tree, &removal_info, nid);
        do_insert(&tree, doc_id, nid, None);
        fire_insert_records(ctx, &tree, doc_id, &added_ids, removal_info, prev_sib, next_sib);
    }
    Ok(JsValue::undefined())
}
