use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

use boa_engine::{
    class::{Class, ClassBuilder},
    js_string,
    native_function::NativeFunction,
    object::JsObject,
    property::PropertyDescriptor,
    Context, JsData, JsError, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::dom::NodeId;

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
}

// ---------------------------------------------------------------------------
// ListenerMap — NodeId -> Vec<ListenerEntry>
// ---------------------------------------------------------------------------

pub(crate) type ListenerMap = HashMap<(usize, NodeId), Vec<ListenerEntry>>;

// ---------------------------------------------------------------------------
// Thread-local storage for the listener map.
// This allows NativeFunction callbacks (addEventListener, removeEventListener)
// to access the listener map without needing a reference to JsRuntime.
// ---------------------------------------------------------------------------

thread_local! {
    pub(crate) static EVENT_LISTENERS: RefCell<Option<Rc<RefCell<ListenerMap>>>> = const { RefCell::new(None) };
}

// ---------------------------------------------------------------------------
// Atomic counter for standalone EventTarget IDs.
// Start at usize::MAX / 2 to avoid collisions with DOM NodeIds (which start at 0).
// ---------------------------------------------------------------------------

static NEXT_EVENT_TARGET_ID: AtomicUsize = AtomicUsize::new(usize::MAX / 2);

fn next_event_target_id() -> usize {
    NEXT_EVENT_TARGET_ID.fetch_add(1, Ordering::Relaxed)
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
    /// Parse the third argument to addEventListener/removeEventListener.
    /// Returns (capture, once, passive).
    fn parse_listener_options(args: &[JsValue], ctx: &mut Context) -> JsResult<(bool, bool, Option<bool>)> {
        let mut capture = false;
        let mut once = false;
        let mut passive = None;

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
                let p = opt_obj.get(js_string!("passive"), ctx)?;
                if !p.is_undefined() {
                    passive = Some(p.to_boolean());
                }
            }
        }

        Ok((capture, once, passive))
    }

    fn add_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let this_obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: `this` is not an object").into()))?;
        let et = this_obj
            .downcast_ref::<JsEventTarget>()
            .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: `this` is not an EventTarget").into()))?;
        let id = et.id;

        let event_type = args
            .first()
            .ok_or_else(|| JsError::from_opaque(js_string!("addEventListener: missing type argument").into()))?
            .to_string(ctx)?
            .to_std_string_escaped();

        // Parse options BEFORE checking for null callback (spec: options getters must be invoked)
        let (capture, once, passive) = Self::parse_listener_options(args, ctx)?;

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
            let entries = map.entry((0usize, id)).or_insert_with(Vec::new);

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
                    passive,
                });
            }
        });

        Ok(JsValue::undefined())
    }

    fn remove_event_listener(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
        let this_obj = this
            .as_object()
            .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: `this` is not an object").into()))?;
        let et = this_obj
            .downcast_ref::<JsEventTarget>()
            .ok_or_else(|| JsError::from_opaque(js_string!("removeEventListener: `this` is not an EventTarget").into()))?;
        let id = et.id;

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

        // For removeEventListener, only capture matters — passive is not considered
        let mut capture = false;
        if let Some(opt_val) = args.get(2) {
            if let Some(b) = opt_val.as_boolean() {
                capture = b;
            } else if let Some(opt_obj) = opt_val.as_object() {
                let c = opt_obj.get(js_string!("capture"), ctx)?;
                if !c.is_undefined() {
                    capture = c.to_boolean();
                }
            }
        }

        EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
            let mut map = listeners_rc.borrow_mut();
            if let Some(entries) = map.get_mut(&(0usize, id)) {
                entries.retain(|entry| {
                    !(entry.event_type == event_type
                        && entry.capture == capture
                        && entry.callback == callback)
                });
                if entries.is_empty() {
                    map.remove(&(0usize, id));
                }
            }
        });

        Ok(JsValue::undefined())
    }

    /// dispatchEvent for standalone EventTarget.
    /// No DOM tree, so no capture/bubble phases — just at-target.
    fn dispatch_event(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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
                JsError::from_native(
                    boa_engine::JsNativeError::typ()
                        .with_message("Failed to execute 'dispatchEvent' on 'EventTarget': 1 argument required, but only 0 present.")
                )
            })?
            .clone();

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

        // Determine event type — check JsEvent or JsCustomEvent
        let is_custom_event;
        let event_type;
        {
            if let Some(evt) = event_obj.downcast_ref::<super::event::JsEvent>() {
                is_custom_event = false;
                event_type = evt.event_type.clone();

                if evt.dispatching {
                    return Err(JsError::from_opaque(
                        js_string!("InvalidStateError: The event is already being dispatched.").into(),
                    ));
                }
            } else if let Some(evt) = event_obj.downcast_ref::<super::event::JsCustomEvent>() {
                is_custom_event = true;
                event_type = evt.event_type.clone();

                if evt.dispatching {
                    return Err(JsError::from_opaque(
                        js_string!("InvalidStateError: The event is already being dispatched.").into(),
                    ));
                }
            } else {
                return Err(JsError::from_opaque(js_string!("dispatchEvent: argument is not an Event").into()));
            }
        }

        // Set dispatching flag and phase
        if is_custom_event {
            let mut evt = event_obj.downcast_mut::<super::event::JsCustomEvent>().unwrap();
            evt.dispatching = true;
            evt.phase = 2; // AT_TARGET
        } else {
            let mut evt = event_obj.downcast_mut::<super::event::JsEvent>().unwrap();
            evt.dispatching = true;
            evt.phase = 2; // AT_TARGET
        }

        // Set event.target and event.currentTarget to `this`
        Self::set_event_prop(&event_obj, "target", this.clone(), ctx)?;
        Self::set_event_prop(&event_obj, "srcElement", this.clone(), ctx)?;
        Self::set_event_prop(&event_obj, "currentTarget", this.clone(), ctx)?;

        // Store `this` in a thread-local for composedPath() access during dispatch
        DISPATCH_TARGET.with(|cell| {
            *cell.borrow_mut() = Some(this.clone());
        });

        // Invoke listeners at-target: all listeners in registration order
        let _should_stop = Self::invoke_listeners(id, &event_type, &event_obj, &event_val, ctx)?;

        // Clear dispatch target
        DISPATCH_TARGET.with(|cell| {
            *cell.borrow_mut() = None;
        });

        // Reset event state
        let default_prevented = if is_custom_event {
            let mut evt = event_obj.downcast_mut::<super::event::JsCustomEvent>().unwrap();
            evt.phase = 0;
            evt.dispatching = false;
            evt.propagation_stopped = false;
            evt.immediate_propagation_stopped = false;
            evt.default_prevented
        } else {
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
        // Snapshot listeners to avoid borrow issues
        let matching: Vec<(JsObject, bool, Option<bool>)> = EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
            let map = listeners_rc.borrow();
            match map.get(&(0usize, id)) {
                Some(entries) => entries
                    .iter()
                    .filter(|entry| entry.event_type == event_type)
                    .map(|entry| (entry.callback.clone(), entry.once, entry.passive))
                    .collect(),
                None => Vec::new(),
            }
        });

        // Save previous CURRENT_EVENT and set to current event (for window.event)
        let prev_event = super::element::CURRENT_EVENT.with(|cell| cell.borrow().clone());
        super::element::CURRENT_EVENT.with(|cell| {
            *cell.borrow_mut() = Some(event_obj.clone());
        });

        for (callback, once, passive) in &matching {
            if *once {
                EVENT_LISTENERS.with(|el| {
                    let rc = el.borrow();
                    let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
                    let mut map = listeners_rc.borrow_mut();
                    if let Some(entries) = map.get_mut(&(0usize, id)) {
                        entries.retain(|entry| {
                            !(entry.event_type == event_type && entry.callback == *callback && entry.once)
                        });
                        if entries.is_empty() {
                            map.remove(&(0usize, id));
                        }
                    }
                });
            }

            let is_passive = passive.unwrap_or(false);
            let call_result = if is_passive {
                let saved_cancelable;
                if let Some(evt) = event_obj.downcast_ref::<super::event::JsEvent>() {
                    saved_cancelable = evt.cancelable;
                } else if let Some(evt) = event_obj.downcast_ref::<super::event::JsCustomEvent>() {
                    saved_cancelable = evt.cancelable;
                } else {
                    saved_cancelable = false;
                }

                if let Some(mut evt) = event_obj.downcast_mut::<super::event::JsEvent>() {
                    evt.cancelable = false;
                } else if let Some(mut evt) = event_obj.downcast_mut::<super::event::JsCustomEvent>() {
                    evt.cancelable = false;
                }

                // Per spec: callable → call with this=currentTarget; object → look up handleEvent
                let current_target = event_obj.get(js_string!("currentTarget"), ctx).unwrap_or(JsValue::undefined());
                let result = if callback.is_callable() {
                    callback.call(&current_target, std::slice::from_ref(event_val), ctx)
                } else {
                    match callback.get(js_string!("handleEvent"), ctx) {
                        Ok(handle) => {
                            if let Some(handle_fn) = handle.as_object().filter(|o| o.is_callable()) {
                                handle_fn.call(&JsValue::from(callback.clone()), std::slice::from_ref(event_val), ctx)
                            } else {
                                Ok(JsValue::undefined())
                            }
                        }
                        Err(e) => Err(e),
                    }
                };

                if let Some(mut evt) = event_obj.downcast_mut::<super::event::JsEvent>() {
                    evt.cancelable = saved_cancelable;
                } else if let Some(mut evt) = event_obj.downcast_mut::<super::event::JsCustomEvent>() {
                    evt.cancelable = saved_cancelable;
                }

                result
            } else {
                // Per spec: callable → call with this=currentTarget; object → look up handleEvent
                let current_target = event_obj.get(js_string!("currentTarget"), ctx).unwrap_or(JsValue::undefined());
                if callback.is_callable() {
                    callback.call(&current_target, std::slice::from_ref(event_val), ctx)
                } else {
                    match callback.get(js_string!("handleEvent"), ctx) {
                        Ok(handle) => {
                            if let Some(handle_fn) = handle.as_object().filter(|o| o.is_callable()) {
                                handle_fn.call(&JsValue::from(callback.clone()), std::slice::from_ref(event_val), ctx)
                            } else {
                                Ok(JsValue::undefined())
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

            let imm_stopped = if let Some(evt) = event_obj.downcast_ref::<super::event::JsEvent>() {
                evt.immediate_propagation_stopped
            } else if let Some(evt) = event_obj.downcast_ref::<super::event::JsCustomEvent>() {
                evt.immediate_propagation_stopped
            } else {
                false
            };

            if imm_stopped {
                // Restore previous CURRENT_EVENT before returning
                super::element::CURRENT_EVENT.with(|cell| {
                    *cell.borrow_mut() = prev_event.clone();
                });
                return Ok(true);
            }
        }

        // Restore previous CURRENT_EVENT
        super::element::CURRENT_EVENT.with(|cell| {
            *cell.borrow_mut() = prev_event;
        });

        let propagation_stopped = if let Some(evt) = event_obj.downcast_ref::<super::event::JsEvent>() {
            evt.propagation_stopped
        } else if let Some(evt) = event_obj.downcast_ref::<super::event::JsCustomEvent>() {
            evt.propagation_stopped
        } else {
            false
        };
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

    fn data_constructor(
        _new_target: &JsValue,
        _args: &[JsValue],
        _context: &mut Context,
    ) -> JsResult<Self> {
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

// ---------------------------------------------------------------------------
// Thread-local for current dispatch target (for composedPath())
// ---------------------------------------------------------------------------

thread_local! {
    pub(crate) static DISPATCH_TARGET: RefCell<Option<JsValue>> = const { RefCell::new(None) };
}

/// composedPath() implementation for Event — returns [target] during dispatch, [] after.
pub(crate) fn composed_path(_this: &JsValue, _args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let target = DISPATCH_TARGET.with(|cell| cell.borrow().clone());
    let array = boa_engine::object::builtins::JsArray::new(ctx);
    if let Some(t) = target {
        array.push(t, ctx)?;
    }
    Ok(array.into())
}
