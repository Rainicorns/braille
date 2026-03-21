use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};

use boa_engine::{
    builtins::promise::ResolvingFunctions,
    js_string,
    native_function::NativeFunction,
    object::{
        builtins::{JsFunction, JsPromise},
        ObjectInitializer,
    },
    property::PropertyDescriptor,
    Context, JsObject, JsResult, JsValue,
};

use crate::js::prop_desc;

static NEXT_FETCH_ID: AtomicU64 = AtomicU64::new(1);

/// A pending fetch request stored in RealmState.
pub(crate) struct PendingFetch {
    pub(crate) id: u64,
    pub(crate) url: String,
    pub(crate) method: String,
    pub(crate) headers: Vec<(String, String)>,
    pub(crate) body: Option<String>,
    pub(crate) resolve: JsFunction,
    pub(crate) reject: JsFunction,
}

/// Native data for Response objects.
#[derive(Debug, boa_engine::JsData, boa_gc::Trace, boa_gc::Finalize)]
pub(crate) struct JsResponse {
    #[unsafe_ignore_trace]
    _status: u16,
    #[unsafe_ignore_trace]
    _status_text: String,
    #[unsafe_ignore_trace]
    _headers: Vec<(String, String)>,
    #[unsafe_ignore_trace]
    pub(crate) body: RefCell<Option<String>>,
    #[unsafe_ignore_trace]
    _url: String,
}

/// Register the global `fetch` function.
pub(crate) fn register_fetch_global(ctx: &mut Context) {
    let fetch_fn = NativeFunction::from_fn_ptr(fetch_impl);
    let realm = ctx.realm().clone();
    ctx.register_global_property(
        js_string!("fetch"),
        fetch_fn.to_js_function(&realm),
        boa_engine::property::Attribute::all(),
    )
    .expect("register fetch global");

    // Also register on window if it exists
    let global = ctx.global_object();
    if let Ok(window_val) = global.get(js_string!("window"), ctx) {
        if let Some(window_obj) = window_val.as_object() {
            let fetch_fn2 = NativeFunction::from_fn_ptr(fetch_impl);
            let _ = window_obj.define_property_or_throw(
                js_string!("fetch"),
                prop_desc::data_prop(fetch_fn2.to_js_function(&realm)),
                ctx,
            );
        }
    }

    // Register Response constructor (for instanceof checks)
    register_response_global(ctx);

    // Register Headers constructor
    register_headers_global(ctx);
}

fn fetch_impl(_this: &JsValue, args: &[JsValue], ctx: &mut Context) -> JsResult<JsValue> {
    // Parse arguments: fetch(input, init?)
    let FetchArgs {
        url,
        method,
        headers,
        body,
    } = parse_fetch_args(args, ctx)?;

    // Create a pending promise
    let (promise, ResolvingFunctions { resolve, reject }) = JsPromise::new_pending(ctx);

    let id = NEXT_FETCH_ID.fetch_add(1, Ordering::Relaxed);

    // Store in RealmState's pending_fetches
    let pending = crate::js::realm_state::pending_fetches(ctx);
    pending.borrow_mut().push(PendingFetch {
        id,
        url,
        method,
        headers,
        body,
        resolve,
        reject,
    });

    Ok(JsValue::from(promise))
}

/// Parsed fetch request data.
struct FetchArgs {
    url: String,
    method: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
}

fn parse_fetch_args(args: &[JsValue], ctx: &mut Context) -> JsResult<FetchArgs> {
    let url = args
        .first()
        .map(|v| v.to_string(ctx))
        .transpose()?
        .map(|s| s.to_std_string_escaped())
        .unwrap_or_default();

    let mut method = "GET".to_string();
    let mut headers = Vec::new();
    let mut body = None;

    if let Some(init_val) = args.get(1) {
        if let Some(init_obj) = init_val.as_object() {
            // method
            if let Ok(m) = init_obj.get(js_string!("method"), ctx) {
                if !m.is_undefined() && !m.is_null() {
                    method = m.to_string(ctx)?.to_std_string_escaped().to_uppercase();
                }
            }

            // headers
            if let Ok(h) = init_obj.get(js_string!("headers"), ctx) {
                if let Some(h_obj) = h.as_object() {
                    // Try to iterate as plain object
                    let keys = h_obj.own_property_keys(ctx)?;
                    for key in keys {
                        let key_str = key.to_string();
                        if let Ok(val) = h_obj.get(key, ctx) {
                            let val_str = val.to_string(ctx)?.to_std_string_escaped();
                            headers.push((key_str, val_str));
                        }
                    }
                }
            }

            // body
            if let Ok(b) = init_obj.get(js_string!("body"), ctx) {
                if !b.is_undefined() && !b.is_null() {
                    body = Some(b.to_string(ctx)?.to_std_string_escaped());
                }
            }
        }
    }

    Ok(FetchArgs {
        url,
        method,
        headers,
        body,
    })
}

/// Create a Response JS object from resolved fetch data.
pub(crate) fn create_response_object(
    status: u16,
    status_text: &str,
    headers: Vec<(String, String)>,
    body: String,
    url: &str,
    ctx: &mut Context,
) -> JsResult<JsObject> {
    let response_data = JsResponse {
        _status: status,
        _status_text: status_text.to_string(),
        _headers: headers.clone(),
        body: RefCell::new(Some(body)),
        _url: url.to_string(),
    };

    let obj = ObjectInitializer::with_native_data(response_data, ctx).build();

    // Set prototype from global Response.prototype
    let global = ctx.global_object();
    if let Ok(ctor_val) = global.get(js_string!("Response"), ctx) {
        if let Some(ctor_obj) = ctor_val.as_object() {
            if let Ok(proto_val) = ctor_obj.get(js_string!("prototype"), ctx) {
                if let Some(proto) = proto_val.as_object() {
                    obj.set_prototype(Some(proto.clone()));
                }
            }
        }
    }

    // Set own properties for ok and status (some code reads these directly)
    obj.define_property_or_throw(
        js_string!("ok"),
        PropertyDescriptor::builder()
            .value(JsValue::from((200..300).contains(&status)))
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;
    obj.define_property_or_throw(
        js_string!("status"),
        PropertyDescriptor::builder()
            .value(JsValue::from(status))
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;
    obj.define_property_or_throw(
        js_string!("statusText"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!(status_text)))
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;
    obj.define_property_or_throw(
        js_string!("url"),
        PropertyDescriptor::builder()
            .value(JsValue::from(js_string!(url)))
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    // Create Headers object for response.headers
    let headers_obj = create_headers_object(&headers, ctx)?;
    obj.define_property_or_throw(
        js_string!("headers"),
        PropertyDescriptor::builder()
            .value(JsValue::from(headers_obj))
            .writable(false)
            .configurable(true)
            .enumerable(true)
            .build(),
        ctx,
    )?;

    Ok(obj)
}

/// Register the Response global (mainly for instanceof checks and prototype methods).
fn register_response_global(ctx: &mut Context) {
    let proto = ObjectInitializer::new(ctx).build();
    let realm = ctx.realm().clone();

    // .json() — parse body as JSON, return resolved promise
    let json_fn = NativeFunction::from_fn_ptr(|this, _args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("Response.json called on non-object")
        })?;
        let resp = obj.downcast_ref::<JsResponse>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("Response.json called on non-Response")
        })?;
        let body = resp.body.borrow_mut().take().unwrap_or_default();

        // Parse JSON using the global JSON.parse
        let global = ctx.global_object();
        let json_obj = global.get(js_string!("JSON"), ctx)?;
        let parse_fn = json_obj.as_object().unwrap().get(js_string!("parse"), ctx)?;
        let parsed = parse_fn
            .as_callable()
            .unwrap()
            .call(&JsValue::undefined(), &[JsValue::from(js_string!(body))], ctx)?;

        let promise = JsPromise::resolve(parsed, ctx);
        Ok(JsValue::from(promise))
    });

    // .text() — return body as resolved promise
    let text_fn = NativeFunction::from_fn_ptr(|this, _args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("Response.text called on non-object")
        })?;
        let resp = obj.downcast_ref::<JsResponse>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("Response.text called on non-Response")
        })?;
        let body = resp.body.borrow_mut().take().unwrap_or_default();
        let promise = JsPromise::resolve(JsValue::from(js_string!(body)), ctx);
        Ok(JsValue::from(promise))
    });

    proto
        .define_property_or_throw(js_string!("json"), prop_desc::data_prop(json_fn.to_js_function(&realm)), ctx)
        .expect("define Response.prototype.json");
    proto
        .define_property_or_throw(js_string!("text"), prop_desc::data_prop(text_fn.to_js_function(&realm)), ctx)
        .expect("define Response.prototype.text");

    // Constructor (mostly a stub for instanceof)
    let response_ctor = NativeFunction::from_fn_ptr(|_this, _args, _ctx| Ok(JsValue::undefined()));
    let ctor: JsObject = boa_engine::object::FunctionObjectBuilder::new(ctx.realm(), response_ctor)
        .name(js_string!("Response"))
        .length(0)
        .constructor(true)
        .build()
        .into();

    ctor.define_property_or_throw(js_string!("prototype"), prop_desc::prototype_on_ctor(proto.clone()), ctx)
        .expect("set Response.prototype");
    proto
        .define_property_or_throw(js_string!("constructor"), prop_desc::constructor_on_proto(ctor.clone()), ctx)
        .expect("set Response.prototype.constructor");

    ctx.global_object()
        .set(js_string!("Response"), JsValue::from(ctor), false, ctx)
        .expect("set Response global");
}

/// Native data for Headers instances.
#[derive(Debug, boa_engine::JsData, boa_gc::Trace, boa_gc::Finalize)]
struct JsHeaders {
    #[unsafe_ignore_trace]
    entries: RefCell<Vec<(String, String)>>,
}

fn create_headers_object(headers: &[(String, String)], ctx: &mut Context) -> JsResult<JsObject> {
    let data = JsHeaders {
        entries: RefCell::new(headers.to_vec()),
    };
    let obj = ObjectInitializer::with_native_data(data, ctx).build();

    // Set Headers.prototype if registered
    let global = ctx.global_object();
    if let Ok(ctor_val) = global.get(js_string!("Headers"), ctx) {
        if let Some(ctor_obj) = ctor_val.as_object() {
            if let Ok(proto_val) = ctor_obj.get(js_string!("prototype"), ctx) {
                if let Some(proto) = proto_val.as_object() {
                    obj.set_prototype(Some(proto.clone()));
                }
            }
        }
    }

    Ok(obj)
}

fn register_headers_global(ctx: &mut Context) {
    let proto = ObjectInitializer::new(ctx).build();
    let realm = ctx.realm().clone();

    // .get(name) — case-insensitive lookup
    let get_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("Headers.get called on non-object")
        })?;
        let headers = obj.downcast_ref::<JsHeaders>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("Headers.get called on non-Headers")
        })?;
        let name = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped().to_ascii_lowercase())
            .unwrap_or_default();
        let entries = headers.entries.borrow();
        for (k, v) in entries.iter() {
            if k.to_ascii_lowercase() == name {
                return Ok(JsValue::from(js_string!(v.clone())));
            }
        }
        Ok(JsValue::null())
    });

    // .has(name)
    let has_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("Headers.has called on non-object")
        })?;
        let headers = obj.downcast_ref::<JsHeaders>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("Headers.has called on non-Headers")
        })?;
        let name = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped().to_ascii_lowercase())
            .unwrap_or_default();
        let entries = headers.entries.borrow();
        let found = entries.iter().any(|(k, _)| k.to_ascii_lowercase() == name);
        Ok(JsValue::from(found))
    });

    // .set(name, value)
    let set_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("Headers.set called on non-object")
        })?;
        let headers = obj.downcast_ref::<JsHeaders>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("Headers.set called on non-Headers")
        })?;
        let name = args
            .first()
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let value = args
            .get(1)
            .map(|v| v.to_string(ctx))
            .transpose()?
            .map(|s| s.to_std_string_escaped())
            .unwrap_or_default();
        let name_lower = name.to_ascii_lowercase();
        let mut entries = headers.entries.borrow_mut();
        entries.retain(|(k, _)| k.to_ascii_lowercase() != name_lower);
        entries.push((name, value));
        Ok(JsValue::undefined())
    });

    // .forEach(callback)
    let for_each_fn = NativeFunction::from_fn_ptr(|this, args, ctx| {
        let obj = this.as_object().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("Headers.forEach called on non-object")
        })?;
        let headers = obj.downcast_ref::<JsHeaders>().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("Headers.forEach called on non-Headers")
        })?;
        let callback = args.first().cloned().unwrap_or(JsValue::undefined());
        let callable = callback.as_callable().ok_or_else(|| {
            boa_engine::JsNativeError::typ().with_message("Headers.forEach callback is not callable")
        })?;
        let entries: Vec<(String, String)> = headers.entries.borrow().clone();
        for (k, v) in entries {
            callable.call(
                &JsValue::undefined(),
                &[
                    JsValue::from(js_string!(v)),
                    JsValue::from(js_string!(k)),
                    JsValue::from(obj.clone()),
                ],
                ctx,
            )?;
        }
        Ok(JsValue::undefined())
    });

    proto
        .define_property_or_throw(js_string!("get"), prop_desc::data_prop(get_fn.to_js_function(&realm)), ctx)
        .expect("define Headers.prototype.get");
    proto
        .define_property_or_throw(js_string!("has"), prop_desc::data_prop(has_fn.to_js_function(&realm)), ctx)
        .expect("define Headers.prototype.has");
    proto
        .define_property_or_throw(js_string!("set"), prop_desc::data_prop(set_fn.to_js_function(&realm)), ctx)
        .expect("define Headers.prototype.set");
    proto
        .define_property_or_throw(
            js_string!("forEach"),
            prop_desc::data_prop(for_each_fn.to_js_function(&realm)),
            ctx,
        )
        .expect("define Headers.prototype.forEach");

    let headers_ctor = NativeFunction::from_fn_ptr(|_this, _args, _ctx| Ok(JsValue::undefined()));
    let ctor: JsObject = boa_engine::object::FunctionObjectBuilder::new(ctx.realm(), headers_ctor)
        .name(js_string!("Headers"))
        .length(0)
        .constructor(true)
        .build()
        .into();

    ctor.define_property_or_throw(js_string!("prototype"), prop_desc::prototype_on_ctor(proto.clone()), ctx)
        .expect("set Headers.prototype");
    proto
        .define_property_or_throw(js_string!("constructor"), prop_desc::constructor_on_proto(ctor.clone()), ctx)
        .expect("set Headers.prototype.constructor");

    ctx.global_object()
        .set(js_string!("Headers"), JsValue::from(ctor), false, ctx)
        .expect("set Headers global");
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
    fn fetch_returns_promise() {
        let mut rt = make_runtime();
        let result = rt
            .eval(
                r#"
            var p = fetch('/api/data');
            p instanceof Promise
        "#,
            )
            .unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn fetch_creates_pending_fetch() {
        let mut rt = make_runtime();
        rt.eval(r#"fetch('/api/data')"#).unwrap();
        let pending = crate::js::realm_state::pending_fetches(&rt.context);
        let fetches = pending.borrow();
        assert_eq!(fetches.len(), 1);
        assert_eq!(fetches[0].url, "/api/data");
        assert_eq!(fetches[0].method, "GET");
    }

    #[test]
    fn fetch_with_options() {
        let mut rt = make_runtime();
        rt.eval(
            r#"fetch('/api/submit', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: '{"key":"value"}'
            })"#,
        )
        .unwrap();
        let pending = crate::js::realm_state::pending_fetches(&rt.context);
        let fetches = pending.borrow();
        assert_eq!(fetches.len(), 1);
        assert_eq!(fetches[0].method, "POST");
        assert_eq!(fetches[0].body.as_deref(), Some(r#"{"key":"value"}"#));
    }

    #[test]
    fn response_and_headers_globals_exist() {
        let mut rt = make_runtime();
        let result = rt
            .eval("typeof Response === 'function' && typeof Headers === 'function'")
            .unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }

    #[test]
    fn fetch_on_window() {
        let mut rt = make_runtime();
        let result = rt.eval("typeof window.fetch === 'function'").unwrap();
        assert_eq!(result.as_boolean(), Some(true));
    }
}
