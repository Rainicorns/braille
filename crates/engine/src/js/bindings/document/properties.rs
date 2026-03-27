use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    property::PropertyDescriptor,
    Context, JsError, JsObject, JsResult, JsValue,
};

use crate::dom::{DomTree, NodeData};

use super::super::element::{get_or_create_js_element, JsElement};
use super::creation::document_create_event;
use super::domimpl::{domimpl_create_document, domimpl_create_html_document, domimpl_has_feature};
use super::validation::validate_and_extract;
use crate::js::realm_state;

/// Add Document-like properties/methods onto a JsElement wrapping a Document node.
/// This allows objects from createHTMLDocument/createDocument to behave like Document.
pub(crate) fn add_document_properties_to_element(
    js_obj: &JsObject,
    new_tree: Rc<RefCell<DomTree>>,
    _content_type: String,
    ctx: &mut Context,
) -> JsResult<()> {
    let realm = ctx.realm().clone();

    // nodeType = 9
    js_obj.define_property_or_throw(
        js_string!("nodeType"),
        PropertyDescriptor::builder()
            .value(JsValue::from(9))
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // nodeName = "#document"
    js_obj.define_property_or_throw(
        js_string!("nodeName"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!("#document")))
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // documentElement getter
    let tree_for_de = new_tree.clone();
    let de_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let tree = tree_for_de.borrow();
            let doc_node = tree.get_node(tree.document());
            for &child_id in &doc_node.children {
                if matches!(tree.get_node(child_id).data, NodeData::Element { .. }) {
                    let tree_rc = tree_for_de.clone();
                    drop(tree);
                    let js_el = get_or_create_js_element(child_id, tree_rc, ctx2)?;
                    return Ok(js_el.into());
                }
            }
            Ok(JsValue::null())
        })
    };
    js_obj.define_property_or_throw(
        js_string!("documentElement"),
        PropertyDescriptor::builder()
            .get(de_getter.to_js_function(&realm))
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // doctype getter
    let tree_for_dt = new_tree.clone();
    let dt_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let tree = tree_for_dt.borrow();
            let doc_node = tree.get_node(tree.document());
            for &child_id in &doc_node.children {
                if matches!(tree.get_node(child_id).data, NodeData::Doctype { .. }) {
                    let tree_rc = tree_for_dt.clone();
                    drop(tree);
                    let js_el = get_or_create_js_element(child_id, tree_rc, ctx2)?;
                    return Ok(js_el.into());
                }
            }
            Ok(JsValue::null())
        })
    };
    js_obj.define_property_or_throw(
        js_string!("doctype"),
        PropertyDescriptor::builder()
            .get(dt_getter.to_js_function(&realm))
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // body getter
    let tree_for_body = new_tree.clone();
    let body_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let tree = tree_for_body.borrow();
            match tree.body() {
                Some(body_id) => {
                    let tree_rc = tree_for_body.clone();
                    drop(tree);
                    let js_el = get_or_create_js_element(body_id, tree_rc, ctx2)?;
                    Ok(js_el.into())
                }
                None => Ok(JsValue::null()),
            }
        })
    };
    js_obj.define_property_or_throw(
        js_string!("body"),
        PropertyDescriptor::builder()
            .get(body_getter.to_js_function(&realm))
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // defaultView getter — returns the window global object
    let dv_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let global = ctx2.global_object();
            let window = global.get(js_string!("window"), ctx2)?;
            Ok(window)
        })
    };
    js_obj.define_property_or_throw(
        js_string!("defaultView"),
        PropertyDescriptor::builder()
            .get(dv_getter.to_js_function(&realm))
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // createElement method — respects is_html_document for lowercasing and namespace
    let tree_for_ce = new_tree.clone();
    let ct_for_ce = _content_type.clone();
    let create_element = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let tag = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_else(|| "undefined".to_string());
            // Validate the element name per spec
            if !crate::dom::is_valid_element_name(&tag) {
                let exc = super::super::create_dom_exception(
                    ctx2,
                    "InvalidCharacterError",
                    "String contains an invalid character",
                    5,
                )?;
                return Err(JsError::from_opaque(exc.into()));
            }
            let is_html = tree_for_ce.borrow().is_html_document();
            let node_id = if is_html {
                // HTML doc: lowercase tag, HTML namespace (create_element default)
                tree_for_ce.borrow_mut().create_element(&tag.to_ascii_lowercase())
            } else if ct_for_ce == "application/xhtml+xml" {
                // XHTML doc: preserve case, XHTML namespace
                tree_for_ce
                    .borrow_mut()
                    .create_element_ns(&tag, vec![], "http://www.w3.org/1999/xhtml")
            } else {
                // XML doc: preserve case, null namespace
                tree_for_ce.borrow_mut().create_element_ns(&tag, vec![], "")
            };
            let js_el = get_or_create_js_element(node_id, tree_for_ce.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(
        js_string!("createElement"),
        create_element.to_js_function(&realm),
        false,
        ctx,
    )?;

    // createComment method
    let tree_for_cc = new_tree.clone();
    let create_comment = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let data = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let node_id = tree_for_cc.borrow_mut().create_comment(&data);
            let js_el = get_or_create_js_element(node_id, tree_for_cc.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(
        js_string!("createComment"),
        create_comment.to_js_function(&realm),
        false,
        ctx,
    )?;

    // createTextNode method
    let tree_for_ct = new_tree.clone();
    let create_text_node = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let text = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let node_id = tree_for_ct.borrow_mut().create_text(&text);
            let js_el = get_or_create_js_element(node_id, tree_for_ct.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(
        js_string!("createTextNode"),
        create_text_node.to_js_function(&realm),
        false,
        ctx,
    )?;

    // createDocumentFragment method
    let tree_for_cdf = new_tree.clone();
    let create_doc_frag = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let node_id = tree_for_cdf.borrow_mut().create_document_fragment();
            let js_el = get_or_create_js_element(node_id, tree_for_cdf.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(
        js_string!("createDocumentFragment"),
        create_doc_frag.to_js_function(&realm),
        false,
        ctx,
    )?;

    // createRange method
    let tree_for_cr = new_tree.clone();
    let create_range = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let doc_id = tree_for_cr.borrow().document();
            let range_obj = super::super::range::create_range(tree_for_cr.clone(), doc_id, ctx2)?;
            Ok(range_obj.into())
        })
    };
    js_obj.set(js_string!("createRange"), create_range.to_js_function(&realm), false, ctx)?;

    // createTreeWalker method
    super::super::tree_walker::register_create_tree_walker(js_obj, new_tree.clone(), ctx);

    // createNodeIterator method
    super::super::node_iterator::register_create_node_iterator(js_obj, new_tree.clone(), ctx);

    // createProcessingInstruction method
    let tree_for_cpi = new_tree.clone();
    let create_pi = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let target = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let data = args
                .get(1)
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let node_id = tree_for_cpi.borrow_mut().create_processing_instruction(&target, &data);
            let js_el = get_or_create_js_element(node_id, tree_for_cpi.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(
        js_string!("createProcessingInstruction"),
        create_pi.to_js_function(&realm),
        false,
        ctx,
    )?;

    // createAttribute method
    let tree_for_ca = new_tree.clone();
    let create_attr_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let local_name = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_else(|| "undefined".to_string());
            if local_name.is_empty() {
                return Err(JsError::from_opaque(
                    js_string!("InvalidCharacterError: The string contains invalid characters.").into(),
                ));
            }
            let local_name = if tree_for_ca.borrow().is_html_document() {
                local_name.to_ascii_lowercase()
            } else {
                local_name
            };
            let node_id = tree_for_ca.borrow_mut().create_attr(&local_name, "", "", "");
            let js_el = get_or_create_js_element(node_id, tree_for_ca.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(
        js_string!("createAttribute"),
        create_attr_fn.to_js_function(&realm),
        false,
        ctx,
    )?;

    // createAttributeNS method
    let tree_for_cans = new_tree.clone();
    let create_attr_ns_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let namespace = match args.first() {
                Some(v) if !v.is_null() && !v.is_undefined() => v.to_string(ctx2)?.to_std_string_escaped(),
                _ => String::new(),
            };
            let qualified_name = args
                .get(1)
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_else(|| "undefined".to_string());
            let (prefix, local_name) = if let Some(colon_pos) = qualified_name.find(':') {
                (
                    qualified_name[..colon_pos].to_string(),
                    qualified_name[colon_pos + 1..].to_string(),
                )
            } else {
                (String::new(), qualified_name)
            };
            let node_id = tree_for_cans
                .borrow_mut()
                .create_attr(&local_name, &namespace, &prefix, "");
            let js_el = get_or_create_js_element(node_id, tree_for_cans.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(
        js_string!("createAttributeNS"),
        create_attr_ns_fn.to_js_function(&realm),
        false,
        ctx,
    )?;

    // head getter
    let tree_for_head = new_tree.clone();
    let head_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let tree = tree_for_head.borrow();
            match tree.head() {
                Some(head_id) => {
                    let tree_rc = tree_for_head.clone();
                    drop(tree);
                    let js_el = get_or_create_js_element(head_id, tree_rc, ctx2)?;
                    Ok(js_el.into())
                }
                None => Ok(JsValue::null()),
            }
        })
    };
    js_obj.define_property_or_throw(
        js_string!("head"),
        PropertyDescriptor::builder()
            .get(head_getter.to_js_function(&realm))
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // implementation.createDocumentType method
    let tree_for_impl = new_tree.clone();
    let impl_create_dt = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let name = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            if !crate::dom::is_valid_doctype_name(&name) {
                let exc = super::super::create_dom_exception(
                    ctx2,
                    "InvalidCharacterError",
                    "String contains an invalid character",
                    5,
                )?;
                return Err(JsError::from_opaque(exc.into()));
            }
            let public_id = args
                .get(1)
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let system_id = args
                .get(2)
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let node_id = tree_for_impl.borrow_mut().create_doctype(&name, &public_id, &system_id);
            let js_el = get_or_create_js_element(node_id, tree_for_impl.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    let has_feature_fn = NativeFunction::from_fn_ptr(domimpl_has_feature);
    let implementation = boa_engine::object::ObjectInitializer::new(ctx)
        .function(impl_create_dt, js_string!("createDocumentType"), 3)
        .function(
            NativeFunction::from_fn_ptr(domimpl_create_html_document),
            js_string!("createHTMLDocument"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(domimpl_create_document),
            js_string!("createDocument"),
            2,
        )
        .function(has_feature_fn, js_string!("hasFeature"), 0)
        .build();
    if let Some(p) = realm_state::domimpl_proto(ctx) {
        implementation.set_prototype(Some(p));
    }
    js_obj.define_property_or_throw(
        js_string!("implementation"),
        PropertyDescriptor::builder()
            .value(JsValue::from(implementation))
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // importNode method
    let tree_for_import = new_tree.clone();
    let import_node_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
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
                tree_for_import
                    .borrow_mut()
                    .import_subtree(&source_tree.borrow(), source_id)
            } else {
                let src = source_tree.borrow();
                let src_node = src.get_node(source_id);
                let mut t = tree_for_import.borrow_mut();
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

            let js_el = get_or_create_js_element(new_id, tree_for_import.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(
        js_string!("importNode"),
        import_node_fn.to_js_function(&realm),
        false,
        ctx,
    )?;

    // adoptNode method
    let tree_for_adopt = new_tree.clone();
    let adopt_node_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
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

            // If node is a Document, throw NotSupportedError
            {
                let src = source_tree.borrow();
                if matches!(src.get_node(source_id).data, NodeData::Document) {
                    return Err(JsError::from_opaque(
                        js_string!("NotSupportedError: Cannot adopt a Document node").into(),
                    ));
                }
            }

            if Rc::ptr_eq(&source_tree, &tree_for_adopt) {
                // Same tree: just remove from parent
                tree_for_adopt.borrow_mut().remove_from_parent(source_id);
                Ok(node_val.clone())
            } else {
                // Different tree: use adopt_node_with_mapping to move node and all descendants
                drop(node_el);
                let (adopted_id, mapping) =
                    super::super::mutation::adopt_node_with_mapping(&source_tree, source_id, &tree_for_adopt);
                // Update all cached JS objects (root + descendants) to point to new tree/nodes
                super::super::mutation::update_node_cache_for_adoption_mapping(&source_tree, &tree_for_adopt, &mapping, ctx2);
                // Also update the root node_obj directly (in case it wasn't cached yet)
                let mut el_mut = node_obj.downcast_mut::<JsElement>().unwrap();
                el_mut.node_id = adopted_id;
                el_mut.tree = tree_for_adopt.clone();
                drop(el_mut);
                Ok(node_val.clone())
            }
        })
    };
    js_obj.set(
        js_string!("adoptNode"),
        adopt_node_fn.to_js_function(&realm),
        false,
        ctx,
    )?;

    // location = null (documents not associated with a browsing context)
    js_obj.define_property_or_throw(
        js_string!("location"),
        PropertyDescriptor::builder()
            .value(JsValue::null())
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // URL = "about:blank" (created documents always have this URL)
    js_obj.define_property_or_throw(
        js_string!("URL"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!("about:blank")))
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // documentURI = "about:blank" (alias for URL per spec)
    js_obj.define_property_or_throw(
        js_string!("documentURI"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!("about:blank")))
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // compatMode = "CSS1Compat" (created documents are always no-quirks mode)
    js_obj.define_property_or_throw(
        js_string!("compatMode"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!("CSS1Compat")))
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // characterSet = "UTF-8" (created documents default to UTF-8)
    js_obj.define_property_or_throw(
        js_string!("characterSet"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!("UTF-8")))
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // charset = "UTF-8" (legacy alias for characterSet)
    js_obj.define_property_or_throw(
        js_string!("charset"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!("UTF-8")))
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // inputEncoding = "UTF-8" (legacy alias for characterSet)
    js_obj.define_property_or_throw(
        js_string!("inputEncoding"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!("UTF-8")))
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // createCDATASection method — throws NotSupportedError on HTML documents per spec
    let tree_for_cdata = new_tree.clone();
    let is_html_for_cdata = tree_for_cdata.borrow().is_html_document();
    let cdata_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            if is_html_for_cdata {
                let exc = super::super::create_dom_exception(
                    ctx2,
                    "NotSupportedError",
                    "This method is not supported for HTML documents",
                    9,
                )?;
                return Err(JsError::from_opaque(exc.into()));
            }
            let data = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let node_id = tree_for_cdata.borrow_mut().create_cdata_section(&data);
            let js_el = get_or_create_js_element(node_id, tree_for_cdata.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(
        js_string!("createCDATASection"),
        cdata_fn.to_js_function(&realm),
        false,
        ctx,
    )?;

    // createEvent method
    js_obj.set(
        js_string!("createEvent"),
        NativeFunction::from_fn_ptr(document_create_event).to_js_function(&realm),
        false,
        ctx,
    )?;

    // getElementById — closure-based version that captures the tree
    let tree_for_gid = new_tree.clone();
    let gid_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let id = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let found = tree_for_gid.borrow().get_element_by_id(&id);
            match found {
                Some(node_id) => {
                    let js_obj = get_or_create_js_element(node_id, tree_for_gid.clone(), ctx2)?;
                    Ok(js_obj.into())
                }
                None => Ok(JsValue::null()),
            }
        })
    };
    js_obj.set(js_string!("getElementById"), gid_fn.to_js_function(&realm), false, ctx)?;

    Ok(())
}

/// Create a blank XML document (used by `new Document()` constructor).
/// Returns a JsElement-backed Document node with all Document-specific methods.
pub(crate) fn create_blank_xml_document(ctx: &mut Context) -> JsResult<JsValue> {
    let new_tree = Rc::new(RefCell::new(crate::dom::DomTree::new_xml()));
    let doc_id = new_tree.borrow().document();
    let js_obj = get_or_create_js_element(doc_id, new_tree.clone(), ctx)?;
    add_document_properties_to_element(&js_obj, new_tree.clone(), "application/xml".to_string(), ctx)?;

    // Set contentType to "application/xml" for XML documents
    js_obj.define_property_or_throw(
        js_string!("contentType"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!("application/xml")))
            .writable(false)
            .configurable(true)
            .enumerable(false)
            .build(),
        ctx,
    )?;

    // Add createCDATASection method
    let tree_for_cdata = new_tree.clone();
    let realm = ctx.realm().clone();
    let cdata_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let data = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let node_id = tree_for_cdata.borrow_mut().create_cdata_section(&data);
            let js_el = get_or_create_js_element(node_id, tree_for_cdata.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(
        js_string!("createCDATASection"),
        cdata_fn.to_js_function(&realm),
        false,
        ctx,
    )?;

    // Add createElementNS method
    let tree_for_cens = new_tree.clone();
    let create_element_ns_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let namespace = match args.first() {
                Some(v) if !v.is_null() && !v.is_undefined() => v.to_string(ctx2)?.to_std_string_escaped(),
                _ => String::new(),
            };
            let qualified_name = args
                .get(1)
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_else(|| "undefined".to_string());
            let (validated_ns, _prefix, _local_name) = validate_and_extract(&namespace, &qualified_name, ctx2)?;
            let ns = if validated_ns.is_empty() {
                ""
            } else {
                validated_ns.as_str()
            };
            let node_id = tree_for_cens
                .borrow_mut()
                .create_element_ns(&qualified_name, Vec::new(), ns);
            let js_el = get_or_create_js_element(node_id, tree_for_cens.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(
        js_string!("createElementNS"),
        create_element_ns_fn.to_js_function(&realm),
        false,
        ctx,
    )?;

    Ok(JsValue::from(js_obj))
}
