use boa_engine::{
    class::ClassBuilder,
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::Attribute,
    Context, JsError, JsResult, JsValue,
};

use crate::dom::{NodeData, NodeId};
use super::element::{JsElement, DOM_TREE, get_or_create_js_element};


// ---------------------------------------------------------------------------
// Node information properties
// ---------------------------------------------------------------------------

/// Native getter for element.nodeType
/// Returns: 1 (ELEMENT_NODE), 3 (TEXT_NODE), 8 (COMMENT_NODE), 9 (DOCUMENT_NODE)
fn get_node_type(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("nodeType getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("nodeType getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);

    let node_type = match &node.data {
        NodeData::Element { .. } => 1,
        NodeData::Attr { .. } => 2,
        NodeData::Text { .. } => 3,
        NodeData::ProcessingInstruction { .. } => 7,
        NodeData::Comment { .. } => 8,
        NodeData::Document => 9,
        NodeData::Doctype { .. } => 10,
        NodeData::DocumentFragment => 11,
    };

    Ok(JsValue::from(node_type))
}

/// Native getter for element.nodeName
/// Returns: tagName for elements (uppercase for HTML namespace, as-is for others),
/// "#text" for text, "#comment" for comment, "#document" for document
fn get_node_name(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("nodeName getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("nodeName getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);

    let node_name = match &node.data {
        NodeData::Element { tag_name, namespace, .. } => {
            // Per spec: only uppercase when element is in an HTML document AND has XHTML namespace
            if namespace == "http://www.w3.org/1999/xhtml" && tree.is_html_document() {
                tag_name.to_uppercase()
            } else {
                tag_name.clone()
            }
        }
        NodeData::Attr { local_name, prefix, .. } => {
            // Per spec: Attr.name is the qualified name (prefix:localName or just localName)
            if prefix.is_empty() {
                local_name.clone()
            } else {
                format!("{}:{}", prefix, local_name)
            }
        }
        NodeData::Text { .. } => "#text".to_string(),
        NodeData::Comment { .. } => "#comment".to_string(),
        NodeData::Document => "#document".to_string(),
        NodeData::Doctype { name, .. } => name.clone(),
        NodeData::ProcessingInstruction { target, .. } => target.clone(),
        NodeData::DocumentFragment => "#document-fragment".to_string(),
    };

    Ok(JsValue::from(js_string!(node_name)))
}

/// Native getter for element.tagName
/// Returns: tag name (uppercase for HTML namespace, as-is for others) for elements, undefined for non-elements
fn get_tag_name(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("tagName getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("tagName getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);

    match &node.data {
        NodeData::Element { tag_name, namespace, .. } => {
            // Per spec: only uppercase when element is in an HTML document AND has XHTML namespace
            if namespace == "http://www.w3.org/1999/xhtml" && tree.is_html_document() {
                Ok(JsValue::from(js_string!(tag_name.to_uppercase())))
            } else {
                Ok(JsValue::from(js_string!(tag_name.clone())))
            }
        }
        _ => Ok(JsValue::undefined()),
    }
}

/// Native getter for element.nodeValue
/// Returns: text content for Text/Comment nodes, null for Element/Document
fn get_node_value(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("nodeValue getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("nodeValue getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);

    match &node.data {
        NodeData::Text { content } => Ok(JsValue::from(js_string!(content.clone()))),
        NodeData::Comment { content } => Ok(JsValue::from(js_string!(content.clone()))),
        NodeData::ProcessingInstruction { data, .. } => Ok(JsValue::from(js_string!(data.clone()))),
        NodeData::Attr { value, .. } => Ok(JsValue::from(js_string!(value.clone()))),
        NodeData::Element { .. } | NodeData::Document | NodeData::Doctype { .. } | NodeData::DocumentFragment => Ok(JsValue::null()),
    }
}

/// Native setter for element.nodeValue
/// Per spec:
/// - Text, Comment: set data (null converts to "")
/// - Element, Document, DocumentType, DocumentFragment: no-op
fn set_node_value(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("nodeValue setter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("nodeValue setter: `this` is not an Element").into()))?;

    {
        let tree = el.tree.borrow();
        let node = tree.get_node(el.node_id);
        // No-op for Element, Document, DocumentType, DocumentFragment
        if !matches!(node.data, NodeData::Text { .. } | NodeData::Comment { .. } | NodeData::ProcessingInstruction { .. } | NodeData::Attr { .. }) {
            return Ok(JsValue::undefined());
        }
    }

    let val = args.first().cloned().unwrap_or(JsValue::undefined());

    // Per spec: setting nodeValue to null converts to ""
    let data = if val.is_null() {
        String::new()
    } else {
        val.to_string(ctx)?.to_std_string_escaped()
    };

    // For Attr nodes, set value; for Text/Comment/PI, set character data
    {
        let tree = el.tree.borrow();
        if matches!(tree.get_node(el.node_id).data, NodeData::Attr { .. }) {
            drop(tree);
            if let NodeData::Attr { ref mut value, .. } = el.tree.borrow_mut().get_node_mut(el.node_id).data {
                *value = data;
            }
            return Ok(JsValue::undefined());
        }
    }

    el.tree.borrow_mut().character_data_set(el.node_id, &data);
    Ok(JsValue::undefined())
}

/// Native getter for element.innerText
/// Returns: the text content of the element (same as textContent for now)
fn get_inner_text(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("innerText getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("innerText getter: `this` is not an Element").into()))?;

    let text = el.tree.borrow().get_text_content(el.node_id);
    Ok(JsValue::from(js_string!(text)))
}

/// Native getter for node.ownerDocument
/// Returns the document that owns this node.
/// For nodes in the global tree, returns window.document.
/// For nodes in created documents, returns that document's node 0.
fn get_owner_document(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("ownerDocument getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("ownerDocument getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);

    // Per spec: Document nodes return null for ownerDocument
    if matches!(node.data, NodeData::Document) {
        return Ok(JsValue::null());
    }
    drop(tree);

    // Check if this node's tree is the global DOM_TREE
    let is_global = DOM_TREE.with(|cell| {
        let rc = cell.borrow();
        match rc.as_ref() {
            Some(global_tree) => std::rc::Rc::ptr_eq(&el.tree, global_tree),
            None => false,
        }
    });

    if is_global {
        // Return the global document object
        let global = ctx.global_object();
        let doc = global.get(js_string!("document"), ctx)?;
        Ok(doc)
    } else {
        // Return the Document node (node 0) of this node's tree
        let doc_obj = get_or_create_js_element(0, el.tree.clone(), ctx)?;
        Ok(doc_obj.into())
    }
}

/// Native getter for node.isConnected
/// Returns true if the node is in the document tree (has a path to the Document root)
fn get_is_connected(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("isConnected getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("isConnected getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    let mut current: NodeId = el.node_id;
    loop {
        let node = tree.get_node(current);
        if matches!(node.data, NodeData::Document) {
            return Ok(JsValue::from(true));
        }
        match node.parent {
            Some(parent_id) => current = parent_id,
            None => return Ok(JsValue::from(false)),
        }
    }
}

/// Native getter for element.namespaceURI
/// Returns: the namespace URI for Element nodes, null for others
fn get_namespace_uri(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("namespaceURI getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("namespaceURI getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);

    match &node.data {
        NodeData::Element { namespace, .. } => {
            if namespace.is_empty() {
                Ok(JsValue::null())
            } else {
                Ok(JsValue::from(js_string!(namespace.clone())))
            }
        }
        _ => Ok(JsValue::null()),
    }
}

/// Native getter for element.prefix
/// Returns: the namespace prefix for Element nodes, null for others
fn get_prefix(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("prefix getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("prefix getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);

    match &node.data {
        NodeData::Element { tag_name, .. } => {
            // If tag_name contains ':', the part before is the prefix
            if let Some(colon_pos) = tag_name.find(':') {
                let prefix = &tag_name[..colon_pos];
                Ok(JsValue::from(js_string!(prefix)))
            } else {
                Ok(JsValue::null())
            }
        }
        _ => Ok(JsValue::null()),
    }
}

/// Native getter for element.localName
/// Returns: the local name for Element nodes, null for others
fn get_local_name(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("localName getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("localName getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);

    match &node.data {
        NodeData::Element { tag_name, .. } => {
            // If tag_name contains ':', the part after is the local name
            let local = if let Some(colon_pos) = tag_name.find(':') {
                &tag_name[colon_pos + 1..]
            } else {
                tag_name.as_str()
            };
            Ok(JsValue::from(js_string!(local)))
        }
        _ => Ok(JsValue::null()),
    }
}

/// Native getter for element.attributes
/// Returns: an array-like NamedNodeMap object with name/value/prefix/namespaceURI for each attribute
fn get_attributes(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("attributes getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("attributes getter: `this` is not an Element").into()))?;

    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);

    match &node.data {
        NodeData::Element { attributes, .. } => {
            // Build a NamedNodeMap-like object: { length: N, 0: Attr, 1: Attr, ... }
            let attrs_obj = ObjectInitializer::new(ctx).build();
            let len = attributes.len();

            for (i, (name, value)) in attributes.iter().enumerate() {
                // Create an Attr-like object with name, value, prefix, namespaceURI, localName
                let attr_obj = ObjectInitializer::new(ctx).build();

                // Parse prefix and localName from the attribute name
                let (prefix, local_name, namespace_uri) = if let Some(colon_pos) = name.find(':') {
                    let pfx = &name[..colon_pos];
                    let local = &name[colon_pos + 1..];
                    // Common namespace prefixes
                    let ns = match pfx {
                        "xmlns" => "http://www.w3.org/2000/xmlns/",
                        "xml" => "http://www.w3.org/XML/1998/namespace",
                        "xlink" => "http://www.w3.org/1999/xlink",
                        _ => "",
                    };
                    (Some(pfx.to_string()), local.to_string(), ns.to_string())
                } else if name == "xmlns" {
                    (None, "xmlns".to_string(), "http://www.w3.org/2000/xmlns/".to_string())
                } else {
                    (None, name.clone(), String::new())
                };

                attr_obj.set(js_string!("name"), JsValue::from(js_string!(name.clone())), false, ctx)?;
                attr_obj.set(js_string!("value"), JsValue::from(js_string!(value.clone())), false, ctx)?;
                attr_obj.set(js_string!("localName"), JsValue::from(js_string!(local_name)), false, ctx)?;

                if let Some(pfx) = prefix {
                    attr_obj.set(js_string!("prefix"), JsValue::from(js_string!(pfx)), false, ctx)?;
                } else {
                    attr_obj.set(js_string!("prefix"), JsValue::null(), false, ctx)?;
                }

                if namespace_uri.is_empty() {
                    attr_obj.set(js_string!("namespaceURI"), JsValue::null(), false, ctx)?;
                } else {
                    attr_obj.set(js_string!("namespaceURI"), JsValue::from(js_string!(namespace_uri)), false, ctx)?;
                }

                // Set nodeType = 2 (ATTRIBUTE_NODE)
                attr_obj.set(js_string!("nodeType"), JsValue::from(2), false, ctx)?;
                // Set nodeName = name
                attr_obj.set(js_string!("nodeName"), JsValue::from(js_string!(name.clone())), false, ctx)?;
                // Set specified = true (always for DOM4)
                attr_obj.set(js_string!("specified"), JsValue::from(true), false, ctx)?;

                // Set by index
                attrs_obj.set(js_string!(i.to_string()), JsValue::from(attr_obj), false, ctx)?;
            }

            attrs_obj.set(js_string!("length"), JsValue::from(len as i32), false, ctx)?;
            Ok(JsValue::from(attrs_obj))
        }
        _ => {
            // Non-element nodes don't have attributes
            Ok(JsValue::undefined())
        }
    }
}

/// Getter for DocumentType.name (returns doctype name, empty string for non-Doctype)
fn get_doctype_name(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("name getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("name getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);
    match &node.data {
        NodeData::Doctype { name, .. } => Ok(JsValue::from(js_string!(name.clone()))),
        _ => Ok(JsValue::from(js_string!(""))),
    }
}

/// Getter for DocumentType.publicId
fn get_doctype_public_id(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("publicId getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("publicId getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);
    match &node.data {
        NodeData::Doctype { public_id, .. } => Ok(JsValue::from(js_string!(public_id.clone()))),
        _ => Ok(JsValue::from(js_string!(""))),
    }
}

/// Getter for DocumentType.systemId
fn get_doctype_system_id(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("systemId getter: `this` is not an object").into()))?;
    let el = obj
        .downcast_ref::<JsElement>()
        .ok_or_else(|| JsError::from_opaque(js_string!("systemId getter: `this` is not an Element").into()))?;
    let tree = el.tree.borrow();
    let node = tree.get_node(el.node_id);
    match &node.data {
        NodeData::Doctype { system_id, .. } => Ok(JsValue::from(js_string!(system_id.clone()))),
        _ => Ok(JsValue::from(js_string!(""))),
    }
}

/// Register all node info getters on the Element class
pub(crate) fn register_node_info(class: &mut ClassBuilder) -> JsResult<()> {
    let realm = class.context().realm().clone();

    // nodeType (read-only)
    let getter = NativeFunction::from_fn_ptr(get_node_type);
    class.accessor(
        js_string!("nodeType"),
        Some(getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // nodeName (read-only)
    let getter = NativeFunction::from_fn_ptr(get_node_name);
    class.accessor(
        js_string!("nodeName"),
        Some(getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // tagName (read-only)
    let getter = NativeFunction::from_fn_ptr(get_tag_name);
    class.accessor(
        js_string!("tagName"),
        Some(getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // nodeValue (getter + setter)
    let getter = NativeFunction::from_fn_ptr(get_node_value);
    let setter = NativeFunction::from_fn_ptr(set_node_value);
    class.accessor(
        js_string!("nodeValue"),
        Some(getter.to_js_function(&realm)),
        Some(setter.to_js_function(&realm)),
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // innerText (read-only for now)
    let getter = NativeFunction::from_fn_ptr(get_inner_text);
    class.accessor(
        js_string!("innerText"),
        Some(getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // ownerDocument (read-only)
    let getter = NativeFunction::from_fn_ptr(get_owner_document);
    class.accessor(
        js_string!("ownerDocument"),
        Some(getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // isConnected (read-only)
    let getter = NativeFunction::from_fn_ptr(get_is_connected);
    class.accessor(
        js_string!("isConnected"),
        Some(getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // namespaceURI (read-only)
    let getter = NativeFunction::from_fn_ptr(get_namespace_uri);
    class.accessor(
        js_string!("namespaceURI"),
        Some(getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // prefix (read-only)
    let getter = NativeFunction::from_fn_ptr(get_prefix);
    class.accessor(
        js_string!("prefix"),
        Some(getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // localName (read-only)
    let getter = NativeFunction::from_fn_ptr(get_local_name);
    class.accessor(
        js_string!("localName"),
        Some(getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // attributes (read-only, NamedNodeMap-like)
    let getter = NativeFunction::from_fn_ptr(get_attributes);
    class.accessor(
        js_string!("attributes"),
        Some(getter.to_js_function(&realm)),
        None,
        Attribute::CONFIGURABLE | Attribute::NON_ENUMERABLE,
    );

    // Node type constants on the prototype (so instances inherit them)
    class.property(js_string!("ELEMENT_NODE"), JsValue::from(1), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("ATTRIBUTE_NODE"), JsValue::from(2), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("TEXT_NODE"), JsValue::from(3), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("CDATA_SECTION_NODE"), JsValue::from(4), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("ENTITY_REFERENCE_NODE"), JsValue::from(5), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("ENTITY_NODE"), JsValue::from(6), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("PROCESSING_INSTRUCTION_NODE"), JsValue::from(7), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("COMMENT_NODE"), JsValue::from(8), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("DOCUMENT_NODE"), JsValue::from(9), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("DOCUMENT_TYPE_NODE"), JsValue::from(10), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("DOCUMENT_FRAGMENT_NODE"), JsValue::from(11), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("NOTATION_NODE"), JsValue::from(12), Attribute::READONLY | Attribute::NON_ENUMERABLE);

    // Document position constants on the prototype
    class.property(js_string!("DOCUMENT_POSITION_DISCONNECTED"), JsValue::from(0x01), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("DOCUMENT_POSITION_PRECEDING"), JsValue::from(0x02), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("DOCUMENT_POSITION_FOLLOWING"), JsValue::from(0x04), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("DOCUMENT_POSITION_CONTAINS"), JsValue::from(0x08), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("DOCUMENT_POSITION_CONTAINED_BY"), JsValue::from(0x10), Attribute::READONLY | Attribute::NON_ENUMERABLE);
    class.property(js_string!("DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC"), JsValue::from(0x20), Attribute::READONLY | Attribute::NON_ENUMERABLE);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::js::JsRuntime;
    use crate::dom::{DomTree, NodeData};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Helper: build a DomTree with document > html > body > div#test
    fn make_test_tree() -> Rc<RefCell<DomTree>> {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let div = t.create_element("div");

            // Set id="test" on the div
            if let NodeData::Element { ref mut attributes, .. } = t.get_node_mut(div).data {
                attributes.push(("id".to_string(), "test".to_string()));
            }

            // Add text content to div
            let text = t.create_text("Hello World");
            t.append_child(div, text);

            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
            t.append_child(body, div);
        }
        tree
    }

    #[test]
    fn node_type_returns_element_node() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.getElementById("test").nodeType"#).unwrap();
        let node_type = result.as_number().expect("nodeType should be a number");
        assert_eq!(node_type, 1.0, "ELEMENT_NODE should be 1");
    }

    #[test]
    fn node_name_returns_uppercase_tag() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.getElementById("test").nodeName"#).unwrap();
        let node_name = result.as_string().expect("nodeName should be a string");
        assert_eq!(node_name.to_std_string_escaped(), "DIV");
    }

    #[test]
    fn tag_name_returns_uppercase_tag() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.getElementById("test").tagName"#).unwrap();
        let tag_name = result.as_string().expect("tagName should be a string");
        assert_eq!(tag_name.to_std_string_escaped(), "DIV");
    }

    #[test]
    fn node_value_returns_null_for_elements() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.getElementById("test").nodeValue"#).unwrap();
        assert!(result.is_null(), "nodeValue should be null for elements");
    }

    #[test]
    fn inner_text_returns_text_content() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.getElementById("test").innerText"#).unwrap();
        let text = result.as_string().expect("innerText should be a string");
        assert_eq!(text.to_std_string_escaped(), "Hello World");
    }

    #[test]
    fn node_type_for_element() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"document.getElementById("test").nodeType"#).unwrap();
        assert_eq!(result.as_number().unwrap(), 1.0, "Element should have nodeType 1");
    }

    #[test]
    fn node_name_matches_tag_name_for_elements() {
        let tree = make_test_tree();
        let mut rt = JsRuntime::new(Rc::clone(&tree));

        let result = rt.eval(r#"
            const el = document.getElementById("test");
            el.nodeName === el.tagName
        "#).unwrap();

        assert!(result.as_boolean().unwrap(), "nodeName should equal tagName for elements");
    }
}
