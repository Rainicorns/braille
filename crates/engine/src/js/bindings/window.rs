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

type ConsoleBuffer = Rc<RefCell<Vec<String>>>;

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
    // Use the shared location_url from RealmState so History API can update it
    let url_str = realm_state::location_url(context);
    *url_str.borrow_mut() = url.to_string();

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

/// In-page history entry for pushState/replaceState.
struct HistoryEntry {
    url: String,
    state: JsValue,
}

/// Build the `window.history` object with pushState/replaceState/back/forward/go.
fn build_history(context: &mut Context) -> boa_engine::JsObject {
    let location_url = realm_state::location_url(context);

    // Initialize history stack with current URL
    let initial_url = location_url.borrow().clone();
    let entries: Rc<RefCell<Vec<HistoryEntry>>> = Rc::new(RefCell::new(vec![HistoryEntry {
        url: initial_url,
        state: JsValue::null(),
    }]));
    let index: Rc<std::cell::Cell<usize>> = Rc::new(std::cell::Cell::new(0));

    let history = ObjectInitializer::new(context).build();
    let realm = context.realm().clone();

    // history.length getter
    let entries_for_length = Rc::clone(&entries);
    let length_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let len = entries_for_length.borrow().len();
            Ok(JsValue::from(len as u32))
        })
    };
    history
        .define_property_or_throw(
            js_string!("length"),
            PropertyDescriptor::builder()
                .get(length_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("define history.length");

    // history.state getter
    let entries_for_state = Rc::clone(&entries);
    let index_for_state = Rc::clone(&index);
    let state_getter = unsafe {
        NativeFunction::from_closure(move |_this, _args, _ctx| {
            let e = entries_for_state.borrow();
            let i = index_for_state.get();
            Ok(e.get(i).map(|entry| entry.state.clone()).unwrap_or(JsValue::null()))
        })
    };
    history
        .define_property_or_throw(
            js_string!("state"),
            PropertyDescriptor::builder()
                .get(state_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("define history.state");

    // history.pushState(state, title, url?)
    let entries_for_push = Rc::clone(&entries);
    let index_for_push = Rc::clone(&index);
    let url_for_push = Rc::clone(&location_url);
    let push_state = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let state = args.first().cloned().unwrap_or(JsValue::null());
            // title (args[1]) is ignored per spec
            let new_url = if let Some(url_val) = args.get(2) {
                if !url_val.is_undefined() && !url_val.is_null() {
                    let url_str = url_val.to_string(ctx)?.to_std_string_escaped();
                    resolve_history_url(&url_for_push.borrow(), &url_str)
                } else {
                    url_for_push.borrow().clone()
                }
            } else {
                url_for_push.borrow().clone()
            };

            let mut e = entries_for_push.borrow_mut();
            let i = index_for_push.get();
            // Truncate forward entries
            e.truncate(i + 1);
            e.push(HistoryEntry {
                url: new_url.clone(),
                state,
            });
            index_for_push.set(e.len() - 1);
            *url_for_push.borrow_mut() = new_url;

            Ok(JsValue::undefined())
        })
    };
    history
        .define_property_or_throw(
            js_string!("pushState"),
            PropertyDescriptor::builder()
                .value(push_state.to_js_function(&realm))
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("define history.pushState");

    // history.replaceState(state, title, url?)
    let entries_for_replace = Rc::clone(&entries);
    let index_for_replace = Rc::clone(&index);
    let url_for_replace = Rc::clone(&location_url);
    let replace_state = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let state = args.first().cloned().unwrap_or(JsValue::null());
            let new_url = if let Some(url_val) = args.get(2) {
                if !url_val.is_undefined() && !url_val.is_null() {
                    let url_str = url_val.to_string(ctx)?.to_std_string_escaped();
                    resolve_history_url(&url_for_replace.borrow(), &url_str)
                } else {
                    url_for_replace.borrow().clone()
                }
            } else {
                url_for_replace.borrow().clone()
            };

            let mut e = entries_for_replace.borrow_mut();
            let i = index_for_replace.get();
            if let Some(entry) = e.get_mut(i) {
                entry.url = new_url.clone();
                entry.state = state;
            }
            *url_for_replace.borrow_mut() = new_url;

            Ok(JsValue::undefined())
        })
    };
    history
        .define_property_or_throw(
            js_string!("replaceState"),
            PropertyDescriptor::builder()
                .value(replace_state.to_js_function(&realm))
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("define history.replaceState");

    // history.go(delta)
    let entries_for_go = Rc::clone(&entries);
    let index_for_go = Rc::clone(&index);
    let url_for_go = Rc::clone(&location_url);
    let go_fn = unsafe {
        NativeFunction::from_closure(move |_this, args, ctx| {
            let delta = args
                .first()
                .map(|v| v.to_i32(ctx).unwrap_or(0))
                .unwrap_or(0);

            let e = entries_for_go.borrow();
            let current = index_for_go.get() as i64;
            let new_index = current + delta as i64;

            if new_index < 0 || new_index >= e.len() as i64 {
                return Ok(JsValue::undefined());
            }

            let new_idx = new_index as usize;
            index_for_go.set(new_idx);
            let new_url = e[new_idx].url.clone();
            let state = e[new_idx].state.clone();
            *url_for_go.borrow_mut() = new_url;
            drop(e);

            // Fire popstate event on window
            fire_popstate(state, ctx)?;

            Ok(JsValue::undefined())
        })
    };
    history
        .define_property_or_throw(
            js_string!("go"),
            PropertyDescriptor::builder()
                .value(go_fn.to_js_function(&realm))
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("define history.go");

    // history.back()
    let entries_for_back = Rc::clone(&entries);
    let index_for_back = Rc::clone(&index);
    let url_for_back = Rc::clone(&location_url);
    let back_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx| {
            let current = index_for_back.get();
            if current == 0 {
                return Ok(JsValue::undefined());
            }
            let new_idx = current - 1;
            index_for_back.set(new_idx);
            let e = entries_for_back.borrow();
            let new_url = e[new_idx].url.clone();
            let state = e[new_idx].state.clone();
            *url_for_back.borrow_mut() = new_url;
            drop(e);
            fire_popstate(state, ctx)?;
            Ok(JsValue::undefined())
        })
    };
    history
        .define_property_or_throw(
            js_string!("back"),
            PropertyDescriptor::builder()
                .value(back_fn.to_js_function(&realm))
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("define history.back");

    // history.forward()
    let entries_for_fwd = Rc::clone(&entries);
    let index_for_fwd = Rc::clone(&index);
    let url_for_fwd = Rc::clone(&location_url);
    let forward_fn = unsafe {
        NativeFunction::from_closure(move |_this, _args, ctx| {
            let current = index_for_fwd.get();
            let e = entries_for_fwd.borrow();
            if current + 1 >= e.len() {
                return Ok(JsValue::undefined());
            }
            let new_idx = current + 1;
            index_for_fwd.set(new_idx);
            let new_url = e[new_idx].url.clone();
            let state = e[new_idx].state.clone();
            *url_for_fwd.borrow_mut() = new_url;
            drop(e);
            fire_popstate(state, ctx)?;
            Ok(JsValue::undefined())
        })
    };
    history
        .define_property_or_throw(
            js_string!("forward"),
            PropertyDescriptor::builder()
                .value(forward_fn.to_js_function(&realm))
                .writable(true)
                .configurable(true)
                .enumerable(false)
                .build(),
            context,
        )
        .expect("define history.forward");

    history
}

/// Resolve a possibly-relative URL for pushState/replaceState against the current URL.
fn resolve_history_url(current: &str, new_url: &str) -> String {
    // Absolute URL
    if new_url.starts_with("http://") || new_url.starts_with("https://") {
        return new_url.to_string();
    }
    // Root-relative
    if new_url.starts_with('/') {
        // Extract origin from current
        if let Some(idx) = current.find("://") {
            let after_scheme = &current[idx + 3..];
            let end = after_scheme.find('/').unwrap_or(after_scheme.len());
            return format!("{}{}", &current[..idx + 3 + end], new_url);
        }
        return new_url.to_string();
    }
    // Fragment
    if new_url.starts_with('#') {
        let base = current.split('#').next().unwrap_or(current);
        return format!("{}{}", base, new_url);
    }
    // Query string
    if new_url.starts_with('?') {
        let base = current.split('?').next().unwrap_or(current);
        let base = base.split('#').next().unwrap_or(base);
        return format!("{}{}", base, new_url);
    }
    // Relative path — join with current directory
    if let Some(idx) = current.rfind('/') {
        return format!("{}/{}", &current[..idx], new_url);
    }
    new_url.to_string()
}

/// Fire a popstate event on the window object.
fn fire_popstate(state: JsValue, ctx: &mut Context) -> JsResult<()> {
    // Create a new Event("popstate")
    let global = ctx.global_object();
    let event_ctor = global.get(js_string!("Event"), ctx)?;
    let event_obj = event_ctor
        .as_callable()
        .unwrap()
        .construct(&[JsValue::from(js_string!("popstate"))], None, ctx)?;

    // Set .state property on the event
    event_obj.define_property_or_throw(
        js_string!("state"),
        PropertyDescriptor::builder()
            .value(state)
            .writable(true)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    // Dispatch on window — invoke both addEventListener listeners and on* handler
    if let Some(window) = realm_state::window_object(ctx) {
        let this_val = JsValue::from(window);
        window_dispatch_event_with_this(&this_val, &[JsValue::from(event_obj.clone())], ctx)?;

        // Also invoke window.onpopstate handler
        super::on_event::invoke_on_event_handler(
            super::on_event::WINDOW_TREE_PTR,
            WINDOW_LISTENER_ID,
            "popstate",
            &this_val,
            &JsValue::from(event_obj.clone()),
            &event_obj,
            ctx,
        );
    }

    Ok(())
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
    let realm = context.realm().clone();

    let navigator = ObjectInitializer::new(context).build();

    // Helper to define a getter property on navigator
    macro_rules! nav_getter {
        ($name:expr, $val:expr) => {
            let getter = unsafe { NativeFunction::from_closure($val) };
            navigator
                .define_property_or_throw(
                    js_string!($name),
                    PropertyDescriptor::builder()
                        .get(getter.to_js_function(&realm))
                        .configurable(true)
                        .enumerable(true)
                        .build(),
                    context,
                )
                .expect(concat!("failed to define navigator.", $name));
        };
    }

    nav_getter!("userAgent", |_this, _args, _ctx| Ok(JsValue::from(
        js_string!("Braille/0.1")
    )));
    nav_getter!("language", |_this, _args, _ctx| Ok(JsValue::from(
        js_string!("en-US")
    )));
    nav_getter!("platform", |_this, _args, _ctx| Ok(JsValue::from(
        js_string!("Linux")
    )));
    nav_getter!("onLine", |_this, _args, _ctx| Ok(JsValue::from(true)));
    nav_getter!("cookieEnabled", |_this, _args, _ctx| Ok(JsValue::from(
        false
    )));
    nav_getter!("maxTouchPoints", |_this, _args, _ctx| Ok(JsValue::from(
        0
    )));
    nav_getter!("hardwareConcurrency", |_this, _args, _ctx| Ok(
        JsValue::from(1)
    ));

    // languages — frozen array ["en-US", "en"]
    let languages_getter = unsafe {
        NativeFunction::from_closure(|_this, _args, ctx| {
            let arr = JsArray::new(ctx);
            arr.push(JsValue::from(js_string!("en-US")), ctx)?;
            arr.push(JsValue::from(js_string!("en")), ctx)?;
            let arr_obj: JsValue = arr.into();
            let frozen = ctx.global_object()
                .get(js_string!("Object"), ctx)?
                .as_object()
                .unwrap()
                .get(js_string!("freeze"), ctx)?
                .as_callable()
                .unwrap()
                .call(&JsValue::undefined(), std::slice::from_ref(&arr_obj), ctx)?;
            Ok(frozen)
        })
    };
    navigator
        .define_property_or_throw(
            js_string!("languages"),
            PropertyDescriptor::builder()
                .get(languages_getter.to_js_function(&realm))
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define navigator.languages");

    // mediaDevices — empty object
    let media_devices = ObjectInitializer::new(context).build();
    navigator
        .define_property_or_throw(
            js_string!("mediaDevices"),
            PropertyDescriptor::builder()
                .value(media_devices)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define navigator.mediaDevices");

    // clipboard — empty object
    let clipboard = ObjectInitializer::new(context).build();
    navigator
        .define_property_or_throw(
            js_string!("clipboard"),
            PropertyDescriptor::builder()
                .value(clipboard)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define navigator.clipboard");

    // serviceWorker — object with register() returning rejected Promise
    let sw_register = NativeFunction::from_fn_ptr(|_this, _args, ctx| {
        use boa_engine::object::builtins::JsPromise;
        let err = boa_engine::JsNativeError::typ().with_message("Service workers are not supported");
        let promise = JsPromise::reject(err, ctx);
        Ok(JsValue::from(promise))
    });
    let service_worker = ObjectInitializer::new(context)
        .function(sw_register, js_string!("register"), 1)
        .build();
    navigator
        .define_property_or_throw(
            js_string!("serviceWorker"),
            PropertyDescriptor::builder()
                .value(service_worker)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define navigator.serviceWorker");

    // permissions — object with query() returning Promise resolving {state: "denied"}
    let perm_query = NativeFunction::from_fn_ptr(|_this, _args, ctx| {
        use boa_engine::object::builtins::JsPromise;
        let result = ObjectInitializer::new(ctx)
            .property(js_string!("state"), js_string!("denied"), Attribute::all())
            .build();
        let promise = JsPromise::resolve(JsValue::from(result), ctx);
        Ok(JsValue::from(promise))
    });
    let permissions = ObjectInitializer::new(context)
        .function(perm_query, js_string!("query"), 1)
        .build();
    navigator
        .define_property_or_throw(
            js_string!("permissions"),
            PropertyDescriptor::builder()
                .value(permissions)
                .writable(true)
                .configurable(true)
                .enumerable(true)
                .build(),
            context,
        )
        .expect("failed to define navigator.permissions");

    navigator
}

/// Build a matchMedia stub: returns a MediaQueryList-like object with matches=false.
fn build_match_media(_context: &mut Context) -> NativeFunction {
    unsafe {
        NativeFunction::from_closure(|_this, args, ctx| {
            let query = args
                .first()
                .and_then(|v| v.as_string())
                .map(|s| s.to_std_string_escaped())
                .unwrap_or_default();

            let noop = NativeFunction::from_fn_ptr(|_, _, _| Ok(JsValue::undefined()));
            let noop_fn = noop.to_js_function(ctx.realm());

            let mql = ObjectInitializer::new(ctx)
                .property(js_string!("matches"), false, Attribute::all())
                .property(js_string!("media"), js_string!(query), Attribute::all())
                .property(js_string!("onchange"), JsValue::null(), Attribute::all())
                .build();

            // addEventListener / removeEventListener / addListener / removeListener — all no-ops
            for name in &["addEventListener", "removeEventListener", "addListener", "removeListener"] {
                mql.define_property_or_throw(
                    js_string!(*name),
                    PropertyDescriptor::builder()
                        .value(noop_fn.clone())
                        .writable(true)
                        .configurable(true)
                        .enumerable(false)
                        .build(),
                    ctx,
                )?;
            }

            Ok(JsValue::from(mql))
        })
    }
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
    let history = build_history(context);

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
    let cancel_raf = make_clear_timer();

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
    let match_media = build_match_media(context);
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

    #[test]
    fn on_animation_end_returns_null() {
        let mut rt = make_runtime();
        let result = rt
            .eval("var d = document.createElement('div'); d.onanimationend === null")
            .unwrap();
        assert_eq!(result.as_boolean(), Some(true), "onanimationend should be null on fresh div");
    }

    #[test]
    fn on_animation_end_after_setting_prefixed() {
        let mut rt = make_runtime();
        let result = rt
            .eval(
                r#"
                var d = document.createElement('div');
                d.onwebkitanimationend = function(){};
                d.onanimationend === null
                "#,
            )
            .unwrap();
        assert_eq!(
            result.as_boolean(),
            Some(true),
            "onanimationend should still be null after setting onwebkitanimationend"
        );
    }

    #[test]
    fn history_exists() {
        let mut rt = make_runtime();
        let result = rt.eval("typeof window.history === 'object'").unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn history_push_state_updates_location() {
        let mut rt = make_runtime();
        rt.eval(r#"window.location.href = "https://example.com/page1""#)
            .unwrap();
        rt.eval(r#"window.history.pushState({page: 2}, "", "/page2")"#)
            .unwrap();
        let result = rt.eval("window.location.pathname").unwrap();
        let path = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(path, "/page2");
    }

    #[test]
    fn history_replace_state() {
        let mut rt = make_runtime();
        rt.eval(r#"window.location.href = "https://example.com/page1""#)
            .unwrap();
        rt.eval(r#"window.history.replaceState({replaced: true}, "", "/replaced")"#)
            .unwrap();
        let result = rt.eval("window.location.pathname").unwrap();
        let path = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(path, "/replaced");
    }

    #[test]
    fn history_length_increments() {
        let mut rt = make_runtime();
        rt.eval(r#"window.location.href = "https://example.com/""#)
            .unwrap();
        let len1 = rt.eval("window.history.length").unwrap();
        rt.eval(r#"window.history.pushState(null, "", "/page2")"#)
            .unwrap();
        let len2 = rt.eval("window.history.length").unwrap();
        assert_eq!(len1.as_number(), Some(1.0));
        assert_eq!(len2.as_number(), Some(2.0));
    }

    #[test]
    fn history_state_getter() {
        let mut rt = make_runtime();
        rt.eval(r#"window.location.href = "https://example.com/""#)
            .unwrap();
        rt.eval(r#"window.history.pushState({myKey: "myVal"}, "", "/s")"#)
            .unwrap();
        let result = rt.eval("window.history.state.myKey").unwrap();
        let val = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(val, "myVal");
    }

    #[test]
    fn history_back_fires_popstate() {
        let mut rt = make_runtime();
        rt.eval(
            r#"
            window.location.href = "https://example.com/";
            var popstateUrl = null;
            window.onpopstate = function(e) { popstateUrl = window.location.pathname; };
            window.history.pushState(null, "", "/page2");
            window.history.back();
        "#,
        )
        .unwrap();
        let result = rt.eval("popstateUrl").unwrap();
        let val = result.to_string(&mut rt.context).unwrap().to_std_string_escaped();
        assert_eq!(val, "/");
    }

    #[test]
    fn history_forward_fires_popstate() {
        let mut rt = make_runtime();
        rt.eval(
            r#"
            window.location.href = "https://example.com/";
            var fwdState = null;
            window.onpopstate = function(e) { fwdState = e.state; };
            window.history.pushState({p: 2}, "", "/page2");
            window.history.back();
            window.history.forward();
        "#,
        )
        .unwrap();
        let result = rt.eval("fwdState && fwdState.p === 2").unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn style_sheet_insert_rule_works() {
        let mut rt = make_runtime();
        let result = rt.eval(r#"
            var style = document.createElement('style');
            document.body.appendChild(style);
            var sheet = style.sheet;
            typeof sheet === 'object' && sheet !== null && typeof sheet.insertRule === 'function'
        "#).unwrap();
        assert_eq!(result.as_boolean(), Some(true), "style.sheet.insertRule should be a function");
    }
}
