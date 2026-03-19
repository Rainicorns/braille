use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::{Attribute, PropertyDescriptor},
    Context, JsResult, JsValue,
};

use super::event_target::ListenerEntry;
use crate::js::realm_state;
use crate::js::realm_state::TimerEntry;

type ConsoleBuffer = Rc<RefCell<Vec<String>>>;

/// Well-known ID for window in the event listeners map.
/// Uses usize::MAX - 1 to avoid collision with DOM NodeIds (start at 0)
/// and standalone EventTarget IDs (start at usize::MAX / 2).
pub(crate) const WINDOW_LISTENER_ID: usize = usize::MAX - 1;

/// Public window dispatchEvent — called from EventTarget.prototype.dispatchEvent for window `this`.
pub(crate) fn window_dispatch_event(args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
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

    {
        let mut evt = event_obj.downcast_mut::<super::event::JsEvent>().unwrap();
        evt.dispatching = true;
        evt.phase = 2;
    }

    let window_val: JsValue = realm_state::window_object(ctx)
        .map(JsValue::from)
        .unwrap_or(JsValue::undefined());

    event_obj.define_property_or_throw(
        js_string!("target"),
        PropertyDescriptor::builder()
            .value(window_val.clone())
            .writable(true)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;
    event_obj.define_property_or_throw(
        js_string!("srcElement"),
        PropertyDescriptor::builder()
            .value(window_val.clone())
            .writable(true)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;
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

fn console_format_args(args: &[JsValue], ctx: &mut Context) -> JsResult<String> {
    let parts: Vec<String> = args
        .iter()
        .map(|v| v.to_string(ctx).map(|s| s.to_std_string_escaped()))
        .collect::<JsResult<Vec<String>>>()?;
    Ok(parts.join(" "))
}

fn make_console_method(buffer: ConsoleBuffer, prefix: Option<&'static str>) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let msg = console_format_args(args, ctx)?;
            let formatted = match prefix {
                Some(p) => format!("{}{}", p, msg),
                None => msg,
            };
            buffer.borrow_mut().push(formatted);
            Ok(JsValue::undefined())
        })
    }
}

fn make_set_timer(is_interval: bool) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let callback = args.first().cloned().unwrap_or(JsValue::undefined());
            let delay_ms = args
                .get(1)
                .map(|v| v.to_u32(ctx).unwrap_or(0))
                .unwrap_or(0);
            let ts = realm_state::timer_state(ctx);
            let mut state = ts.borrow_mut();
            let id = state.next_id;
            state.next_id += 1;
            let registered_at = state.current_time_ms;
            state.entries.insert(
                id,
                TimerEntry {
                    id,
                    callback,
                    delay_ms,
                    is_interval,
                    registered_at,
                },
            );
            Ok(JsValue::from(id))
        })
    }
}

fn make_clear_timer() -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            if let Some(id_val) = args.first() {
                let id = id_val.to_u32(ctx)?;
                let ts = realm_state::timer_state(ctx);
                ts.borrow_mut().entries.remove(&id);
            }
            Ok(JsValue::undefined())
        })
    }
}
fn build_location(url: &str, context: &mut Context) -> boa_engine::JsObject {
    let url_str = Rc::new(RefCell::new(url.to_string()));

    let url_for_href_get = Rc::clone(&url_str);
    let href_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let val = url_for_href_get.borrow().clone();
            Ok(JsValue::from(js_string!(val)))
        })
    };

    let url_for_href_set = Rc::clone(&url_str);
    let href_setter = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            if let Some(v) = args.first() {
                let new_url = v.to_string(ctx)?.to_std_string_escaped();
                *url_for_href_set.borrow_mut() = new_url;
            }
            Ok(JsValue::undefined())
        })
    };

    let url_for_pathname = Rc::clone(&url_str);
    let pathname_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let u = url_for_pathname.borrow().clone();
            let path = extract_pathname(&u);
            Ok(JsValue::from(js_string!(path)))
        })
    };

    let url_for_hostname = Rc::clone(&url_str);
    let hostname_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let u = url_for_hostname.borrow().clone();
            let host = extract_hostname(&u);
            Ok(JsValue::from(js_string!(host)))
        })
    };

    let url_for_protocol = Rc::clone(&url_str);
    let protocol_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let u = url_for_protocol.borrow().clone();
            let proto = extract_protocol(&u);
            Ok(JsValue::from(js_string!(proto)))
        })
    };

    let url_for_search = Rc::clone(&url_str);
    let search_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let u = url_for_search.borrow().clone();
            let search = extract_search(&u);
            Ok(JsValue::from(js_string!(search)))
        })
    };

    let url_for_hash = Rc::clone(&url_str);
    let hash_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let u = url_for_hash.borrow().clone();
            let hash = extract_hash(&u);
            Ok(JsValue::from(js_string!(hash)))
        })
    };

    let location = ObjectInitializer::new(context).build();
    let realm = context.realm().clone();

    location
        .define_property_or_throw(
            js_string!("href"),
            PropertyDescriptor::builder()
                .get(href_getter.to_js_function(&realm))
                .set(href_setter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define location.href");

    location
        .define_property_or_throw(
            js_string!("pathname"),
            PropertyDescriptor::builder()
                .get(pathname_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define location.pathname");

    location
        .define_property_or_throw(
            js_string!("hostname"),
            PropertyDescriptor::builder()
                .get(hostname_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define location.hostname");

    location
        .define_property_or_throw(
            js_string!("protocol"),
            PropertyDescriptor::builder()
                .get(protocol_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define location.protocol");

    location
        .define_property_or_throw(
            js_string!("search"),
            PropertyDescriptor::builder()
                .get(search_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define location.search");

    location
        .define_property_or_throw(
            js_string!("hash"),
            PropertyDescriptor::builder()
                .get(hash_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define location.hash");

    location
}
fn extract_protocol(url: &str) -> String {
    if let Some(idx) = url.find("://") {
        format!("{}:", &url[..idx])
    } else {
        String::new()
    }
}

fn extract_hostname(url: &str) -> String {
    let after_scheme = if let Some(idx) = url.find("://") {
        &url[idx + 3..]
    } else {
        return String::new();
    };
    let end = after_scheme.find(['/', ':', '?', '#']).unwrap_or(after_scheme.len());
    after_scheme[..end].to_string()
}

fn extract_pathname(url: &str) -> String {
    let after_scheme = if let Some(idx) = url.find("://") {
        &url[idx + 3..]
    } else {
        return "/".to_string();
    };
    let path_start = match after_scheme.find('/') {
        Some(idx) => idx,
        None => return "/".to_string(),
    };
    let path_portion = &after_scheme[path_start..];
    let end = path_portion.find(['?', '#']).unwrap_or(path_portion.len());
    path_portion[..end].to_string()
}

fn extract_search(url: &str) -> String {
    if let Some(q_idx) = url.find('?') {
        let after_q = &url[q_idx..];
        let end = after_q.find('#').unwrap_or(after_q.len());
        after_q[..end].to_string()
    } else {
        String::new()
    }
}

fn extract_hash(url: &str) -> String {
    if let Some(h_idx) = url.find('#') {
        url[h_idx..].to_string()
    } else {
        String::new()
    }
}
fn build_navigator(context: &mut Context) -> boa_engine::JsObject {
    let ua_getter =
        unsafe { NativeFunction::from_closure(|_this, _args, _ctx| Ok(JsValue::from(js_string!("Braille/0.1")))) };

    let navigator = ObjectInitializer::new(context).build();
    let realm = context.realm().clone();

    navigator
        .define_property_or_throw(
            js_string!("userAgent"),
            PropertyDescriptor::builder()
                .get(ua_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define navigator.userAgent");

    navigator
}

pub(crate) fn register_window(
    context: &mut Context,
    console_output: ConsoleBuffer,
    tree: Rc<RefCell<crate::dom::DomTree>>,
) {
    let console_log = make_console_method(Rc::clone(&console_output), None);
    let console_warn = make_console_method(Rc::clone(&console_output), Some("WARN: "));
    let console_error = make_console_method(Rc::clone(&console_output), Some("ERROR: "));
    let console_info = make_console_method(Rc::clone(&console_output), Some("INFO: "));

    let console = ObjectInitializer::new(context)
        .function(console_log, js_string!("log"), 0)
        .function(console_warn, js_string!("warn"), 0)
        .function(console_error, js_string!("error"), 0)
        .function(console_info, js_string!("info"), 0)
        .build();

    context
        .register_global_property(js_string!("console"), console, Attribute::all())
        .expect("failed to register console global");

    let set_timeout = make_set_timer(false);
    let clear_timeout = make_clear_timer();
    let set_interval = make_set_timer(true);
    let clear_interval = make_clear_timer();

    // Register timer functions as globals (testharness.js calls them without window. prefix)
    let g_set_timeout = make_set_timer(false);
    let g_clear_timeout = make_clear_timer();
    let g_set_interval = make_set_timer(true);
    let g_clear_interval = make_clear_timer();
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

    let location = build_location("about:blank", context);
    let navigator = build_navigator(context);

    // Window event listeners — stored in event_listeners with WINDOW_LISTENER_ID
    let add_event_listener = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let event_type = args
                .first()
                .map(|v| v.to_string(ctx).map(|s| s.to_std_string_escaped()))
                .transpose()?
                .unwrap_or_default();

            // Parse options (3rd argument): boolean or {capture, once, passive}
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

            // Compute default passive for window (always a passive-by-default target)
            let passive = match passive {
                Some(v) => Some(v),
                None => {
                    if super::element::is_passive_default_event(&event_type) {
                        Some(true)
                    } else {
                        None
                    }
                }
            };

            let callback_val = match args.get(1) {
                Some(v) => v,
                None => return Ok(JsValue::undefined()),
            };
            if callback_val.is_null() || callback_val.is_undefined() {
                return Ok(JsValue::undefined());
            }
            let callback = callback_val
                .as_object()
                .ok_or_else(|| {
                    boa_engine::JsError::from_opaque(js_string!("addEventListener: callback is not an object").into())
                })?
                .clone();

            {
                let listeners = realm_state::event_listeners(ctx);
                let mut map = listeners.borrow_mut();
                let entries = map.entry((usize::MAX, WINDOW_LISTENER_ID)).or_default();

                let duplicate = entries.iter().any(|entry| {
                    entry.event_type == event_type && entry.capture == capture && entry.callback == callback
                });

                if !duplicate {
                    entries.push(ListenerEntry {
                        event_type,
                        callback,
                        capture,
                        once,
                        passive,
                        removed: std::rc::Rc::new(std::cell::Cell::new(false)),
                    });
                }
            }

            Ok(JsValue::undefined())
        })
    };

    let remove_event_listener = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let event_type = args
                .first()
                .map(|v| v.to_string(ctx).map(|s| s.to_std_string_escaped()))
                .transpose()?
                .unwrap_or_default();

            let callback_val = match args.get(1) {
                Some(v) => v,
                None => return Ok(JsValue::undefined()),
            };
            if callback_val.is_null() || callback_val.is_undefined() {
                return Ok(JsValue::undefined());
            }
            let callback = callback_val
                .as_object()
                .ok_or_else(|| {
                    boa_engine::JsError::from_opaque(
                        js_string!("removeEventListener: callback is not an object").into(),
                    )
                })?
                .clone();

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

            {
                let listeners = realm_state::event_listeners(ctx);
                let mut map = listeners.borrow_mut();
                if let Some(entries) = map.get_mut(&(usize::MAX, WINDOW_LISTENER_ID)) {
                    entries.retain(|entry| {
                        if entry.event_type == event_type && entry.capture == capture && entry.callback == callback {
                            entry.removed.set(true);
                            false
                        } else {
                            true
                        }
                    });
                    if entries.is_empty() {
                        map.remove(&(usize::MAX, WINDOW_LISTENER_ID));
                    }
                }
            }

            Ok(JsValue::undefined())
        })
    };

    let dispatch_event = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
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

            {
                let mut evt = event_obj.downcast_mut::<super::event::JsEvent>().unwrap();
                evt.dispatching = true;
                evt.phase = 2;
            }

            let window_val: JsValue = realm_state::window_object(ctx)
                .map(JsValue::from)
                .unwrap_or(JsValue::undefined());

            event_obj.define_property_or_throw(
                js_string!("target"),
                PropertyDescriptor::builder()
                    .value(window_val.clone())
                    .writable(true)
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                ctx,
            )?;
            event_obj.define_property_or_throw(
                js_string!("srcElement"),
                PropertyDescriptor::builder()
                    .value(window_val.clone())
                    .writable(true)
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                ctx,
            )?;
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
        })
    };

    let window = ObjectInitializer::new(context)
        .function(set_timeout, js_string!("setTimeout"), 2)
        .function(clear_timeout, js_string!("clearTimeout"), 1)
        .function(set_interval, js_string!("setInterval"), 2)
        .function(clear_interval, js_string!("clearInterval"), 1)
        .function(add_event_listener, js_string!("addEventListener"), 2)
        .function(remove_event_listener, js_string!("removeEventListener"), 2)
        .function(dispatch_event, js_string!("dispatchEvent"), 1)
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
        ],
        context,
    );

    // frames getter -- returns array-like object of iframe contentWindow objects
    let tree_for_frames = Rc::clone(&tree);
    let frames_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let tree_ref = tree_for_frames.borrow();
            let tree_ptr = Rc::as_ptr(&tree_for_frames) as usize;

            // Collect iframe node IDs in document order
            let mut iframe_ids = Vec::new();
            let doc = tree_ref.document();
            collect_iframes(&tree_ref, doc, &mut iframe_ids);
            drop(tree_ref);

            let frames_obj = ObjectInitializer::new(ctx2).build();

            // Set numeric indices
            for (i, &nid) in iframe_ids.iter().enumerate() {
                // Ensure iframe content doc + realm is created
                let _doc_obj = super::element::ensure_iframe_content_doc(tree_ptr, nid, ctx2)?;

                // Look up the iframe's realm and return its real window object
                let cw = get_iframe_window(tree_ptr, nid, ctx2);

                frames_obj.define_property_or_throw(
                    js_string!(i.to_string()),
                    PropertyDescriptor::builder()
                        .value(cw)
                        .writable(true)
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    ctx2,
                )?;
            }

            // Set length
            frames_obj.define_property_or_throw(
                js_string!("length"),
                PropertyDescriptor::builder()
                    .value(JsValue::from(iframe_ids.len() as u32))
                    .writable(false)
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                ctx2,
            )?;

            Ok(JsValue::from(frames_obj))
        })
    };

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
    let tree_for_frames_global = Rc::clone(&tree);
    let frames_getter_global = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx2| {
            let tree_ref = tree_for_frames_global.borrow();
            let tree_ptr = Rc::as_ptr(&tree_for_frames_global) as usize;

            let mut iframe_ids = Vec::new();
            let doc = tree_ref.document();
            collect_iframes(&tree_ref, doc, &mut iframe_ids);
            drop(tree_ref);

            let frames_obj = ObjectInitializer::new(ctx2).build();

            for (i, &nid) in iframe_ids.iter().enumerate() {
                let _doc_obj = super::element::ensure_iframe_content_doc(tree_ptr, nid, ctx2)?;
                let cw = get_iframe_window(tree_ptr, nid, ctx2);
                frames_obj.define_property_or_throw(
                    js_string!(i.to_string()),
                    PropertyDescriptor::builder()
                        .value(cw)
                        .writable(true)
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    ctx2,
                )?;
            }

            frames_obj.define_property_or_throw(
                js_string!("length"),
                PropertyDescriptor::builder()
                    .value(JsValue::from(iframe_ids.len() as u32))
                    .writable(false)
                    .configurable(true)
                    .enumerable(true)
                    .build(),
                ctx2,
            )?;

            Ok(JsValue::from(frames_obj))
        })
    };

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

/// Look up the real window object for an iframe's realm.
/// If the iframe has a realm, enters it to read its window object.
/// Falls back to a plain object with just `document` if no realm exists.
fn get_iframe_window(tree_ptr: usize, nid: crate::dom::NodeId, ctx: &mut Context) -> JsValue {
    let realms = realm_state::iframe_realms(ctx);
    let realm_opt = realms.borrow().get(&(tree_ptr, nid)).cloned();

    if let Some(realm) = realm_opt {
        // Enter the iframe realm to read its window object
        let win = realm_state::with_realm(ctx, &realm, |ctx| realm_state::window_object(ctx));
        match win {
            Some(w) => JsValue::from(w),
            None => JsValue::undefined(),
        }
    } else {
        // Fallback: no realm, create a plain object with just document
        let doc_obj = super::element::ensure_iframe_content_doc(tree_ptr, nid, ctx);
        match doc_obj {
            Ok(doc) => {
                let cw = ObjectInitializer::new(ctx).build();
                let _ = cw.define_property_or_throw(
                    js_string!("document"),
                    PropertyDescriptor::builder()
                        .value(JsValue::from(doc))
                        .writable(true)
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    ctx,
                );
                JsValue::from(cw)
            }
            Err(_) => JsValue::undefined(),
        }
    }
}

/// Recursively collects NodeIds of `<iframe>` elements in document order.
fn collect_iframes(tree: &crate::dom::DomTree, node_id: crate::dom::NodeId, out: &mut Vec<crate::dom::NodeId>) {
    use crate::dom::NodeData;
    let node = tree.get_node(node_id);
    if let NodeData::Element { ref tag_name, .. } = node.data {
        if tag_name == "iframe" {
            out.push(node_id);
        }
    }
    for child in tree.children(node_id) {
        collect_iframes(tree, child, out);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use crate::dom::DomTree;
    use crate::js::JsRuntime;

    fn make_runtime() -> JsRuntime {
        let tree = Rc::new(RefCell::new(DomTree::new()));
        {
            let mut t = tree.borrow_mut();
            let html = t.create_element("html");
            let body = t.create_element("body");
            let doc = t.document();
            t.append_child(doc, html);
            t.append_child(html, body);
        }
        JsRuntime::new(tree)
    }

    #[test]
    fn window_exists_and_self_referential() {
        let mut rt = make_runtime();
        let result = rt.eval("window.window === window").unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn window_dot_window_dot_window() {
        let mut rt = make_runtime();
        let result = rt.eval("window.window.window === window").unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn window_document_exists() {
        let mut rt = make_runtime();
        let result = rt
            .eval("window.document !== undefined && window.document !== null")
            .unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn window_document_same_as_global_document() {
        let mut rt = make_runtime();
        let result = rt.eval("typeof window.document.createElement === 'function'").unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn window_location_href_default() {
        let mut rt = make_runtime();
        let result = rt.eval("window.location.href").unwrap();
        let href = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(href, "about:blank");
    }

    #[test]
    fn window_location_href_setter() {
        let mut rt = make_runtime();
        rt.eval(r#"window.location.href = "https://example.com/path?q=1#sec""#)
            .unwrap();
        let result = rt.eval("window.location.href").unwrap();
        let href = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(href, "https://example.com/path?q=1#sec");
    }

    #[test]
    fn window_location_parts() {
        let mut rt = make_runtime();
        rt.eval(r#"window.location.href = "https://example.com:8080/foo/bar?q=hello&b=2#section""#)
            .unwrap();

        let protocol = rt.eval("window.location.protocol").unwrap();
        let protocol_str = protocol.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(protocol_str, "https:");

        let hostname = rt.eval("window.location.hostname").unwrap();
        let hostname_str = hostname.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(hostname_str, "example.com");

        let pathname = rt.eval("window.location.pathname").unwrap();
        let pathname_str = pathname.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(pathname_str, "/foo/bar");

        let search = rt.eval("window.location.search").unwrap();
        let search_str = search.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(search_str, "?q=hello&b=2");

        let hash = rt.eval("window.location.hash").unwrap();
        let hash_str = hash.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(hash_str, "#section");
    }

    #[test]
    fn window_location_pathname_default() {
        let mut rt = make_runtime();
        let result = rt.eval("window.location.pathname").unwrap();
        let path = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(path, "/");
    }

    #[test]
    fn console_log_stores_message() {
        let mut rt = make_runtime();
        rt.eval(r#"console.log("hello world")"#).unwrap();
        let output = rt.console_output();
        assert_eq!(output, vec!["hello world"]);
    }

    #[test]
    fn console_warn_prefixes() {
        let mut rt = make_runtime();
        rt.eval(r#"console.warn("something bad")"#).unwrap();
        let output = rt.console_output();
        assert_eq!(output, vec!["WARN: something bad"]);
    }

    #[test]
    fn console_error_prefixes() {
        let mut rt = make_runtime();
        rt.eval(r#"console.error("fatal")"#).unwrap();
        let output = rt.console_output();
        assert_eq!(output, vec!["ERROR: fatal"]);
    }

    #[test]
    fn console_info_prefixes() {
        let mut rt = make_runtime();
        rt.eval(r#"console.info("note")"#).unwrap();
        let output = rt.console_output();
        assert_eq!(output, vec!["INFO: note"]);
    }

    #[test]
    fn console_log_multiple_args_joined() {
        let mut rt = make_runtime();
        rt.eval(r#"console.log("a", "b", "c")"#).unwrap();
        let output = rt.console_output();
        assert_eq!(output, vec!["a b c"]);
    }

    #[test]
    fn console_multiple_calls_accumulate() {
        let mut rt = make_runtime();
        rt.eval(r#"console.log("first")"#).unwrap();
        rt.eval(r#"console.log("second")"#).unwrap();
        rt.eval(r#"console.warn("third")"#).unwrap();
        let output = rt.console_output();
        assert_eq!(output, vec!["first", "second", "WARN: third"]);
    }

    #[test]
    fn set_timeout_returns_numeric_id() {
        let mut rt = make_runtime();
        let result = rt.eval("window.setTimeout(function(){}, 100)").unwrap();
        assert!(result.is_number(), "setTimeout should return a number");
        let id = result.as_number().unwrap();
        assert!(id >= 1.0, "timer ID should be >= 1");
    }

    #[test]
    fn set_interval_returns_numeric_id() {
        let mut rt = make_runtime();
        let result = rt.eval("window.setInterval(function(){}, 100)").unwrap();
        assert!(result.is_number(), "setInterval should return a number");
    }

    #[test]
    fn set_timeout_ids_increment() {
        let mut rt = make_runtime();
        let r1 = rt.eval("window.setTimeout(function(){}, 100)").unwrap();
        let r2 = rt.eval("window.setTimeout(function(){}, 200)").unwrap();
        let id1 = r1.as_number().unwrap();
        let id2 = r2.as_number().unwrap();
        assert!(id2 > id1, "timer IDs should increment");
    }

    #[test]
    fn clear_timeout_does_not_crash() {
        let mut rt = make_runtime();
        rt.eval("var id = window.setTimeout(function(){}, 100); window.clearTimeout(id)")
            .unwrap();
    }

    #[test]
    fn clear_interval_does_not_crash() {
        let mut rt = make_runtime();
        rt.eval("var id = window.setInterval(function(){}, 100); window.clearInterval(id)")
            .unwrap();
    }

    #[test]
    fn navigator_user_agent() {
        let mut rt = make_runtime();
        let result = rt.eval("window.navigator.userAgent").unwrap();
        let ua = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(ua, "Braille/0.1");
    }

    #[test]
    fn console_output_accessible_from_runtime() {
        let mut rt = make_runtime();
        rt.eval(r#"console.log("from runtime")"#).unwrap();
        let output = rt.console_output();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0], "from runtime");
    }
}
