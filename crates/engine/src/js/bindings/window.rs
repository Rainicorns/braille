use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::{JsObject, ObjectInitializer},
    property::{Attribute, PropertyDescriptor},
    Context, JsResult, JsValue,
};

use super::event_target::{ListenerEntry, EVENT_LISTENERS};

type ConsoleBuffer = Rc<RefCell<Vec<String>>>;
type TimerMap = Rc<RefCell<HashMap<u32, JsValue>>>;

/// Well-known ID for window in the EVENT_LISTENERS map.
/// Uses usize::MAX - 1 to avoid collision with DOM NodeIds (start at 0)
/// and standalone EventTarget IDs (start at usize::MAX / 2).
pub(crate) const WINDOW_LISTENER_ID: usize = usize::MAX - 1;

thread_local! {
    /// Thread-local storing the window JsObject so dispatch_event in element.rs
    /// can include window in the event propagation path.
    pub(crate) static WINDOW_OBJECT: RefCell<Option<JsObject>> = const { RefCell::new(None) };
}

fn console_format_args(args: &[JsValue], ctx: &mut Context) -> JsResult<String> {
    let parts: Vec<String> = args
        .iter()
        .map(|v| v.to_string(ctx).map(|s| s.to_std_string_escaped()))
        .collect::<JsResult<Vec<String>>>()?;
    Ok(parts.join(" "))
}

fn make_console_method(buffer: ConsoleBuffer, prefix: Option<&'static str>) -> NativeFunction {
    unsafe { NativeFunction::from_closure(move |_this, args, ctx| {
        let msg = console_format_args(args, ctx)?;
        let formatted = match prefix {
            Some(p) => format!("{}{}", p, msg),
            None => msg,
        };
        buffer.borrow_mut().push(formatted);
        Ok(JsValue::undefined())
    }) }
}

fn make_set_timer(timers: TimerMap, next_id: Rc<RefCell<u32>>) -> NativeFunction {
    unsafe { NativeFunction::from_closure(move |_this, args, _ctx| {
        let callback = args.first().cloned().unwrap_or(JsValue::undefined());
        let mut id_ref = next_id.borrow_mut();
        let id = *id_ref;
        *id_ref += 1;
        timers.borrow_mut().insert(id, callback);
        Ok(JsValue::from(id))
    }) }
}

fn make_clear_timer(timers: TimerMap) -> NativeFunction {
    unsafe { NativeFunction::from_closure(move |_this, args, ctx| {
        if let Some(id_val) = args.first() {
            let id = id_val.to_u32(ctx)?;
            timers.borrow_mut().remove(&id);
        }
        Ok(JsValue::undefined())
    }) }
}
fn build_location(url: &str, context: &mut Context) -> boa_engine::JsObject {
    let url_str = Rc::new(RefCell::new(url.to_string()));

    let url_for_href_get = Rc::clone(&url_str);
    let href_getter = unsafe { NativeFunction::from_closure(move |_this, _args, _ctx| {
        let val = url_for_href_get.borrow().clone();
        Ok(JsValue::from(js_string!(val)))
    }) };

    let url_for_href_set = Rc::clone(&url_str);
    let href_setter = unsafe { NativeFunction::from_closure(move |_this, args, ctx| {
        if let Some(v) = args.first() {
            let new_url = v.to_string(ctx)?.to_std_string_escaped();
            *url_for_href_set.borrow_mut() = new_url;
        }
        Ok(JsValue::undefined())
    }) };

    let url_for_pathname = Rc::clone(&url_str);
    let pathname_getter = unsafe { NativeFunction::from_closure(move |_this, _args, _ctx| {
        let u = url_for_pathname.borrow().clone();
        let path = extract_pathname(&u);
        Ok(JsValue::from(js_string!(path)))
    }) };

    let url_for_hostname = Rc::clone(&url_str);
    let hostname_getter = unsafe { NativeFunction::from_closure(move |_this, _args, _ctx| {
        let u = url_for_hostname.borrow().clone();
        let host = extract_hostname(&u);
        Ok(JsValue::from(js_string!(host)))
    }) };

    let url_for_protocol = Rc::clone(&url_str);
    let protocol_getter = unsafe { NativeFunction::from_closure(move |_this, _args, _ctx| {
        let u = url_for_protocol.borrow().clone();
        let proto = extract_protocol(&u);
        Ok(JsValue::from(js_string!(proto)))
    }) };

    let url_for_search = Rc::clone(&url_str);
    let search_getter = unsafe { NativeFunction::from_closure(move |_this, _args, _ctx| {
        let u = url_for_search.borrow().clone();
        let search = extract_search(&u);
        Ok(JsValue::from(js_string!(search)))
    }) };

    let url_for_hash = Rc::clone(&url_str);
    let hash_getter = unsafe { NativeFunction::from_closure(move |_this, _args, _ctx| {
        let u = url_for_hash.borrow().clone();
        let hash = extract_hash(&u);
        Ok(JsValue::from(js_string!(hash)))
    }) };

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
    let end = after_scheme
        .find(|c: char| c == '/' || c == ':' || c == '?' || c == '#')
        .unwrap_or(after_scheme.len());
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
    let end = path_portion
        .find(|c: char| c == '?' || c == '#')
        .unwrap_or(path_portion.len());
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
    let ua_getter = unsafe { NativeFunction::from_closure(|_this, _args, _ctx| {
        Ok(JsValue::from(js_string!("Braille/0.1")))
    }) };

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

    let timers: TimerMap = Rc::new(RefCell::new(HashMap::new()));
    let next_timer_id: Rc<RefCell<u32>> = Rc::new(RefCell::new(1));

    let set_timeout = make_set_timer(Rc::clone(&timers), Rc::clone(&next_timer_id));
    let clear_timeout = make_clear_timer(Rc::clone(&timers));
    let set_interval = make_set_timer(Rc::clone(&timers), Rc::clone(&next_timer_id));
    let clear_interval = make_clear_timer(Rc::clone(&timers));

    // Register timer functions as globals (testharness.js calls them without window. prefix)
    let g_set_timeout = make_set_timer(Rc::clone(&timers), Rc::clone(&next_timer_id));
    let g_clear_timeout = make_clear_timer(Rc::clone(&timers));
    let g_set_interval = make_set_timer(Rc::clone(&timers), Rc::clone(&next_timer_id));
    let g_clear_interval = make_clear_timer(Rc::clone(&timers));
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

    // Window event listeners — stored in EVENT_LISTENERS with WINDOW_LISTENER_ID
    let add_event_listener = unsafe { NativeFunction::from_closure(move |_this, args, ctx| {
        let event_type = args.first()
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

        let callback_val = match args.get(1) {
            Some(v) => v,
            None => return Ok(JsValue::undefined()),
        };
        if callback_val.is_null() || callback_val.is_undefined() {
            return Ok(JsValue::undefined());
        }
        let callback = callback_val
            .as_object()
            .ok_or_else(|| boa_engine::JsError::from_opaque(js_string!("addEventListener: callback is not an object").into()))?
            .clone();

        EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
            let mut map = listeners_rc.borrow_mut();
            let entries = map.entry(WINDOW_LISTENER_ID).or_insert_with(Vec::new);

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
    }) };

    let remove_event_listener = unsafe { NativeFunction::from_closure(move |_this, args, ctx| {
        let event_type = args.first()
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
            .ok_or_else(|| boa_engine::JsError::from_opaque(js_string!("removeEventListener: callback is not an object").into()))?
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

        EVENT_LISTENERS.with(|el| {
            let rc = el.borrow();
            let listeners_rc = rc.as_ref().expect("EVENT_LISTENERS not initialized");
            let mut map = listeners_rc.borrow_mut();
            if let Some(entries) = map.get_mut(&WINDOW_LISTENER_ID) {
                entries.retain(|entry| {
                    !(entry.event_type == event_type
                        && entry.capture == capture
                        && entry.callback == callback)
                });
                if entries.is_empty() {
                    map.remove(&WINDOW_LISTENER_ID);
                }
            }
        });

        Ok(JsValue::undefined())
    }) };

    let dispatch_event = unsafe { NativeFunction::from_closure(move |_this, args, ctx| {
        let event_val = args.first().cloned().unwrap_or(JsValue::undefined());
        if event_val.is_null() || event_val.is_undefined() {
            return Ok(JsValue::from(true));
        }
        let event_obj = match event_val.as_object() {
            Some(o) => o.clone(),
            None => return Ok(JsValue::from(true)),
        };

        let is_custom_event;
        let event_type;
        if let Some(evt) = event_obj.downcast_ref::<super::event::JsEvent>() {
            is_custom_event = false;
            event_type = evt.event_type.clone();
        } else if let Some(evt) = event_obj.downcast_ref::<super::event::JsCustomEvent>() {
            is_custom_event = true;
            event_type = evt.event_type.clone();
        } else {
            return Ok(JsValue::from(true));
        }

        if is_custom_event {
            let mut evt = event_obj.downcast_mut::<super::event::JsCustomEvent>().unwrap();
            evt.dispatching = true;
            evt.phase = 2;
        } else {
            let mut evt = event_obj.downcast_mut::<super::event::JsEvent>().unwrap();
            evt.dispatching = true;
            evt.phase = 2;
        }

        let window_val: JsValue = WINDOW_OBJECT.with(|cell: &RefCell<Option<JsObject>>| {
            cell.borrow().as_ref().map(|w| JsValue::from(w.clone()))
        }).unwrap_or(JsValue::undefined());

        event_obj.define_property_or_throw(
            js_string!("target"),
            PropertyDescriptor::builder().value(window_val.clone()).writable(true).configurable(true).enumerable(true).build(),
            ctx,
        )?;
        event_obj.define_property_or_throw(
            js_string!("srcElement"),
            PropertyDescriptor::builder().value(window_val.clone()).writable(true).configurable(true).enumerable(true).build(),
            ctx,
        )?;
        event_obj.define_property_or_throw(
            js_string!("currentTarget"),
            PropertyDescriptor::builder().value(window_val).writable(true).configurable(true).enumerable(true).build(),
            ctx,
        )?;

        super::element::invoke_listeners_for_node(
            WINDOW_LISTENER_ID, &event_type, &event_obj, &event_val, false, true, ctx,
        )?;

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

        event_obj.define_property_or_throw(
            js_string!("currentTarget"),
            PropertyDescriptor::builder().value(JsValue::null()).writable(true).configurable(true).enumerable(true).build(),
            ctx,
        )?;

        Ok(JsValue::from(!default_prevented))
    }) };

    let window = ObjectInitializer::new(context)
        .function(set_timeout, js_string!("setTimeout"), 2)
        .function(clear_timeout, js_string!("clearTimeout"), 1)
        .function(set_interval, js_string!("setInterval"), 2)
        .function(clear_interval, js_string!("clearInterval"), 1)
        .function(add_event_listener, js_string!("addEventListener"), 2)
        .function(remove_event_listener, js_string!("removeEventListener"), 2)
        .function(dispatch_event, js_string!("dispatchEvent"), 1)
        .build();

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

    // Store the window object in the thread-local so dispatch_event in element.rs
    // can include window in event propagation paths.
    WINDOW_OBJECT.with(|cell: &RefCell<Option<JsObject>>| {
        *cell.borrow_mut() = Some(window.clone());
    });

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
        let result = rt
            .eval("typeof window.document.createElement === 'function'")
            .unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn window_location_href_default() {
        let mut rt = make_runtime();
        let result = rt.eval("window.location.href").unwrap();
        let href = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(href, "about:blank");
    }

    #[test]
    fn window_location_href_setter() {
        let mut rt = make_runtime();
        rt.eval(r#"window.location.href = "https://example.com/path?q=1#sec""#)
            .unwrap();
        let result = rt.eval("window.location.href").unwrap();
        let href = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(href, "https://example.com/path?q=1#sec");
    }

    #[test]
    fn window_location_parts() {
        let mut rt = make_runtime();
        rt.eval(
            r#"window.location.href = "https://example.com:8080/foo/bar?q=hello&b=2#section""#,
        )
        .unwrap();

        let protocol = rt.eval("window.location.protocol").unwrap();
        let protocol_str = protocol
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(protocol_str, "https:");

        let hostname = rt.eval("window.location.hostname").unwrap();
        let hostname_str = hostname
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(hostname_str, "example.com");

        let pathname = rt.eval("window.location.pathname").unwrap();
        let pathname_str = pathname
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(pathname_str, "/foo/bar");

        let search = rt.eval("window.location.search").unwrap();
        let search_str = search
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(search_str, "?q=hello&b=2");

        let hash = rt.eval("window.location.hash").unwrap();
        let hash_str = hash
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
        assert_eq!(hash_str, "#section");
    }

    #[test]
    fn window_location_pathname_default() {
        let mut rt = make_runtime();
        let result = rt.eval("window.location.pathname").unwrap();
        let path = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
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
        let ua = result
            .to_string(&mut rt.context)
            .unwrap()
            .to_std_string_escaped();
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
