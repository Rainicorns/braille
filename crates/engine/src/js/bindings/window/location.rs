use std::rc::Rc;

use boa_engine::{
    js_string,
    native_function::NativeFunction,
    object::ObjectInitializer,
    property::PropertyDescriptor,
    Context, JsValue,
};

use crate::js::realm_state;

pub(super) fn build_location(url: &str, context: &mut Context) -> boa_engine::JsObject {
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

pub(crate) fn extract_protocol(url: &str) -> String {
    if let Some(idx) = url.find("://") {
        format!("{}:", &url[..idx])
    } else {
        String::new()
    }
}

pub(crate) fn extract_hostname(url: &str) -> String {
    let after_scheme = if let Some(idx) = url.find("://") {
        &url[idx + 3..]
    } else {
        return String::new();
    };
    let end = after_scheme.find(['/', ':', '?', '#']).unwrap_or(after_scheme.len());
    after_scheme[..end].to_string()
}

pub(crate) fn extract_pathname(url: &str) -> String {
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

pub(crate) fn extract_search(url: &str) -> String {
    if let Some(q_idx) = url.find('?') {
        let after_q = &url[q_idx..];
        let end = after_q.find('#').unwrap_or(after_q.len());
        after_q[..end].to_string()
    } else {
        String::new()
    }
}

pub(crate) fn extract_hash(url: &str) -> String {
    if let Some(h_idx) = url.find('#') {
        url[h_idx..].to_string()
    } else {
        String::new()
    }
}
