use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    Context, JsError, JsNativeError, JsResult, JsValue,
};

use crate::dom::{DomTree, NodeData, NodeId};
use super::element::{JsElement, get_or_create_js_element};
use std::cell::RefCell;
use std::rc::Rc;

pub(crate) fn register_mutation(class: &mut ClassBuilder) -> JsResult<()> {
    class.method(js_string!("insertBefore"), 2, NativeFunction::from_fn_ptr(insert_before));
    class.method(js_string!("replaceChild"), 2, NativeFunction::from_fn_ptr(replace_child));
    class.method(js_string!("removeChild"), 1, NativeFunction::from_fn_ptr(remove_child));
    class.method(js_string!("cloneNode"), 1, NativeFunction::from_fn_ptr(clone_node));
    class.method(js_string!("append"), 0, NativeFunction::from_fn_ptr(append));
    class.method(js_string!("prepend"), 0, NativeFunction::from_fn_ptr(prepend));
    class.method(js_string!("replaceChildren"), 0, NativeFunction::from_fn_ptr(replace_children));
    class.method(js_string!("before"), 0, NativeFunction::from_fn_ptr(child_node_before));
    class.method(js_string!("after"), 0, NativeFunction::from_fn_ptr(child_node_after));
    class.method(js_string!("replaceWith"), 0, NativeFunction::from_fn_ptr(child_node_replace_with));
    class.method(js_string!("insertAdjacentElement"), 2, NativeFunction::from_fn_ptr(insert_adjacent_element));
    class.method(js_string!("insertAdjacentText"), 2, NativeFunction::from_fn_ptr(insert_adjacent_text));
    class.method(js_string!("normalize"), 0, NativeFunction::from_fn_ptr(normalize));
    Ok(())
}

fn insert_before(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: this is not an object").into()))?;
    let parent = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: this is not an Element").into()))?;
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    let new_node_arg = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: missing first argument").into()))?;
    let new_node_obj = new_node_arg
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: first argument is not an object").into()))?;
    let new_node = new_node_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: first argument is not an Element").into()))?;
    let new_node_id = new_node.node_id;

    let ref_arg = args.get(1).cloned().unwrap_or(JsValue::null());

    // Per spec: if new_node is a DocumentFragment, insert its children instead
    let is_fragment = matches!(tree.borrow().get_node(new_node_id).data, NodeData::DocumentFragment);

    if ref_arg.is_null() || ref_arg.is_undefined() {
        if is_fragment {
            let children: Vec<NodeId> = tree.borrow().get_node(new_node_id).children.clone();
            for frag_child in children {
                tree.borrow_mut().append_child(parent_id, frag_child);
            }
        } else {
            tree.borrow_mut().append_child(parent_id, new_node_id);
        }
    } else {
        let ref_obj = ref_arg
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: second argument is not an object or null").into()))?;
        let ref_el = ref_obj
            .downcast_ref::<JsElement>()
            .ok_or_else(|| JsError::from_opaque(js_string!("insertBefore: second argument is not an Element").into()))?;
        let ref_id = ref_el.node_id;
        if is_fragment {
            let children: Vec<NodeId> = tree.borrow().get_node(new_node_id).children.clone();
            for frag_child in children {
                tree.borrow_mut().insert_child_before(parent_id, frag_child, ref_id);
            }
        } else {
            tree.borrow_mut().insert_child_before(parent_id, new_node_id, ref_id);
        }
    }

    Ok(new_node_arg.clone())
}

fn replace_child(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: this is not an object").into()))?;
    let parent = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: this is not an Element").into()))?;
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    let new_child_arg = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: missing first argument").into()))?;
    let new_child_obj = new_child_arg
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: first argument is not an object").into()))?;
    let new_child = new_child_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: first argument is not an Element").into()))?;
    let new_child_id = new_child.node_id;

    let old_child_arg = args
        .get(1)
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: missing second argument").into()))?;
    let old_child_obj = old_child_arg
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: second argument is not an object").into()))?;
    let old_child = old_child_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChild: second argument is not an Element").into()))?;
    let old_child_id = old_child.node_id;

    tree.borrow_mut().replace_child(parent_id, new_child_id, old_child_id);

    let js_obj = get_or_create_js_element(old_child_id, tree, ctx)?;
    Ok(js_obj.into())
}

fn remove_child(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: this is not an object").into()))?;
    let parent = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: this is not an Element").into()))?;
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    let child_arg = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: missing argument").into()))?;
    let child_obj = child_arg
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: argument is not an object").into()))?;
    let child = child_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: argument is not an Element").into()))?;
    let child_id = child.node_id;

    {
        let t = tree.borrow();
        let parent_node = t.get_node(parent_id);
        if !parent_node.children.contains(&child_id) {
            return Err(JsError::from_opaque(
                js_string!("removeChild: the node to be removed is not a child of this node").into(),
            ));
        }
    }

    tree.borrow_mut().remove_child(parent_id, child_id);

    let js_obj = get_or_create_js_element(child_id, tree, ctx)?;
    Ok(js_obj.into())
}

fn clone_node(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("cloneNode: this is not an object").into()))?;
    let el = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("cloneNode: this is not an Element").into()))?;
    let node_id = el.node_id;
    let tree = el.tree.clone();

    let deep = args
        .first()
        .map(|v| v.to_boolean())
        .unwrap_or(false);

    let cloned_id = tree.borrow_mut().clone_node(node_id, deep);

    let js_obj = get_or_create_js_element(cloned_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Convert variadic args (nodes or strings) into a Vec<NodeId>.
/// String arguments become new Text nodes; JsElement arguments yield their node_id.
fn convert_nodes_from_args(
    args: &[JsValue],
    tree: &Rc<RefCell<DomTree>>,
    ctx: &mut Context,
) -> JsResult<Vec<NodeId>> {
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
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("append: this is not an object").into()))?;
    let parent = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("append: this is not an Element").into()))?;
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    let node_ids = convert_nodes_from_args(args, &tree, ctx)?;
    for nid in node_ids {
        tree.borrow_mut().append_child(parent_id, nid);
    }
    Ok(JsValue::undefined())
}

/// ParentNode.prepend(...nodes)
fn prepend(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("prepend: this is not an object").into()))?;
    let parent = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("prepend: this is not an Element").into()))?;
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    let node_ids = convert_nodes_from_args(args, &tree, ctx)?;
    // Capture the original first child once before any insertions.
    // Insert each new node before this reference node so they appear in order.
    let original_first_child = tree.borrow().first_child(parent_id);
    for nid in node_ids {
        match original_first_child {
            Some(fc) => tree.borrow_mut().insert_child_before(parent_id, nid, fc),
            None => tree.borrow_mut().append_child(parent_id, nid),
        }
    }
    Ok(JsValue::undefined())
}

/// ParentNode.replaceChildren(...nodes)
fn replace_children(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChildren: this is not an object").into()))?;
    let parent = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceChildren: this is not an Element").into()))?;
    let parent_id = parent.node_id;
    let tree = parent.tree.clone();

    let node_ids = convert_nodes_from_args(args, &tree, ctx)?;
    tree.borrow_mut().clear_children(parent_id);
    for nid in node_ids {
        tree.borrow_mut().append_child(parent_id, nid);
    }
    Ok(JsValue::undefined())
}

/// ChildNode.before(...nodes)
fn child_node_before(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("before: this is not an object").into()))?;
    let el = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("before: this is not an Element").into()))?;
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

    for &nid in &node_ids {
        tree.borrow_mut().remove_from_parent(nid);
    }

    let reference = match viable_prev {
        Some(prev_id) => tree.borrow().next_sibling(prev_id),
        None => tree.borrow().first_child(parent_id),
    };

    for nid in node_ids {
        match reference {
            Some(ref_id) => tree.borrow_mut().insert_child_before(parent_id, nid, ref_id),
            None => tree.borrow_mut().append_child(parent_id, nid),
        }
    }

    Ok(JsValue::undefined())
}

/// ChildNode.after(...nodes)
fn child_node_after(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("after: this is not an object").into()))?;
    let el = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("after: this is not an Element").into()))?;
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
                for i in (pos + 1)..parent_children.len() {
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

    for &nid in &node_ids {
        tree.borrow_mut().remove_from_parent(nid);
    }

    for nid in node_ids {
        match viable_next {
            Some(ref_id) => tree.borrow_mut().insert_child_before(parent_id, nid, ref_id),
            None => tree.borrow_mut().append_child(parent_id, nid),
        }
    }

    Ok(JsValue::undefined())
}

/// ChildNode.replaceWith(...nodes)
fn child_node_replace_with(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceWith: this is not an object").into()))?;
    let el = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("replaceWith: this is not an Element").into()))?;
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
                for i in (pos + 1)..parent_children.len() {
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

    tree.borrow_mut().remove_from_parent(this_id);

    for &nid in &node_ids {
        tree.borrow_mut().remove_from_parent(nid);
    }

    for nid in node_ids {
        match viable_next {
            Some(ref_id) => tree.borrow_mut().insert_child_before(parent_id, nid, ref_id),
            None => tree.borrow_mut().append_child(parent_id, nid),
        }
    }

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
            .with_message(format!("The value provided ('{}') is not one of 'beforeBegin', 'afterBegin', 'beforeEnd', or 'afterEnd'.", pos))
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
                    .with_message("HierarchyRequestError: Cannot insert before the document element's parent is a Document")
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
                    .with_message("HierarchyRequestError: Cannot insert after the document element's parent is a Document")
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
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertAdjacentElement: this is not an object").into()))?;
    let el = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertAdjacentElement: this is not an Element").into()))?;
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

    let inserted = do_insert_adjacent(&tree, this_id, new_el_id, position)?;
    if inserted {
        Ok(new_el_arg.clone())
    } else {
        Ok(JsValue::null())
    }
}

/// Element.insertAdjacentText(position, text)
fn insert_adjacent_text(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertAdjacentText: this is not an object").into()))?;
    let el = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("insertAdjacentText: this is not an Element").into()))?;
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
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("normalize: this is not an object").into()))?;
    let el = this_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("normalize: this is not an Element").into()))?;
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
        tree.borrow_mut().append_child(doc_id, nid);
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
        match original_first_child {
            Some(fc) => tree.borrow_mut().insert_child_before(doc_id, nid, fc),
            None => tree.borrow_mut().append_child(doc_id, nid),
        }
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
    tree.borrow_mut().clear_children(doc_id);
    for nid in node_ids {
        tree.borrow_mut().append_child(doc_id, nid);
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
                attributes.push(("id".to_string(), "parent".to_string()));
            }
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span_a).data {
                attributes.push(("id".to_string(), "a".to_string()));
            }
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span_b).data {
                attributes.push(("id".to_string(), "b".to_string()));
            }
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(span_c).data {
                attributes.push(("id".to_string(), "c".to_string()));
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
        rt.eval(r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            parent.insertBefore(newNode, b);
        "#).unwrap();
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
        rt.eval(r#"
            var parent = document.getElementById("parent");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            parent.insertBefore(newNode, null);
        "#).unwrap();
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
        rt.eval(r#"
            var parent = document.getElementById("parent");
            var a = document.getElementById("a");
            var c = document.getElementById("c");
            parent.insertBefore(a, c);
        "#).unwrap();
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
        rt.eval(r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            parent.replaceChild(newNode, b);
        "#).unwrap();
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        let children = &t.get_node(parent_id).children;
        assert_eq!(children.len(), 3);
        let new_id = t.get_element_by_id("new").unwrap();
        let a_id = t.get_element_by_id("a").unwrap();
        let c_id = t.get_element_by_id("c").unwrap();
        assert_eq!(children, &vec![a_id, new_id, c_id]);
        let b_id = t.get_element_by_id("b").unwrap();
        assert!(t.get_node(b_id).parent.is_none());
    }

    #[test]
    fn replace_child_detaches_new_child() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        rt.eval(r#"
            var parent = document.getElementById("parent");
            var a = document.getElementById("a");
            var b = document.getElementById("b");
            parent.replaceChild(a, b);
        "#).unwrap();
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
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var removed = parent.removeChild(b);
            removed.getAttribute("id");
        "#).unwrap();
        let id_str = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(id_str, "b");
        let t = tree.borrow();
        let parent_id = t.get_element_by_id("parent").unwrap();
        assert_eq!(t.get_node(parent_id).children.len(), 2);
        let b_id = t.get_element_by_id("b").unwrap();
        assert!(t.get_node(b_id).parent.is_none());
    }

    #[test]
    fn remove_child_on_non_child_returns_error() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var body = document.body;
            var a = document.getElementById("a");
            try { body.removeChild(a); "no error"; } catch(e) { "error"; }
        "#).unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "error");
    }

    #[test]
    fn clone_node_shallow_copy() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(false);
            clone.hasChildNodes();
        "#).unwrap();
        assert!(!result.to_boolean());
    }

    #[test]
    fn clone_node_deep_copy_with_children() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(true);
            clone.childNodes.length;
        "#).unwrap();
        let length = result.to_i32(&mut rt.context).unwrap();
        assert_eq!(length, 3);
    }

    #[test]
    fn clone_node_preserves_attributes() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(false);
            clone.getAttribute("id");
        "#).unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "parent");
    }

    #[test]
    fn clone_node_has_no_parent() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var clone = parent.cloneNode(true);
            clone.parentNode === null;
        "#).unwrap();
        assert!(result.to_boolean());
    }

    #[test]
    fn insert_before_returns_new_node() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            newNode.setAttribute("id", "new");
            var returned = parent.insertBefore(newNode, b);
            returned.getAttribute("id");
        "#).unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "new");
    }

    #[test]
    fn replace_child_returns_old_child() {
        let tree = make_mutation_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));
        let result = rt.eval(r#"
            var parent = document.getElementById("parent");
            var b = document.getElementById("b");
            var newNode = document.createElement("p");
            var returned = parent.replaceChild(newNode, b);
            returned.getAttribute("id");
        "#).unwrap();
        let s = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(s, "b");
    }

}
