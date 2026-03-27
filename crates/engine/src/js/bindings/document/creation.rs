use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{js_string, Context, JsError, JsResult, JsValue};

use crate::dom::is_valid_xml_name;
use crate::dom::DomTree;

use super::super::element::{get_or_create_js_element, JsElement};
use super::super::event::{EventKind, JsEvent};
use super::validation::validate_and_extract;
use super::JsDocument;

/// Native implementation of document.createElement(tagName)
pub(crate) fn document_create_element(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

    // Validate the element name per spec
    if !crate::dom::is_valid_element_name(&tag) {
        let exc = super::super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    // Global document is always HTML — lowercase the tag name per spec
    let tag_lower = tag.to_ascii_lowercase();
    let node_id = tree.borrow_mut().create_element(&tag_lower);

    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.createTextNode(text)
pub(crate) fn document_create_text_node(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

/// Native implementation of document.createComment(data)
pub(crate) fn document_create_comment(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
pub(crate) fn document_create_processing_instruction(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this.as_object().ok_or_else(|| {
        JsError::from_opaque(js_string!("createProcessingInstruction: `this` is not an object").into())
    })?;
    let doc = obj.downcast_ref::<JsDocument>().ok_or_else(|| {
        JsError::from_opaque(js_string!("createProcessingInstruction: `this` is not document").into())
    })?;
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
            super::super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?
                .into(),
        ));
    }
    if data.contains("?>") {
        return Err(JsError::from_opaque(
            super::super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?
                .into(),
        ));
    }

    let node_id = tree.borrow_mut().create_processing_instruction(&target, &data);

    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.createDocumentFragment()
pub(crate) fn document_create_document_fragment(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

/// Native implementation of document.createRange()
pub(crate) fn document_create_range(this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createRange: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("createRange: `this` is not document").into()))?;
    let tree = doc.tree.clone();
    let doc_id = tree.borrow().document();
    let range_obj = super::super::range::create_range(tree, doc_id, ctx)?;
    Ok(range_obj.into())
}

/// Native implementation of document.createAttribute(localName)
pub(crate) fn document_create_attribute(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

    // Per spec: validate the attribute name
    if !crate::dom::is_valid_attribute_name(&local_name) {
        let exc = super::super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
        return Err(JsError::from_opaque(exc.into()));
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
pub(crate) fn document_create_attribute_ns(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createAttributeNS: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("createAttributeNS: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    // First arg: namespace URI (can be null)
    let namespace = match args.first() {
        Some(v) if !v.is_null() && !v.is_undefined() => v.to_string(ctx)?.to_std_string_escaped(),
        _ => String::new(),
    };

    // Second arg: qualified name
    let qualified_name = args
        .get(1)
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_else(|| "undefined".to_string());

    // Validate the qualified name for attribute names
    if let Some(colon_pos) = qualified_name.find(':') {
        let prefix_part = &qualified_name[..colon_pos];
        let local_part = &qualified_name[colon_pos + 1..];
        let invalid_prefix =
            prefix_part.is_empty() || prefix_part.contains(['\0', '\t', '\n', '\x0C', '\r', ' ', '/', '>']);
        if invalid_prefix || !crate::dom::is_valid_attribute_name(local_part) {
            let exc =
                super::super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
            return Err(JsError::from_opaque(exc.into()));
        }
    } else if !crate::dom::is_valid_attribute_name(&qualified_name) {
        let exc = super::super::create_dom_exception(ctx, "InvalidCharacterError", "String contains an invalid character", 5)?;
        return Err(JsError::from_opaque(exc.into()));
    }

    // Parse prefix and local name from qualified name
    let (prefix, local_name) = if let Some(colon_pos) = qualified_name.find(':') {
        (
            qualified_name[..colon_pos].to_string(),
            qualified_name[colon_pos + 1..].to_string(),
        )
    } else {
        (String::new(), qualified_name)
    };

    let node_id = tree.borrow_mut().create_attr(&local_name, &namespace, &prefix, "");

    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.createElementNS(namespaceURI, qualifiedName)
pub(crate) fn document_create_element_ns(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createElementNS: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("createElementNS: `this` is not document").into()))?;
    let tree = doc.tree.clone();

    // First arg: namespace URI (can be null)
    let namespace = match args.first() {
        Some(v) if !v.is_null() && !v.is_undefined() => v.to_string(ctx)?.to_std_string_escaped(),
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

    let node_id = tree.borrow_mut().create_element_ns(&qualified_name, Vec::new(), ns_ref);

    let js_obj = get_or_create_js_element(node_id, tree, ctx)?;
    Ok(js_obj.into())
}

/// Native implementation of document.createEvent(type)
/// Legacy event creation — returns an uninitialized Event
pub(crate) fn document_create_event(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let _obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("createEvent: `this` is not an object").into()))?;

    let event_interface = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    // Spec alias table per https://dom.spec.whatwg.org/#dom-document-createevent
    // Case-insensitive match against known legacy aliases
    let lower = event_interface.to_ascii_lowercase();
    let (kind, proto_ctor_name): (EventKind, Option<&str>) = match lower.as_str() {
        // Event aliases
        "event" | "events" | "htmlevents" | "svgevents" => (EventKind::Standard, None),
        // CustomEvent
        "customevent" => (
            EventKind::Custom {
                detail: JsValue::null(),
            },
            Some("CustomEvent"),
        ),
        // UIEvent aliases
        "uievent" | "uievents" => (EventKind::Standard, Some("UIEvent")),
        // FocusEvent
        "focusevent" => (EventKind::Focus, Some("FocusEvent")),
        // MouseEvent aliases
        "mouseevent" | "mouseevents" => (EventKind::mouse_default(), Some("MouseEvent")),
        // KeyboardEvent
        "keyboardevent" => (EventKind::Keyboard, Some("KeyboardEvent")),
        // CompositionEvent
        "compositionevent" => (EventKind::Composition, Some("CompositionEvent")),
        // Legacy aliases we recognize but create as base Event (missing full constructors)
        "beforeunloadevent" | "devicemotionevent" | "deviceorientationevent" | "dragevent"
        | "hashchangeevent" | "messageevent" | "storageevent" | "textevent"
        | "touchevent" => {
            (EventKind::Standard, None)
        }
        // Anything else is NOT a recognized alias — throw NotSupportedError
        _ => {
            let exc = super::super::create_dom_exception(
                ctx,
                "NotSupportedError",
                &format!("The provided event type ('{}') is invalid", event_interface),
                9,
            )?;
            return Err(JsError::from_opaque(exc.into()));
        }
    };

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
        time_stamp: super::super::event::dom_high_res_time_stamp(ctx),
        initialized: false,
        composed: false,
        kind,
    };
    let js_obj = JsEvent::from_data(event, ctx)?;
    // Set correct prototype for subclass instances
    if let Some(ctor_name) = proto_ctor_name {
        let global = ctx.global_object();
        if let Ok(ctor_val) = global.get(js_string!(ctor_name), ctx) {
            if let Some(ctor_obj) = ctor_val.as_object() {
                if let Ok(proto) = ctor_obj.get(js_string!("prototype"), ctx) {
                    if let Some(proto_obj) = proto.as_object() {
                        js_obj.set_prototype(Some(proto_obj.clone()));
                    }
                }
            }
        }
    }
    super::super::event::attach_is_trusted_own_property(&js_obj, ctx)?;
    Ok(js_obj.into())
}
