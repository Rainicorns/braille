use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    property::PropertyDescriptor,
    Context, JsError, JsNativeError, JsResult, JsValue,
};

use crate::dom::NodeData;

use super::super::element::{get_or_create_js_element, JsElement};
use super::properties::add_document_properties_to_element;
use super::validation::validate_and_extract;
use crate::js::realm_state;

/// DOMImplementation.hasFeature() — per spec, always returns true.
pub(crate) fn domimpl_has_feature(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::from(true))
}

/// Native implementation of document.implementation.createDocumentType(name, publicId, systemId)
pub(crate) fn domimpl_create_document_type(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Validate doctype name per spec
    if !crate::dom::is_valid_doctype_name(&name) {
        let exc = super::super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    let public_id = args
        .get(1)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let system_id = args
        .get(2)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let tree = realm_state::dom_tree(ctx);

    let node_id = tree.borrow_mut().create_doctype(&name, &public_id, &system_id);
    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.implementation.createHTMLDocument(title?)
/// Creates a new Document with basic structure: doctype, html, head, title, body
pub(crate) fn domimpl_create_html_document(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let title_text = args
        .first()
        .and_then(|v| {
            if v.is_undefined() {
                None
            } else {
                Some(v.to_string(ctx).map(|s| s.to_std_string_escaped()))
            }
        })
        .transpose()?;

    let _tree = realm_state::dom_tree(ctx);

    // Create a new DomTree for the new document
    let new_tree = Rc::new(RefCell::new(crate::dom::DomTree::new()));
    {
        let mut t = new_tree.borrow_mut();
        let doctype = t.create_doctype("html", "", "");
        let html = t.create_element("html");
        let head = t.create_element("head");
        let body = t.create_element("body");

        let doc = t.document();
        t.append_child(doc, doctype);
        t.append_child(doc, html);
        t.append_child(html, head);
        t.append_child(html, body);

        if let Some(ref title_str) = title_text {
            let title = t.create_element("title");
            let text = t.create_text(title_str);
            t.append_child(title, text);
            t.append_child(head, title);
        }
    }

    // Return the new document node as a JsElement (it's a Document node)
    let doc_id = new_tree.borrow().document();
    let js_obj = get_or_create_js_element(doc_id, new_tree.clone(), ctx)?;
    add_document_properties_to_element(&js_obj, new_tree, "text/html".to_string(), ctx)?;

    // Set contentType to "text/html" for HTML documents
    js_obj.define_property_or_throw(
        js_string!("contentType"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!("text/html")))
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    Ok(js_obj.into())
}

/// Native implementation of document.implementation.createDocument(namespace, qualifiedName, doctype)
pub(crate) fn domimpl_create_document(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    if args.len() < 2 {
        return Err(JsNativeError::typ()
            .with_message("Failed to execute 'createDocument' on 'DOMImplementation': 2 arguments required")
            .into());
    }
    let namespace = args
        .first()
        .map(|v| {
            if v.is_null() || v.is_undefined() {
                Ok(String::new())
            } else {
                v.to_string(ctx).map(|s| s.to_std_string_escaped())
            }
        })
        .transpose()?
        .unwrap_or_default();

    let qualified_name = args
        .get(1)
        .map(|v| {
            if v.is_null() {
                Ok(String::new())
            } else {
                v.to_string(ctx).map(|s| s.to_std_string_escaped())
            }
        })
        .transpose()?
        .unwrap_or_default();

    // Validate and extract per DOM spec (must happen before creating the tree)
    let (validated_ns, _prefix, _local_name) = validate_and_extract(&namespace, &qualified_name, ctx)?;

    // Handle the optional doctype argument (3rd arg)
    // Per spec, must be null, undefined, or a DocumentType node — anything else is TypeError.
    // We preserve the JS object reference so that `doc.doctype === doctype` holds after adoption.
    let doctype_arg: Option<(Rc<RefCell<crate::dom::DomTree>>, crate::dom::NodeId, boa_engine::JsObject)> = match args.get(2) {
        None => None,
        Some(v) if v.is_null() || v.is_undefined() => None,
        Some(v) => {
            let obj = v
                .as_object()
                .ok_or_else(|| JsNativeError::typ().with_message("Value provided is not a DocumentType"))?;
            let el = obj
                .downcast_ref::<JsElement>()
                .ok_or_else(|| JsNativeError::typ().with_message("Value provided is not a DocumentType"))?;
            let is_doctype = matches!(el.tree.borrow().get_node(el.node_id).data, NodeData::Doctype { .. });
            if is_doctype {
                Some((el.tree.clone(), el.node_id, obj.clone()))
            } else {
                return Err(JsNativeError::typ()
                    .with_message("Value provided is not a DocumentType")
                    .into());
            }
        }
    };

    // Create a new DomTree for the new document (XML, not HTML)
    let new_tree = Rc::new(RefCell::new(crate::dom::DomTree::new_xml()));

    // If doctype was provided, adopt it from its source tree into the new tree.
    // This preserves JS object identity so `doc.doctype === originalDoctype` holds.
    if let Some((ref src_tree, src_id, ref doctype_obj)) = doctype_arg {
        let adopted_id = if Rc::ptr_eq(src_tree, &new_tree) {
            // Same tree (unlikely but handle it): just remove from parent and reuse
            new_tree.borrow_mut().remove_from_parent(src_id);
            src_id
        } else {
            // Cross-tree adoption: move the node and update caches
            let (adopted_id, mapping) = super::super::mutation::adopt_node_with_mapping(src_tree, src_id, &new_tree);
            super::super::mutation::update_node_cache_for_adoption_mapping(src_tree, &new_tree, &mapping, ctx);
            // Update the doctype JS object directly to point to the new tree/node
            let mut el_mut = doctype_obj.downcast_mut::<JsElement>().unwrap();
            el_mut.node_id = adopted_id;
            el_mut.tree = new_tree.clone();
            drop(el_mut);
            adopted_id
        };
        let doc = new_tree.borrow().document();
        new_tree.borrow_mut().append_child(doc, adopted_id);
    }

    {
        // If qualified name is non-empty, create a document element
        if !qualified_name.is_empty() {
            let mut t = new_tree.borrow_mut();
            let doc = t.document();
            let ns_ref = if validated_ns.is_empty() { "" } else { &validated_ns };
            let elem = t.create_element_ns(&qualified_name, Vec::new(), ns_ref);
            t.append_child(doc, elem);
        }
    }

    // Compute contentType based on namespace (needed by both createElement and the property)
    let content_type = match validated_ns.as_str() {
        "http://www.w3.org/1999/xhtml" => "application/xhtml+xml",
        "http://www.w3.org/2000/svg" => "image/svg+xml",
        _ => "application/xml",
    };

    let doc_id = new_tree.borrow().document();
    let js_obj = get_or_create_js_element(doc_id, new_tree.clone(), ctx)?;

    // Per DOM spec, createDocument returns an XMLDocument (subinterface of Document).
    // Override the prototype from Document.prototype to XMLDocument.prototype so that
    // `doc instanceof XMLDocument` returns true.
    if let Some(ref p) = realm_state::dom_prototypes(ctx) {
        if let Some(ref xml_proto) = p.xml_document_proto {
            js_obj.set_prototype(Some(xml_proto.clone()));
        }
    }

    add_document_properties_to_element(&js_obj, new_tree, content_type.to_string(), ctx)?;

    // Set contentType property on the document object
    js_obj.define_property_or_throw(
        js_string!("contentType"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!(content_type)))
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    Ok(js_obj.into())
}
