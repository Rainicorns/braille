use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    object::JsObject,
    property::PropertyDescriptor,
    Context, JsData, JsError, JsNativeError, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::{DomTree, NodeId};
use crate::js::realm_state;

// ---------------------------------------------------------------------------
// resolve_event_target_key — identify listener key from any `this` value
// ---------------------------------------------------------------------------

type ListenerKeyResult = ((usize, NodeId), Option<Rc<std::cell::RefCell<DomTree>>>);

/// Given a `this` value, resolve to a `(tree_ptr, node_id)` listener key.
/// Handles JsEventTarget, JsElement, JsDocument, window object, and null/undefined (→ window).
/// Returns the key and optionally the DomTree Rc (for passive default computation).
fn resolve_event_target_key(
    this: &JsValue,
    ctx: &mut boa_engine::Context,
) -> JsResult<ListenerKeyResult> {
    // null/undefined → window
    if this.is_null() || this.is_undefined() {
        return Ok(((usize::MAX, super::window::WINDOW_LISTENER_ID), None));
    }

    let this_obj = match this.as_object() {
        Some(obj) => obj,
        None => return Ok(((usize::MAX, super::window::WINDOW_LISTENER_ID), None)),
    };

    // JsAbortSignal (inherits EventTarget behavior)
    if let Some(sig) = this_obj.downcast_ref::<super::abort::JsAbortSignal>() {
        return Ok(((0usize, sig.event_target_id), None));
    }

    // JsEventTarget (standalone)
    if let Some(et) = this_obj.downcast_ref::<JsEventTarget>() {
        return Ok(((0usize, et.id), None));
    }

    // JsElement (DOM node)
    if let Some(el) = this_obj.downcast_ref::<super::element::JsElement>() {
        let tree = el.tree.clone();
        let key = (Rc::as_ptr(&tree) as usize, el.node_id);
        return Ok((key, Some(tree)));
    }

    // JsDocument
    if let Some(doc) = this_obj.downcast_ref::<super::document::JsDocument>() {
        let tree = doc.tree.clone();
        let node_id = tree.borrow().document();
        let key = (Rc::as_ptr(&tree) as usize, node_id);
        return Ok((key, Some(tree)));
    }

    // Check if this is the window object by comparing to realm_state::window_object
    if let Some(window) = realm_state::window_object(ctx) {
        if this_obj.clone() == window {
            return Ok(((usize::MAX, super::window::WINDOW_LISTENER_ID), None));
        }
    }

    // Fallback: treat as window
    Ok(((usize::MAX, super::window::WINDOW_LISTENER_ID), None))
}

// ---------------------------------------------------------------------------
// ListenerEntry — one registered event listener
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub(crate) struct ListenerEntry {
    pub(crate) event_type: String,
    pub(crate) callback: JsObject,
    pub(crate) capture: bool,
    pub(crate) once: bool,
    pub(crate) passive: Option<bool>,
    /// Set to true when removeEventListener removes this entry during dispatch.
    /// Snapshot-based dispatch loops check this flag before invoking.
    pub(crate) removed: Rc<Cell<bool>>,
}

// ---------------------------------------------------------------------------
// ListenerMap — NodeId -> Vec<ListenerEntry>
// ---------------------------------------------------------------------------

pub(crate) type ListenerMap = HashMap<(usize, NodeId), Vec<ListenerEntry>>;

// ---------------------------------------------------------------------------
// Atomic counter for standalone EventTarget IDs.
// Start at usize::MAX / 2 to avoid collisions with DOM NodeIds (which start at 0).
// ---------------------------------------------------------------------------

pub(crate) static NEXT_EVENT_TARGET_ID: AtomicUsize = AtomicUsize::new(usize::MAX / 2);

fn next_event_target_id() -> usize {
    NEXT_EVENT_TARGET_ID.fetch_add(1, Ordering::Relaxed)
}

// ---------------------------------------------------------------------------
// Shared addEventListener/removeEventListener implementation
// ---------------------------------------------------------------------------

/// Parse the third argument to addEventListener/removeEventListener.
/// Returns (capture, once, passive, signal).
///
/// Handles both boolean shorthand and EventListenerOptions dictionary.
/// Non-object, non-boolean values are coerced via `to_boolean()` per spec
/// (e.g., `addEventListener("click", fn, 1)` → capture=true).
pub(crate) fn unified_parse_listener_options(
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<(bool, bool, Option<bool>, Option<JsObject>)> {
    let mut capture = false;
    let mut once = false;
    let mut passive = None;
    let mut signal = None;

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
            let p = opt_obj.get(js_string!("passive"), ctx)?;
            if !p.is_undefined() {
                passive = Some(p.to_boolean());
            }
            let s = opt_obj.get(js_string!("signal"), ctx)?;
            if !s.is_undefined() {
                if s.is_null() {
                    return Err(JsNativeError::typ()
                        .with_message(
                            "Failed to execute 'addEventListener' on 'EventTarget': member signal is not of type AbortSignal.",
                        )
                        .into());
                }
                if let Some(sig_obj) = s.as_object() {
                    signal = Some(sig_obj.clone());
                }
            }
        } else {
            // Coerce non-object values to boolean (handles booleans, numbers, strings, null, undefined)
            capture = opt_val.to_boolean();
        }
    }

    Ok((capture, once, passive, signal))
}

/// Shared body of addEventListener for all target types.
///
/// `listener_key`: the `(tree_ptr, node_id)` identifying this target.
/// `tree_for_passive`: if Some, used to check `is_passive_default_target`.
pub(crate) fn add_event_listener_impl(
    listener_key: (usize, NodeId),
    tree_for_passive: Option<&std::cell::RefCell<crate::dom::DomTree>>,
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let event_type = args
        .first()
        .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: missing type argument").into()))?
        .to_string(ctx)?
        .to_std_string_escaped();

    // Parse options BEFORE checking for null callback (spec: options getters must be invoked)
    let (capture, once, passive, signal) = unified_parse_listener_options(args, ctx)?;

    // If signal is already aborted, return early without adding the listener
    if let Some(ref sig_obj) = signal {
        if let Some(sig) = sig_obj.downcast_ref::<super::abort::JsAbortSignal>() {
            if sig.aborted.get() {
                return Ok(JsValue::undefined());
            }
        }
    }

    // Compute default passive value per spec §2.10 when not explicitly set
    let passive = match passive {
        Some(v) => Some(v),
        None => {
            if super::element::is_passive_default_event(&event_type) {
                let is_passive_target =
                    if listener_key == (usize::MAX, super::window::WINDOW_LISTENER_ID) {
                        true // window is always a passive-default target
                    } else if let Some(tree_ref) = tree_for_passive {
                        super::element::is_passive_default_target(listener_key.1, &tree_ref.borrow())
                    } else {
                        false
                    };
                if is_passive_target {
                    Some(true)
                } else {
                    None
                }
            } else {
                None
            }
        }
    };

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

    {
        let listeners = realm_state::event_listeners(ctx);
        let mut map = listeners.borrow_mut();
        let entries = map.entry(listener_key).or_default();

        let duplicate = entries
            .iter()
            .any(|entry| entry.event_type == event_type && entry.capture == capture && entry.callback == callback);

        if !duplicate {
            entries.push(ListenerEntry {
                event_type: event_type.clone(),
                callback: callback.clone(),
                capture,
                once,
                passive,
                removed: Rc::new(Cell::new(false)),
            });
        } else {
            return Ok(JsValue::undefined());
        }
    }

    // If signal was provided, register an abort listener that removes this listener
    if let Some(sig_obj) = signal {
        if let Some(sig) = sig_obj.downcast_ref::<super::abort::JsAbortSignal>() {
            let sig_id = sig.event_target_id;
            let removal_key = listener_key;
            let removal_type = event_type;
            let removal_callback = callback;
            let removal_capture = capture;

            let removal_fn = unsafe {
                boa_engine::native_function::NativeFunction::from_closure(move |_this, _args, ctx| {
                    remove_event_listener_impl(
                        removal_key,
                        &[
                            JsValue::from(js_string!(removal_type.as_str())),
                            JsValue::from(removal_callback.clone()),
                            JsValue::from(removal_capture),
                        ],
                        ctx,
                    )
                })
            };

            let handler_fn = removal_fn.to_js_function(ctx.realm());
            let listeners = realm_state::event_listeners(ctx);
            let mut map = listeners.borrow_mut();
            let entries = map.entry((0usize, sig_id)).or_default();
            entries.push(ListenerEntry {
                event_type: "abort".to_string(),
                callback: handler_fn.into(),
                capture: false,
                once: true,
                passive: None,
                removed: Rc::new(Cell::new(false)),
            });
        }
    }

    Ok(JsValue::undefined())
}

/// Shared body of removeEventListener for all target types.
pub(crate) fn remove_event_listener_impl(
    listener_key: (usize, NodeId),
    args: &[JsValue],
    ctx: &mut Context,
) -> JsResult<JsValue> {
    let event_type = args
        .first()
        .ok_or_else(|| {
            JsError::from_opaque(js_string!("removeEventListener: missing type argument").into())
        })?
        .to_string(ctx)?
        .to_std_string_escaped();

    // Parse options BEFORE checking for null callback (spec: options getters must be invoked)
    let mut capture = false;
    if let Some(opt_val) = args.get(2) {
        if let Some(opt_obj) = opt_val.as_object() {
            let c = opt_obj.get(js_string!("capture"), ctx)?;
            if !c.is_undefined() {
                capture = c.to_boolean();
            }
        } else {
            capture = opt_val.to_boolean();
        }
    }

    let callback_val = args
        .get(1)
        .ok_or_else(|| {
            JsError::from_opaque(js_string!("removeEventListener: missing callback argument").into())
        })?;

    if callback_val.is_null() || callback_val.is_undefined() {
        return Ok(JsValue::undefined());
    }

    let callback = callback_val
        .as_object()
        .ok_or_else(|| {
            JsError::from_opaque(js_string!("removeEventListener: callback is not an object").into())
        })?
        .clone();

    {
        let listeners = realm_state::event_listeners(ctx);
        let mut map = listeners.borrow_mut();
        if let Some(entries) = map.get_mut(&listener_key) {
            entries.retain(|entry| {
                if entry.event_type == event_type && entry.capture == capture && entry.callback == callback {
                    entry.removed.set(true);
                    false
                } else {
                    true
                }
            });
            if entries.is_empty() {
                map.remove(&listener_key);
            }
        }
    }

    Ok(JsValue::undefined())
}

// ---------------------------------------------------------------------------
// JsEventTarget — standalone EventTarget constructor
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsEventTarget {
    #[unsafe_ignore_trace]
    pub(crate) id: usize,
}

impl JsEventTarget {
    fn add_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let (listener_key, tree_for_passive) = resolve_event_target_key(this, ctx)?;
        add_event_listener_impl(listener_key, tree_for_passive.as_deref(), args, ctx)
    }

    fn remove_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let (listener_key, _tree) = resolve_event_target_key(this, ctx)?;
        remove_event_listener_impl(listener_key, args, ctx)
    }

    /// Universal dispatchEvent — handles standalone EventTarget, JsElement, JsDocument, and window.
    fn dispatch_event(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        // For JsElement or JsDocument, delegate to their own dispatch
        if let Some(obj) = this.as_object() {
            if obj.downcast_ref::<super::element::JsElement>().is_some() {
                return super::element::JsElement::dispatch_event_public(this, args, ctx);
            }
            if obj.downcast_ref::<super::document::JsDocument>().is_some() {
                return super::document::document_dispatch_event_public(this, args, ctx);
            }
        }
        // Check for window or null/undefined → delegate to window dispatch
        if this.is_null() || this.is_undefined() {
            return super::window::window_dispatch_event(args, ctx);
        }
        if let Some(obj) = this.as_object() {
            if let Some(window) = realm_state::window_object(ctx) {
                if obj.clone() == window {
                    return super::window::window_dispatch_event(args, ctx);
                }
            }
        }

        // Standalone EventTarget dispatch
        let this_obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: `this` is not an object").into()))?;
        let et = this_obj
            .downcast_ref::<JsEventTarget>()
            .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: `this` is not an EventTarget").into()))?;
        let id = et.id;

        let event_val = args
            .first()
            .ok_or_else(|| {
                JsError::from_native(boa_engine::JsNativeError::typ().with_message(
                    "Failed to execute 'dispatchEvent' on 'EventTarget': 1 argument required, but only 0 present.",
                ))
            })?
            .clone();

        if event_val.is_null() || event_val.is_undefined() {
            return Err(JsError::from_native(boa_engine::JsNativeError::typ().with_message(
                "Failed to execute 'dispatchEvent' on 'EventTarget': parameter 1 is not of type 'Event'.",
            )));
        }

        let event_obj = event_val
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: argument is not an object").into()))?
            .clone();

        // Read event type and check state
        let event_type = {
            let evt = event_obj
                .downcast_ref::<super::event::JsEvent>()
                .ok_or_else(|| JsError::from_opaque(js_string!("dispatchEvent: argument is not an Event").into()))?;
            if evt.dispatching {
                return Err(JsError::from_opaque(
                    js_string!("InvalidStateError: The event is already being dispatched.").into(),
                ));
            }
            evt.event_type.clone()
        };

        // Set dispatching flag and phase
        {
            let mut evt = event_obj.downcast_mut::<super::event::JsEvent>().unwrap();
            evt.dispatching = true;
            evt.phase = 2; // AT_TARGET
        }

        // Set event.target and event.currentTarget to `this`
        Self::set_event_prop(&event_obj, "target", this.clone(), ctx)?;
        Self::set_event_prop(&event_obj, "srcElement", this.clone(), ctx)?;
        Self::set_event_prop(&event_obj, "currentTarget", this.clone(), ctx)?;

        // Store `this` in realm state for composedPath() access during dispatch
        realm_state::set_dispatch_target(ctx, Some(this.clone()));

        // Invoke listeners at-target: all listeners in registration order
        let _should_stop = Self::invoke_listeners(id, &event_type, &event_obj, &event_val, ctx)?;

        // Clear dispatch target
        realm_state::set_dispatch_target(ctx, None);

        // Reset event state
        let default_prevented = {
            let mut evt = event_obj.downcast_mut::<super::event::JsEvent>().unwrap();
            evt.phase = 0;
            evt.dispatching = false;
            evt.propagation_stopped = false;
            evt.immediate_propagation_stopped = false;
            evt.default_prevented
        };

        // After dispatch: currentTarget is null, target stays
        Self::set_event_prop(&event_obj, "currentTarget", JsValue::null(), ctx)?;

        Ok(JsValue::from(!default_prevented))
    }

    /// Invoke all matching listeners for a standalone EventTarget at-target phase.
    fn invoke_listeners(
        id: usize,
        event_type: &str,
        event_obj: &JsObject,
        event_val: &JsValue,
        ctx: &mut Context,
    ) -> JsResult<bool> {
        // Snapshot listeners to avoid borrow issues (include removed flag for mid-dispatch removal)
        #[allow(clippy::type_complexity)]
        let matching: Vec<(JsObject, bool, Option<bool>, Rc<Cell<bool>>)> = {
            let listeners = realm_state::event_listeners(ctx);
            let map = listeners.borrow();
            match map.get(&(0usize, id)) {
                Some(entries) => entries
                    .iter()
                    .filter(|entry| entry.event_type == event_type)
                    .map(|entry| (entry.callback.clone(), entry.once, entry.passive, entry.removed.clone()))
                    .collect(),
                None => Vec::new(),
            }
        };

        // Save previous CURRENT_EVENT and set to current event (for window.event)
        let prev_event = realm_state::current_event(ctx);
        realm_state::set_current_event(ctx, Some(event_obj.clone()));

        for (callback, once, passive, removed_flag) in &matching {
            // Skip listeners that were removed during this dispatch
            if removed_flag.get() {
                continue;
            }

            if *once {
                removed_flag.set(true);
                let listeners = realm_state::event_listeners(ctx);
                let mut map = listeners.borrow_mut();
                if let Some(entries) = map.get_mut(&(0usize, id)) {
                    entries
                        .retain(|entry| !(entry.event_type == event_type && entry.callback == *callback && entry.once));
                    if entries.is_empty() {
                        map.remove(&(0usize, id));
                    }
                }
            }

            let is_passive = passive.unwrap_or(false);
            let call_result = if is_passive {
                let saved_cancelable = event_obj.downcast_ref::<super::event::JsEvent>().unwrap().cancelable;
                event_obj.downcast_mut::<super::event::JsEvent>().unwrap().cancelable = false;

                // Per spec: callable → call with this=currentTarget; object → look up handleEvent
                let current_target = event_obj
                    .get(js_string!("currentTarget"), ctx)
                    .unwrap_or(JsValue::undefined());
                let result = if callback.is_callable() {
                    callback.call(&current_target, std::slice::from_ref(event_val), ctx)
                } else {
                    match callback.get(js_string!("handleEvent"), ctx) {
                        Ok(handle) => {
                            if let Some(handle_fn) = handle.as_object().filter(|o| o.is_callable()) {
                                handle_fn.call(&JsValue::from(callback.clone()), std::slice::from_ref(event_val), ctx)
                            } else {
                                // Per spec: if handleEvent is not callable, throw TypeError
                                Err(JsNativeError::typ()
                                    .with_message("EventListener.handleEvent is not a function")
                                    .into())
                            }
                        }
                        Err(e) => Err(e),
                    }
                };

                event_obj.downcast_mut::<super::event::JsEvent>().unwrap().cancelable = saved_cancelable;

                result
            } else {
                // Per spec: callable → call with this=currentTarget; object → look up handleEvent
                let current_target = event_obj
                    .get(js_string!("currentTarget"), ctx)
                    .unwrap_or(JsValue::undefined());
                if callback.is_callable() {
                    callback.call(&current_target, std::slice::from_ref(event_val), ctx)
                } else {
                    match callback.get(js_string!("handleEvent"), ctx) {
                        Ok(handle) => {
                            if let Some(handle_fn) = handle.as_object().filter(|o| o.is_callable()) {
                                handle_fn.call(&JsValue::from(callback.clone()), std::slice::from_ref(event_val), ctx)
                            } else {
                                // Per spec: if handleEvent is not callable, throw TypeError
                                Err(JsNativeError::typ()
                                    .with_message("EventListener.handleEvent is not a function")
                                    .into())
                            }
                        }
                        Err(e) => Err(e),
                    }
                }
            };

            // If the listener threw, report via window.onerror and continue
            if let Err(err) = call_result {
                super::element::report_listener_error(err, ctx);
            }

            let imm_stopped = event_obj
                .downcast_ref::<super::event::JsEvent>()
                .unwrap()
                .immediate_propagation_stopped;

            if imm_stopped {
                // Restore previous CURRENT_EVENT before returning
                realm_state::set_current_event(ctx, prev_event.clone());
                return Ok(true);
            }
        }

        // Restore previous CURRENT_EVENT
        realm_state::set_current_event(ctx, prev_event);

        let propagation_stopped = event_obj
            .downcast_ref::<super::event::JsEvent>()
            .unwrap()
            .propagation_stopped;
        Ok(propagation_stopped)
    }

    fn set_event_prop(event_obj: &JsObject, name: &str, value: JsValue, ctx: &mut Context) -> JsResult<()> {
        event_obj.define_property_or_throw(
            js_string!(name),
            PropertyDescriptor::builder()
                .value(value)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )?;
        Ok(())
    }
}

impl Class for JsEventTarget {
    const NAME: &'static str = "EventTarget";
    const LENGTH: usize = 0;

    fn data_constructor(_new_target: &JsValue, _args: &[JsValue], _context: &mut Context) -> JsResult<Self> {
        Ok(JsEventTarget {
            id: next_event_target_id(),
        })
    }

    fn init(class: &mut ClassBuilder) -> JsResult<()> {
        class.method(
            js_string!("addEventListener"),
            2,
            NativeFunction::from_fn_ptr(Self::add_event_listener),
        );

        class.method(
            js_string!("removeEventListener"),
            2,
            NativeFunction::from_fn_ptr(Self::remove_event_listener),
        );

        class.method(
            js_string!("dispatchEvent"),
            1,
            NativeFunction::from_fn_ptr(Self::dispatch_event),
        );

        Ok(())
    }
}

/// composedPath() implementation for Event — returns [target] during dispatch, [] after.
pub(crate) fn composed_path(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let target = realm_state::dispatch_target(ctx);
    let array = boa_engine::object::builtins::JsArray::new(ctx);
    if let Some(t) = target {
        array.push(t, ctx)?;
    }
    Ok(array.into())
}
