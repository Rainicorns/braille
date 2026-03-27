use std::cell::RefCell;
use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::PropertyDescriptor,
    Context, JsResult, JsValue,
};

use crate::js::realm_state;

use super::WINDOW_LISTENER_ID;

/// In-page history entry for pushState/replaceState.
struct HistoryEntry {
    url: String,
    state: JsValue,
}

/// Build the `window.history` object with pushState/replaceState/back/forward/go.
pub(super) fn build_history(context: &mut Context) -> boa_engine::JsObject {
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
        super::window_dispatch_event_with_this(&this_val, &[JsValue::from(event_obj.clone())], ctx)?;

        // Also invoke window.onpopstate handler
        super::super::on_event::invoke_on_event_handler(
            super::super::on_event::WINDOW_TREE_PTR,
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
