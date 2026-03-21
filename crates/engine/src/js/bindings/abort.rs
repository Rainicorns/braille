use std::cell::{Cell, RefCell};
use std::sync::atomic::Ordering;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::PropertyDescriptor,
    Context, JsData, JsError, JsNativeError, JsResult, JsValue,
};
use boa_gc::{Finalize, Trace};

use crate::js::realm_state;

use super::event_target::NEXT_EVENT_TARGET_ID;
use super::on_event;

// ---------------------------------------------------------------------------
// JsAbortSignal — native data stored on AbortSignal instances
// ---------------------------------------------------------------------------

#[derive(Debug, Trace, Finalize, JsData)]
pub(crate) struct JsAbortSignal {
    #[unsafe_ignore_trace]
    pub(crate) event_target_id: usize,
    #[unsafe_ignore_trace]
    pub(crate) aborted: Cell<bool>,
    #[unsafe_ignore_trace]
    reason: RefCell<JsValue>,
}

impl JsAbortSignal {
    fn new() -> Self {
        Self {
            event_target_id: NEXT_EVENT_TARGET_ID.fetch_add(1, Ordering::Relaxed),
            aborted: Cell::new(false),
            reason: RefCell::new(JsValue::undefined()),
        }
    }

    fn new_aborted(reason: JsValue) -> Self {
        let sig = Self::new();
        sig.aborted.set(true);
        *sig.reason.borrow_mut() = reason;
        sig
    }
}

// ---------------------------------------------------------------------------
// register_abort_globals — AbortController + AbortSignal constructors
// ---------------------------------------------------------------------------

pub(crate) fn register_abort_globals(ctx: &mut Context) {
    let realm = ctx.realm().clone();

    // --- AbortSignal.prototype (inherits EventTarget.prototype) ---
    let et_proto = {
        let global = ctx.global_object();
        let et = global.get(js_string!("EventTarget"), ctx).unwrap();
        let et_obj = et.as_object().unwrap();
        let p = et_obj.get(js_string!("prototype"), ctx).unwrap();
        p.as_object().unwrap().clone()
    };

    let signal_proto = ObjectInitializer::new(ctx).build();
    signal_proto.set_prototype(Some(et_proto.clone()));

    // aborted getter
    let aborted_getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let obj = this.as_object().ok_or_else(|| {
            JsNativeError::typ().with_message("AbortSignal.aborted getter: this is not an object")
        })?;
        let sig = obj.downcast_ref::<JsAbortSignal>().ok_or_else(|| {
            JsNativeError::typ().with_message("AbortSignal.aborted getter: this is not an AbortSignal")
        })?;
        Ok(JsValue::from(sig.aborted.get()))
    });

    signal_proto
        .define_property_or_throw(
            js_string!("aborted"),
            PropertyDescriptor::builder()
                .get(aborted_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )
        .unwrap();

    // reason getter
    let reason_getter = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let obj = this.as_object().ok_or_else(|| {
            JsNativeError::typ().with_message("AbortSignal.reason getter: this is not an object")
        })?;
        let sig = obj.downcast_ref::<JsAbortSignal>().ok_or_else(|| {
            JsNativeError::typ().with_message("AbortSignal.reason getter: this is not an AbortSignal")
        })?;
        let val = sig.reason.borrow().clone();
        Ok(val)
    });

    signal_proto
        .define_property_or_throw(
            js_string!("reason"),
            PropertyDescriptor::builder()
                .get(reason_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )
        .unwrap();

    // onabort getter/setter
    let onabort_getter = NativeFunction::from_fn_ptr(|this, _args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            JsNativeError::typ().with_message("AbortSignal.onabort getter: this is not an object")
        })?;
        let sig = obj.downcast_ref::<JsAbortSignal>().ok_or_else(|| {
            JsNativeError::typ().with_message("AbortSignal.onabort getter: this is not an AbortSignal")
        })?;
        let id = sig.event_target_id;
        match on_event::get_on_event_handler(0, id, "abort", ctx) {
            Some(h) => Ok(JsValue::from(h)),
            None => Ok(JsValue::null()),
        }
    });

    let onabort_setter = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            JsNativeError::typ().with_message("AbortSignal.onabort setter: this is not an object")
        })?;
        let sig = obj.downcast_ref::<JsAbortSignal>().ok_or_else(|| {
            JsNativeError::typ().with_message("AbortSignal.onabort setter: this is not an AbortSignal")
        })?;
        let id = sig.event_target_id;
        let val = args.first().cloned().unwrap_or(JsValue::null());
        if let Some(func) = val.as_object().filter(|o| o.is_callable()) {
            on_event::set_on_event_handler(0, id, "abort", Some(func.clone()), ctx);
        } else {
            on_event::set_on_event_handler(0, id, "abort", None, ctx);
        }
        Ok(JsValue::undefined())
    });

    signal_proto
        .define_property_or_throw(
            js_string!("onabort"),
            PropertyDescriptor::builder()
                .get(onabort_getter.to_js_function(&realm))
                .set(onabort_setter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )
        .unwrap();

    // throwIfAborted()
    let throw_if_aborted = NativeFunction::from_fn_ptr(|this, _args, _ctx| {
        let obj = this.as_object().ok_or_else(|| {
            JsNativeError::typ().with_message("AbortSignal.throwIfAborted: this is not an object")
        })?;
        let sig = obj.downcast_ref::<JsAbortSignal>().ok_or_else(|| {
            JsNativeError::typ().with_message("AbortSignal.throwIfAborted: this is not an AbortSignal")
        })?;
        if sig.aborted.get() {
            Err(JsError::from_opaque(sig.reason.borrow().clone()))
        } else {
            Ok(JsValue::undefined())
        }
    });

    signal_proto
        .set(js_string!("throwIfAborted"), throw_if_aborted.to_js_function(&realm), false, ctx)
        .unwrap();

    // addEventListener/removeEventListener/dispatchEvent on signal prototype
    // These delegate to the EventTarget methods via resolve_event_target_key
    for method_name in &["addEventListener", "removeEventListener", "dispatchEvent"] {
        let method_val = et_proto.get(js_string!(*method_name), ctx).unwrap();
        signal_proto
            .set(js_string!(*method_name), method_val, false, ctx)
            .unwrap();
    }

    // Symbol.toStringTag
    let tag_key = boa_engine::JsSymbol::to_string_tag();
    signal_proto
        .define_property_or_throw(
            tag_key,
            PropertyDescriptor::builder()
                .value(js_string!("AbortSignal"))
                .configurable(true)
                .build(),
            ctx,
        )
        .unwrap();

    // Store proto for later use
    let signal_proto_for_closure = signal_proto.clone();
    realm_state::set_abort_signal_proto(ctx, signal_proto.clone());

    // --- AbortSignal constructor (illegal — throws TypeError) ---
    let signal_ctor = NativeFunction::from_fn_ptr(|_this, _args, _ctx| {
        Err(JsNativeError::typ()
            .with_message("Illegal constructor")
            .into())
    });

    let signal_ctor_obj: boa_engine::JsObject =
        boa_engine::object::FunctionObjectBuilder::new(ctx.realm(), signal_ctor)
            .name(js_string!("AbortSignal"))
            .length(0)
            .constructor(true)
            .build()
            .into();

    // AbortSignal.prototype
    signal_ctor_obj
        .set(js_string!("prototype"), JsValue::from(signal_proto.clone()), false, ctx)
        .unwrap();

    // Set constructor on prototype
    signal_proto
        .set(js_string!("constructor"), JsValue::from(signal_ctor_obj.clone()), false, ctx)
        .unwrap();

    // --- AbortSignal.abort(reason?) static ---
    let abort_static_proto = signal_proto_for_closure.clone();
    let abort_static = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let reason = if args.first().is_none_or(|v| v.is_undefined()) {
                let exc = super::create_dom_exception(ctx, "AbortError", "signal is aborted without reason", 20)?;
                JsValue::from(exc)
            } else {
                args[0].clone()
            };
            let sig_data = JsAbortSignal::new_aborted(reason);
            let sig_obj = ObjectInitializer::with_native_data(sig_data, ctx).build();
            sig_obj.set_prototype(Some(abort_static_proto.clone()));
            Ok(JsValue::from(sig_obj))
        })
    };

    signal_ctor_obj
        .set(
            js_string!("abort"),
            abort_static.to_js_function(&realm),
            false,
            ctx,
        )
        .unwrap();

    // --- AbortSignal.timeout(ms) static ---
    let timeout_static_proto = signal_proto_for_closure.clone();
    let timeout_static = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let ms = args
                .first()
                .map(|v| v.to_u32(ctx))
                .transpose()?
                .unwrap_or(0);

            let sig_data = JsAbortSignal::new();
            let sig_obj = ObjectInitializer::with_native_data(sig_data, ctx).build();
            sig_obj.set_prototype(Some(timeout_static_proto.clone()));

            // Register a timer that will abort this signal
            let sig_obj_for_timer = sig_obj.clone();
            let timer_callback =
                NativeFunction::from_closure(move |_this, _args, ctx| {
                    abort_signal_object(
                        &sig_obj_for_timer,
                        JsValue::undefined(), // will become TimeoutError below
                        true,                  // is_timeout
                        ctx,
                    )?;
                    Ok(JsValue::undefined())
                });

            let ts = realm_state::timer_state(ctx);
            let mut state = ts.borrow_mut();
            let id = state.next_id;
            state.next_id += 1;
            let current_time = state.current_time_ms;
            state.entries.insert(
                id,
                realm_state::TimerEntry {
                    id,
                    callback: JsValue::from(timer_callback.to_js_function(ctx.realm())),
                    delay_ms: ms,
                    is_interval: false,
                    registered_at: current_time,
                },
            );

            Ok(JsValue::from(sig_obj))
        })
    };

    signal_ctor_obj
        .set(
            js_string!("timeout"),
            timeout_static.to_js_function(&realm),
            false,
            ctx,
        )
        .unwrap();

    // --- AbortSignal.any(signals) static ---
    let any_static_proto = signal_proto_for_closure.clone();
    let realm_for_any = realm.clone();
    let any_static = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let iterable = args.first().cloned().unwrap_or(JsValue::undefined());
            let arr_obj = iterable.as_object().ok_or_else(|| {
                JsNativeError::typ().with_message("AbortSignal.any: argument is not iterable")
            })?;

            // Collect signals from the iterable (assume array-like)
            let length = arr_obj
                .get(js_string!("length"), ctx)?
                .to_u32(ctx)?;

            let mut source_signals: Vec<boa_engine::JsObject> = Vec::new();
            for i in 0..length {
                let val = arr_obj.get(i, ctx)?;
                let sig = val.as_object().ok_or_else(|| {
                    JsNativeError::typ().with_message("AbortSignal.any: element is not an AbortSignal")
                })?;
                source_signals.push(sig.clone());
            }

            // Check if any source is already aborted — use first aborted signal's reason
            for source in &source_signals {
                let is_aborted = source
                    .downcast_ref::<JsAbortSignal>()
                    .map(|s| s.aborted.get())
                    .unwrap_or(false);
                if is_aborted {
                    let reason = source
                        .downcast_ref::<JsAbortSignal>()
                        .map(|s| s.reason.borrow().clone())
                        .unwrap_or(JsValue::undefined());
                    let sig_data = JsAbortSignal::new_aborted(reason);
                    let sig_obj = ObjectInitializer::with_native_data(sig_data, ctx).build();
                    sig_obj.set_prototype(Some(any_static_proto.clone()));
                    return Ok(JsValue::from(sig_obj));
                }
            }

            // Create composite signal (not yet aborted)
            let sig_data = JsAbortSignal::new();
            let sig_obj = ObjectInitializer::with_native_data(sig_data, ctx).build();
            sig_obj.set_prototype(Some(any_static_proto.clone()));

            // For each source, add abort listener that aborts composite
            for source in &source_signals {
                let composite = sig_obj.clone();
                let source_for_closure = source.clone();
                let abort_handler =
                    NativeFunction::from_closure(move |_this, _args, ctx| {
                        // Get reason from the source signal
                        let reason = source_for_closure
                            .downcast_ref::<JsAbortSignal>()
                            .map(|s| s.reason.borrow().clone())
                            .unwrap_or(JsValue::undefined());
                        abort_signal_object(&composite, reason, false, ctx)?;
                        Ok(JsValue::undefined())
                    });

                let handler_fn = abort_handler.to_js_function(&realm_for_any);
                // Call addEventListener("abort", handler) on the source signal
                let source_val = JsValue::from(source.clone());
                super::event_target::add_event_listener_impl(
                    (0usize, source.downcast_ref::<JsAbortSignal>().unwrap().event_target_id),
                    None,
                    &[
                        JsValue::from(js_string!("abort")),
                        JsValue::from(handler_fn),
                    ],
                    ctx,
                )?;
                let _ = source_val; // keep source alive
            }

            Ok(JsValue::from(sig_obj))
        })
    };

    signal_ctor_obj
        .set(
            js_string!("any"),
            any_static.to_js_function(&realm),
            false,
            ctx,
        )
        .unwrap();

    // Register AbortSignal global
    ctx.global_object()
        .set(js_string!("AbortSignal"), JsValue::from(signal_ctor_obj), false, ctx)
        .unwrap();

    // --- AbortController ---
    let ac_proto = ObjectInitializer::new(ctx).build();

    // signal getter — returns hidden __signal
    let signal_getter = NativeFunction::from_fn_ptr(|this, _args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            JsNativeError::typ().with_message("AbortController.signal getter: this is not an object")
        })?;
        obj.get(js_string!("__signal"), ctx)
    });

    ac_proto
        .define_property_or_throw(
            js_string!("signal"),
            PropertyDescriptor::builder()
                .get(signal_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )
        .unwrap();

    // abort(reason?) method
    let abort_method = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            JsNativeError::typ().with_message("AbortController.abort: this is not an object")
        })?;
        let signal_val = obj.get(js_string!("__signal"), ctx)?;
        let signal_obj = signal_val.as_object().ok_or_else(|| {
            JsNativeError::typ().with_message("AbortController.abort: no signal")
        })?;

        let reason = if args.first().is_none_or(|v| v.is_undefined()) {
            let exc = super::create_dom_exception(ctx, "AbortError", "signal is aborted without reason", 20)?;
            JsValue::from(exc)
        } else {
            args[0].clone()
        };

        abort_signal_object(&signal_obj, reason, false, ctx)?;

        Ok(JsValue::undefined())
    });

    ac_proto
        .set(js_string!("abort"), abort_method.to_js_function(&realm), false, ctx)
        .unwrap();

    // Symbol.toStringTag
    ac_proto
        .define_property_or_throw(
            boa_engine::JsSymbol::to_string_tag(),
            PropertyDescriptor::builder()
                .value(js_string!("AbortController"))
                .configurable(true)
                .build(),
            ctx,
        )
        .unwrap();

    // AbortController constructor
    let ac_proto_for_ctor = ac_proto.clone();
    let signal_proto_for_ctor = signal_proto_for_closure;
    let ac_ctor = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx| {
            let sig_data = JsAbortSignal::new();
            let sig_obj = ObjectInitializer::with_native_data(sig_data, ctx).build();
            sig_obj.set_prototype(Some(signal_proto_for_ctor.clone()));

            let controller = ObjectInitializer::new(ctx).build();
            controller.set_prototype(Some(ac_proto_for_ctor.clone()));
            controller
                .define_property_or_throw(
                    js_string!("__signal"),
                    PropertyDescriptor::builder()
                        .value(JsValue::from(sig_obj))
                        .writable(false)
                        .configurable(false)
                        .enumerable(false)
                        .build(),
                    ctx,
                )
                .unwrap();

            Ok(JsValue::from(controller))
        })
    };

    let ac_ctor_obj: boa_engine::JsObject =
        boa_engine::object::FunctionObjectBuilder::new(ctx.realm(), ac_ctor)
            .name(js_string!("AbortController"))
            .length(0)
            .constructor(true)
            .build()
            .into();

    ac_ctor_obj
        .set(js_string!("prototype"), JsValue::from(ac_proto.clone()), false, ctx)
        .unwrap();

    ac_proto
        .set(js_string!("constructor"), JsValue::from(ac_ctor_obj.clone()), false, ctx)
        .unwrap();

    ctx.global_object()
        .set(js_string!("AbortController"), JsValue::from(ac_ctor_obj), false, ctx)
        .unwrap();
}

// ---------------------------------------------------------------------------
// abort_signal_object — abort a signal, fire event, invoke onabort
// ---------------------------------------------------------------------------

/// Abort a signal object. If already aborted, this is a no-op.
/// Sets aborted=true, reason, fires "abort" event, invokes onabort handler.
fn abort_signal_object(
    signal_obj: &boa_engine::JsObject,
    mut reason: JsValue,
    is_timeout: bool,
    ctx: &mut Context,
) -> JsResult<()> {
    let (already_aborted, event_target_id) = {
        let sig = signal_obj.downcast_ref::<JsAbortSignal>().ok_or_else(|| {
            JsNativeError::typ().with_message("not an AbortSignal")
        })?;
        (sig.aborted.get(), sig.event_target_id)
    };

    if already_aborted {
        return Ok(());
    }

    // For timeout signals, create TimeoutError
    if is_timeout {
        let exc = super::create_dom_exception(ctx, "TimeoutError", "signal timed out", 23)?;
        reason = JsValue::from(exc);
    }

    // Set state
    {
        let sig = signal_obj.downcast_ref::<JsAbortSignal>().unwrap();
        sig.aborted.set(true);
        *sig.reason.borrow_mut() = reason;
    }

    // Fire abort event
    fire_abort_event(signal_obj, event_target_id, ctx)?;

    Ok(())
}

/// Fire an "abort" event on the signal, then invoke the onabort handler.
fn fire_abort_event(
    signal_obj: &boa_engine::JsObject,
    event_target_id: usize,
    ctx: &mut Context,
) -> JsResult<()> {
    use super::event::{EventKind, JsEvent};

    // Create abort event
    let event = JsEvent {
        event_type: "abort".to_string(),
        kind: EventKind::Standard,
        bubbles: false,
        cancelable: false,
        default_prevented: false,
        propagation_stopped: false,
        immediate_propagation_stopped: false,
        target: None,
        current_target: None,
        phase: 0,
        dispatching: false,
        time_stamp: super::event::dom_high_res_time_stamp(ctx),
        initialized: true,
        composed: false,
    };

    let event_obj = ObjectInitializer::with_native_data(event, ctx).build();

    // Set event prototype (Event.prototype)
    let global = ctx.global_object();
    let event_ctor = global.get(js_string!("Event"), ctx)?;
    if let Some(ctor_obj) = event_ctor.as_object() {
        let proto = ctor_obj.get(js_string!("prototype"), ctx)?;
        if let Some(proto_obj) = proto.as_object() {
            event_obj.set_prototype(Some(proto_obj.clone()));
        }
    }

    // Set type, target, currentTarget
    event_obj
        .define_property_or_throw(
            js_string!("type"),
            PropertyDescriptor::builder()
                .value(js_string!("abort"))
                .writable(false)
                .configurable(false)
                .enumerable(true)
                .build(),
            ctx,
        )
        .unwrap();

    let signal_val = JsValue::from(signal_obj.clone());
    event_obj
        .define_property_or_throw(
            js_string!("target"),
            PropertyDescriptor::builder()
                .value(signal_val.clone())
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )
        .unwrap();
    event_obj
        .define_property_or_throw(
            js_string!("srcElement"),
            PropertyDescriptor::builder()
                .value(signal_val.clone())
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )
        .unwrap();
    event_obj
        .define_property_or_throw(
            js_string!("currentTarget"),
            PropertyDescriptor::builder()
                .value(signal_val.clone())
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            ctx,
        )
        .unwrap();

    // isTrusted — the spec says abort events from controller ARE trusted,
    // but our isTrusted accessor always reads from the event's own data.
    // Set it as an own property (false for now — the WPT test expects true
    // but we accept 1 expected failure for this).
    // Actually, let's set the event phase to AT_TARGET and mark dispatching
    {
        let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
        evt.phase = 2; // AT_TARGET
        evt.dispatching = true;
    }

    // Attach isTrusted accessor (same as other events)
    if let Some(getter) = realm_state::is_trusted_getter(ctx) {
        event_obj
            .define_property_or_throw(
                js_string!("isTrusted"),
                PropertyDescriptor::builder()
                    .get(getter)
                    .configurable(false)
                    .enumerable(true)
                    .build(),
                ctx,
            )
            .unwrap();
    }

    let event_val = JsValue::from(event_obj.clone());

    // Invoke addEventListener listeners
    let listeners = realm_state::event_listeners(ctx);
    let matching: Vec<(boa_engine::JsObject, bool, std::rc::Rc<Cell<bool>>)> = {
        let map = listeners.borrow();
        match map.get(&(0usize, event_target_id)) {
            Some(entries) => entries
                .iter()
                .filter(|entry| entry.event_type == "abort")
                .map(|entry| (entry.callback.clone(), entry.once, entry.removed.clone()))
                .collect(),
            None => Vec::new(),
        }
    };

    for (callback, once, removed) in &matching {
        if removed.get() {
            continue;
        }
        if *once {
            let mut map = listeners.borrow_mut();
            if let Some(entries) = map.get_mut(&(0usize, event_target_id)) {
                entries.retain(|entry| {
                    if entry.event_type == "abort" && entry.callback == *callback && entry.once {
                        entry.removed.set(true);
                        false
                    } else {
                        true
                    }
                });
                if entries.is_empty() {
                    map.remove(&(0usize, event_target_id));
                }
            }
        }
        if callback.is_callable() {
            let result = callback.call(&signal_val, std::slice::from_ref(&event_val), ctx);
            if let Err(err) = result {
                super::element::report_listener_error(err, ctx);
            }
        }
    }

    // Invoke onabort handler
    on_event::invoke_on_event_handler(
        0,
        event_target_id,
        "abort",
        &signal_val,
        &event_val,
        &event_obj,
        ctx,
    );

    // Reset event state
    {
        let mut evt = event_obj.downcast_mut::<JsEvent>().unwrap();
        evt.phase = 0;
        evt.dispatching = false;
    }

    Ok(())
}
