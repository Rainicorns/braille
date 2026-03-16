use boa_engine::{
    js_string, native_function::NativeFunction, property::PropertyDescriptor, Context, JsObject, JsValue, Source,
};

use crate::dom::NodeId;
use crate::js::realm_state;

use super::element::JsElement;
use super::window::WINDOW_LISTENER_ID;

/// Get an on* event handler for a given node.
pub(crate) fn get_on_event_handler(
    tree_ptr: usize,
    node_id: NodeId,
    event_name: &str,
    ctx: &Context,
) -> Option<JsObject> {
    let handlers = realm_state::on_event_handlers(ctx);
    let map = handlers.borrow();
    map.get(&(tree_ptr, node_id, event_name.to_string())).cloned()
}

/// Set (or clear) an on* event handler for a given node.
/// Pass a callable JsObject to set, or None to clear.
pub(crate) fn set_on_event_handler(
    tree_ptr: usize,
    node_id: NodeId,
    event_name: &str,
    handler: Option<JsObject>,
    ctx: &Context,
) {
    let handlers = realm_state::on_event_handlers(ctx);
    let mut map = handlers.borrow_mut();
    let key = (tree_ptr, node_id, event_name.to_string());
    match handler {
        Some(h) => {
            map.insert(key, h);
        }
        None => {
            map.remove(&key);
        }
    }
}

/// Compile an inline event handler attribute (e.g., `onclick="alert(1)"`) into a JS function
/// and register it in the per-realm on-event handler map.
/// The attribute value is wrapped as `(function(event) { <value> })` and evaluated.
/// On compilation error, the handler is silently not set (per spec).
pub(crate) fn compile_inline_event_handler(
    tree_ptr: usize,
    node_id: NodeId,
    event_name: &str,
    attr_value: &str,
    ctx: &mut Context,
) {
    let code = format!("(function(event) {{ {} }})", attr_value);
    match ctx.eval(Source::from_bytes(code.as_bytes())) {
        Ok(val) => {
            if let Some(func) = val.as_object().filter(|o| o.is_callable()) {
                set_on_event_handler(tree_ptr, node_id, event_name, Some(func.clone()), ctx);
            }
        }
        Err(_) => {
            // Per spec: compilation error → no handler set
        }
    }
}

/// Invoke the on* event handler for a given node, if one exists.
/// Called during dispatch after addEventListener listeners.
/// Returns true if a handler was found and invoked.
pub(crate) fn invoke_on_event_handler(
    tree_ptr: usize,
    node_id: NodeId,
    event_name: &str,
    this_val: &JsValue,
    event_val: &JsValue,
    event_obj: &JsObject,
    ctx: &mut Context,
) -> bool {
    let handler = get_on_event_handler(tree_ptr, node_id, event_name, ctx);
    if let Some(handler_fn) = handler {
        if handler_fn.is_callable() {
            let result = handler_fn.call(this_val, std::slice::from_ref(event_val), ctx);
            match result {
                Ok(ret_val) => {
                    // Per HTML spec: if inline handler returns false, call preventDefault()
                    if ret_val == JsValue::from(false) {
                        if let Some(mut ev) = event_obj.downcast_mut::<super::event::JsEvent>() {
                            if ev.cancelable {
                                ev.default_prevented = true;
                            }
                        }
                    }
                }
                Err(err) => {
                    super::element::report_listener_error(err, ctx);
                }
            }
            return true;
        }
    }
    false
}

/// Register on* accessor properties (getter/setter) on an element prototype.
/// `event_names` is a list like &["click", "change", "input", "load", ...].
/// Each creates an `onclick`, `onchange`, etc. accessor that reads/writes the per-realm on-event handler map.
pub(crate) fn register_on_event_accessors(
    proto: &JsObject,
    event_names: &[&str],
    ctx: &mut Context,
) {
    let realm = ctx.realm().clone();

    for &event_name in event_names {
        let prop_name = format!("on{event_name}");
        let event_name_for_get = event_name.to_string();
        let event_name_for_set = event_name.to_string();

        let getter = unsafe {
            NativeFunction::from_closure(move |this, _args, ctx2| {
                let obj = match this.as_object() {
                    Some(o) => o,
                    None => return Ok(JsValue::null()),
                };
                let (tree_ptr, node_id) = match obj.downcast_ref::<JsElement>() {
                    Some(el) => (std::rc::Rc::as_ptr(&el.tree) as usize, el.node_id),
                    None => return Ok(JsValue::null()),
                };
                match get_on_event_handler(tree_ptr, node_id, &event_name_for_get, ctx2) {
                    Some(h) => Ok(JsValue::from(h)),
                    None => Ok(JsValue::null()),
                }
            })
        };

        let setter = unsafe {
            NativeFunction::from_closure(move |this, args, ctx2| {
                let obj = match this.as_object() {
                    Some(o) => o,
                    None => return Ok(JsValue::undefined()),
                };
                let (tree_ptr, node_id) = match obj.downcast_ref::<JsElement>() {
                    Some(el) => (std::rc::Rc::as_ptr(&el.tree) as usize, el.node_id),
                    None => return Ok(JsValue::undefined()),
                };
                let val = args.first().cloned().unwrap_or(JsValue::null());
                if let Some(func) = val.as_object().filter(|o| o.is_callable()) {
                    set_on_event_handler(tree_ptr, node_id, &event_name_for_set, Some(func.clone()), ctx2);
                } else {
                    set_on_event_handler(tree_ptr, node_id, &event_name_for_set, None, ctx2);
                }
                Ok(JsValue::undefined())
            })
        };

        proto
            .define_property_or_throw(
                js_string!(prop_name),
                PropertyDescriptor::builder()
                    .get(getter.to_js_function(&realm))
                    .set(setter.to_js_function(&realm))
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                ctx,
            )
            .expect("failed to define on* accessor");
    }
}

/// Register on* accessor properties on the window object.
/// Window handlers use (usize::MAX, WINDOW_LISTENER_ID) as key.
pub(crate) fn register_window_on_event_accessors(
    window: &JsObject,
    event_names: &[&str],
    ctx: &mut Context,
) {
    let realm = ctx.realm().clone();

    for &event_name in event_names {
        let prop_name = format!("on{event_name}");
        let event_name_for_get = event_name.to_string();
        let event_name_for_set = event_name.to_string();

        let getter = unsafe {
            NativeFunction::from_closure(move |_this, _args, ctx2| {
                match get_on_event_handler(usize::MAX, WINDOW_LISTENER_ID, &event_name_for_get, ctx2) {
                    Some(h) => Ok(JsValue::from(h)),
                    None => Ok(JsValue::null()),
                }
            })
        };

        let setter = unsafe {
            NativeFunction::from_closure(move |_this, args, ctx2| {
                let val = args.first().cloned().unwrap_or(JsValue::null());
                if let Some(func) = val.as_object().filter(|o| o.is_callable()) {
                    set_on_event_handler(usize::MAX, WINDOW_LISTENER_ID, &event_name_for_set, Some(func.clone()), ctx2);
                } else {
                    set_on_event_handler(usize::MAX, WINDOW_LISTENER_ID, &event_name_for_set, None, ctx2);
                }
                Ok(JsValue::undefined())
            })
        };

        window
            .define_property_or_throw(
                js_string!(prop_name),
                PropertyDescriptor::builder()
                    .get(getter.to_js_function(&realm))
                    .set(setter.to_js_function(&realm))
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                ctx,
            )
            .expect("failed to define window on* accessor");
    }
}
