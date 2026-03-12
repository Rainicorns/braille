use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    class::Class,
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::{Attribute, PropertyDescriptor},
    Context, JsData, JsError, JsObject, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::DomTree;

use crate::dom::NodeData;

use super::element::{JsElement, get_or_create_js_element};
use super::event::{JsEvent, JsCustomEvent};
use super::event_target::{ListenerEntry, EVENT_LISTENERS};
use super::class_list::register_class_list_class;
use super::style::register_style_class;
use super::query;

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

    let node_id = tree.borrow_mut().create_element(&tag);

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
/// We implement this as a Comment node since we don't have a native PI node type.
/// This is sufficient for most tests that just need a node to exist.
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

    // target is first arg, data is second arg
    let _target = args
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

    // Use a Comment node as a stand-in for ProcessingInstruction
    let node_id = tree.borrow_mut().create_comment(&data);

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
        };
        let js_obj = JsCustomEvent::from_data(event, ctx)?;
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
        };
        let js_obj = JsEvent::from_data(event, ctx)?;
        Ok(js_obj.into())
    }
}

/// Parse the third argument to addEventListener/removeEventListener.
/// Returns (capture, once).
fn parse_listener_options(args: &[JsValue], ctx: &mut Context) -> JsResult<(bool, bool)> {
    let mut capture = false;
    let mut once = false;

    if let Some(opt_val) = args.get(2) {
        if let Some(b) = opt_val.as_boolean() {
            capture = b;
        } else if let Some(opt_obj) = opt_val.as_object() {
            let c = opt_obj.get(js_string!("capture"), ctx)?;
            if !c.is_undefined() {
                capture = c.to_boolean();
            }
            let o = opt_obj.get(js_string!("once"), ctx)?;
            if !o.is_undefined() {
                once = o.to_boolean();
            }
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

    let (capture, once) = parse_listener_options(args, ctx)?;

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

    let (capture, _once) = parse_listener_options(args, ctx)?;

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
        .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: missing event argument").into()))?
        .clone();
    let event_obj = event_val
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: argument is not an object").into()))?
        .clone();

    let is_custom_event;
    let (event_type, bubbles) = if let Some(evt) = event_obj.downcast_ref::<JsEvent>() {
        is_custom_event = false;
        (evt.event_type.clone(), evt.bubbles)
    } else if let Some(evt) = event_obj.downcast_ref::<JsCustomEvent>() {
        is_custom_event = true;
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

/// Builds the `document` global object and registers it on the context.
pub(crate) fn register_document(tree: Rc<RefCell<DomTree>>, context: &mut Context) {
    // Register the Element class first so from_data works
    context.register_global_class::<JsElement>().unwrap();

    // Register the ClassList class so from_data works for classList getter
    register_class_list_class(context);

    // Register the CSSStyleDeclaration class so from_data works for style getter
    register_style_class(context);

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

    context
        .register_global_property(js_string!("document"), document, Attribute::all())
        .expect("failed to register document global");
}
