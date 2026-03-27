use std::rc::Rc;

use boa_engine::{
    js_string,
    property::PropertyDescriptor,
    Context, JsError, JsResult, JsValue,
};

use super::super::event::JsEvent;
use super::JsDocument;
use crate::js::realm_state;

/// Native implementation of document.addEventListener(type, callback, options?)
pub(crate) fn document_add_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: `this` is not document").into()))?;
    let tree = doc.tree.clone();
    let node_id = tree.borrow().document();
    let listener_key = (Rc::as_ptr(&tree) as usize, node_id);
    drop(doc);
    super::super::event_target::add_event_listener_impl(listener_key, Some(&tree), args, ctx)
}

/// Native implementation of document.removeEventListener(type, callback, options?)
pub(crate) fn document_remove_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let obj = this
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: `this` is not an object").into()))?;
    let doc = obj
        .downcast_ref::<JsDocument>()
        .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: `this` is not document").into()))?;
    let tree = doc.tree.clone();
    let node_id = tree.borrow().document();
    let listener_key = (Rc::as_ptr(&tree) as usize, node_id);
    drop(doc);
    super::super::event_target::remove_event_listener_impl(listener_key, args, ctx)
}

/// Public entry point for document.dispatchEvent — called from EventTarget.prototype.dispatchEvent
pub(crate) fn document_dispatch_event_public(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    document_dispatch_event(this, args, ctx)
}

/// Native implementation of document.dispatchEvent(event)
/// Delegates to the same dispatch algorithm used by elements.
pub(crate) fn document_dispatch_event(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
            JsError::from_native(boa_engine::JsNativeError::typ().with_message(
                "Failed to execute 'dispatchEvent' on 'EventTarget': 1 argument required, but only 0 present.",
            ))
        })?
        .clone();

    // null/undefined arg -> TypeError
    if event_val.is_null() || event_val.is_undefined() {
        return Err(JsError::from_native(boa_engine::JsNativeError::typ().with_message(
            "Failed to execute 'dispatchEvent' on 'EventTarget': parameter 1 is not of type 'Event'.",
        )));
    }

    let event_obj = event_val
        .as_object()
        .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: argument is not an object").into()))?
        .clone();

    let (event_type, bubbles) = {
        let evt = event_obj
            .downcast_ref::<JsEvent>()
            .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: argument is not an Event").into()))?;
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
    };

    // Set event.target and dispatching flag
    {
        let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
        evt.target = Some(target_node_id);
        evt.dispatching = true;
    }

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

    use super::super::window::WINDOW_LISTENER_ID;

    let is_global_doc = {
        let global_tree = realm_state::dom_tree(ctx);
        Rc::ptr_eq(&tree, &global_tree)
    };

    let mut stopped = false;

    // Capture phase on window (if global document)
    if is_global_doc && !stopped {
        {
            let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
            evt.current_target = Some(WINDOW_LISTENER_ID);
            evt.phase = 1; // CAPTURING_PHASE
        }
        let window_val: JsValue = realm_state::window_object(ctx)
            .map(JsValue::from)
            .unwrap_or(JsValue::undefined());
        event_obj.define_property_or_throw(
            js_string!("currentTarget"),
            PropertyDescriptor::builder()
                .value(window_val)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
        stopped = super::super::element::invoke_listeners_for_node(
            (usize::MAX, WINDOW_LISTENER_ID),
            &event_type,
            &event_obj,
            &event_val,
            true,
            false,
            ctx,
        )?;
    }

    // At-target phase (document is both root and target)
    if !stopped {
        {
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
        stopped = super::super::element::invoke_listeners_for_node(
            (Rc::as_ptr(&tree) as usize, target_node_id),
            &event_type,
            &event_obj,
            &event_val,
            false,
            true,
            ctx,
        )?;
    }

    // Bubble phase on window (if global document AND bubbles)
    if is_global_doc && bubbles && !stopped {
        {
            let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
            evt.current_target = Some(WINDOW_LISTENER_ID);
            evt.phase = 3; // BUBBLING_PHASE
        }
        let window_val: JsValue = realm_state::window_object(ctx)
            .map(JsValue::from)
            .unwrap_or(JsValue::undefined());
        event_obj.define_property_or_throw(
            js_string!("currentTarget"),
            PropertyDescriptor::builder()
                .value(window_val)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
        let _should_stop = super::super::element::invoke_listeners_for_node(
            (usize::MAX, WINDOW_LISTENER_ID),
            &event_type,
            &event_obj,
            &event_val,
            false,
            false,
            ctx,
        )?;
    }

    // Reset event state
    let default_prevented = {
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

    Ok(JsValue::from(!default_prevented))
}
