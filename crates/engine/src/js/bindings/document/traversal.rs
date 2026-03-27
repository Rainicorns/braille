use boa_engine::{js_string, Context, JsError, JsResult, JsValue};

use crate::dom::NodeData;

use super::super::element::get_or_create_js_element;
use super::JsDocument;

/// Native getter for document.body
pub(crate) fn document_get_body(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("body getter: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("body getter: `this` is not document").into()))?;
    let tree = doc.tree.borrow();
    match tree.body() {
        Some(body_id) => {
            let tree_rc = doc.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(body_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for document.head
pub(crate) fn document_get_head(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("head getter: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("head getter: `this` is not document").into()))?;
    let tree = doc.tree.borrow();
    match tree.head() {
        Some(head_id) => {
            let tree_rc = doc.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(head_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for document.title
pub(crate) fn document_get_title(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("title getter: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("title getter: `this` is not document").into()))?;
    let tree = doc.tree.borrow();
    let titles = tree.get_elements_by_tag_name("title");
    if let Some(&title_id) = titles.first() {
        let text = tree.get_text_content(title_id);
        Ok(JsValue::from(js_string!(text)))
    } else {
        Ok(JsValue::from(js_string!("")))
    }
}

/// Native setter for document.title
pub(crate) fn document_set_title(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("title setter: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("title setter: `this` is not document").into()))?;
    let text = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let mut tree = doc.tree.borrow_mut();
    let titles = tree.get_elements_by_tag_name("title");
    if let Some(&title_id) = titles.first() {
        tree.set_text_content(title_id, &text);
    } else {
        // Create <title> element if it doesn't exist
        let title_id = tree.create_element("title");
        tree.set_text_content(title_id, &text);
        // Try to append to <head> if it exists, otherwise to document
        if let Some(head_id) = tree.head() {
            tree.append_child(head_id, title_id);
        } else {
            let doc_id = tree.document();
            tree.append_child(doc_id, title_id);
        }
    }
    Ok(JsValue::undefined())
}

/// Native getter for document.documentElement
pub(crate) fn document_get_document_element(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("documentElement getter: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("documentElement getter: `this` is not document").into()))?;
    let tree = doc.tree.borrow();
    let doc_node = tree.get_node(tree.document());
    // documentElement is the first Element child of the Document node
    for &child_id in &doc_node.children {
        if matches!(tree.get_node(child_id).data, NodeData::Element { .. }) {
            let tree_rc = doc.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(child_id, tree_rc, ctx)?;
            return Ok(js_obj.into());
        }
    }
    Ok(JsValue::null())
}

/// Native getter for document.doctype
/// Returns the first Doctype child of the document, or null.
pub(crate) fn document_get_doctype(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("doctype getter: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("doctype getter: `this` is not document").into()))?;
    let tree = doc.tree.borrow();
    let doc_node = tree.get_node(tree.document());
    for &child_id in &doc_node.children {
        if matches!(tree.get_node(child_id).data, NodeData::Doctype { .. }) {
            let tree_rc = doc.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(child_id, tree_rc, ctx)?;
            return Ok(js_obj.into());
        }
    }
    Ok(JsValue::null())
}

/// Native getter for document.childNodes
pub(crate) fn document_get_child_nodes(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("childNodes getter: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("childNodes getter: `this` is not document").into()))?;
    let tree_rc = doc.tree.clone();
    let tree = tree_rc.borrow();
    let children = tree.children(tree.document());
    drop(tree);

    let arr = boa_engine::object::builtins::JsArray::new(ctx);
    for child_id in children {
        let js_obj = get_or_create_js_element(child_id, tree_rc.clone(), ctx)?;
        arr.push(js_obj, ctx)?;
    }
    Ok(arr.into())
}

/// Native getter for document.firstChild
pub(crate) fn document_get_first_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("firstChild getter: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("firstChild getter: `this` is not document").into()))?;
    let tree = doc.tree.borrow();
    match tree.first_child(tree.document()) {
        Some(child_id) => {
            let tree_rc = doc.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(child_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for document.lastChild
pub(crate) fn document_get_last_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("lastChild getter: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("lastChild getter: `this` is not document").into()))?;
    let tree = doc.tree.borrow();
    match tree.last_child(tree.document()) {
        Some(child_id) => {
            let tree_rc = doc.tree.clone();
            drop(tree);
            let js_obj = get_or_create_js_element(child_id, tree_rc, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}

/// Native getter for document.parentNode — always null
pub(crate) fn document_get_parent_node(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::null())
}

/// Native getter for document.parentElement — always null
pub(crate) fn document_get_parent_element(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::null())
}

/// Native method for document.hasChildNodes()
pub(crate) fn document_has_child_nodes(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("hasChildNodes: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("hasChildNodes: `this` is not document").into()))?;
    let tree = doc.tree.borrow();
    Ok(JsValue::from(!tree.children_ref(tree.document()).is_empty()))
}

/// Native implementation of document.getElementById(id)
pub(crate) fn document_get_element_by_id(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("getElementById: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("getElementById: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    let id = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let found = tree.borrow().get_element_by_id(&id);
    match found {
        Some(node_id) => {
            let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
            Ok(js_obj.into())
        }
        None => Ok(JsValue::null()),
    }
}
