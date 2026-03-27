use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::{
    js_string,
    property::PropertyDescriptor,
    Context, JsObject, JsResult, JsValue,
};

use crate::dom::node::ShadowRootMode;
use crate::dom::{DomTree, NodeData, NodeId};

use crate::js::realm_state;

use super::setup::{
    compile_parsed_inline_handlers, is_known_html_element, setup_attr_node_properties,
    setup_iframe_properties, setup_style_sheet, setup_template_content_getter,
};
use super::JsElement;

// ---------------------------------------------------------------------------
// NodeId -> JsObject cache (stored in RealmState)
// ---------------------------------------------------------------------------

/// Cache key is (tree_ptr, node_id) so nodes from different trees don't collide.
pub(crate) type NodeCache = HashMap<(usize, NodeId), JsObject>;

/// Lazily creates (or retrieves) the content document for an `<iframe>` element.
/// Returns the JsObject representing the iframe's content document.
pub(crate) fn ensure_iframe_content_doc(tree_ptr: usize, node_id: NodeId, ctx: &mut Context) -> JsResult<JsObject> {
    // Check if we already have a content doc for this iframe
    let existing = {
        let docs = realm_state::iframe_content_docs(ctx);
        let map = docs.borrow();
        map.get(&(tree_ptr, node_id)).cloned()
    };

    if let Some(ref existing_tree) = existing {
        // Already exists -- return its document JsObject from NODE_CACHE
        let doc_id = existing_tree.borrow().document();
        return get_or_create_js_element(doc_id, existing_tree.clone(), ctx);
    }

    // Check if the iframe has a `src` attribute with pre-fetched content
    let prefetched_html: Option<String>;
    let src_fragment: Option<String>;
    {
        let dom_tree = realm_state::dom_tree(ctx);
        let t = dom_tree.borrow();
        let node = t.get_node(node_id);
        if let NodeData::Element { attributes, .. } = &node.data {
            let src = attributes
                .iter()
                .find(|a| a.local_name == "src")
                .map(|a| a.value.clone());
            match src {
                Some(src_val) => {
                    let src_no_fragment = src_val.split('#').next().unwrap_or(&src_val).to_string();
                    // Extract fragment (part after #) for :target pseudo-class
                    src_fragment = src_val.find('#').map(|idx| src_val[idx + 1..].to_string());
                    drop(t);
                    let src_content = realm_state::iframe_src_content(ctx);
                    let map = src_content.borrow();
                    prefetched_html = map.get(&src_no_fragment).cloned();
                }
                None => {
                    prefetched_html = None;
                    src_fragment = None;
                }
            }
        } else {
            prefetched_html = None;
            src_fragment = None;
        }
    };

    // Create a new DomTree for the iframe content document
    let new_tree = if let Some(ref html_content) = prefetched_html {
        // Parse the pre-fetched HTML content into a real DomTree
        crate::html::parse_html(html_content)
    } else {
        // No pre-fetched content — create a minimal empty document
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let head = t.create_element("head");
            let body = t.create_element("body");
            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, head);
            t.append_child(html, body);
        }
        tree
    };

    // Set URL fragment on the iframe's tree for :target pseudo-class matching
    if let Some(ref fragment) = src_fragment {
        new_tree.borrow_mut().url_fragment = Some(fragment.clone());
    }

    // Store in IFRAME_CONTENT_DOCS
    {
        let docs = realm_state::iframe_content_docs(ctx);
        let mut map = docs.borrow_mut();
        map.insert((tree_ptr, node_id), new_tree.clone());
    }

    // Create a per-iframe Realm with full globals (MutationObserver, Function, Error, etc.)
    let _ = realm_state::create_iframe_realm(ctx, Rc::clone(&new_tree), tree_ptr, node_id)?;

    // Create document JsObject (in the main realm — for backward compat)
    let doc_id = new_tree.borrow().document();
    let js_obj = get_or_create_js_element(doc_id, new_tree.clone(), ctx)?;
    super::super::document::add_document_properties_to_element(&js_obj, new_tree, "text/html".to_string(), ctx)?;

    Ok(js_obj)
}

// ---------------------------------------------------------------------------
// Prototype objects for proper instanceof support
// (Node.prototype -> CharacterData.prototype -> Text.prototype / Comment.prototype)
// ---------------------------------------------------------------------------

/// Holds the prototype objects for DOM node types.
/// Stored in RealmState so get_or_create_js_element can assign the right prototype.
#[derive(Clone)]
pub(crate) struct DomPrototypes {
    pub(crate) text_proto: JsObject,
    pub(crate) comment_proto: JsObject,
    /// ProcessingInstruction.prototype
    pub(crate) pi_proto: Option<JsObject>,
    /// Attr.prototype
    pub(crate) attr_proto: Option<JsObject>,
    /// Map from lowercase tag name -> prototype object (e.g. "div" -> HTMLDivElement.prototype)
    pub(crate) html_tag_protos: HashMap<String, JsObject>,
    /// Fallback HTMLElement.prototype for HTML elements without a specific type
    pub(crate) html_element_proto: Option<JsObject>,
    /// HTMLUnknownElement.prototype for unknown HTML elements
    pub(crate) html_unknown_proto: Option<JsObject>,
    /// DocumentFragment.prototype
    pub(crate) document_fragment_proto: Option<JsObject>,
    /// ShadowRoot.prototype
    pub(crate) shadow_root_proto: Option<JsObject>,
    /// DocumentType.prototype
    pub(crate) document_type_proto: Option<JsObject>,
    /// Document.prototype
    pub(crate) document_proto: Option<JsObject>,
    /// XMLDocument.prototype
    pub(crate) xml_document_proto: Option<JsObject>,
}

/// Look up or create the JsObject wrapper for a given NodeId.
/// Returns the same JsObject every time for the same NodeId, preserving `===` identity.
/// For Text and Comment nodes, sets the prototype to Text.prototype / Comment.prototype
/// so that `instanceof` checks work correctly.
pub(crate) fn get_or_create_js_element(
    node_id: NodeId,
    tree: Rc<RefCell<DomTree>>,
    ctx: &mut Context,
) -> JsResult<JsObject> {
    // Cache key includes tree pointer so nodes from different trees don't collide
    let tree_ptr = Rc::as_ptr(&tree) as usize;
    let cache_key = (tree_ptr, node_id);

    // Check cache first
    let cached = {
        let cache = realm_state::node_cache(ctx);
        let val = cache.borrow().get(&cache_key).cloned();
        val
    };

    if let Some(obj) = cached {
        return Ok(obj);
    }

    // Cache miss — create and store
    let js_obj = create_js_element(node_id, tree, ctx)?;

    {
        let cache = realm_state::node_cache(ctx);
        cache.borrow_mut().insert(cache_key, js_obj.clone());
    }

    Ok(js_obj)
}

/// Creates a new JS object for a DOM node: determines its kind, assigns the correct prototype,
/// sets up kind-specific properties, and compiles inline event handlers.
fn create_js_element(
    node_id: NodeId,
    tree: Rc<RefCell<DomTree>>,
    ctx: &mut Context,
) -> JsResult<JsObject> {
    // Determine node kind before creating the object
    enum NodeKind {
        Text,
        Comment,
        ProcessingInstruction {
            target: String,
        },
        Attr {
            local_name: String,
            namespace: String,
            prefix: String,
        },
        HtmlElement(String), // lowercase tag name
        NonHtmlElement,
        DocumentFragment,
        ShadowRoot,
        Doctype {
            name: String,
            public_id: String,
            system_id: String,
        },
        Document,
    }

    let node_kind = {
        let tree_ref = tree.borrow();
        let node = tree_ref.get_node(node_id);
        match &node.data {
            NodeData::Text { .. } | NodeData::CDATASection { .. } => NodeKind::Text,
            NodeData::Comment { .. } => NodeKind::Comment,
            NodeData::Element {
                tag_name, namespace, ..
            } => {
                if namespace == "http://www.w3.org/1999/xhtml" {
                    // Extract local name (after colon if present)
                    // Preserve original case — createElement and parser already lowercase,
                    // but createElementNS("SPAN") should NOT match "span" in html_tag_protos
                    let local = if let Some(pos) = tag_name.find(':') {
                        &tag_name[pos + 1..]
                    } else {
                        tag_name.as_str()
                    };
                    NodeKind::HtmlElement(local.to_string())
                } else {
                    NodeKind::NonHtmlElement
                }
            }
            NodeData::DocumentFragment => NodeKind::DocumentFragment,
            NodeData::ShadowRoot { .. } => NodeKind::ShadowRoot,
            NodeData::Doctype {
                name,
                public_id,
                system_id,
            } => NodeKind::Doctype {
                name: name.clone(),
                public_id: public_id.clone(),
                system_id: system_id.clone(),
            },
            NodeData::ProcessingInstruction { target, .. } => {
                NodeKind::ProcessingInstruction { target: target.clone() }
            }
            NodeData::Attr {
                local_name,
                namespace,
                prefix,
                ..
            } => NodeKind::Attr {
                local_name: local_name.clone(),
                namespace: namespace.clone(),
                prefix: prefix.clone(),
            },
            NodeData::Document => NodeKind::Document,
        }
    };

    let element = JsElement::new(node_id, tree.clone());
    let js_obj = JsElement::from_data(element, ctx)?;

    // Set the right prototype based on node kind (for instanceof support)
    if let Some(ref p) = realm_state::dom_prototypes(ctx) {
        match &node_kind {
            NodeKind::Text => {
                js_obj.set_prototype(Some(p.text_proto.clone()));
            }
            NodeKind::Comment => {
                js_obj.set_prototype(Some(p.comment_proto.clone()));
            }
            NodeKind::ProcessingInstruction { .. } => {
                if let Some(ref proto) = p.pi_proto {
                    js_obj.set_prototype(Some(proto.clone()));
                }
            }
            NodeKind::Attr { .. } => {
                if let Some(ref proto) = p.attr_proto {
                    js_obj.set_prototype(Some(proto.clone()));
                }
            }
            NodeKind::HtmlElement(tag) => {
                // Custom elements (tag contains `-`): look up custom element registry first
                if tag.contains('-') {
                    if let Some(ce_proto) = super::super::custom_elements::lookup_custom_element_proto(tag, ctx) {
                        js_obj.set_prototype(Some(ce_proto));
                    } else {
                        // Undefined custom element — per spec, use HTMLElement.prototype
                        if let Some(ref proto) = p.html_element_proto {
                            js_obj.set_prototype(Some(proto.clone()));
                        }
                    }
                } else if let Some(proto) = p.html_tag_protos.get(tag.as_str()) {
                    // Look up specific HTML element prototype by tag name
                    js_obj.set_prototype(Some(proto.clone()));
                } else if is_known_html_element(tag) {
                    // Known HTML element without a specific type -> HTMLElement
                    if let Some(ref proto) = p.html_element_proto {
                        js_obj.set_prototype(Some(proto.clone()));
                    }
                } else {
                    // Unknown HTML element -> HTMLUnknownElement
                    if let Some(ref proto) = p.html_unknown_proto {
                        js_obj.set_prototype(Some(proto.clone()));
                    }
                }
            }
            NodeKind::NonHtmlElement => {
                // Non-HTML namespace elements keep Element.prototype (default from ClassBuilder)
            }
            NodeKind::DocumentFragment => {
                if let Some(ref proto) = p.document_fragment_proto {
                    js_obj.set_prototype(Some(proto.clone()));
                }
            }
            NodeKind::ShadowRoot => {
                if let Some(ref proto) = p.shadow_root_proto {
                    js_obj.set_prototype(Some(proto.clone()));
                } else if let Some(ref proto) = p.document_fragment_proto {
                    // Fallback to DocumentFragment.prototype
                    js_obj.set_prototype(Some(proto.clone()));
                }
            }
            NodeKind::Doctype { .. } => {
                if let Some(ref proto) = p.document_type_proto {
                    js_obj.set_prototype(Some(proto.clone()));
                }
            }
            NodeKind::Document => {
                if let Some(ref proto) = p.document_proto {
                    js_obj.set_prototype(Some(proto.clone()));
                }
            }
        }
    }

    // Set own property for ProcessingInstruction nodes (target)
    if let NodeKind::ProcessingInstruction { target } = &node_kind {
        js_obj.define_property_or_throw(
            js_string!("target"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!(target.clone())))
                .writable(false)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
    }

    // Set own properties for Attr nodes
    if let NodeKind::Attr {
        local_name,
        namespace,
        prefix,
    } = &node_kind
    {
        setup_attr_node_properties(&js_obj, tree.clone(), node_id, local_name, namespace, prefix, ctx)?;
    }

    // Set own properties for Doctype nodes (name, publicId, systemId)
    if let NodeKind::Doctype {
        name,
        public_id,
        system_id,
    } = &node_kind
    {
        js_obj.define_property_or_throw(
            js_string!("name"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!(name.clone())))
                .writable(false)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
        js_obj.define_property_or_throw(
            js_string!("publicId"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!(public_id.clone())))
                .writable(false)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
        js_obj.define_property_or_throw(
            js_string!("systemId"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!(system_id.clone())))
                .writable(false)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
    }

    // Set own property for <template> elements (content -> DocumentFragment)
    if let NodeKind::HtmlElement(ref tag) = node_kind {
        if tag == "template" {
            setup_template_content_getter(&js_obj, tree.clone(), node_id, ctx)?;
        }
    }

    // Set sheet property for <style> elements (CSSStyleSheet stub)
    if let NodeKind::HtmlElement(ref tag) = node_kind {
        if tag == "style" {
            setup_style_sheet(&js_obj, ctx)?;
        }
    }

    // Set contentDocument/contentWindow/src/onload for <iframe> elements
    if let NodeKind::HtmlElement(ref tag) = node_kind {
        if tag == "iframe" {
            setup_iframe_properties(&js_obj, tree.clone(), node_id, ctx)?;
        }
    }

    // Compile inline event handler attributes (e.g., onclick="...") into JS functions
    compile_parsed_inline_handlers(tree, node_id, ctx);

    Ok(js_obj)
}
