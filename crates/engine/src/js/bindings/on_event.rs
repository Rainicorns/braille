use boa_engine::{
    js_string, native_function::NativeFunction, property::PropertyDescriptor, Context, JsObject, JsValue, Source,
};

use crate::dom::NodeId;
use crate::js::realm_state;

use super::element::JsElement;
use super::window::WINDOW_LISTENER_ID;

/// Sentinel tree pointer used as the key for window-level on* event handlers.
/// Window is not a DOM node so it has no real tree pointer — `usize::MAX` is
/// guaranteed to never collide with any `Rc::as_ptr` value.
pub(crate) const WINDOW_TREE_PTR: usize = usize::MAX;

/// Known on* event names, mapped to `&'static str` so the HashMap key avoids allocation.
/// Returns `None` for unrecognised names (which can never match a registered handler).
pub(crate) fn intern_event_name(name: &str) -> Option<&'static str> {
    match name {
        "click" => Some("click"),
        "change" => Some("change"),
        "input" => Some("input"),
        "submit" => Some("submit"),
        "reset" => Some("reset"),
        "toggle" => Some("toggle"),
        "load" => Some("load"),
        "error" => Some("error"),
        "mousedown" => Some("mousedown"),
        "mouseup" => Some("mouseup"),
        "mouseover" => Some("mouseover"),
        "mouseout" => Some("mouseout"),
        "mousemove" => Some("mousemove"),
        "keydown" => Some("keydown"),
        "keyup" => Some("keyup"),
        "keypress" => Some("keypress"),
        "focus" => Some("focus"),
        "blur" => Some("blur"),
        "resize" => Some("resize"),
        "scroll" => Some("scroll"),
        "hashchange" => Some("hashchange"),
        "popstate" => Some("popstate"),
        "unload" => Some("unload"),
        "beforeunload" => Some("beforeunload"),
        "abort" => Some("abort"),
        "animationstart" => Some("animationstart"),
        "animationend" => Some("animationend"),
        "animationiteration" => Some("animationiteration"),
        "transitionend" => Some("transitionend"),
        "transitionstart" => Some("transitionstart"),
        "transitionrun" => Some("transitionrun"),
        "webkitanimationstart" | "webkitAnimationStart" => Some("webkitanimationstart"),
        "webkitanimationend" | "webkitAnimationEnd" => Some("webkitanimationend"),
        "webkitanimationiteration" | "webkitAnimationIteration" => Some("webkitanimationiteration"),
        "webkittransitionend" | "webkitTransitionEnd" => Some("webkittransitionend"),
        _ => None,
    }
}

/// Get an on* event handler for a given node.
pub(crate) fn get_on_event_handler(
    tree_ptr: usize,
    node_id: NodeId,
    event_name: &'static str,
    ctx: &Context,
) -> Option<JsObject> {
    let handlers = realm_state::on_event_handlers(ctx);
    let map = handlers.borrow();
    map.get(&(tree_ptr, node_id, event_name)).cloned()
}

/// Set (or clear) an on* event handler for a given node.
/// Pass a callable JsObject to set, or None to clear.
pub(crate) fn set_on_event_handler(
    tree_ptr: usize,
    node_id: NodeId,
    event_name: &'static str,
    handler: Option<JsObject>,
    ctx: &Context,
) {
    let handlers = realm_state::on_event_handlers(ctx);
    let mut map = handlers.borrow_mut();
    let key = (tree_ptr, node_id, event_name);
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
///
/// `event_name` is the bare name (e.g. `"click"`), not the attribute name.
/// If the name is not a known event name, the handler is silently ignored.
pub(crate) fn compile_inline_event_handler(
    tree_ptr: usize,
    node_id: NodeId,
    event_name: &str,
    attr_value: &str,
    ctx: &mut Context,
) {
    let Some(interned) = intern_event_name(event_name) else {
        return;
    };
    let code = format!("(function(event) {{ {} }})", attr_value);
    match ctx.eval(Source::from_bytes(code.as_bytes())) {
        Ok(val) => {
            if let Some(func) = val.as_object().filter(|o| o.is_callable()) {
                set_on_event_handler(tree_ptr, node_id, interned, Some(func.clone()), ctx);
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
///
/// `event_name` is the bare name (e.g. `"click"`).
/// If the name is not a known event name, returns false immediately.
pub(crate) fn invoke_on_event_handler(
    tree_ptr: usize,
    node_id: NodeId,
    event_name: &str,
    this_val: &JsValue,
    event_val: &JsValue,
    event_obj: &JsObject,
    ctx: &mut Context,
) -> bool {
    let Some(interned) = intern_event_name(event_name) else {
        return false;
    };
    let handler = get_on_event_handler(tree_ptr, node_id, interned, ctx);
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
pub(crate) fn register_on_event_accessors(proto: &JsObject, event_names: &[&'static str], ctx: &mut Context) {
    let realm = ctx.realm().clone();

    for &event_name in event_names {
        let prop_name = format!("on{event_name}");

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
                match get_on_event_handler(tree_ptr, node_id, event_name, ctx2) {
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
                    set_on_event_handler(tree_ptr, node_id, event_name, Some(func.clone()), ctx2);
                } else {
                    set_on_event_handler(tree_ptr, node_id, event_name, None, ctx2);
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
/// Window handlers use (WINDOW_TREE_PTR, WINDOW_LISTENER_ID) as key.
pub(crate) fn register_window_on_event_accessors(window: &JsObject, event_names: &[&'static str], ctx: &mut Context) {
    let realm = ctx.realm().clone();

    for &event_name in event_names {
        let prop_name = format!("on{event_name}");

        let getter = unsafe {
            NativeFunction::from_closure(move |_this, _args, ctx2| {
                match get_on_event_handler(WINDOW_TREE_PTR, WINDOW_LISTENER_ID, event_name, ctx2) {
                    Some(h) => Ok(JsValue::from(h)),
                    None => Ok(JsValue::null()),
                }
            })
        };

        let setter = unsafe {
            NativeFunction::from_closure(move |_this, args, ctx2| {
                let val = args.first().cloned().unwrap_or(JsValue::null());
                if let Some(func) = val.as_object().filter(|o| o.is_callable()) {
                    set_on_event_handler(
                        WINDOW_TREE_PTR,
                        WINDOW_LISTENER_ID,
                        event_name,
                        Some(func.clone()),
                        ctx2,
                    );
                } else {
                    set_on_event_handler(WINDOW_TREE_PTR, WINDOW_LISTENER_ID, event_name, None, ctx2);
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
