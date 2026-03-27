mod console;
mod frames;
mod history;
mod location;
mod match_media;
mod navigator;
mod timers;

#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::{builtins::JsArray, ObjectInitializer},
    property::{Attribute, PropertyDescriptor},
    Context, JsResult, JsValue,
};

use crate::js::realm_state;
use crate::js::realm_state::TimerEntry;

pub(crate) use console::ConsoleBuffer;

/// Well-known ID for window in the event listeners map.
/// Uses usize::MAX - 1 to avoid collision with DOM NodeIds (start at 0)
/// and standalone EventTarget IDs (start at usize::MAX / 2).
pub(crate) const WINDOW_LISTENER_ID: usize = usize::MAX - 1;

/// Public window dispatchEvent — called from EventTarget.prototype.dispatchEvent for window `this`.
pub(crate) fn window_dispatch_event_with_this(this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    let event_val = args.first().cloned().unwrap_or(JsValue::undefined());
    if event_val.is_null() || event_val.is_undefined() {
        return Ok(JsValue::from(true));
    }
    let event_obj = match event_val.as_object() {
        Some(o) => o.clone(),
        None => return Ok(JsValue::from(true)),
    };

    let event_type = match event_obj.downcast_ref::<super::event::JsEvent>() {
        Some(evt) => evt.event_type.clone(),
        None => return Ok(JsValue::from(true)),
    };

    // Retarget relatedTarget for window dispatch (non-node target)
    super::event_target::retarget_related_target_for_non_node(&event_obj, ctx)?;

    {
        let mut evt = event_obj.downcast_mut::<super::event::JsEvent>().unwrap();
        evt.dispatching = true;
        evt.phase = 2;
    }

    // Use `this` as the target so that `event.target === self` works
    // (self may be the global object, which differs from our window object)
    let target_val = this.clone();

    event_obj.define_property_or_throw(
        js_string!("target"),
        PropertyDescriptor::builder()
            .value(target_val.clone())
            .writable(true)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;
    event_obj.define_property_or_throw(
        js_string!("srcElement"),
        PropertyDescriptor::builder()
            .value(target_val.clone())
            .writable(true)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;
    event_obj.define_property_or_throw(
        js_string!("currentTarget"),
        PropertyDescriptor::builder()
            .value(target_val)
            .writable(true)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    super::element::invoke_listeners_for_node(
        (usize::MAX, WINDOW_LISTENER_ID),
        &event_type,
        &event_obj,
        &event_val,
        false,
        true,
        ctx,
    )?;

    let default_prevented = {
        let mut evt = event_obj.downcast_mut::<super::event::JsEvent>().unwrap();
        evt.phase = 0;
        evt.dispatching = false;
        evt.propagation_stopped = false;
        evt.immediate_propagation_stopped = false;
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

pub(crate) fn register_window(
    context: &mut Context,
    console_output: ConsoleBuffer,
    tree: Rc<RefCell<crate::dom::DomTree>>,
) {
    let console_log = console::make_console_method(Rc::clone(&console_output), None);
    let console_warn = console::make_console_method(Rc::clone(&console_output), Some("WARN: "));
    let console_error = console::make_console_method(Rc::clone(&console_output), Some("ERROR: "));
    let console_info = console::make_console_method(Rc::clone(&console_output), Some("INFO: "));

    let console = ObjectInitializer::new(context)
        .function(console_log, js_string!("log"), 0)
        .function(console_warn, js_string!("warn"), 0)
        .function(console_error, js_string!("error"), 0)
        .function(console_info, js_string!("info"), 0)
        .build();

    context
        .register_global_property(js_string!("console"), console, Attribute::all())
        .expect("failed to register console global");

    let set_timeout = timers::make_set_timer(false);
    let clear_timeout = timers::make_clear_timer();
    let set_interval = timers::make_set_timer(true);
    let clear_interval = timers::make_clear_timer();

    // Register timer functions as globals (testharness.js calls them without window. prefix)
    let g_set_timeout = timers::make_set_timer(false);
    let g_clear_timeout = timers::make_clear_timer();
    let g_set_interval = timers::make_set_timer(true);
    let g_clear_interval = timers::make_clear_timer();
    context
        .register_global_property(
            js_string!("setTimeout"),
            g_set_timeout.to_js_function(context.realm()),
            Attribute::all(),
        )
        .expect("failed to register setTimeout global");
    context
        .register_global_property(
            js_string!("clearTimeout"),
            g_clear_timeout.to_js_function(context.realm()),
            Attribute::all(),
        )
        .expect("failed to register clearTimeout global");
    context
        .register_global_property(
            js_string!("setInterval"),
            g_set_interval.to_js_function(context.realm()),
            Attribute::all(),
        )
        .expect("failed to register setInterval global");
    context
        .register_global_property(
            js_string!("clearInterval"),
            g_clear_interval.to_js_function(context.realm()),
            Attribute::all(),
        )
        .expect("failed to register clearInterval global");

    let location = location::build_location("about:blank", context);
    let navigator = navigator::build_navigator(context);
    let history = history::build_history(context);

    // Window event listeners — stored in event_listeners with WINDOW_LISTENER_ID
    let window = ObjectInitializer::new(context)
        .function(set_timeout, js_string!("setTimeout"), 2)
        .function(clear_timeout, js_string!("clearTimeout"), 1)
        .function(set_interval, js_string!("setInterval"), 2)
        .function(clear_interval, js_string!("clearInterval"), 1)
        .build();

    // window.event getter — returns the current event during dispatch, undefined otherwise
    let event_getter = unsafe {
        NativeFunction::from_closure(|_this, _args, ctx| {
            let event = crate::js::realm_state::current_event(ctx);
            match event {
                Some(obj) => Ok(JsValue::from(obj)),
                None => Ok(JsValue::undefined()),
            }
        })
    };

    let realm = context.realm().clone();
    window
        .define_property_or_throw(
            js_string!("event"),
            PropertyDescriptor::builder()
                .get(event_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define window.event");

    // Register unified on* event handler accessors on window
    super::on_event::register_window_on_event_accessors(
        &window,
        &[
            "load",
            "error",
            "click",
            "change",
            "input",
            "submit",
            "reset",
            "mousedown",
            "mouseup",
            "mouseover",
            "mouseout",
            "mousemove",
            "keydown",
            "keyup",
            "keypress",
            "focus",
            "blur",
            "resize",
            "scroll",
            "hashchange",
            "popstate",
            "unload",
            "beforeunload",
            "animationstart",
            "animationend",
            "animationiteration",
            "transitionend",
            "transitionstart",
            "transitionrun",
            "webkitanimationstart",
            "webkitanimationend",
            "webkitanimationiteration",
            "webkittransitionend",
        ],
        context,
    );

    // frames getter -- returns array-like object of iframe contentWindow objects
    let frames_getter = frames::make_frames_getter(Rc::clone(&tree));

    let realm_for_frames = context.realm().clone();
    window
        .define_property_or_throw(
            js_string!("frames"),
            PropertyDescriptor::builder()
                .get(frames_getter.to_js_function(&realm_for_frames))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define window.frames");

    window
        .define_property_or_throw(
            js_string!("location"),
            PropertyDescriptor::builder()
                .value(location)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define window.location");

    window
        .define_property_or_throw(
            js_string!("navigator"),
            PropertyDescriptor::builder()
                .value(navigator)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define window.navigator");

    window
        .define_property_or_throw(
            js_string!("history"),
            PropertyDescriptor::builder()
                .value(history)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define window.history");

    let window_clone = window.clone();
    window
        .define_property_or_throw(
            js_string!("window"),
            PropertyDescriptor::builder()
                .value(window_clone)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define window.window");

    let global = context.global_object();
    let doc_val = global
        .get(js_string!("document"), context)
        .expect("document global should exist");
    window
        .define_property_or_throw(
            js_string!("document"),
            PropertyDescriptor::builder()
                .value(doc_val)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define window.document");

    // Copy DOMParser from the global to the window object so `window.DOMParser` works
    let dom_parser_val = global
        .get(js_string!("DOMParser"), context)
        .expect("DOMParser global should exist");
    window
        .define_property_or_throw(
            js_string!("DOMParser"),
            PropertyDescriptor::builder()
                .value(dom_parser_val)
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define window.DOMParser");

    // requestAnimationFrame — schedule callback as a zero-delay timer so it fires
    // on the next settle() iteration (async, like a real browser).
    // The callback receives a DOMHighResTimeStamp from performance.now().
    let raf = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let user_callback = args.first().cloned().unwrap_or(JsValue::undefined());
            // Wrap user callback: call performance.now() and pass result as timestamp arg
            let wrapper = NativeFunction::from_closure(move |_this, _args, ctx| {
                if let Some(cb) = user_callback.as_callable() {
                    let perf = ctx.global_object().get(js_string!("performance"), ctx)?;
                    let now_val = if let Some(perf_obj) = perf.as_object() {
                        let now_fn = perf_obj.get(js_string!("now"), ctx)?;
                        if let Some(callable) = now_fn.as_callable() {
                            callable.call(&perf, &[], ctx)?
                        } else {
                            JsValue::from(0.0)
                        }
                    } else {
                        JsValue::from(0.0)
                    };
                    cb.call(&JsValue::undefined(), &[now_val], ctx)?;
                }
                Ok(JsValue::undefined())
            });
            let wrapper_fn = JsValue::from(wrapper.to_js_function(ctx.realm()));

            // Register as a zero-delay one-shot timer
            let ts = realm_state::timer_state(ctx);
            let mut state = ts.borrow_mut();
            let id = state.next_id;
            state.next_id += 1;
            let registered_at = state.current_time_ms;
            state.entries.insert(
                id,
                TimerEntry {
                    id,
                    callback: wrapper_fn,
                    delay_ms: 0,
                    is_interval: false,
                    registered_at,
                },
            );
            Ok(JsValue::from(id))
        })
    };
    let cancel_raf = timers::make_clear_timer();

    let raf_fn = raf.to_js_function(context.realm());
    let cancel_raf_fn = cancel_raf.to_js_function(context.realm());

    context
        .register_global_property(js_string!("requestAnimationFrame"), raf_fn.clone(), Attribute::all())
        .expect("failed to register requestAnimationFrame global");
    context
        .register_global_property(js_string!("cancelAnimationFrame"), cancel_raf_fn.clone(), Attribute::all())
        .expect("failed to register cancelAnimationFrame global");

    window
        .define_property_or_throw(
            js_string!("requestAnimationFrame"),
            PropertyDescriptor::builder()
                .value(raf_fn)
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define window.requestAnimationFrame");
    window
        .define_property_or_throw(
            js_string!("cancelAnimationFrame"),
            PropertyDescriptor::builder()
                .value(cancel_raf_fn)
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define window.cancelAnimationFrame");

    // getSelection — stub returning object with rangeCount: 0
    let get_selection = NativeFunction::from_fn_ptr(|_this, _args, ctx| {
        let obj = boa_engine::object::ObjectInitializer::new(ctx)
            .property(js_string!("rangeCount"), 0, Attribute::all())
            .function(
                NativeFunction::from_fn_ptr(|_, _, _| Ok(JsValue::undefined())),
                js_string!("removeAllRanges"),
                0,
            )
            .function(
                NativeFunction::from_fn_ptr(|_, _, _| Ok(JsValue::undefined())),
                js_string!("addRange"),
                1,
            )
            .build();
        Ok(obj.into())
    });
    let get_selection_fn = get_selection.to_js_function(context.realm());
    context
        .register_global_property(js_string!("getSelection"), get_selection_fn.clone(), Attribute::all())
        .expect("failed to register getSelection global");
    window
        .define_property_or_throw(
            js_string!("getSelection"),
            PropertyDescriptor::builder()
                .value(get_selection_fn)
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define window.getSelection");

    // getComputedStyle — register on window and as global
    let gcs = super::computed_style::make_get_computed_style(Rc::clone(&tree));
    let gcs_fn = gcs.to_js_function(context.realm());
    window
        .define_property_or_throw(
            js_string!("getComputedStyle"),
            PropertyDescriptor::builder()
                .value(gcs_fn.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define window.getComputedStyle");

    // matchMedia — returns MediaQueryList-like stub
    let match_media = match_media::build_match_media(context);
    let mm_fn = match_media.to_js_function(context.realm());
    window
        .define_property_or_throw(
            js_string!("matchMedia"),
            PropertyDescriptor::builder()
                .value(mm_fn.clone())
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("failed to define window.matchMedia");
    context
        .register_global_property(js_string!("matchMedia"), mm_fn, Attribute::all())
        .expect("failed to register matchMedia global");

    // Pre-initialize analytics globals so sites don't crash on missing dataLayer/ga/gtag
    let data_layer = JsArray::new(context);
    context
        .register_global_property(js_string!("dataLayer"), data_layer, Attribute::all())
        .expect("failed to register dataLayer global");
    let ga_noop = NativeFunction::from_fn_ptr(|_, _, _| Ok(JsValue::undefined()));
    context
        .register_global_property(
            js_string!("ga"),
            ga_noop.to_js_function(context.realm()),
            Attribute::all(),
        )
        .expect("failed to register ga global");
    let gtag_noop = NativeFunction::from_fn_ptr(|_, _, _| Ok(JsValue::undefined()));
    context
        .register_global_property(
            js_string!("gtag"),
            gtag_noop.to_js_function(context.realm()),
            Attribute::all(),
        )
        .expect("failed to register gtag global");

    // Store the window object in realm state so dispatch_event in element.rs
    // can include window in event propagation paths.
    realm_state::set_window_object(context, window.clone());

    context
        .register_global_property(js_string!("window"), window, Attribute::all())
        .expect("failed to register window global");

    // Register `self` as the actual global object.
    // testharness.js does (function(global_scope){...})(self) and uses expose()
    // to set properties on global_scope. For these to become true globals,
    // `self` must be the real global object, not our window proxy.
    let global_for_self = context.global_object();
    context
        .register_global_property(js_string!("self"), global_for_self, Attribute::all())
        .expect("failed to register self global");

    // Also register getComputedStyle as a direct global
    context
        .register_global_property(js_string!("getComputedStyle"), gcs_fn, Attribute::all())
        .expect("failed to register getComputedStyle global");

    // Register `frames` as a direct global getter so bare `frames[0]` works
    let frames_getter_global = frames::make_frames_getter_global(Rc::clone(&tree));

    let realm_for_frames_global = context.realm().clone();
    let global = context.global_object();
    global
        .define_property_or_throw(
            js_string!("frames"),
            PropertyDescriptor::builder()
                .get(frames_getter_global.to_js_function(&realm_for_frames_global))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define global frames");
}
