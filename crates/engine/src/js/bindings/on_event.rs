use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::{
    js_string, native_function::NativeFunction, property::PropertyDescriptor, Context, JsObject, JsValue,
};

use crate::dom::NodeId;

use super::element::JsElement;
use super::window::WINDOW_LISTENER_ID;

/// Unified on* IDL event handler storage.
/// Key: (tree_ptr, node_id, event_name) — for window handlers, tree_ptr = usize::MAX.
type OnEventKey = (usize, NodeId, String);

type OnEventMap = HashMap<OnEventKey, JsObject>;

thread_local! {
    #[allow(clippy::type_complexity)]
    pub(crate) static ON_EVENT_HANDLERS: RefCell<Option<Rc<RefCell<OnEventMap>>>> = const { RefCell::new(None) };
}

/// Initialize the ON_EVENT_HANDLERS thread-local (called from JsRuntime::new).
pub(crate) fn init_on_event_handlers() {
    ON_EVENT_HANDLERS.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            *opt = Some(Rc::new(RefCell::new(HashMap::new())));
        }
    });
}

/// Get an on* event handler for a given node.
pub(crate) fn get_on_event_handler(tree_ptr: usize, node_id: NodeId, event_name: &str) -> Option<JsObject> {
    ON_EVENT_HANDLERS.with(|cell| {
        let rc = cell.borrow();
        let map_rc = rc.as_ref()?;
        let map = map_rc.borrow();
        map.get(&(tree_ptr, node_id, event_name.to_string())).cloned()
    })
}

/// Set (or clear) an on* event handler for a given node.
/// Pass a callable JsObject to set, or None to clear.
pub(crate) fn set_on_event_handler(tree_ptr: usize, node_id: NodeId, event_name: &str, handler: Option<JsObject>) {
    ON_EVENT_HANDLERS.with(|cell| {
        let rc = cell.borrow();
        let map_rc = rc.as_ref().expect("ON_EVENT_HANDLERS not initialized");
        let mut map = map_rc.borrow_mut();
        let key = (tree_ptr, node_id, event_name.to_string());
        match handler {
            Some(h) => {
                map.insert(key, h);
            }
            None => {
                map.remove(&key);
            }
        }
    });
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
    ctx: &mut Context,
) -> bool {
    let handler = get_on_event_handler(tree_ptr, node_id, event_name);
    if let Some(handler_fn) = handler {
        if handler_fn.is_callable() {
            let result = handler_fn.call(this_val, std::slice::from_ref(event_val), ctx);
            if let Err(err) = result {
                super::element::report_listener_error(err, ctx);
            }
            return true;
        }
    }
    false
}

/// Register on* accessor properties (getter/setter) on an element prototype.
/// `event_names` is a list like &["click", "change", "input", "load", ...].
/// Each creates an `onclick`, `onchange`, etc. accessor that reads/writes ON_EVENT_HANDLERS.
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
            NativeFunction::from_closure(move |this, _args, _ctx| {
                let obj = match this.as_object() {
                    Some(o) => o,
                    None => return Ok(JsValue::null()),
                };
                let (tree_ptr, node_id) = match obj.downcast_ref::<JsElement>() {
                    Some(el) => (std::rc::Rc::as_ptr(&el.tree) as usize, el.node_id),
                    None => return Ok(JsValue::null()),
                };
                match get_on_event_handler(tree_ptr, node_id, &event_name_for_get) {
                    Some(h) => Ok(JsValue::from(h)),
                    None => Ok(JsValue::null()),
                }
            })
        };

        let setter = unsafe {
            NativeFunction::from_closure(move |this, args, _ctx| {
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
                    set_on_event_handler(tree_ptr, node_id, &event_name_for_set, Some(func.clone()));
                } else {
                    set_on_event_handler(tree_ptr, node_id, &event_name_for_set, None);
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
            NativeFunction::from_closure(move |_this, _args, _ctx| {
                match get_on_event_handler(usize::MAX, WINDOW_LISTENER_ID, &event_name_for_get) {
                    Some(h) => Ok(JsValue::from(h)),
                    None => Ok(JsValue::null()),
                }
            })
        };

        let setter = unsafe {
            NativeFunction::from_closure(move |_this, args, _ctx| {
                let val = args.first().cloned().unwrap_or(JsValue::null());
                if let Some(func) = val.as_object().filter(|o| o.is_callable()) {
                    set_on_event_handler(usize::MAX, WINDOW_LISTENER_ID, &event_name_for_set, Some(func.clone()));
                } else {
                    set_on_event_handler(usize::MAX, WINDOW_LISTENER_ID, &event_name_for_set, None);
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
