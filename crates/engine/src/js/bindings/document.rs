use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    class::Class,
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::{Attribute, PropertyDescriptor},
    Context, JsData, JsError, JsNativeError, JsObject, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::DomTree;
use crate::dom::{is_valid_dom_name, is_valid_xml_name};

use crate::dom::NodeData;

use super::element::{JsElement, get_or_create_js_element, NODE_CACHE, DOM_PROTOTYPES};
use super::event::{JsEvent, JsCustomEvent};
use super::event_target::{ListenerEntry, EVENT_LISTENERS};
use super::class_list::register_class_list_class;
use super::style::register_style_class;
use super::query;

// ---------------------------------------------------------------------------
// DOM "validate and extract" algorithm (namespace validation)
// https://dom.spec.whatwg.org/#validate-and-extract
// ---------------------------------------------------------------------------

/// Implements the DOM spec's "validate and extract a namespace and qualifiedName" algorithm.
/// Returns (namespace, prefix, local_name) or throws InvalidCharacterError / NamespaceError.
fn validate_and_extract(
    namespace: &str,
    qualified_name: &str,
    ctx: &mut Context,
) -> JsResult<(String, String, String)> {
    // Step 1: Validate the qualifiedName as an XML QName
    if let Some(colon_pos) = qualified_name.find(':') {
        let prefix_part = &qualified_name[..colon_pos];
        let local_part = &qualified_name[colon_pos + 1..];
        // Both parts must be non-empty with valid start chars (lenient browser behavior)
        if prefix_part.is_empty()
            || local_part.is_empty()
            || !is_valid_dom_name(prefix_part)
            || !is_valid_dom_name(local_part)
        {
            let exc = super::create_dom_exception(
                ctx,
                "InvalidCharacterError",
                "String contains an invalid character",
                5,
            )?;
            return Err(JsError::from_opaque(exc.into()));
        }
    } else {
        // No colon — must have a valid start char (lenient browser behavior)
        if !qualified_name.is_empty() && !is_valid_dom_name(qualified_name) {
            let exc = super::create_dom_exception(
                ctx,
                "InvalidCharacterError",
                "String contains an invalid character",
                5,
            )?;
            return Err(JsError::from_opaque(exc.into()));
        }
    }

    // Step 2: Extract prefix and localName
    let (prefix, local_name) = if let Some(colon_pos) = qualified_name.find(':') {
        (qualified_name[..colon_pos].to_string(), qualified_name[colon_pos + 1..].to_string())
    } else {
        (String::new(), qualified_name.to_string())
    };

    let ns = namespace.to_string();

    // Step 3: Namespace validation
    // 3a: prefix present but namespace is empty
    if !prefix.is_empty() && ns.is_empty() {
        let exc = super::create_dom_exception(
            ctx,
            "NamespaceError",
            "Namespace error",
            14,
        )?;
        return Err(JsError::from_opaque(exc.into()));
    }

    // 3b: prefix is "xml" but namespace is not the XML namespace
    if prefix == "xml" && ns != "http://www.w3.org/XML/1998/namespace" {
        let exc = super::create_dom_exception(ctx, "NamespaceError", "Namespace error", 14)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    // 3c: prefix or qualifiedName is "xmlns" but namespace is not the XMLNS namespace
    if (prefix == "xmlns" || qualified_name == "xmlns")
        && ns != "http://www.w3.org/2000/xmlns/"
    {
        let exc = super::create_dom_exception(ctx, "NamespaceError", "Namespace error", 14)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    // 3d: namespace is XMLNS but neither prefix nor qualifiedName is "xmlns"
    if ns == "http://www.w3.org/2000/xmlns/" && !qualified_name.is_empty() && prefix != "xmlns" && qualified_name != "xmlns" {
        let exc = super::create_dom_exception(ctx, "NamespaceError", "Namespace error", 14)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    Ok((ns, prefix, local_name))
}

// ---------------------------------------------------------------------------
// DOMImplementation prototype (for instanceof checks)
// ---------------------------------------------------------------------------

thread_local! {
    static DOMIMPL_PROTO: RefCell<Option<JsObject>> = const { RefCell::new(None) };
}

/// Register the DOMImplementation global constructor (illegal — just for instanceof)
pub(crate) fn register_domimplementation(ctx: &mut Context) {
    let proto = ObjectInitializer::new(ctx).build();

    let ctor = unsafe {
        NativeFunction::from_closure(|_this, _args, _ctx| {
            Err(JsError::from_opaque(JsValue::from(js_string!(
                "Illegal constructor"
            ))))
        })
    };
    let ctor_obj: JsObject = boa_engine::object::FunctionObjectBuilder::new(ctx.realm(), ctor)
        .name(js_string!("DOMImplementation"))
        .length(0)
        .constructor(true)
        .build()
        .into();

    ctor_obj
        .define_property_or_throw(
            js_string!("prototype"),
            PropertyDescriptor::builder()
                .value(proto.clone())
                .writable(false)
                .configurable(false)
                .enumerable(false)
                .build(),
            ctx,
        )
        .expect("failed to define DOMImplementation.prototype");

    proto
        .define_property_or_throw(
            js_string!("constructor"),
            PropertyDescriptor::builder()
                .value(ctor_obj.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            ctx,
        )
        .expect("failed to set DOMImplementation.prototype.constructor");

    DOMIMPL_PROTO.with(|cell| {
        *cell.borrow_mut() = Some(proto);
    });

    ctx.register_global_property(
        js_string!("DOMImplementation"),
        ctor_obj,
        Attribute::WRITABLE | Attribute::CONFIGURABLE,
    )
    .expect("failed to register DOMImplementation global");
}

/// DOMImplementation.hasFeature() — per spec, always returns true.
fn domimpl_has_feature(
    _this: &JsValue,
    _args: &[JsValue],
    _ctx: &mut Context,
) -> JsResult<JsValue> {
    Ok(JsValue::from(true))
}

// ---------------------------------------------------------------------------
// JsDocument — singleton global `document` object backed by DomTree
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsDocument {
    #[unsafe_ignore_trace]
    pub(crate) tree: Rc<RefCell<DomTree>>,
}

/// Native implementation of document.createElement(tagName)
fn document_create_element(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createElement: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("createElement: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    let tag = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_else(|| "undefined".to_string());

    // Global document is always HTML — lowercase the tag name per spec
    let tag_lower = tag.to_ascii_lowercase();
    let node_id = tree.borrow_mut().create_element(&tag_lower);

    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.getElementById(id)
fn document_get_element_by_id(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
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

/// Native getter for document.body
fn document_get_body(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
fn document_get_head(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
fn document_get_title(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
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
fn document_set_title(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

/// Native implementation of document.createTextNode(text)
fn document_create_text_node(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createTextNode: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("createTextNode: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    let text = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let node_id = tree.borrow_mut().create_text(&text);

    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native getter for document.documentElement
fn document_get_document_element(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

/// Native implementation of document.createComment(data)
fn document_create_comment(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createComment: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("createComment: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    let data = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let node_id = tree.borrow_mut().create_comment(&data);

    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.createProcessingInstruction(target, data)
fn document_create_processing_instruction(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createProcessingInstruction: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("createProcessingInstruction: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    let target = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let data = args
        .get(1)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    if !is_valid_xml_name(&target) {
        return Err(JsError::from_opaque(
            super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?.into(),
        ));
    }
    if data.contains("?>") {
        return Err(JsError::from_opaque(
            super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?.into(),
        ));
    }

    let node_id = tree.borrow_mut().create_processing_instruction(&target, &data);

    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.createDocumentFragment()
fn document_create_document_fragment(
    this: &JsValue,
    _args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createDocumentFragment: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("createDocumentFragment: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    let node_id = tree.borrow_mut().create_document_fragment();

    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.createAttribute(localName)
fn document_create_attribute(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createAttribute: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("createAttribute: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    let local_name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_else(|| "undefined".to_string());

    // Per spec: validate the name — empty string is not a valid XML name
    if local_name.is_empty() {
        return Err(JsError::from_opaque(
            js_string!("InvalidCharacterError: The string contains invalid characters.").into(),
        ));
    }

    // Per spec: if the document is an HTML document, lowercase the name
    let local_name = if tree.borrow().is_html_document() {
        local_name.to_ascii_lowercase()
    } else {
        local_name
    };

    let node_id = tree.borrow_mut().create_attr(&local_name, "", "", "");

    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.createAttributeNS(namespace, qualifiedName)
fn document_create_attribute_ns(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createAttributeNS: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("createAttributeNS: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    // First arg: namespace URI (can be null)
    let namespace = match args.first() {
        Some(v) if !v.is_null() && !v.is_undefined() => {
            v.to_string(ctx)?.to_std_string_escaped()
        }
        _ => String::new(),
    };

    // Second arg: qualified name
    let qualified_name = args
        .get(1)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_else(|| "undefined".to_string());

    // Parse prefix and local name from qualified name
    let (prefix, local_name) = if let Some(colon_pos) = qualified_name.find(':') {
        (qualified_name[..colon_pos].to_string(), qualified_name[colon_pos + 1..].to_string())
    } else {
        (String::new(), qualified_name)
    };

    let node_id = tree.borrow_mut().create_attr(&local_name, &namespace, &prefix, "");

    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.createElementNS(namespaceURI, qualifiedName)
fn document_create_element_ns(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createElementNS: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("createElementNS: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    // First arg: namespace URI (can be null)
    let namespace = match args.first() {
        Some(v) if !v.is_null() && !v.is_undefined() => {
            v.to_string(ctx)?.to_std_string_escaped()
        }
        _ => String::new(),
    };

    // Second arg: qualified name
    let qualified_name = args
        .get(1)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_else(|| "undefined".to_string());

    // Validate and extract per DOM spec
    let (ns, _prefix, _local_name) = validate_and_extract(&namespace, &qualified_name, ctx)?;

    let ns_ref = if ns.is_empty() { "" } else { &ns };

    let node_id = tree.borrow_mut().create_element_ns(
        &qualified_name,
        Vec::new(),
        ns_ref,
    );

    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.createEvent(type)
/// Legacy event creation — returns an uninitialized Event
fn document_create_event(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let _obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createEvent: `this` is not an object").into()))?;

    let event_interface = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    if event_interface.eq_ignore_ascii_case("customevent") {
        let event = JsCustomEvent {
            event_type: String::new(),
            bubbles: false,
            cancelable: false,
            default_prevented: false,
            propagation_stopped: false,
            immediate_propagation_stopped: false,
            detail: JsValue::null(),
            target: None,
            current_target: None,
            phase: 0,
            dispatching: false,
            time_stamp: super::event::dom_high_res_time_stamp(),
            initialized: false,
        };
        let js_obj = JsCustomEvent::from_data(event, ctx)?;
        super::event::attach_is_trusted_own_property(&js_obj, ctx)?;
        Ok(js_obj.into())
    } else {
        let event = JsEvent {
            event_type: String::new(),
            bubbles: false,
            cancelable: false,
            default_prevented: false,
            propagation_stopped: false,
            immediate_propagation_stopped: false,
            target: None,
            current_target: None,
            phase: 0,
            dispatching: false,
            time_stamp: super::event::dom_high_res_time_stamp(),
            initialized: false,
        };
        let js_obj = JsEvent::from_data(event, ctx)?;
        super::event::attach_is_trusted_own_property(&js_obj, ctx)?;
        Ok(js_obj.into())
    }
}

/// Parse the third argument to addEventListener/removeEventListener.
/// Returns (capture, once).
fn parse_listener_options(args: &[JsValue], ctx: &mut Context) -> JsResult<(bool, bool)> {
    let mut capture = false;
    let mut once = false;

    if let Some(opt_val) = args.get(2) {
        if let Some(opt_obj) = opt_val.as_object() {
            let c = opt_obj.get(js_string!("capture"), ctx)?;
            if !c.is_undefined() {
                capture = c.to_boolean();
            }
            let o = opt_obj.get(js_string!("once"), ctx)?;
            if !o.is_undefined() {
                once = o.to_boolean();
            }
        } else {
            // Coerce non-object values to boolean (handles numbers, strings, null, undefined, etc.)
            capture = opt_val.to_boolean();
        }
    }

    Ok((capture, once))
}

/// Native implementation of document.addEventListener(type, callback, options?)
fn document_add_event_listener(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: `this` is not document").into()))?;
    let node_id = doc.tree.borrow().document();

    let event_type = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: missing type argument").into()))?
        .to_string(ctx)?
        .to_std_string_escaped();

    // Parse options BEFORE checking for null callback (spec: options getters must be invoked)
    let (capture, once) = parse_listener_options(args, ctx)?;

    let callback_val = args
        .get(1)
        .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: missing callback argument").into()))?;

    if callback_val.is_null() || callback_val.is_undefined() {
        return Ok(JsValue::undefined());
    }

    let callback = callback_val
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: callback is not an object").into()))?
        .clone();

    EVENT_LISTENERS.with(|el| {
        let rc = el.borrow();
        let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
        let mut map = listeners_rc.borrow_mut();
        let entries = map.entry(node_id).or_insert_with(Vec::new);

        let duplicate = entries.iter().any(|entry| {
            entry.event_type == event_type
                && entry.capture == capture
                && entry.callback == callback
        });

        if !duplicate {
            entries.push(ListenerEntry {
                event_type,
                callback,
                capture,
                once,
                passive: None,
            });
        }
    });

    Ok(JsValue::undefined())
}

/// Native implementation of document.removeEventListener(type, callback, options?)
fn document_remove_event_listener(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: `this` is not document").into()))?;
    let node_id = doc.tree.borrow().document();

    let event_type = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: missing type argument").into()))?
        .to_string(ctx)?
        .to_std_string_escaped();

    // Parse options BEFORE checking for null callback (spec: options getters must be invoked)
    let (capture, _once) = parse_listener_options(args, ctx)?;

    let callback_val = args
        .get(1)
        .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: missing callback argument").into()))?;

    if callback_val.is_null() || callback_val.is_undefined() {
        return Ok(JsValue::undefined());
    }

    let callback = callback_val
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: callback is not an object").into()))?
        .clone();

    EVENT_LISTENERS.with(|el| {
        let rc = el.borrow();
        let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
        let mut map = listeners_rc.borrow_mut();
        if let Some(entries) = map.get_mut(&node_id) {
            entries.retain(|entry| {
                !(entry.event_type == event_type
                    && entry.capture == capture
                    && entry.callback == callback)
            });
            if entries.is_empty() {
                map.remove(&node_id);
            }
        }
    });

    Ok(JsValue::undefined())
}

/// Native implementation of document.dispatchEvent(event)
/// Delegates to the same dispatch algorithm used by elements.
fn document_dispatch_event(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: `this` is not document").into()))?;
    let tree = doc.tree.clone();
    let target_node_id = tree.borrow().document();

    let event_val = args
        .first()
        .ok_or_else(|| {
            JsError::from_native(
                boa_engine::JsNativeError::typ()
                    .with_message("Failed to execute 'dispatchEvent' on 'EventTarget': 1 argument required, but only 0 present.")
            )
        })?
        .clone();

    // null/undefined arg -> TypeError
    if event_val.is_null() || event_val.is_undefined() {
        return Err(JsError::from_native(
            boa_engine::JsNativeError::typ()
                .with_message("Failed to execute 'dispatchEvent' on 'EventTarget': parameter 1 is not of type 'Event'.")
        ));
    }

    let event_obj = event_val
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: argument is not an object").into()))?
        .clone();

    let is_custom_event;
    let (event_type, bubbles) = if let Some(evt) = event_obj.downcast_ref::<JsEvent>() {
        is_custom_event = false;
        // Check initialized flag
        if !evt.initialized {
            return Err(JsError::from_opaque(
                js_string!("InvalidStateError: The event is not initialized.").into(),
            ));
        }
        // Check dispatching flag
        if evt.dispatching {
            return Err(JsError::from_opaque(
                js_string!("InvalidStateError: The event is already being dispatched.").into(),
            ));
        }
        (evt.event_type.clone(), evt.bubbles)
    } else if let Some(evt) = event_obj.downcast_ref::<JsCustomEvent>() {
        is_custom_event = true;
        if !evt.initialized {
            return Err(JsError::from_opaque(
                js_string!("InvalidStateError: The event is not initialized.").into(),
            ));
        }
        if evt.dispatching {
            return Err(JsError::from_opaque(
                js_string!("InvalidStateError: The event is already being dispatched.").into(),
            ));
        }
        (evt.event_type.clone(), evt.bubbles)
    } else {
        return Err(JsError::from_opaque(js_string!("dispatchEvent: argument is not an Event").into()));
    };

    // Document is the root, so propagation path is just [document]
    let propagation_path = vec![target_node_id];

    // Set event.target and dispatching flag
    if is_custom_event {
        let mut evt = event_obj.downcast_mut::<JsCustomEvent>().unwrap();
        evt.target = Some(target_node_id);
        evt.dispatching = true;
    } else {
        let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
        evt.target = Some(target_node_id);
        evt.dispatching = true;
    }

    use boa_engine::property::PropertyDescriptor;
    event_obj.define_property_or_throw(
        js_string!("target"),
        PropertyDescriptor::builder()
            .value(this.clone())
            .writable(true)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;
    event_obj.define_property_or_throw(
        js_string!("srcElement"),
        PropertyDescriptor::builder()
            .value(this.clone())
            .writable(true)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    // At-target phase (document is both root and target)
    if is_custom_event {
        let mut evt = event_obj.downcast_mut::<JsCustomEvent>().unwrap();
        evt.current_target = Some(target_node_id);
        evt.phase = 2; // AT_TARGET
    } else {
        let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
        evt.current_target = Some(target_node_id);
        evt.phase = 2; // AT_TARGET
    }
    event_obj.define_property_or_throw(
        js_string!("currentTarget"),
        PropertyDescriptor::builder()
            .value(this.clone())
            .writable(true)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    // Invoke listeners at target
    let _should_stop = super::element::invoke_listeners_for_node(
        target_node_id, &event_type, &event_obj, &event_val, false, true, ctx,
    )?;

    // Reset event state
    let default_prevented = if is_custom_event {
        let mut evt = event_obj.downcast_mut::<JsCustomEvent>().unwrap();
        evt.phase = 0;
        evt.current_target = None;
        evt.propagation_stopped = false;
        evt.immediate_propagation_stopped = false;
        evt.dispatching = false;
        evt.default_prevented
    } else {
        let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
        evt.phase = 0;
        evt.current_target = None;
        evt.propagation_stopped = false;
        evt.immediate_propagation_stopped = false;
        evt.dispatching = false;
        evt.default_prevented
    };
    event_obj.define_property_or_throw(
        js_string!("currentTarget"),
        PropertyDescriptor::builder()
            .value(JsValue::null())
            .writable(true)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    let _ = (bubbles, propagation_path); // document has no parent to bubble to
    Ok(JsValue::from(!default_prevented))
}

/// document.getRootNode() — document is always its own root
fn document_get_root_node(this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    // Document node has no parent, so getRootNode() returns itself
    Ok(this.clone())
}

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

    // createElement method — respects is_html_document for lowercasing and namespace
    let tree_for_ce = new_tree.clone();
    let create_element = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let tag = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_else(|| "undefined".to_string());
            let is_html = tree_for_ce.borrow().is_html_document();
            let node_id = if is_html {
                // HTML doc: lowercase tag, HTML namespace (create_element default)
                tree_for_ce.borrow_mut().create_element(&tag.to_ascii_lowercase())
            } else {
                // XML/XHTML doc: preserve case, null namespace
                tree_for_ce.borrow_mut().create_element_ns(&tag, vec![], "")
            };
            let js_el = get_or_create_js_element(node_id, tree_for_ce.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(js_string!("createElement"), create_element.to_js_function(&realm), false, ctx)?;

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
    js_obj.set(js_string!("createComment"), create_comment.to_js_function(&realm), false, ctx)?;

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
    js_obj.set(js_string!("createTextNode"), create_text_node.to_js_function(&realm), false, ctx)?;

    // createDocumentFragment method
    let tree_for_cdf = new_tree.clone();
    let create_doc_frag = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let node_id = tree_for_cdf.borrow_mut().create_document_fragment();
            let js_el = get_or_create_js_element(node_id, tree_for_cdf.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(js_string!("createDocumentFragment"), create_doc_frag.to_js_function(&realm), false, ctx)?;

    // createProcessingInstruction method
    let tree_for_cpi = new_tree.clone();
    let create_pi = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let target = args.first().map(|v| v.to_string(ctx2)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
            let data = args.get(1).map(|v| v.to_string(ctx2)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
            let node_id = tree_for_cpi.borrow_mut().create_processing_instruction(&target, &data);
            let js_el = get_or_create_js_element(node_id, tree_for_cpi.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(js_string!("createProcessingInstruction"), create_pi.to_js_function(&realm), false, ctx)?;

    // createAttribute method
    let tree_for_ca = new_tree.clone();
    let create_attr_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let local_name = args.first().map(|v| v.to_string(ctx2)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_else(|| "undefined".to_string());
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
    js_obj.set(js_string!("createAttribute"), create_attr_fn.to_js_function(&realm), false, ctx)?;

    // createAttributeNS method
    let tree_for_cans = new_tree.clone();
    let create_attr_ns_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let namespace = match args.first() {
                Some(v) if !v.is_null() && !v.is_undefined() => v.to_string(ctx2)?.to_std_string_escaped(),
                _ => String::new(),
            };
            let qualified_name = args.get(1).map(|v| v.to_string(ctx2)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_else(|| "undefined".to_string());
            let (prefix, local_name) = if let Some(colon_pos) = qualified_name.find(':') {
                (qualified_name[..colon_pos].to_string(), qualified_name[colon_pos + 1..].to_string())
            } else {
                (String::new(), qualified_name)
            };
            let node_id = tree_for_cans.borrow_mut().create_attr(&local_name, &namespace, &prefix, "");
            let js_el = get_or_create_js_element(node_id, tree_for_cans.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(js_string!("createAttributeNS"), create_attr_ns_fn.to_js_function(&realm), false, ctx)?;

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
            let name = args.first().map(|v| v.to_string(ctx2)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
            let public_id = args.get(1).map(|v| v.to_string(ctx2)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
            let system_id = args.get(2).map(|v| v.to_string(ctx2)).transpose()?.map(|s| s.to_std_string_escaped()).unwrap_or_default();
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
    let domimpl_proto = DOMIMPL_PROTO.with(|cell| cell.borrow().clone());
    if let Some(p) = domimpl_proto {
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
            let node_val = args.first()
                .ok_or_else(|| JsError::from_opaque(js_string!("importNode: missing argument").into()))?;
            let node_obj = node_val.as_object()
                .ok_or_else(|| JsError::from_opaque(js_string!("importNode: argument is not an object").into()))?;
            let node_el = node_obj.downcast_ref::<JsElement>()
                .ok_or_else(|| JsError::from_opaque(js_string!("importNode: argument is not a Node").into()))?;

            let source_tree = node_el.tree.clone();
            let source_id = node_el.node_id;
            let deep = args.get(1).map(|v| v.to_boolean()).unwrap_or(false);

            let new_id = if deep {
                tree_for_import.borrow_mut().import_subtree(&source_tree.borrow(), source_id)
            } else {
                let src = source_tree.borrow();
                let src_node = src.get_node(source_id);
                let mut t = tree_for_import.borrow_mut();
                match &src_node.data {
                    NodeData::Element { tag_name, attributes, namespace } => {
                        t.create_element_ns(tag_name, attributes.clone(), namespace)
                    }
                    NodeData::Text { content } => t.create_text(content),
                    NodeData::Comment { content } => t.create_comment(content),
                    NodeData::Doctype { name, public_id, system_id } => {
                        t.create_doctype(name, public_id, system_id)
                    }
                    NodeData::DocumentFragment => t.create_document_fragment(),
                    NodeData::ProcessingInstruction { target, data } => {
                        t.create_processing_instruction(target, data)
                    }
                    NodeData::Attr { local_name, namespace, prefix, value } => {
                        t.create_attr(local_name, namespace, prefix, value)
                    }
                    NodeData::Document => {
                        return Err(JsError::from_opaque(js_string!("NotSupportedError: Cannot import a Document node").into()));
                    }
                }
            };

            let js_el = get_or_create_js_element(new_id, tree_for_import.clone(), ctx2)?;
            Ok(js_el.into())
        })
    };
    js_obj.set(js_string!("importNode"), import_node_fn.to_js_function(&realm), false, ctx)?;

    // adoptNode method
    let tree_for_adopt = new_tree.clone();
    let adopt_node_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, _ctx2| {
            let node_val = args.first()
                .ok_or_else(|| JsError::from_opaque(js_string!("adoptNode: missing argument").into()))?;
            let node_obj = node_val.as_object()
                .ok_or_else(|| JsError::from_opaque(js_string!("adoptNode: argument is not an object").into()))?;
            let node_el = node_obj.downcast_ref::<JsElement>()
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
                let (adopted_id, mapping) = super::mutation::adopt_node_with_mapping(
                    &source_tree, source_id, &tree_for_adopt,
                );
                // Update all cached JS objects (root + descendants) to point to new tree/nodes
                super::mutation::update_node_cache_for_adoption_mapping(
                    &source_tree, &tree_for_adopt, &mapping,
                );
                // Also update the root node_obj directly (in case it wasn't cached yet)
                let mut el_mut = node_obj.downcast_mut::<JsElement>().unwrap();
                el_mut.node_id = adopted_id;
                el_mut.tree = tree_for_adopt.clone();
                drop(el_mut);
                Ok(node_val.clone())
            }
        })
    };
    js_obj.set(js_string!("adoptNode"), adopt_node_fn.to_js_function(&realm), false, ctx)?;

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

    // createCDATASection method (uses Text node as stand-in)
    let tree_for_cdata = new_tree.clone();
    let cdata_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx2| {
            let data = args
                .first()
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();
            let node_id = tree_for_cdata.borrow_mut().create_text(&data);
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

    Ok(())
}

/// Native implementation of document.implementation.createDocumentType(name, publicId, systemId)
fn domimpl_create_document_type(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let name = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Validate qualified name per spec
    if name.contains('>') || name.contains(' ') {
        return Err(JsError::from_opaque(js_string!("InvalidCharacterError: The string contains invalid characters").into()));
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

    let tree = super::element::DOM_TREE.with(|cell| {
        let rc = cell.borrow();
        rc.as_ref().expect("DOM_TREE not initialized").clone()
    });

    let node_id = tree.borrow_mut().create_doctype(&name, &public_id, &system_id);
    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.implementation.createHTMLDocument(title?)
/// Creates a new Document with basic structure: doctype, html, head, title, body
fn domimpl_create_html_document(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
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

    let _tree = super::element::DOM_TREE.with(|cell| {
        let rc = cell.borrow();
        rc.as_ref().expect("DOM_TREE not initialized").clone()
    });

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
fn domimpl_create_document(
    _this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
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
    let (validated_ns, _prefix, _local_name) =
        validate_and_extract(&namespace, &qualified_name, ctx)?;

    // Handle the optional doctype argument (3rd arg)
    let doctype_info: Option<(String, String, String)> = args
        .get(2)
        .and_then(|v| {
            if v.is_null() || v.is_undefined() {
                return None;
            }
            let obj = v.as_object()?;
            let el = obj.downcast_ref::<JsElement>()?;
            let tree = el.tree.borrow();
            let node = tree.get_node(el.node_id);
            if let NodeData::Doctype { name, public_id, system_id } = &node.data {
                Some((name.clone(), public_id.clone(), system_id.clone()))
            } else {
                None
            }
        });

    // Create a new DomTree for the new document (XML, not HTML)
    let new_tree = Rc::new(RefCell::new(crate::dom::DomTree::new_xml()));
    {
        let mut t = new_tree.borrow_mut();
        let doc = t.document();

        // If doctype was provided, create it in the new tree
        if let Some((name, public_id, system_id)) = doctype_info {
            let dt = t.create_doctype(&name, &public_id, &system_id);
            t.append_child(doc, dt);
        }

        // If qualified name is non-empty, create a document element
        if !qualified_name.is_empty() {
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
    DOM_PROTOTYPES.with(|cell| {
        let protos = cell.borrow();
        if let Some(ref p) = *protos {
            if let Some(ref xml_proto) = p.xml_document_proto {
                js_obj.set_prototype(Some(xml_proto.clone()));
            }
        }
    });

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

/// Native implementation of document.importNode(node, deep)
fn document_import_node(
    this: &JsValue,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
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

    let deep = args.get(1).map(|v| v.to_boolean()).unwrap_or(false);

    let new_id = if deep {
        target_tree.borrow_mut().import_subtree(&source_tree.borrow(), source_id)
    } else {
        // Shallow import: clone just the node, no children
        let src = source_tree.borrow();
        let src_node = src.get_node(source_id);
        let mut t = target_tree.borrow_mut();
        match &src_node.data {
            NodeData::Element { tag_name, attributes, namespace } => {
                t.create_element_ns(tag_name, attributes.clone(), namespace)
            }
            NodeData::Text { content } => t.create_text(content),
            NodeData::Comment { content } => t.create_comment(content),
            NodeData::Doctype { name, public_id, system_id } => {
                t.create_doctype(name, public_id, system_id)
            }
            NodeData::DocumentFragment => t.create_document_fragment(),
            NodeData::ProcessingInstruction { target, data } => {
                t.create_processing_instruction(target, data)
            }
            NodeData::Attr { local_name, namespace, prefix, value } => {
                t.create_attr(local_name, namespace, prefix, value)
            }
            NodeData::Document => {
                return Err(JsError::from_opaque(js_string!("NotSupportedError: Cannot import a Document node").into()));
            }
        }
    };

    let js_obj = get_or_create_js_element(new_id, target_tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.adoptNode(node)
fn document_adopt_node(
    this: &JsValue,
    args: &[JsValue],
    _ctx: &mut Context,
) -> JsResult<JsValue> {
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
        let (adopted_id, mapping) = super::mutation::adopt_node_with_mapping(
            &source_tree, source_id, &target_tree,
        );
        // Update all cached JS objects (root + descendants) to point to new tree/nodes
        super::mutation::update_node_cache_for_adoption_mapping(
            &source_tree, &target_tree, &mapping,
        );
        // Also update the root node_obj directly (in case it wasn't cached yet)
        let mut el_mut = node_obj.downcast_mut::<JsElement>().unwrap();
        el_mut.node_id = adopted_id;
        el_mut.tree = target_tree.clone();
        drop(el_mut);
        Ok(node_val.clone())
    }
}

/// Native implementation of document.appendChild(child)
fn document_append_child(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
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

    let is_fragment = matches!(doc.tree.borrow().get_node(child_id).data, NodeData::DocumentFragment);
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
fn document_remove_child(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
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

/// Native getter for document.parentNode — always null
fn document_get_parent_node(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::null())
}

/// Native getter for document.parentElement — always null
fn document_get_parent_element(_this: &JsValue, _args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
    Ok(JsValue::null())
}

/// Native implementation of document.contains(other)
/// Returns true if other is a descendant of the document (inclusive).
fn document_contains(this: &JsValue, args: &[JsValue], _ctx: &mut Context) -> JsResult<JsValue> {
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

/// Native getter for document.childNodes
fn document_get_child_nodes(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
fn document_get_first_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
fn document_get_last_child(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

/// Native getter for document.doctype
/// Returns the first Doctype child of the document, or null.
fn document_get_doctype(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

/// Builds the `document` global object and registers it on the context.
pub(crate) fn register_document(tree: Rc<RefCell<DomTree>>, context: &mut Context) {
    // Register the Element class first so from_data works
    context.register_global_class::<JsElement>().unwrap();

    // Register the ClassList class so from_data works for classList getter
    register_class_list_class(context);

    // Register the CSSStyleDeclaration class so from_data works for style getter
    register_style_class(context);

    // Save tree pointer and doc_id for NODE_CACHE registration below
    let tree_ptr = Rc::as_ptr(&tree) as usize;
    let doc_id = tree.borrow().document();

    let doc_data = JsDocument { tree };

    let document: JsObject = ObjectInitializer::with_native_data(doc_data, context)
        .function(
            NativeFunction::from_fn_ptr(document_create_element),
            js_string!("createElement"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_get_element_by_id),
            js_string!("getElementById"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_text_node),
            js_string!("createTextNode"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(query::document_query_selector),
            js_string!("querySelector"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(query::document_query_selector_all),
            js_string!("querySelectorAll"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(query::document_get_elements_by_class_name),
            js_string!("getElementsByClassName"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(query::document_get_elements_by_tag_name),
            js_string!("getElementsByTagName"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_element_ns),
            js_string!("createElementNS"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_comment),
            js_string!("createComment"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_processing_instruction),
            js_string!("createProcessingInstruction"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_document_fragment),
            js_string!("createDocumentFragment"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_attribute),
            js_string!("createAttribute"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_attribute_ns),
            js_string!("createAttributeNS"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_event),
            js_string!("createEvent"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_add_event_listener),
            js_string!("addEventListener"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_remove_event_listener),
            js_string!("removeEventListener"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_dispatch_event),
            js_string!("dispatchEvent"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(super::mutation::document_append),
            js_string!("append"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(super::mutation::document_prepend),
            js_string!("prepend"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(super::mutation::document_replace_children),
            js_string!("replaceChildren"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(super::mutation::document_normalize),
            js_string!("normalize"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(document_get_root_node),
            js_string!("getRootNode"),
            0,
        )
        .function(
            NativeFunction::from_fn_ptr(document_create_element_ns),
            js_string!("createElementNS"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_contains),
            js_string!("contains"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_append_child),
            js_string!("appendChild"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_remove_child),
            js_string!("removeChild"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(document_import_node),
            js_string!("importNode"),
            2,
        )
        .function(
            NativeFunction::from_fn_ptr(document_adopt_node),
            js_string!("adoptNode"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(super::element::node_compare_document_position),
            js_string!("compareDocumentPosition"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(super::element::node_is_equal_node),
            js_string!("isEqualNode"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(super::element::node_is_same_node),
            js_string!("isSameNode"),
            1,
        )
        .function(
            NativeFunction::from_fn_ptr(super::element::node_contains),
            js_string!("contains"),
            1,
        )
        .build();

    // Add accessor properties (body, head, title)
    let realm = context.realm().clone();

    // document.body (getter only)
    let body_getter = NativeFunction::from_fn_ptr(document_get_body);
    document
        .define_property_or_throw(
            js_string!("body"),
            PropertyDescriptor::builder()
                .get(body_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.body");

    // document.head (getter only)
    let head_getter = NativeFunction::from_fn_ptr(document_get_head);
    document
        .define_property_or_throw(
            js_string!("head"),
            PropertyDescriptor::builder()
                .get(head_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.head");

    // document.documentElement (getter only)
    let document_element_getter = NativeFunction::from_fn_ptr(document_get_document_element);
    document
        .define_property_or_throw(
            js_string!("documentElement"),
            PropertyDescriptor::builder()
                .get(document_element_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.documentElement");

    // document.title (getter and setter)
    let title_getter = NativeFunction::from_fn_ptr(document_get_title);
    let title_setter = NativeFunction::from_fn_ptr(document_set_title);
    document
        .define_property_or_throw(
            js_string!("title"),
            PropertyDescriptor::builder()
                .get(title_getter.to_js_function(&realm))
                .set(title_setter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.title");

    // document.implementation — object with createDocumentType, createHTMLDocument, createDocument, hasFeature
    let implementation = ObjectInitializer::new(context)
        .function(
            NativeFunction::from_fn_ptr(domimpl_create_document_type),
            js_string!("createDocumentType"),
            3,
        )
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
        .function(
            NativeFunction::from_fn_ptr(domimpl_has_feature),
            js_string!("hasFeature"),
            0,
        )
        .build();
    // Set DOMImplementation prototype for instanceof checks
    let domimpl_proto = DOMIMPL_PROTO.with(|cell| cell.borrow().clone());
    if let Some(p) = domimpl_proto {
        implementation.set_prototype(Some(p));
    }
    document
        .define_property_or_throw(
            js_string!("implementation"),
            PropertyDescriptor::builder()
                .value(JsValue::from(implementation))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.implementation");

    // document.doctype (getter only)
    let doctype_getter = NativeFunction::from_fn_ptr(document_get_doctype);
    document
        .define_property_or_throw(
            js_string!("doctype"),
            PropertyDescriptor::builder()
                .get(doctype_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.doctype");

    // document.nodeName (always "#document")
    document
        .define_property_or_throw(
            js_string!("nodeName"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("#document")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.nodeName");

    // document.nodeType (always 9)
    document
        .define_property_or_throw(
            js_string!("nodeType"),
            PropertyDescriptor::builder()
                .value(JsValue::from(9))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.nodeType");

    // document.textContent (getter returns null, setter is no-op)
    let text_content_getter = NativeFunction::from_fn_ptr(|_this, _args, _ctx| {
        Ok(JsValue::null())
    });
    let text_content_setter = NativeFunction::from_fn_ptr(|_this, _args, _ctx| {
        Ok(JsValue::undefined())
    });
    document
        .define_property_or_throw(
            js_string!("textContent"),
            PropertyDescriptor::builder()
                .get(text_content_getter.to_js_function(&realm))
                .set(text_content_setter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.textContent");

    // document.nodeValue (getter returns null, setter is no-op)
    let node_value_getter = NativeFunction::from_fn_ptr(|_this, _args, _ctx| {
        Ok(JsValue::null())
    });
    let node_value_setter = NativeFunction::from_fn_ptr(|_this, _args, _ctx| {
        Ok(JsValue::undefined())
    });
    document
        .define_property_or_throw(
            js_string!("nodeValue"),
            PropertyDescriptor::builder()
                .get(node_value_getter.to_js_function(&realm))
                .set(node_value_setter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.nodeValue");

    // document.parentNode (getter returns null)
    let parent_node_getter = NativeFunction::from_fn_ptr(document_get_parent_node);
    document
        .define_property_or_throw(
            js_string!("parentNode"),
            PropertyDescriptor::builder()
                .get(parent_node_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.parentNode");

    // document.parentElement (getter returns null)
    let parent_element_getter = NativeFunction::from_fn_ptr(document_get_parent_element);
    document
        .define_property_or_throw(
            js_string!("parentElement"),
            PropertyDescriptor::builder()
                .get(parent_element_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.parentElement");

    // document.childNodes (getter)
    let child_nodes_getter = NativeFunction::from_fn_ptr(document_get_child_nodes);
    document
        .define_property_or_throw(
            js_string!("childNodes"),
            PropertyDescriptor::builder()
                .get(child_nodes_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.childNodes");

    // document.firstChild (getter)
    let first_child_getter = NativeFunction::from_fn_ptr(document_get_first_child);
    document
        .define_property_or_throw(
            js_string!("firstChild"),
            PropertyDescriptor::builder()
                .get(first_child_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.firstChild");

    // document.lastChild (getter)
    let last_child_getter = NativeFunction::from_fn_ptr(document_get_last_child);
    document
        .define_property_or_throw(
            js_string!("lastChild"),
            PropertyDescriptor::builder()
                .get(last_child_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.lastChild");

    // document.URL (always "about:blank" — no real URL context in the engine)
    document
        .define_property_or_throw(
            js_string!("URL"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("about:blank")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.URL");

    // document.documentURI (alias for URL per spec)
    document
        .define_property_or_throw(
            js_string!("documentURI"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("about:blank")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.documentURI");

    // document.compatMode (always "CSS1Compat" — no-quirks mode)
    document
        .define_property_or_throw(
            js_string!("compatMode"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("CSS1Compat")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.compatMode");

    // document.characterSet (always "UTF-8")
    document
        .define_property_or_throw(
            js_string!("characterSet"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("UTF-8")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.characterSet");

    // document.charset (legacy alias for characterSet)
    document
        .define_property_or_throw(
            js_string!("charset"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("UTF-8")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.charset");

    // document.inputEncoding (legacy alias for characterSet)
    document
        .define_property_or_throw(
            js_string!("inputEncoding"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("UTF-8")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.inputEncoding");

    // document.contentType (always "text/html" for the global parsed document)
    document
        .define_property_or_throw(
            js_string!("contentType"),
            PropertyDescriptor::builder()
                .value(JsValue::from(js_string!("text/html")))
                .writable(false)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define document.contentType");

    // Store the document JsObject in NODE_CACHE so that get_or_create_js_element
    // returns this same object when looking up the Document node. This ensures
    // evt.currentTarget === document during event propagation.
    NODE_CACHE.with(|cell| {
        let rc = cell.borrow();
        if let Some(cache_rc) = rc.as_ref() {
            let mut cache = cache_rc.borrow_mut();
            cache.insert((tree_ptr, doc_id), document.clone());
        }
    });

    context
        .register_global_property(js_string!("document"), document, Attribute::all())
        .expect("failed to register document global");
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

    // Add createCDATASection method -- uses Text node as stand-in
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
            let node_id = tree_for_cdata.borrow_mut().create_text(&data);
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
                Some(v) if !v.is_null() && !v.is_undefined() => {
                    v.to_string(ctx2)?.to_std_string_escaped()
                }
                _ => String::new(),
            };
            let qualified_name = args
                .get(1)
                .map(|v| v.to_string(ctx2))
                .transpose()?
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_else(|| "undefined".to_string());
            let (validated_ns, _prefix, _local_name) =
                validate_and_extract(&namespace, &qualified_name, ctx2)?;
            let ns = if validated_ns.is_empty() { "" } else { validated_ns.as_str() };
            let node_id = tree_for_cens.borrow_mut().create_element_ns(
                &qualified_name,
                Vec::new(),
                ns,
            );
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
