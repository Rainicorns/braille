use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{js_string, Context, JsError, JsResult, JsValue};

use crate::dom::{DomTree, NodeData};

use super::super::element::{get_or_create_js_element, JsElement};
use super::properties::add_document_properties_to_element;
use super::JsDocument;

/// document.cloneNode(deep) — clone the document into a new tree
pub(crate) fn document_clone_node(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("cloneNode: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("cloneNode: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    let deep = args.first().map(|v| v.to_boolean()).unwrap_or(false);

    let is_html = tree.borrow().is_html_document();
    let new_tree = Rc::new(RefCell::new(if is_html {
        crate::dom::DomTree::new()
    } else {
        crate::dom::DomTree::new_xml()
    }));

    if deep {
        let doc_node_id = tree.borrow().document();
        let child_ids: Vec<crate::dom::NodeId> = tree.borrow().get_node(doc_node_id).children.clone();
        let new_doc_id = new_tree.borrow().document();
        for child_id in child_ids {
            let cloned = super::super::mutation::clone_node_cross_tree(&tree.borrow(), child_id, &mut new_tree.borrow_mut());
            new_tree.borrow_mut().append_child(new_doc_id, cloned);
        }
    }

    let doc_id = new_tree.borrow().document();
    let js_obj = get_or_create_js_element(doc_id, new_tree.clone(), ctx)?;
    let content_type = if is_html { "text/html" } else { "application/xml" };
    add_document_properties_to_element(&js_obj, new_tree, content_type.to_string(), ctx)?;
    Ok(js_obj.into())
}

/// document.getRootNode() — document is always its own root
pub(crate) fn document_get_root_node(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    // Document node has no parent, so getRootNode() returns itself
    Ok(this.clone())
}

/// Native implementation of document.importNode(node, deep)
pub(crate) fn document_import_node(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("importNode: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("importNode: `this` is not document").into()))?;
    let target_tree = doc.tree.clone();

    let node_val = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("importNode: missing argument").into()))?;
    let node_obj = node_val
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("importNode: argument is not an object").into()))?;
    let node_el = node_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("importNode: argument is not a Node").into()))?;

    let source_tree = node_el.tree.clone();
    let source_id = node_el.node_id;

    // If node is a Document, throw NotSupportedError
    {
        let src = source_tree.borrow();
        if matches!(src.get_node(source_id).data, NodeData::Document) {
            return Err(JsError::from_opaque(
                js_string!("NotSupportedError: Cannot import a Document node").into(),
            ));
        }
    }

    let deep = args.get(1).map(|v| v.to_boolean()).unwrap_or(false);

    let new_id = if deep {
        target_tree
            .borrow_mut()
            .import_subtree(&source_tree.borrow(), source_id)
    } else {
        // Shallow import: clone just the node, no children
        let src = source_tree.borrow();
        let src_node = src.get_node(source_id);
        let mut t = target_tree.borrow_mut();
        match &src_node.data {
            NodeData::Element {
                tag_name,
                attributes,
                namespace,
            } => t.create_element_ns(tag_name, attributes.clone(), namespace),
            NodeData::Text { content } => t.create_text(content),
            NodeData::CDATASection { content } => t.create_cdata_section(content),
            NodeData::Comment { content } => t.create_comment(content),
            NodeData::Doctype {
                name,
                public_id,
                system_id,
            } => t.create_doctype(name, public_id, system_id),
            NodeData::DocumentFragment | NodeData::ShadowRoot { .. } => t.create_document_fragment(),
            NodeData::ProcessingInstruction { target, data } => t.create_processing_instruction(target, data),
            NodeData::Attr {
                local_name,
                namespace,
                prefix,
                value,
            } => t.create_attr(local_name, namespace, prefix, value),
            NodeData::Document => unreachable!("Document check above"),
        }
    };

    let js_obj = get_or_create_js_element(new_id, target_tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.adoptNode(node)
pub(crate) fn document_adopt_node(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("adoptNode: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("adoptNode: `this` is not document").into()))?;
    let target_tree = doc.tree.clone();

    let node_val = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("adoptNode: missing argument").into()))?;
    let node_obj = node_val
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("adoptNode: argument is not an object").into()))?;
    let node_el = node_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("adoptNode: argument is not a Node").into()))?;

    let source_tree = node_el.tree.clone();
    let source_id = node_el.node_id;

    // Step 1: If node is a Document, throw NotSupportedError
    {
        let src = source_tree.borrow();
        if matches!(src.get_node(source_id).data, NodeData::Document) {
            return Err(JsError::from_opaque(
                js_string!("NotSupportedError: Cannot adopt a Document node").into(),
            ));
        }
    }

    if Rc::ptr_eq(&source_tree, &target_tree) {
        // Same tree: just remove from parent
        target_tree.borrow_mut().remove_from_parent(source_id);
        // Return the same JS object
        Ok(node_val.clone())
    } else {
        // Different tree: use adopt_node_with_mapping to move node and all descendants
        drop(node_el);
        let (adopted_id, mapping) = super::super::mutation::adopt_node_with_mapping(&source_tree, source_id, &target_tree);
        // Update all cached JS objects (root + descendants) to point to new tree/nodes
        super::super::mutation::update_node_cache_for_adoption_mapping(&source_tree, &target_tree, &mapping, ctx);
        // Also update the root node_obj directly (in case it wasn't cached yet)
        let mut el_mut = node_obj.downcast_mut::<JsElement>().unwrap();
        el_mut.node_id = adopted_id;
        el_mut.tree = target_tree.clone();
        drop(el_mut);
        Ok(node_val.clone())
    }
}

/// Native implementation of document.appendChild(child)
pub(crate) fn document_append_child(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("appendChild: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("appendChild: `this` is not document").into()))?;
    let doc_id = doc.tree.borrow().document();

    let child_arg = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("appendChild: missing argument").into()))?;
    let child_obj = child_arg
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("appendChild: argument is not an object").into()))?;
    let child = child_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("appendChild: argument is not a Node").into()))?;
    let child_id = child.node_id;

    let is_fragment = matches!(doc.tree.borrow().get_node(child_id).data, NodeData::DocumentFragment | NodeData::ShadowRoot { .. });
    if is_fragment {
        let children: Vec<crate::dom::NodeId> = doc.tree.borrow().get_node(child_id).children.clone();
        for frag_child in children {
            doc.tree.borrow_mut().append_child(doc_id, frag_child);
        }
    } else {
        doc.tree.borrow_mut().append_child(doc_id, child_id);
    }

    Ok(child_arg.clone())
}

/// Native implementation of document.removeChild(child)
pub(crate) fn document_remove_child(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: `this` is not document").into()))?;

    let child_arg = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: missing argument").into()))?;
    let child_obj = child_arg
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: argument is not an object").into()))?;
    let child = child_obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeChild: argument is not a Node").into()))?;
    let child_id = child.node_id;
    let doc_id = doc.tree.borrow().document();

    doc.tree.borrow_mut().remove_child(doc_id, child_id);
    Ok(child_arg.clone())
}

/// Native implementation of document.contains(other)
/// Returns true if other is a descendant of the document (inclusive).
pub(crate) fn document_contains(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let this_obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("contains: `this` is not an object").into()))?;
    let doc = this_obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("contains: `this` is not document").into()))?;
    let doc_id = doc.tree.borrow().document();

    let other_val = match args.first() {
        Some(v) if !v.is_null() && !v.is_undefined() => v,
        _ => return Ok(JsValue::from(false)),
    };
    let other_obj = match other_val.as_object() {
        Some(o) => o,
        None => return Ok(JsValue::from(false)),
    };
    // Check if other is a JsDocument (e.g., document.contains(document))
    if let Some(other_doc) = other_obj.downcast_ref::<JsDocument>() {
        // document.contains(document) is true when same tree
        return Ok(JsValue::from(Rc::ptr_eq(&doc.tree, &other_doc.tree)));
    }

    let other_el = match other_obj.downcast_ref::<JsElement>() {
        Some(e) => e,
        None => return Ok(JsValue::from(false)),
    };
    let other_id = other_el.node_id;

    // If other is from a different tree, it can't be contained
    if !Rc::ptr_eq(&doc.tree, &other_el.tree) {
        return Ok(JsValue::from(false));
    }

    let tree = doc.tree.borrow();
    let mut current = other_id;
    loop {
        if current == doc_id {
            return Ok(JsValue::from(true));
        }
        match tree.get_node(current).parent {
            Some(parent_id) => current = parent_id,
            None => return Ok(JsValue::from(false)),
        }
    }
}
